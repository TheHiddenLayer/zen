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

use crate::render::{RenderState, SessionView, WorkflowView};
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
    }
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
}
