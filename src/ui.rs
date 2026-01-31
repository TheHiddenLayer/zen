//! Terminal UI rendering for the Zen TUI.
//!
//! Design philosophy: "Pilot's Seat"
//! - Minimal chrome: no box drawing, no ASCII borders, no decorative labels
//! - Whitespace as structure: position and spacing create hierarchy
//! - HUD uses grayscale + one accent (#F3037E) for selection
//! - Viewport preserves full terminal colors from tmux sessions
//! - Scrolloff navigation: selection stays centered, content flows past
//!
//! This module renders from RenderState (immutable snapshot) - it never
//! mutates application state. This enables the decoupled game loop.

use ansi_to_tui::IntoText;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Clear, Gauge, Paragraph},
    Frame,
};

use crate::agent::AgentStatus;
use crate::core::task::TaskStatus;
use crate::render::{AgentView, RenderState, SessionView, TaskDAGView, TaskView, WorkflowView};
use crate::session::SessionStatus;
use crate::tea::{InputKind, Mode, Notification, NotificationLevel};
use crate::workflow::WorkflowStatus;

// Color tokens (selection uses REVERSED modifier to adapt to terminal theme)
const COLOR_TEXT_DIMMED: Color = Color::Gray;
const COLOR_TEXT_MUTED: Color = Color::DarkGray;
const COLOR_SEPARATOR: Color = Color::White;

// Workflow status colors
const COLOR_WORKFLOW_RUNNING: Color = Color::Green;
const COLOR_WORKFLOW_COMPLETED: Color = Color::Cyan;
const COLOR_WORKFLOW_FAILED: Color = Color::Red;
const COLOR_WORKFLOW_PAUSED: Color = Color::Yellow;

// Phase indicator colors
const COLOR_PHASE_CURRENT: Color = Color::Cyan;
const COLOR_PHASE_COMPLETED: Color = Color::Green;
const COLOR_PHASE_PENDING: Color = Color::DarkGray;

// Status color coding for faster visual parsing (uses terminal palette)
const COLOR_STATUS_BUSY: Color = Color::Green;
const COLOR_STATUS_IDLE: Color = Color::Red;
const COLOR_STATUS_LOCKED: Color = Color::Gray;

// Agent grid status colors
const COLOR_AGENT_RUNNING: Color = Color::Green;
const COLOR_AGENT_STUCK: Color = Color::Yellow;
const COLOR_AGENT_FAILED: Color = Color::Red;
const COLOR_AGENT_IDLE: Color = Color::Gray;
const COLOR_AGENT_TERMINATED: Color = Color::Cyan;

// Layout constants
const HUD_HEIGHT: u16 = 8;

// Column widths for the session list
const STATUS_WIDTH: usize = 6;
const PROJECT_WIDTH: usize = 12;
const BRANCH_WIDTH: usize = 28;
const BASE_WIDTH: usize = 16;
const AGENT_WIDTH: usize = 8;
const SPACING: usize = 2;

/// Session status for display styling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypographyState {
    Busy,    // Agent actively working (pane output changing)
    Ready,   // Agent idle/waiting for user (no pane activity)
    Locked,  // Session locked - whole row muted
    Unknown, // Initial state before activity is calculated
}

impl TypographyState {
    pub fn from_session(session: &SessionView) -> Self {
        match session.status {
            SessionStatus::Locked => Self::Locked,
            SessionStatus::Running => match session.is_active {
                None => Self::Unknown,
                Some(true) => Self::Busy,
                Some(false) => Self::Ready,
            },
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Busy => "Busy",
            Self::Ready => "Idle",
            Self::Locked => "Locked",
            Self::Unknown => "",
        }
    }
}

// -----------------------------------------------------------------------------
// Context-sensitive keymap system
// -----------------------------------------------------------------------------

/// Context for determining which keybindings to display.
/// Derived from RenderState - this is the "view model" for the statusbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapContext {
    /// Normal list browsing - shows navigation and session actions
    List {
        selected_session: Option<SelectedSessionContext>,
    },
    /// Text input mode (session name, prompt)
    TextInput,
    /// Delete confirmation mode
    DeleteConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedSessionContext {
    pub is_locked: bool,
}

impl KeymapContext {
    /// Derive keymap context from render state.
    pub fn from_render_state(state: &RenderState) -> Self {
        match state.mode {
            Mode::Input(InputKind::Confirm) => KeymapContext::DeleteConfirm,
            Mode::Input(_) => KeymapContext::TextInput,
            Mode::List => {
                let selected_session =
                    state
                        .sessions
                        .get(state.selected)
                        .map(|s| SelectedSessionContext {
                            is_locked: s.status == SessionStatus::Locked,
                        });
                KeymapContext::List { selected_session }
            }
        }
    }
}

/// A single keybinding entry for display.
struct Keybinding(&'static str, &'static str);

/// A group of related keybindings (separated by │).
struct KeybindingGroup(Vec<Keybinding>);

/// Get keybindings for a given context.
fn keybindings_for_context(ctx: KeymapContext) -> Vec<KeybindingGroup> {
    match ctx {
        KeymapContext::List { selected_session } => {
            let session_actions = vec![Keybinding("n", "new"), Keybinding("d", "delete")];

            let attach_group = match selected_session {
                Some(ctx) if ctx.is_locked => {
                    vec![Keybinding("l", "unlock")]
                }
                Some(_) => vec![Keybinding("o", "open"), Keybinding("l", "lock")],
                None => vec![],
            };

            vec![
                KeybindingGroup(session_actions),
                KeybindingGroup(attach_group),
                KeybindingGroup(vec![Keybinding("q", "quit")]),
            ]
        }
        KeymapContext::TextInput => vec![KeybindingGroup(vec![
            Keybinding("Enter", "submit"),
            Keybinding("Esc", "cancel"),
        ])],
        KeymapContext::DeleteConfirm => vec![KeybindingGroup(vec![
            Keybinding("Enter", "delete"),
            Keybinding("Esc", "cancel"),
        ])],
    }
}

/// Main render function - entry point for all UI drawing.
/// Takes an immutable RenderState snapshot.
pub fn draw(frame: &mut Frame, state: &RenderState) {
    // All modes use main layout - the HUD status bar handles
    // conditional display (keymap vs input prompt) on the bottom line
    render_main_layout(frame, state);

    // Render notification if present
    if let Some(ref notification) = state.notification {
        render_notification(frame, notification, frame.area());
    }
}

/// Render the main layout: viewport + separator + HUD + status bar.
fn render_main_layout(frame: &mut Frame, state: &RenderState) {
    let area = frame.area();

    if area.height < 3 {
        render_hud(frame, state, area);
        return;
    }

    let hud_height = HUD_HEIGHT.min(area.height.saturating_sub(3));
    let separator_height = if area.height > hud_height + 2 { 1 } else { 0 };

    let chunks = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(separator_height),
        Constraint::Length(hud_height),
        Constraint::Length(1),
    ])
    .split(area);

    render_viewport(frame, state, chunks[0]);
    if separator_height > 0 {
        render_separator(frame, chunks[1]);
    }
    render_hud(frame, state, chunks[2]);
    render_statusbar(frame, state, chunks[3]);
}

/// Render the viewport - raw tmux capture with colors.
fn render_viewport(frame: &mut Frame, state: &RenderState, area: Rect) {
    let content = state.preview.as_deref().unwrap_or("");
    let text: Text = content.into_text().unwrap_or_default();

    let total_lines = text.lines.len();
    let visible_lines = area.height as usize;
    let start = total_lines.saturating_sub(visible_lines);
    let lines: Vec<Line> = text.lines.into_iter().skip(start).collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Render the separator - solid divider line between viewport and HUD.
fn render_separator(frame: &mut Frame, area: Rect) {
    let solid = "─".repeat(area.width as usize);
    let line = Line::from(Span::styled(solid, Style::default().fg(COLOR_SEPARATOR)));
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

/// Render the HUD status bar - single bottom line with conditional display.
/// Shows either:
/// - Input prompt (when in Input mode - no '?' shown)
/// - "?" indicator only (when keymap is collapsed)
/// - "? │ <full keymap>" (when keymap is expanded via '?' toggle)
///
/// When trust mode is enabled, shows "TRUSTING" badge on the right side.
fn render_statusbar(frame: &mut Frame, state: &RenderState, area: Rect) {
    let line = match state.mode {
        Mode::Input(kind) => render_input_line(state, kind, state.trust_enabled, area.width),
        _ => render_keymap_line(state, state.trust_enabled, area.width),
    };
    frame.render_widget(Paragraph::new(line), area);
}

/// Render keybindings legend for the bottom line.
/// When show_keymap is false: Shows just "?" (grayed out)
/// When show_keymap is true: Shows "? │ <full keymap legend>" with bright "?"
/// When trust mode is enabled, shows "TRUSTING" badge on the right side.
fn render_keymap_line(state: &RenderState, trust_enabled: bool, width: u16) -> Line<'static> {
    let ctx = KeymapContext::from_render_state(state);
    let groups = keybindings_for_context(ctx);

    let key_style = Style::default().fg(COLOR_TEXT_DIMMED);
    let desc_style = Style::default().fg(COLOR_TEXT_MUTED);
    let sep_style = Style::default().fg(COLOR_TEXT_MUTED);

    let mut spans: Vec<Span> = Vec::new();

    // Always show '?' toggle indicator first
    // When collapsed: dimmed '?'
    // When expanded: bright '?' followed by the full keymap
    let help_style = if state.show_keymap {
        Style::default() // Bright (default foreground)
    } else {
        Style::default().fg(COLOR_TEXT_MUTED) // Grayed out
    };
    spans.push(Span::styled("?", help_style));

    // Only show the full keymap legend when expanded
    if state.show_keymap {
        for group in groups.iter() {
            if group.0.is_empty() {
                continue;
            }

            // Separator before each group (including first, since we have '?' prefix)
            if !spans.is_empty() {
                spans.push(Span::styled(" │ ", sep_style));
            }

            for (key_idx, keybinding) in group.0.iter().enumerate() {
                if key_idx > 0 {
                    spans.push(Span::styled(" • ", sep_style));
                }
                spans.push(Span::styled(keybinding.0, key_style));
                spans.push(Span::styled(format!(" {}", keybinding.1), desc_style));
            }
        }
    }

    // Add TRUSTING badge on the right side if trust mode is enabled
    if trust_enabled {
        let trust_badge = " TRUSTING ";
        let trust_badge_len = trust_badge.len();

        // Calculate current content width
        let content_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();

        // Calculate spacer to right-align the badge
        let spacer_width = (width as usize)
            .saturating_sub(content_width)
            .saturating_sub(trust_badge_len);

        if spacer_width > 0 {
            spans.push(Span::raw(" ".repeat(spacer_width)));
        }

        // Caution sign colorscheme: black text on yellow background
        spans.push(Span::styled(
            trust_badge,
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ));
    }

    Line::from(spans)
}

/// Render input prompt for the bottom line (replaces keymap when in input mode).
/// When trust mode is enabled, shows "TRUSTING" badge on the right side.
fn render_input_line(
    state: &RenderState,
    kind: InputKind,
    trust_enabled: bool,
    width: u16,
) -> Line<'static> {
    let hint_key_style = Style::default().fg(COLOR_TEXT_MUTED);
    let hint_sep_style = Style::default().fg(COLOR_TEXT_MUTED);
    let label_style = Style::default().fg(Color::Reset);
    let input_style = Style::default().fg(Color::White);
    let cursor_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::SLOW_BLINK);

    let label = kind.label();
    let buffer = state.input_buffer.clone();

    // Build hints first (left side)
    let mut spans: Vec<Span> = if matches!(kind, InputKind::Confirm) {
        // Confirm mode: Enter/Esc only
        vec![
            Span::styled("Enter ", hint_key_style),
            Span::styled("• ", hint_sep_style),
            Span::styled("Esc ", hint_key_style),
            Span::styled(" ", hint_sep_style),
        ]
    } else {
        // Text input mode: Enter/Tab/Esc
        vec![
            Span::styled("Enter ", hint_key_style),
            Span::styled("• ", hint_sep_style),
            Span::styled("Tab ", hint_key_style),
            Span::styled("• ", hint_sep_style),
            Span::styled("Esc ", hint_key_style),
            Span::styled(" ", hint_sep_style),
        ]
    };

    // Add label and input
    if matches!(kind, InputKind::Confirm) {
        spans.push(Span::styled(label.to_string(), label_style));
    } else {
        spans.push(Span::styled(format!("{label}: "), label_style));
        spans.push(Span::styled(buffer, input_style));
        spans.push(Span::styled("_", cursor_style));
    }

    // Add TRUSTING badge on the right side if trust mode is enabled
    if trust_enabled {
        let trust_badge = " TRUSTING ";
        let trust_badge_len = trust_badge.len();

        // Calculate current content width
        let content_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();

        // Calculate spacer to right-align the badge
        let spacer_width = (width as usize)
            .saturating_sub(content_width)
            .saturating_sub(trust_badge_len);

        if spacer_width > 0 {
            spans.push(Span::raw(" ".repeat(spacer_width)));
        }

        // Caution sign colorscheme: black text on yellow background
        spans.push(Span::styled(
            trust_badge,
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ));
    }

    Line::from(spans)
}

/// Render the HUD - session list with scrolloff navigation.
fn render_hud(frame: &mut Frame, state: &RenderState, area: Rect) {
    if state.sessions.is_empty() {
        let msg = Line::from(Span::styled(
            "No sessions. Press 'n' to create one.",
            Style::default().fg(COLOR_TEXT_DIMMED),
        ));
        let paragraph = Paragraph::new(msg);
        frame.render_widget(paragraph, area);
        return;
    }

    // Reserve 1 line for header
    let header_height = 1;
    let content_height = area.height.saturating_sub(header_height as u16) as usize;

    // Scrolloff implementation: keep selection centered
    let center = content_height / 2;
    let start = state.selected.saturating_sub(center);
    let end = (start + content_height).min(state.sessions.len());
    let start = end.saturating_sub(content_height);

    // Build lines starting with header
    let mut lines: Vec<Line> = Vec::with_capacity(content_height + header_height);

    // Add header row (bold)
    lines.push(render_header_row(area.width));

    // Add session rows
    lines.extend(
        state
            .sessions
            .iter()
            .enumerate()
            .skip(start)
            .take(content_height)
            .map(|(idx, session)| render_session_row(session, idx == state.selected, area.width)),
    );

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Render the column header row (bold to distinguish from data rows).
fn render_header_row(width: u16) -> Line<'static> {
    let header_style = Style::default()
        .fg(COLOR_TEXT_DIMMED)
        .add_modifier(Modifier::BOLD);
    let spacing = "  ";

    // Minimum usable width check
    if width < 20 {
        return Line::from(Span::styled("SESSION", header_style));
    }

    let total_fixed =
        STATUS_WIDTH + PROJECT_WIDTH + BRANCH_WIDTH + BASE_WIDTH + AGENT_WIDTH + SPACING * 5;
    let session_width = (width as usize).saturating_sub(total_fixed);

    let status = format!("{:<width$}", "STATUS", width = STATUS_WIDTH);
    let project = format!("{:<width$}", "PROJECT", width = PROJECT_WIDTH);
    let session = format!("{:<width$}", "SESSION", width = session_width);
    let branch = format!("{:<width$}", "BRANCH", width = BRANCH_WIDTH);
    let base = format!("{:<width$}", "BASE", width = BASE_WIDTH);
    let agent = format!("{:<width$}", "AGENT", width = AGENT_WIDTH);

    Line::from(vec![
        Span::styled(status, header_style),
        Span::styled(spacing, header_style),
        Span::styled(session, header_style),
        Span::styled(spacing, header_style),
        Span::styled(project, header_style),
        Span::styled(spacing, header_style),
        Span::styled(branch, header_style),
        Span::styled(spacing, header_style),
        Span::styled(base, header_style),
        Span::styled(spacing, header_style),
        Span::styled(agent, header_style),
    ])
}

/// Render a single session row with column layout.
/// Columns: STATUS (~6ch) | SESSION (flex) | PROJECT (~12ch) | BRANCH (~28ch) | BASE (~16ch) | AGENT (~8ch)
fn render_session_row(session: &SessionView, is_selected: bool, width: u16) -> Line<'static> {
    let typo_state = TypographyState::from_session(session);

    if width < 20 {
        let session_style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        return Line::from(Span::styled(
            truncate(&session.name, width as usize),
            session_style,
        ));
    }

    let total_fixed =
        STATUS_WIDTH + PROJECT_WIDTH + BRANCH_WIDTH + BASE_WIDTH + AGENT_WIDTH + SPACING * 5;
    let session_width = (width as usize).saturating_sub(total_fixed);

    // Format STATUS column
    let status_padded = format!("{:<width$}", typo_state.label(), width = STATUS_WIDTH);

    let project = truncate(&session.project, PROJECT_WIDTH);
    let project_padded = format!("{:<width$}", project, width = PROJECT_WIDTH);

    let session_name = truncate(&session.name, session_width);
    let session_padded = format!("{:<width$}", session_name, width = session_width);

    // Format BRANCH (the session's working branch)
    let branch = truncate(&session.branch, BRANCH_WIDTH);
    let branch_padded = format!("{:<width$}", branch, width = BRANCH_WIDTH);

    // Format BASE as "branch @ commit_short" (e.g., "main @ fe645a")
    let base_display = format_base_display(&session.base_branch, &session.base_commit);
    let base = truncate(&base_display, BASE_WIDTH);
    let base_padded = format!("{:<width$}", base, width = BASE_WIDTH);

    let agent = truncate(&session.agent, AGENT_WIDTH);
    let agent_padded = format!("{:<width$}", agent, width = AGENT_WIDTH);

    // Determine styles based on selection and typography state
    let spacing = "  ";

    // Status gets color coding based on state (uses terminal palette colors)
    let status_color = match typo_state {
        TypographyState::Ready => COLOR_STATUS_IDLE,
        TypographyState::Busy => COLOR_STATUS_BUSY,
        TypographyState::Locked => COLOR_STATUS_LOCKED,
        TypographyState::Unknown => COLOR_TEXT_DIMMED,
    };

    let (status_style, primary_style, secondary_style) = if is_selected {
        let selected = Style::default().add_modifier(Modifier::REVERSED);
        (selected, selected, selected)
    } else {
        (
            Style::default().fg(status_color),
            Style::default(),
            Style::default().fg(COLOR_TEXT_DIMMED),
        )
    };

    Line::from(vec![
        Span::styled(status_padded, status_style),
        Span::styled(spacing, primary_style),
        Span::styled(session_padded, primary_style),
        Span::styled(spacing, primary_style),
        Span::styled(project_padded, secondary_style),
        Span::styled(spacing, primary_style),
        Span::styled(branch_padded, secondary_style),
        Span::styled(spacing, primary_style),
        Span::styled(base_padded, secondary_style),
        Span::styled(spacing, primary_style),
        Span::styled(agent_padded, secondary_style),
    ])
}

/// Render notification message on the bottom line of the screen.
///
/// Displays a single-line notification with appropriate styling based on the notification level:
/// - Error: Red text with "Error:" prefix and bold styling
/// - Info: Green text without prefix
fn render_notification(frame: &mut Frame, notification: &Notification, area: Rect) {
    let notification_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };

    frame.render_widget(Clear, notification_area);

    let line = match notification.level {
        NotificationLevel::Error => Line::from(vec![
            Span::styled(
                "Error: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                notification.message.clone(),
                Style::default().fg(Color::Red),
            ),
        ]),
        NotificationLevel::Info => Line::from(Span::styled(
            notification.message.clone(),
            Style::default().fg(Color::Green),
        )),
    };

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, notification_area);
}

// Helper functions

/// Format base branch and commit as "branch @ commit_short" (e.g., "main @ fe645a")
fn format_base_display(base_branch: &str, base_commit: &str) -> String {
    let commit_short = if base_commit.len() >= 7 {
        &base_commit[..7]
    } else {
        base_commit
    };

    if base_branch.is_empty() {
        // Fallback if no branch name stored (older sessions)
        commit_short.to_string()
    } else {
        format!("{} @ {}", base_branch, commit_short)
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        s.chars().take(max_len).collect()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}~", truncated)
    }
}

// -----------------------------------------------------------------------------
// Workflow UI Components
// -----------------------------------------------------------------------------

/// Render workflow header with name and status.
///
/// Displays the workflow name and current status when a workflow is active,
/// or "No active workflow" message when no workflow is running.
pub fn render_workflow_header(frame: &mut Frame, area: Rect, workflow: Option<&WorkflowView>) {
    let line = match workflow {
        Some(wf) => {
            let status_color = workflow_status_color(&wf.status);
            let status_label = workflow_status_label(&wf.status);

            Line::from(vec![
                Span::styled("Workflow: ", Style::default().fg(COLOR_TEXT_DIMMED)),
                Span::styled(
                    wf.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [", Style::default().fg(COLOR_TEXT_MUTED)),
                Span::styled(status_label, Style::default().fg(status_color)),
                Span::styled("]", Style::default().fg(COLOR_TEXT_MUTED)),
            ])
        }
        None => Line::from(Span::styled(
            "No active workflow",
            Style::default().fg(COLOR_TEXT_DIMMED),
        )),
    };

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

/// Render phase progress indicator and progress bar.
///
/// Shows all 5 phases with the current phase highlighted, plus a progress bar
/// showing overall completion percentage.
pub fn render_phase_progress(frame: &mut Frame, area: Rect, workflow: Option<&WorkflowView>) {
    let Some(wf) = workflow else {
        // No workflow - render empty or placeholder
        return;
    };

    // Need at least 2 lines: one for phases, one for progress bar
    if area.height < 2 {
        return;
    }

    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);

    // Render phase indicator on first line
    render_phase_indicator(frame, chunks[0], wf);

    // Render progress bar on second line
    render_progress_bar(frame, chunks[1], wf);
}

/// Render the 5-phase indicator with current phase highlighted.
fn render_phase_indicator(frame: &mut Frame, area: Rect, workflow: &WorkflowView) {
    let phase_names = WorkflowView::phase_names();
    let current_idx = workflow.current_phase_index();

    let mut spans: Vec<Span> = Vec::new();

    for (i, name) in phase_names.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" → ", Style::default().fg(COLOR_TEXT_MUTED)));
        }

        let style = if i < current_idx {
            // Completed phase
            Style::default().fg(COLOR_PHASE_COMPLETED)
        } else if i == current_idx && current_idx < 5 {
            // Current phase (bold + highlighted)
            Style::default()
                .fg(COLOR_PHASE_CURRENT)
                .add_modifier(Modifier::BOLD)
        } else {
            // Pending phase
            Style::default().fg(COLOR_PHASE_PENDING)
        };

        spans.push(Span::styled(*name, style));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

/// Render progress bar using Gauge widget.
fn render_progress_bar(frame: &mut Frame, area: Rect, workflow: &WorkflowView) {
    let percentage = workflow.progress_percentage();
    let label = format!(
        "{}% ({}/{})",
        percentage, workflow.phase_progress.0, workflow.phase_progress.1
    );

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(COLOR_PHASE_CURRENT).bg(Color::DarkGray))
        .percent(percentage)
        .label(label);

    frame.render_widget(gauge, area);
}

/// Get color for workflow status.
fn workflow_status_color(status: &WorkflowStatus) -> Color {
    match status {
        WorkflowStatus::Running => COLOR_WORKFLOW_RUNNING,
        WorkflowStatus::Completed => COLOR_WORKFLOW_COMPLETED,
        WorkflowStatus::Failed => COLOR_WORKFLOW_FAILED,
        WorkflowStatus::Paused => COLOR_WORKFLOW_PAUSED,
        WorkflowStatus::Pending => COLOR_TEXT_DIMMED,
        WorkflowStatus::Accepted => Color::Cyan,
        WorkflowStatus::Rejected => Color::Magenta,
    }
}

/// Get display label for workflow status.
fn workflow_status_label(status: &WorkflowStatus) -> &'static str {
    match status {
        WorkflowStatus::Running => "Running",
        WorkflowStatus::Completed => "Completed",
        WorkflowStatus::Failed => "Failed",
        WorkflowStatus::Paused => "Paused",
        WorkflowStatus::Pending => "Pending",
        WorkflowStatus::Accepted => "Accepted",
        WorkflowStatus::Rejected => "Rejected",
    }
}

// -----------------------------------------------------------------------------
// Agent Grid UI Components
// -----------------------------------------------------------------------------

/// Render a grid of agents for parallel execution monitoring.
///
/// Displays agents in a 2x2 or 3x2 grid layout depending on count.
/// Each cell shows agent status, task name, elapsed time, and output preview.
pub fn render_agent_grid(
    frame: &mut Frame,
    area: Rect,
    agents: &[AgentView],
    selected: usize,
) {
    if agents.is_empty() {
        let msg = Line::from(Span::styled(
            "No active agents",
            Style::default().fg(COLOR_TEXT_DIMMED),
        ));
        let paragraph = Paragraph::new(msg);
        frame.render_widget(paragraph, area);
        return;
    }

    // Calculate grid layout based on agent count
    let (rows, cols) = calculate_grid_layout(agents.len());

    // Split area into rows
    let row_constraints: Vec<Constraint> = (0..rows)
        .map(|_| Constraint::Ratio(1, rows as u32))
        .collect();
    let row_chunks = Layout::vertical(row_constraints).split(area);

    // Render each row
    let mut agent_idx = 0;
    for (_row_idx, row_area) in row_chunks.iter().enumerate() {
        // Split row into columns
        let col_constraints: Vec<Constraint> = (0..cols)
            .map(|_| Constraint::Ratio(1, cols as u32))
            .collect();
        let col_chunks = Layout::horizontal(col_constraints).split(*row_area);

        for col_area in col_chunks.iter() {
            if agent_idx < agents.len() {
                let is_selected = agent_idx == selected;
                render_agent_cell(frame, *col_area, &agents[agent_idx], is_selected);
                agent_idx += 1;
            }
        }

        // Stop if we've rendered all agents
        if agent_idx >= agents.len() {
            break;
        }
    }
}

/// Calculate optimal grid layout (rows, cols) based on agent count.
///
/// Returns a (rows, cols) tuple that best fits the agents:
/// - 1-2 agents: 1 row, 2 cols (1x2)
/// - 3-4 agents: 2 rows, 2 cols (2x2)
/// - 5-6 agents: 2 rows, 3 cols (2x3)
/// - 7-9 agents: 3 rows, 3 cols (3x3)
fn calculate_grid_layout(agent_count: usize) -> (usize, usize) {
    match agent_count {
        0 => (0, 0),
        1 => (1, 1),
        2 => (1, 2),
        3..=4 => (2, 2),
        5..=6 => (2, 3),
        7..=9 => (3, 3),
        _ => {
            // For larger counts, aim for roughly square grid
            let cols = (agent_count as f64).sqrt().ceil() as usize;
            let rows = (agent_count + cols - 1) / cols;
            (rows, cols)
        }
    }
}

/// Render a single agent cell in the grid.
fn render_agent_cell(frame: &mut Frame, area: Rect, agent: &AgentView, is_selected: bool) {
    if area.height < 2 || area.width < 10 {
        // Not enough space for meaningful content
        return;
    }

    // Calculate layout within cell
    // Line 1: Task name + status indicator
    // Line 2: Elapsed time
    // Lines 3+: Output preview (last 3 lines)

    let chunks = Layout::vertical([
        Constraint::Length(1), // Task name + status
        Constraint::Length(1), // Elapsed time
        Constraint::Min(1),    // Output preview
    ])
    .split(area);

    // Get status color
    let status_color = agent_status_color(&agent.status);

    // Selection style
    let base_style = if is_selected {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };

    // Render task name + status
    render_agent_header(frame, chunks[0], agent, status_color, base_style);

    // Render elapsed time
    render_agent_elapsed(frame, chunks[1], agent, base_style);

    // Render output preview
    render_agent_output_preview(frame, chunks[2], agent, base_style);
}

/// Render agent header with task name and status indicator.
fn render_agent_header(
    frame: &mut Frame,
    area: Rect,
    agent: &AgentView,
    status_color: Color,
    base_style: Style,
) {
    let status_indicator = match &agent.status {
        AgentStatus::Running { .. } => "●",  // Filled circle for running
        AgentStatus::Stuck { .. } => "⚠",   // Warning for stuck
        AgentStatus::Failed { .. } => "✗",  // X for failed
        AgentStatus::Idle => "○",            // Empty circle for idle
        AgentStatus::Terminated => "✓",     // Check for done
    };

    let available_width = area.width as usize;
    let status_width = 2; // indicator + space
    let status_label_width = agent.status_label().len() + 2; // " [label]"
    let task_width = available_width.saturating_sub(status_width + status_label_width);

    let task_name = truncate(&agent.task_name, task_width);

    let line = Line::from(vec![
        Span::styled(format!("{} ", status_indicator), Style::default().fg(status_color)),
        Span::styled(task_name, base_style.add_modifier(Modifier::BOLD)),
        Span::styled(" [", base_style.fg(COLOR_TEXT_MUTED)),
        Span::styled(agent.status_label(), base_style.fg(status_color)),
        Span::styled("]", base_style.fg(COLOR_TEXT_MUTED)),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

/// Render agent elapsed time.
fn render_agent_elapsed(frame: &mut Frame, area: Rect, agent: &AgentView, base_style: Style) {
    let elapsed = agent.format_elapsed();
    let line = Line::from(vec![
        Span::styled("  ", base_style),
        Span::styled("Time: ", base_style.fg(COLOR_TEXT_DIMMED)),
        Span::styled(elapsed, base_style),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// Render agent output preview (last few lines).
fn render_agent_output_preview(
    frame: &mut Frame,
    area: Rect,
    agent: &AgentView,
    base_style: Style,
) {
    let max_lines = area.height as usize;
    let output_lines = agent.output_lines(max_lines);

    if output_lines.is_empty() {
        let line = Line::from(Span::styled(
            "  (no output)",
            base_style.fg(COLOR_TEXT_MUTED),
        ));
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    let lines: Vec<Line> = output_lines
        .iter()
        .map(|line| {
            let truncated = truncate(line, area.width.saturating_sub(2) as usize);
            Line::from(Span::styled(format!("  {}", truncated), base_style.fg(COLOR_TEXT_DIMMED)))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}

/// Get color for agent status.
fn agent_status_color(status: &AgentStatus) -> Color {
    match status {
        AgentStatus::Running { .. } => COLOR_AGENT_RUNNING,
        AgentStatus::Stuck { .. } => COLOR_AGENT_STUCK,
        AgentStatus::Failed { .. } => COLOR_AGENT_FAILED,
        AgentStatus::Idle => COLOR_AGENT_IDLE,
        AgentStatus::Terminated => COLOR_AGENT_TERMINATED,
    }
}

// -----------------------------------------------------------------------------
// DAG Visualization UI Components
// -----------------------------------------------------------------------------

// Task status colors for DAG visualization
const COLOR_TASK_COMPLETED: Color = Color::Green;
const COLOR_TASK_RUNNING: Color = Color::Cyan;
const COLOR_TASK_PENDING: Color = Color::DarkGray;
const COLOR_TASK_READY: Color = Color::Yellow;
const COLOR_TASK_FAILED: Color = Color::Red;
const COLOR_TASK_BLOCKED: Color = Color::Magenta;

/// Render the task DAG visualization as ASCII art.
///
/// Displays tasks as ASCII boxes with dependency arrows connecting them.
/// Uses topological layers to arrange tasks horizontally.
///
/// Example output:
/// ```text
/// [A:done] ──┐
///            ├──> [C:running]
/// [B:done] ──┘
///
/// [D:pending] ──> [E:pending]
/// ```
pub fn render_task_dag(frame: &mut Frame, area: Rect, dag: &TaskDAGView) {
    if dag.tasks.is_empty() {
        let msg = Line::from(Span::styled(
            "No tasks in DAG",
            Style::default().fg(COLOR_TEXT_DIMMED),
        ));
        frame.render_widget(Paragraph::new(msg), area);
        return;
    }

    let layers = dag.compute_layers();
    let ascii_lines = render_dag_ascii(dag, &layers, area.width as usize);

    let text: Vec<Line> = ascii_lines
        .into_iter()
        .take(area.height as usize)
        .collect();

    frame.render_widget(Paragraph::new(text), area);
}

/// Render DAG as ASCII lines with task boxes and arrows.
fn render_dag_ascii(dag: &TaskDAGView, layers: &[Vec<usize>], max_width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Calculate positions for each task
    let positions = calculate_task_positions(dag, layers);

    // Track which layer each task is in
    let mut task_layer: Vec<usize> = vec![0; dag.tasks.len()];
    for (layer_idx, layer) in layers.iter().enumerate() {
        for &task_idx in layer {
            task_layer[task_idx] = layer_idx;
        }
    }

    // Render each layer
    for (layer_idx, layer) in layers.iter().enumerate() {
        let is_last_layer = layer_idx == layers.len() - 1;

        // Render tasks in this layer (may span multiple lines)
        let layer_lines = render_layer_boxes(dag, layer, &positions, max_width);
        lines.extend(layer_lines);

        // Render arrows to next layer (if not last layer)
        if !is_last_layer {
            let arrow_lines = render_layer_arrows(dag, layer, &task_layer, layer_idx, &positions);
            lines.extend(arrow_lines);
        }
    }

    lines
}

/// Calculate task positions (row offset within layer) for layout.
fn calculate_task_positions(_dag: &TaskDAGView, layers: &[Vec<usize>]) -> Vec<usize> {
    let mut positions = vec![0; layers.iter().map(|l| l.iter().max().copied().unwrap_or(0) + 1).max().unwrap_or(0)];

    for layer in layers {
        for (row, &task_idx) in layer.iter().enumerate() {
            if task_idx < positions.len() {
                positions[task_idx] = row;
            } else {
                positions.resize(task_idx + 1, 0);
                positions[task_idx] = row;
            }
        }
    }

    positions
}

/// Render task boxes for a single layer.
fn render_layer_boxes(
    dag: &TaskDAGView,
    layer: &[usize],
    _positions: &[usize],
    max_width: usize,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for &task_idx in layer {
        let task = &dag.tasks[task_idx];
        let box_line = render_task_box(task, max_width);
        lines.push(box_line);
    }

    lines
}

/// Render a single task as an ASCII box.
fn render_task_box(task: &TaskView, max_width: usize) -> Line<'static> {
    let status_label = task.status_label();
    let color = task_status_color(&task.status);

    // Format: [name:status]
    // Truncate name if needed to fit in max_width
    let max_name_len = max_width.saturating_sub(status_label.len() + 4); // 4 for [, :, ], space
    let name = if task.name.len() > max_name_len {
        format!("{}~", &task.name[..max_name_len.saturating_sub(1)])
    } else {
        task.name.clone()
    };

    let box_text = format!("[{}:{}]", name, status_label);

    Line::from(vec![Span::styled(box_text, Style::default().fg(color))])
}

/// Render arrows connecting a layer to subsequent layers.
fn render_layer_arrows(
    dag: &TaskDAGView,
    layer: &[usize],
    task_layer: &[usize],
    current_layer_idx: usize,
    _positions: &[usize],
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Collect edges from this layer to the next
    let mut has_edges = false;
    for &task_idx in layer {
        let outgoing = dag.outgoing_edges(task_idx);
        for &to_idx in &outgoing {
            // Only show arrows to the immediate next layer
            if task_layer[to_idx] == current_layer_idx + 1 {
                has_edges = true;
                break;
            }
        }
        if has_edges {
            break;
        }
    }

    if has_edges {
        // Simple arrow representation
        let mut spans: Vec<Span<'static>> = Vec::new();

        // Collect all edges from this layer
        let mut edge_targets: Vec<usize> = Vec::new();
        for &task_idx in layer {
            for &to_idx in &dag.outgoing_edges(task_idx) {
                if task_layer[to_idx] == current_layer_idx + 1 && !edge_targets.contains(&to_idx) {
                    edge_targets.push(to_idx);
                }
            }
        }

        if layer.len() == 1 && edge_targets.len() == 1 {
            // Simple single arrow: ──>
            spans.push(Span::styled("  ──> ", Style::default().fg(COLOR_TEXT_MUTED)));
        } else if layer.len() > 1 && edge_targets.len() == 1 {
            // Multiple sources to single target: merge
            spans.push(Span::styled("  ├──> ", Style::default().fg(COLOR_TEXT_MUTED)));
        } else if layer.len() == 1 && edge_targets.len() > 1 {
            // Single source to multiple targets: fan out
            spans.push(Span::styled("  ┬──> ", Style::default().fg(COLOR_TEXT_MUTED)));
        } else {
            // Complex case: multiple to multiple
            spans.push(Span::styled("  │ ", Style::default().fg(COLOR_TEXT_MUTED)));
        }

        lines.push(Line::from(spans));
    }

    lines
}

/// Get color for task status.
fn task_status_color(status: &TaskStatus) -> Color {
    match status {
        TaskStatus::Completed => COLOR_TASK_COMPLETED,
        TaskStatus::Running => COLOR_TASK_RUNNING,
        TaskStatus::Ready => COLOR_TASK_READY,
        TaskStatus::Pending => COLOR_TASK_PENDING,
        TaskStatus::Failed { .. } => COLOR_TASK_FAILED,
        TaskStatus::Blocked { .. } => COLOR_TASK_BLOCKED,
        TaskStatus::Cancelled { .. } => COLOR_TEXT_DIMMED,
    }
}

/// Generate ASCII representation of DAG for text output.
///
/// Returns a vector of strings representing the DAG,
/// useful for testing and debugging.
pub fn dag_to_ascii_string(dag: &TaskDAGView) -> Vec<String> {
    let layers = dag.compute_layers();
    let mut result: Vec<String> = Vec::new();

    for (layer_idx, layer) in layers.iter().enumerate() {
        // Task boxes
        for &task_idx in layer {
            let task = &dag.tasks[task_idx];
            result.push(format!("[{}:{}]", task.name, task.status_label()));
        }

        // Arrows to next layer
        if layer_idx < layers.len() - 1 {
            let mut has_edges = false;
            for &task_idx in layer {
                if !dag.outgoing_edges(task_idx).is_empty() {
                    has_edges = true;
                    break;
                }
            }
            if has_edges {
                if layer.len() == 1 {
                    result.push("  ──>".to_string());
                } else {
                    result.push("  ├──>".to_string());
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello w~");
        assert_eq!(truncate("hello", 0), "");
        assert_eq!(truncate("hello", 3), "hel");
    }

    #[test]
    fn test_format_base_display() {
        // Normal case with full commit hash
        assert_eq!(
            format_base_display("main", "abc123def456789"),
            "main @ abc123d"
        );
        // Short branch name
        assert_eq!(
            format_base_display("dev", "fe645a123456789"),
            "dev @ fe645a1"
        );
        // Empty branch name (fallback for older sessions)
        assert_eq!(format_base_display("", "abc123def456789"), "abc123d");
        // Short commit hash (less than 7 chars)
        assert_eq!(format_base_display("main", "abc"), "main @ abc");
        // Longer branch name
        assert_eq!(
            format_base_display("jsmith/dev", "c453da123456789"),
            "jsmith/dev @ c453da1"
        );
    }

    // Workflow UI Component tests

    use crate::workflow::{WorkflowId, WorkflowPhase};

    #[test]
    fn test_workflow_status_color_running() {
        assert_eq!(workflow_status_color(&WorkflowStatus::Running), COLOR_WORKFLOW_RUNNING);
    }

    #[test]
    fn test_workflow_status_color_completed() {
        assert_eq!(workflow_status_color(&WorkflowStatus::Completed), COLOR_WORKFLOW_COMPLETED);
    }

    #[test]
    fn test_workflow_status_color_failed() {
        assert_eq!(workflow_status_color(&WorkflowStatus::Failed), COLOR_WORKFLOW_FAILED);
    }

    #[test]
    fn test_workflow_status_color_paused() {
        assert_eq!(workflow_status_color(&WorkflowStatus::Paused), COLOR_WORKFLOW_PAUSED);
    }

    #[test]
    fn test_workflow_status_color_pending() {
        assert_eq!(workflow_status_color(&WorkflowStatus::Pending), COLOR_TEXT_DIMMED);
    }

    #[test]
    fn test_workflow_status_label_all_statuses() {
        assert_eq!(workflow_status_label(&WorkflowStatus::Running), "Running");
        assert_eq!(workflow_status_label(&WorkflowStatus::Completed), "Completed");
        assert_eq!(workflow_status_label(&WorkflowStatus::Failed), "Failed");
        assert_eq!(workflow_status_label(&WorkflowStatus::Paused), "Paused");
        assert_eq!(workflow_status_label(&WorkflowStatus::Pending), "Pending");
    }

    #[test]
    fn test_workflow_view_for_implementation_phase() {
        // Given workflow in Implementation phase
        let view = WorkflowView::new(
            WorkflowId::new(),
            "build-auth".to_string(),
            WorkflowPhase::Implementation,
            WorkflowStatus::Running,
        );

        // Then current_phase_index returns 2 (Implementation is third phase)
        assert_eq!(view.current_phase_index(), 2);

        // And phase_progress shows 2 completed phases
        assert_eq!(view.phase_progress, (2, 5));
    }

    #[test]
    fn test_workflow_view_progress_bar_60_percent() {
        // Given 3 of 5 phases complete (Merging phase)
        let view = WorkflowView::new(
            WorkflowId::new(),
            "test".to_string(),
            WorkflowPhase::Merging,
            WorkflowStatus::Running,
        );

        // Then progress bar shows 60%
        assert_eq!(view.progress_percentage(), 60);
    }

    #[test]
    fn test_workflow_view_phase_names_ordered() {
        let names = WorkflowView::phase_names();
        // Verify phase names are in correct order
        assert_eq!(names[0], "Planning");
        assert_eq!(names[1], "TaskGen");
        assert_eq!(names[2], "Impl");
        assert_eq!(names[3], "Merge");
        assert_eq!(names[4], "Docs");
    }

    #[test]
    fn test_workflow_view_all_phase_indices() {
        // Test that current_phase_index returns correct index for each phase
        let test_cases = [
            (WorkflowPhase::Planning, 0),
            (WorkflowPhase::TaskGeneration, 1),
            (WorkflowPhase::Implementation, 2),
            (WorkflowPhase::Merging, 3),
            (WorkflowPhase::Documentation, 4),
            (WorkflowPhase::Complete, 5),
        ];

        for (phase, expected_idx) in test_cases {
            let view = WorkflowView::new(
                WorkflowId::new(),
                "test".to_string(),
                phase,
                WorkflowStatus::Running,
            );
            assert_eq!(
                view.current_phase_index(),
                expected_idx,
                "Phase {:?} should have index {}",
                phase,
                expected_idx
            );
        }
    }

    // Agent Grid UI Component tests

    use crate::agent::AgentId;
    use crate::workflow::TaskId;
    use std::time::Duration;

    #[allow(dead_code)]
    fn create_test_agent(name: &str, status: AgentStatus, elapsed_secs: u64) -> AgentView {
        AgentView::new(
            AgentId::new(),
            name.to_string(),
            status,
            Duration::from_secs(elapsed_secs),
            "Line 1\nLine 2\nLine 3".to_string(),
        )
    }

    #[test]
    fn test_grid_layout_4_agents_2x2() {
        // Given 4 active agents
        // When calculate_grid_layout is called
        // Then 2x2 grid is returned
        let (rows, cols) = calculate_grid_layout(4);
        assert_eq!(rows, 2);
        assert_eq!(cols, 2);
    }

    #[test]
    fn test_grid_layout_1_agent() {
        let (rows, cols) = calculate_grid_layout(1);
        assert_eq!(rows, 1);
        assert_eq!(cols, 1);
    }

    #[test]
    fn test_grid_layout_2_agents() {
        let (rows, cols) = calculate_grid_layout(2);
        assert_eq!(rows, 1);
        assert_eq!(cols, 2);
    }

    #[test]
    fn test_grid_layout_3_agents() {
        let (rows, cols) = calculate_grid_layout(3);
        assert_eq!(rows, 2);
        assert_eq!(cols, 2);
    }

    #[test]
    fn test_grid_layout_6_agents_2x3() {
        // Given 6 agents
        // When calculate_grid_layout is called
        // Then 2x3 grid is returned
        let (rows, cols) = calculate_grid_layout(6);
        assert_eq!(rows, 2);
        assert_eq!(cols, 3);
    }

    #[test]
    fn test_grid_layout_0_agents() {
        let (rows, cols) = calculate_grid_layout(0);
        assert_eq!(rows, 0);
        assert_eq!(cols, 0);
    }

    #[test]
    fn test_agent_status_color_running() {
        // Given agent in Running status
        // When rendered
        // Then green indicator is shown
        let task_id = TaskId::new();
        let status = AgentStatus::Running { task_id };
        assert_eq!(agent_status_color(&status), COLOR_AGENT_RUNNING);
    }

    #[test]
    fn test_agent_status_color_stuck() {
        let status = AgentStatus::Stuck {
            since: std::time::Instant::now(),
            reason: "timeout".to_string(),
        };
        assert_eq!(agent_status_color(&status), COLOR_AGENT_STUCK);
    }

    #[test]
    fn test_agent_status_color_failed() {
        let status = AgentStatus::Failed {
            error: "process exited".to_string(),
        };
        assert_eq!(agent_status_color(&status), COLOR_AGENT_FAILED);
    }

    #[test]
    fn test_agent_status_color_idle() {
        assert_eq!(agent_status_color(&AgentStatus::Idle), COLOR_AGENT_IDLE);
    }

    #[test]
    fn test_agent_status_color_terminated() {
        assert_eq!(agent_status_color(&AgentStatus::Terminated), COLOR_AGENT_TERMINATED);
    }

    #[test]
    fn test_render_state_default_agents_empty() {
        let state = RenderState::default();
        assert!(state.agents.is_empty());
        assert_eq!(state.selected_agent, 0);
    }

    // DAG Visualization tests

    #[test]
    fn test_task_status_color_completed() {
        assert_eq!(task_status_color(&TaskStatus::Completed), COLOR_TASK_COMPLETED);
    }

    #[test]
    fn test_task_status_color_running() {
        assert_eq!(task_status_color(&TaskStatus::Running), COLOR_TASK_RUNNING);
    }

    #[test]
    fn test_task_status_color_pending() {
        assert_eq!(task_status_color(&TaskStatus::Pending), COLOR_TASK_PENDING);
    }

    #[test]
    fn test_task_status_color_ready() {
        assert_eq!(task_status_color(&TaskStatus::Ready), COLOR_TASK_READY);
    }

    #[test]
    fn test_task_status_color_failed() {
        let status = TaskStatus::Failed { error: "error".to_string() };
        assert_eq!(task_status_color(&status), COLOR_TASK_FAILED);
    }

    #[test]
    fn test_task_status_color_blocked() {
        let status = TaskStatus::Blocked { reason: "waiting".to_string() };
        assert_eq!(task_status_color(&status), COLOR_TASK_BLOCKED);
    }

    #[test]
    fn test_task_status_color_cancelled() {
        let status = TaskStatus::Cancelled { reason: "replanned".to_string() };
        assert_eq!(task_status_color(&status), COLOR_TEXT_DIMMED);
    }

    #[test]
    fn test_dag_to_ascii_5_tasks() {
        // Given 5 tasks in DAG
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("task-1".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("task-2".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("task-3".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("task-4".to_string(), TaskStatus::Pending));
        dag.add_task(TaskView::new("task-5".to_string(), TaskStatus::Pending));

        // When rendered to ASCII
        let ascii = dag_to_ascii_string(&dag);

        // Then 5 ASCII boxes are displayed with task names
        let task_boxes: Vec<_> = ascii.iter()
            .filter(|line| line.starts_with('[') && line.ends_with(']'))
            .collect();
        assert_eq!(task_boxes.len(), 5, "Should have 5 task boxes");

        // Verify all task names are present
        assert!(ascii.iter().any(|s| s.contains("task-1")));
        assert!(ascii.iter().any(|s| s.contains("task-2")));
        assert!(ascii.iter().any(|s| s.contains("task-3")));
        assert!(ascii.iter().any(|s| s.contains("task-4")));
        assert!(ascii.iter().any(|s| s.contains("task-5")));
    }

    #[test]
    fn test_dag_to_ascii_dependency_arrow() {
        // Given A->C dependency
        let mut dag = TaskDAGView::new();
        let a_idx = dag.add_task(TaskView::new("A".to_string(), TaskStatus::Completed));
        let c_idx = dag.add_task(TaskView::new("C".to_string(), TaskStatus::Running));
        dag.add_edge(a_idx, c_idx);

        // When rendered to ASCII
        let ascii = dag_to_ascii_string(&dag);

        // Then arrow connects A box to C box (arrow line present)
        assert!(ascii.iter().any(|s| s.contains("──>")), "Should have arrow: {:?}", ascii);
    }

    #[test]
    fn test_dag_to_ascii_completed_status() {
        // Given completed task A
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("A".to_string(), TaskStatus::Completed));

        // When rendered to ASCII
        let ascii = dag_to_ascii_string(&dag);

        // Then A's box shows done status
        assert!(ascii.iter().any(|s| s.contains("[A:done]")), "Should show A:done, got: {:?}", ascii);
    }

    #[test]
    fn test_dag_to_ascii_running_status() {
        // Given running task
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("B".to_string(), TaskStatus::Running));

        let ascii = dag_to_ascii_string(&dag);
        assert!(ascii.iter().any(|s| s.contains("[B:running]")));
    }

    #[test]
    fn test_dag_to_ascii_pending_status() {
        // Given pending task
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("C".to_string(), TaskStatus::Pending));

        let ascii = dag_to_ascii_string(&dag);
        assert!(ascii.iter().any(|s| s.contains("[C:pending]")));
    }

    #[test]
    fn test_dag_to_ascii_linear_chain() {
        // A -> B -> C
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("A".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("B".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("C".to_string(), TaskStatus::Pending));
        dag.add_edge(0, 1);
        dag.add_edge(1, 2);

        let ascii = dag_to_ascii_string(&dag);

        // All three tasks should be present
        assert!(ascii.iter().any(|s| s.contains("[A:done]")));
        assert!(ascii.iter().any(|s| s.contains("[B:running]")));
        assert!(ascii.iter().any(|s| s.contains("[C:pending]")));

        // Should have arrows
        let arrow_count = ascii.iter().filter(|s| s.contains("──>")).count();
        assert!(arrow_count >= 2, "Should have at least 2 arrows for chain");
    }

    #[test]
    fn test_dag_to_ascii_diamond_pattern() {
        // A -> B, A -> C, B -> D, C -> D
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("A".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("B".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("C".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("D".to_string(), TaskStatus::Pending));
        dag.add_edge(0, 1);
        dag.add_edge(0, 2);
        dag.add_edge(1, 3);
        dag.add_edge(2, 3);

        let ascii = dag_to_ascii_string(&dag);

        // All four tasks should be present
        assert!(ascii.iter().any(|s| s.contains("[A:done]")));
        assert!(ascii.iter().any(|s| s.contains("[B:running]")));
        assert!(ascii.iter().any(|s| s.contains("[C:running]")));
        assert!(ascii.iter().any(|s| s.contains("[D:pending]")));
    }

    #[test]
    fn test_dag_to_ascii_complex_10_tasks() {
        // Given DAG with 10 tasks and multiple paths
        let mut dag = TaskDAGView::new();
        for i in 0..10 {
            let status = match i {
                0..=2 => TaskStatus::Completed,
                3..=5 => TaskStatus::Running,
                _ => TaskStatus::Pending,
            };
            dag.add_task(TaskView::new(format!("task-{}", i), status));
        }
        // Add some dependencies
        dag.add_edge(0, 3);
        dag.add_edge(1, 4);
        dag.add_edge(2, 5);
        dag.add_edge(3, 6);
        dag.add_edge(4, 7);
        dag.add_edge(5, 8);
        dag.add_edge(6, 9);
        dag.add_edge(7, 9);
        dag.add_edge(8, 9);

        // When rendered to ASCII
        let ascii = dag_to_ascii_string(&dag);

        // Then all dependencies are visible (may scroll)
        // All 10 tasks should be present
        let task_boxes: Vec<_> = ascii.iter()
            .filter(|line| line.starts_with('[') && line.ends_with(']'))
            .collect();
        assert_eq!(task_boxes.len(), 10, "Should have 10 task boxes: {:?}", ascii);
    }

    #[test]
    fn test_render_state_default_dag_none() {
        let state = RenderState::default();
        assert!(state.dag.is_none());
        assert!(!state.show_dag);
    }
}
