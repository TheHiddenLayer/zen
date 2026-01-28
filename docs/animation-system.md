# Animation System: Swiss-Style Typography Effects

> Subtle, purposeful animations for status communication through text treatment.

**Related docs:**
- [UI Design](./ui-design.md) - HUD layout and idleness metric
- [Theme Design](./theme-design.md) - Color tokens and typography states

---

## Design Philosophy

The Swiss-Style typography system communicates status through **grayscale intensity and typographic manipulation** rather than colorful icons or spinners. This creates a sophisticated, high-end aesthetic that feels like print design rather than a video game.

### Why Typography-Based Animation?

| Approach | Problem |
|----------|---------|
| Spinners (`\|/-\`) | Feels like loading, not working |
| Colored dots | Semantic overload, accessibility issues |
| Icon-based | Requires learning, cultural assumptions |
| **Typography effects** | Universal, elegant, glanceable |

### Core Principles

1. **Subtle over flashy** - Animations should be noticed subconsciously
2. **Purposeful motion** - Every animation conveys meaning
3. **Brightness over hue** - Works for color blindness
4. **Configurable** - Can be disabled for reduced motion preference

---

## Typography States

| State | Effect | Meaning | Implementation |
|-------|--------|---------|----------------|
| **ACTIVE** | Shimmer | Agent is processing | Wave of brightness L-to-R |
| **DONE** | Bold + Pulse | Task complete, needs attention | Slow breathing brightness |
| **IDLE** | Normal | Ready and listening | No animation |
| **PAUSED** | Muted | Dormant, low priority | 50% opacity, static |

---

## Shimmer Effect (ACTIVE State)

A wave of white intensity moves left-to-right across the text, making it feel alive and processing.

### Visual Effect

```
Frame 0:  [###]writing sql migration...
Frame 1:  w[###]iting sql migration...
Frame 2:  wr[###]ting sql migration...
Frame 3:  wri[###]ng sql migration...
...
```

Where `[###]` represents the bright "shimmer band" moving across the text.

### Implementation

```rust
use ratatui::text::{Line, Span};
use ratatui::style::Style;

pub struct Shimmer {
    text: String,
    position: i32,
    width: i32,       // Width of the shimmer band (3-5 chars)
    base_l: f32,      // Base brightness (0.30 = dim)
    peak_l: f32,      // Peak brightness (0.90 = bright white)
    speed: u64,       // Frames per position move
    frame: u64,
}

impl Shimmer {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            position: 0,
            width: 3,
            base_l: 0.30,
            peak_l: 0.90,
            speed: 3,  // Move every 3 frames (150ms at 50ms tick)
            frame: 0,
        }
    }

    pub fn render(&self) -> Line<'static> {
        let spans: Vec<Span> = self.text
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let brightness = self.get_brightness(i as i32);
                let gray = (brightness * 255.0) as u8;
                Span::styled(
                    c.to_string(),
                    Style::new().fg(Color::Rgb(gray, gray, gray)),
                )
            })
            .collect();

        Line::from(spans)
    }

    fn get_brightness(&self, pos: i32) -> f32 {
        let dist = (pos - self.position).abs();
        if dist >= self.width {
            return self.base_l;
        }

        // Smooth quadratic falloff from peak
        let t = 1.0 - (dist as f32 / self.width as f32);
        self.base_l + (self.peak_l - self.base_l) * t * t
    }

    pub fn advance(&mut self) {
        self.frame += 1;
        if self.frame >= self.speed {
            self.frame = 0;
            self.position += 1;

            let len = self.text.chars().count() as i32;
            if self.position >= len + self.width {
                self.position = -self.width; // Loop
            }
        }
    }
}
```

### Parameters

| Parameter | Value | Effect |
|-----------|-------|--------|
| `width` | 3-5 chars | Wider = more gentle wave |
| `base_l` | 0.30 | Dim base state |
| `peak_l` | 0.90 | Nearly white peak |
| `speed` | 3 frames | ~150ms per step at 50ms tick |

---

## Pulse/Breathing Effect (DONE State)

A slow breathing effect where the text dims and brightens, making it feel substantial and resolved, waiting for attention.

### Visual Effect

The entire text slowly oscillates between slightly dim and full brightness:

```
Phase 0.0:  refactoring complete.  (brightness: 0.6)
Phase 0.5:  refactoring complete.  (brightness: 1.0)
Phase 1.0:  refactoring complete.  (brightness: 0.6)
```

### Implementation

```rust
use std::f32::consts::PI;
use ratatui::style::{Style, Modifier};

pub struct Pulse {
    base_brightness: f32,    // 0.60 - minimum brightness
    amplitude: f32,          // 0.40 - brightness variation
    frequency: f32,          // 0.5 Hz - cycles per second
    frame: u64,
    fps: f32,
}

impl Pulse {
    pub fn new() -> Self {
        Self {
            base_brightness: 0.60,
            amplitude: 0.40,
            frequency: 0.5,  // One full cycle every 2 seconds
            frame: 0,
            fps: 20.0,
        }
    }

    pub fn current_brightness(&self) -> f32 {
        let t = self.frame as f32 / self.fps;
        let phase = (2.0 * PI * self.frequency * t).sin();
        (self.base_brightness + self.amplitude * (phase + 1.0) / 2.0).clamp(0.4, 1.0)
    }

    pub fn render(&self, text: &str) -> Span<'static> {
        let brightness = self.current_brightness();
        let gray = (brightness * 255.0) as u8;

        Span::styled(
            text.to_string(),
            Style::new()
                .fg(Color::Rgb(gray, gray, gray))
                .add_modifier(Modifier::BOLD),  // DONE state is bold
        )
    }

    pub fn advance(&mut self) {
        self.frame += 1;
    }
}
```

### Parameters

| Parameter | Value | Effect |
|-----------|-------|--------|
| `base_brightness` | 0.60 | Never fully dim |
| `amplitude` | 0.40 | Subtle variation |
| `frequency` | 0.5 Hz | Slow, breathing pace |

---

## Muted Effect (PAUSED State)

A static 50% opacity effect that makes the text recede into the background.

### Implementation

```rust
pub fn render_muted(text: &str) -> Span<'static> {
    // 50% brightness = gray text
    Span::styled(
        text.to_string(),
        Style::new().fg(Color::DarkGray),
    )
}
```

No animation needed - just static dimmed text.

---

## Normal State (IDLE)

Standard text with no effects. This is the baseline state.

### Implementation

```rust
pub fn render_normal(text: &str) -> Span<'static> {
    Span::styled(
        text.to_string(),
        Style::default(),  // Inherits terminal colors
    )
}
```

---

## Animation Architecture

### Tick-Based Animation Loop

The animation system runs at 20 FPS (50ms tick rate):

```rust
use std::time::{Duration, Instant};
use std::collections::HashMap;
use crossterm::event::{self, Event, KeyCode};
use ratatui::prelude::*;

const TICK_RATE: Duration = Duration::from_millis(50); // 20 FPS

pub struct App {
    frame: u64,
    last_tick: Instant,
    // Typography animation states per session
    shimmers: HashMap<SessionId, Shimmer>,
    pulses: HashMap<SessionId, Pulse>,
    // Animation config (for reduced motion)
    animation_enabled: bool,
}

impl App {
    pub fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> Result<()> {
        self.last_tick = Instant::now();

        loop {
            // Render
            terminal.draw(|f| self.render(f))?;

            // Event polling with timeout
            let timeout = TICK_RATE
                .checked_sub(self.last_tick.elapsed())
                .unwrap_or(Duration::ZERO);

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key)? {
                        break; // Exit requested
                    }
                }
            }

            // Animation tick (only if animations enabled)
            if self.last_tick.elapsed() >= TICK_RATE {
                if self.animation_enabled {
                    self.tick_animations();
                }
                self.last_tick = Instant::now();
            }
        }

        Ok(())
    }

    fn tick_animations(&mut self) {
        self.frame += 1;

        // Advance shimmer effects for ACTIVE sessions
        for shimmer in self.shimmers.values_mut() {
            shimmer.advance();
        }

        // Advance pulse effects for DONE sessions
        for pulse in self.pulses.values_mut() {
            pulse.advance();
        }
    }
}
```

### TypographyState Enum

```rust
/// Typography treatment for a session row
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypographyState {
    /// Wave of brightness moving across text
    Active,
    /// Bold text with slow breathing brightness
    Done,
    /// Normal text, no effects
    Idle,
    /// Muted to 50% opacity
    Paused,
}

impl TypographyState {
    /// Determine typography state from session status and activity
    pub fn from_session(session: &Session, idle_duration: Duration) -> Self {
        match session.status {
            SessionStatus::Paused => Self::Paused,
            SessionStatus::Running => {
                // Check if actively working (recent activity)
                if idle_duration.as_secs() < 10 {
                    Self::Active
                }
                // Otherwise, idle but running
                else {
                    Self::Idle
                }
            }
        }
    }
}
```

---

## Unified Row Renderer

A single function that renders a session row with the appropriate typography treatment:

```rust
use ratatui::text::{Line, Span};
use ratatui::style::{Style, Modifier, Color};
use std::time::Duration;

/// Render a complete HUD row with typography effects
pub fn render_session_row(
    session: &Session,
    typography_state: TypographyState,
    idle_duration: Duration,
    is_selected: bool,
    width: u16,
) -> Line<'static> {
    let mut spans = Vec::new();

    // Calculate widths: PROJECT (~12ch) + AGENT (~8ch) + IDLE (~6ch) + spacing = ~30ch
    // SESSION gets the rest (flex)
    let fixed_width = 30_u16;
    let session_width = width.saturating_sub(fixed_width).max(10) as usize;

    let base_style = if is_selected {
        Style::new().bg(Color::Rgb(0, 122, 255)).fg(Color::White)
    } else {
        Style::new().fg(Color::DarkGray)
    };

    // PROJECT column (~12ch) - always dimmed
    spans.push(Span::styled(
        format!("{:<12}", truncate(&session.project, 12)),
        base_style,
    ));

    // SESSION column (flex) - color indicates status
    let session_style = if is_selected {
        Style::new().bg(Color::Rgb(0, 122, 255)).fg(Color::White)
    } else {
        session_status_style(&session.status, typography_state)
    };
    spans.push(Span::styled(
        format!("  {:<width$}", truncate(&session.name, session_width), width = session_width),
        session_style,
    ));

    // AGENT column (~8ch) - always dimmed
    spans.push(Span::styled(
        format!("  {:<8}", truncate(&session.agent, 8)),
        base_style,
    ));

    // IDLE column (~6ch) - color coded by duration
    let idle_text = format_idle_duration(idle_duration);
    let idle_style = if is_selected {
        Style::new().bg(Color::Rgb(0, 122, 255)).fg(Color::White)
    } else {
        idle_duration_style(idle_duration)
    };
    spans.push(Span::styled(
        format!("  {:>6}", idle_text),
        idle_style,
    ));

    Line::from(spans)
}

fn session_status_style(status: &SessionStatus, typography: TypographyState) -> Style {
    match (status, typography) {
        (SessionStatus::Paused, _) => Style::new().fg(Color::DarkGray),
        (_, TypographyState::Active) => Style::new().fg(Color::Cyan),
        _ => Style::default(),
    }
}

fn idle_duration_style(duration: Duration) -> Style {
    let minutes = duration.as_secs() / 60;
    let color = match minutes {
        0..=4 => Color::DarkGray,   // Healthy
        5..=29 => Color::Yellow,   // Warning
        _ => Color::Red,           // Critical
    };
    Style::new().fg(color)
}

fn format_idle_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}
```

---

## Integration with App State

### Animation State Management

```rust
use std::collections::HashMap;
use std::time::Instant;

pub struct AnimationState {
    /// Shimmer effects for ACTIVE sessions
    pub shimmers: HashMap<SessionId, Shimmer>,
    /// Pulse effects for DONE sessions
    pub pulses: HashMap<SessionId, Pulse>,
    /// Last activity time per session (for idle duration)
    pub last_activity: HashMap<SessionId, Instant>,
    /// Whether animations are enabled (respects reduced motion)
    pub enabled: bool,
}

impl AnimationState {
    pub fn new(enabled: bool) -> Self {
        Self {
            shimmers: HashMap::new(),
            pulses: HashMap::new(),
            last_activity: HashMap::new(),
            enabled,
        }
    }

    /// Update animation states based on session state changes
    pub fn update_for_session(&mut self, session: &Session, idle_duration: Duration) {
        let typography = TypographyState::from_session(session, idle_duration);

        match typography {
            TypographyState::Active => {
                // Ensure shimmer exists, remove pulse
                self.pulses.remove(&session.id);
                self.shimmers.entry(session.id).or_insert_with(|| {
                    Shimmer::new(&session.name)
                });
            }
            _ => {
                // No animations for IDLE or PAUSED
                self.shimmers.remove(&session.id);
                self.pulses.remove(&session.id);
            }
        }
    }

    /// Advance all active animations by one frame
    pub fn tick(&mut self) {
        if !self.enabled {
            return;
        }

        for shimmer in self.shimmers.values_mut() {
            shimmer.advance();
        }
        for pulse in self.pulses.values_mut() {
            pulse.advance();
        }
    }

    /// Record activity for a session (resets idle timer)
    pub fn record_activity(&mut self, session_id: SessionId) {
        self.last_activity.insert(session_id, Instant::now());
    }

    /// Get idle duration for a session
    pub fn get_idle_duration(&self, session_id: SessionId) -> Duration {
        self.last_activity
            .get(&session_id)
            .map(|instant| instant.elapsed())
            .unwrap_or(Duration::ZERO)
    }
}
```

### Rendering HUD with Animations

```rust
impl App {
    fn render_hud(&self, frame: &mut Frame, area: Rect) {
        let hud_height = area.height as usize;
        let center = hud_height / 2;

        // Calculate visible sessions (scrolloff)
        let start = self.selected.saturating_sub(center);
        let end = (start + hud_height).min(self.sessions.len());
        let start = end.saturating_sub(hud_height);

        for (i, session) in self.sessions[start..end].iter().enumerate() {
            let y = area.y + i as u16;
            let is_selected = start + i == self.selected;
            let idle_duration = self.animation_state.get_idle_duration(session.id);
            let typography = TypographyState::from_session(session, idle_duration);

            let line = render_session_row(
                session,
                typography,
                idle_duration,
                self.animation_state.shimmers.get(&session.id),
                self.animation_state.pulses.get(&session.id),
                is_selected,
            );

            frame.render_widget(
                Paragraph::new(line),
                Rect::new(area.x, y, area.width, 1),
            );
        }
    }
}
```

---

## Performance Considerations

### Frame Rate

The animation system runs at 20 FPS (50ms tick). This provides smooth shimmer movement while remaining efficient:

```rust
pub const TICK_RATE: Duration = Duration::from_millis(50);
```

### Selective Animation

Animations only run for sessions that need them:
- ACTIVE sessions get shimmer (most expensive - per-character colors)
- DONE sessions get pulse (cheap - single color calculation)
- IDLE and PAUSED sessions have no animations

### Memory Efficiency

Animation state is created/destroyed based on session state:

```rust
// Only ACTIVE sessions have shimmers
if typography != TypographyState::Active {
    self.shimmers.remove(&session.id);
}

// Only DONE sessions have pulses
if typography != TypographyState::Done {
    self.pulses.remove(&session.id);
}
```

### Reduced Motion Support

For accessibility, animations can be globally disabled:

```rust
impl AnimationState {
    pub fn tick(&mut self) {
        if !self.enabled {
            return;  // Skip all animation updates
        }
        // ... advance animations
    }
}
```

When animations are disabled:
- ACTIVE state renders as bright static text
- DONE state renders as bold static text
- All other states unchanged

---

## Terminal Compatibility

### Grayscale Fallback

Since Swiss-Style effects use grayscale brightness, they work well on all terminal types:

```rust
/// Convert brightness (0.0-1.0) to grayscale color
fn brightness_to_gray(brightness: f32) -> Color {
    let gray = (brightness * 255.0) as u8;
    Color::Rgb(gray, gray, gray)
}

/// Fallback for 256-color terminals
fn brightness_to_indexed(brightness: f32) -> Color {
    // Use grayscale ramp: 232-255 (24 grays)
    let gray_index = (brightness * 23.0) as u8;
    Color::Indexed(232 + gray_index.min(23))
}

/// Fallback for 16-color terminals
fn brightness_to_basic(brightness: f32) -> Color {
    if brightness > 0.7 {
        Color::White
    } else if brightness > 0.3 {
        Color::Gray
    } else {
        Color::DarkGray
    }
}
```

### Detection

```rust
fn supports_truecolor() -> bool {
    std::env::var("COLORTERM")
        .map(|v| v == "truecolor" || v == "24bit")
        .unwrap_or(false)
}

fn supports_256color() -> bool {
    std::env::var("TERM")
        .map(|v| v.contains("256color"))
        .unwrap_or(false)
}
```

---

## Summary

The Swiss-Style typography system provides status indication through:

1. **Shimmer** (ACTIVE) - Wave of brightness for "alive and processing"
2. **Pulse** (DONE) - Breathing brightness for "needs attention"
3. **Normal** (IDLE) - Static text for "ready and listening"
4. **Muted** (PAUSED) - Dimmed text for "dormant"

Combined with:
- **Project name color** - Status indicator (cyan=active, red=error)
- **Idle column color** - Urgency indicator (green < yellow < red)

This creates a sophisticated, accessible interface that communicates status at a glance without the "video game" aesthetic of spinners and icons.

---

## Cargo.toml Dependencies

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
```
