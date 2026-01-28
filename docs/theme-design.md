# Theme Design: Grayscale + Blue Accent

> Terminal-native colors. One accent. Zero decoration.

The interface uses a minimal color approach that works on both light and dark terminals by relying on terminal defaults for most text and using a single accent color for selection.

---

## Design Philosophy

### Principles

1. **Inherit terminal colors** - Default text uses terminal foreground, not fixed colors
2. **One accent color** - Selection highlighted with `#007AFF` (iMessage blue)
3. **Grayscale for hierarchy** - Dimmed text for secondary information
4. **No semantic color coding** - Status conveyed through text, not color
5. **Works everywhere** - Light terminals, dark terminals, high contrast modes

### Why This Approach?

Previous "chromatic motion" designs with multiple accent colors, breathing animations, and semantic color coding:
- Required TrueColor support
- Looked bad on light terminals
- Created visual noise
- Increased cognitive load

The Pilot's Seat philosophy strips this back to essentials: **grayscale + one accent**.

---

## Color Palette

### Primary Colors

| Token | Hex | Usage |
|-------|-----|-------|
| `SELECTION_BG` | `#007AFF` | Selected row background |
| `SELECTION_FG` | `#FFFFFF` | Text on selected row |

### Grayscale

| Token | Hex | Usage |
|-------|-----|-------|
| `TEXT_DEFAULT` | (terminal fg) | Normal text |
| `TEXT_DIMMED` | `#808080` | Secondary info, unselected rows |
| `SEPARATOR` | `#404040` | Separator line |

### Optional Status Colors

If explicit status indication is needed beyond text:

| Token | Hex | Usage |
|-------|-----|-------|
| `ERROR` | `#FF3B30` | Error status (use sparingly) |

---

## Visual Hierarchy

All hierarchy through brightness and the single accent:

| Level | Style | Usage |
|-------|-------|-------|
| Selected | White on `#007AFF` | Current selection |
| Primary | Terminal default | Active sessions, important text |
| Secondary | `#808080` (gray) | Idle sessions, metadata |

No other colors needed. If you need more hierarchy, use spacing instead.

---

## Selection Styling

The ONLY use of the accent color is selection:

```
  session-1           idle      waiting for input            45m    <- Gray text
  session-2           idle      parsing readme.md            2h     <- Gray text
[ auth-fix            active    compiling typescript...      12m ]  <- White on #007AFF
  sql-migration       active    analyzing schema diffs       5m     <- Gray text
```

### Implementation

```rust
let selected_style = Style::new()
    .bg(Color::Rgb(0, 122, 255))  // #007AFF
    .fg(Color::White);

let normal_style = Style::new()
    .fg(Color::Gray);
```

---

## Swiss-Style Typography Effects

The Pilot's Seat uses **subtle, purposeful animations** based on Swiss-Style typography principles. Status is communicated through text treatment rather than icons or spinners.

### Typography States

| State | Treatment | Animation | Vibe |
|-------|-----------|-----------|------|
| **ACTIVE** | Shimmering | Wave of white intensity moves L-to-R | Alive, processing |
| **DONE** | Bold + Pulsing | Slow brightness breathing | Substantial, resolved |
| **IDLE** | Normal | None | Ready, present |
| **PAUSED** | Muted (50%) | None | Dormant, background |

### Why Typography-Based Status?

This approach uses "grayscale intensity and typographic manipulation" - avoiding the "video game" look of colorful spinners. It feels like high-end print design or "Swiss Style" typography:

- Sophisticated, minimal aesthetic
- Status is glanceable without being distracting
- Works on both light and dark terminals
- No color blindness issues (relies on brightness, not hue)

See [Animation System](./animation-system.md) for implementation details.

---

## Status Indication

Status is conveyed through **typography treatment** and **session name color**:

### Session Name Color (Status Light)

The session name acts as the status indicator:

| Status | Color | Meaning |
|--------|-------|---------|
| Active/Thinking | Cyan/Blue | Agent is working |
| Error | Red | Something went wrong |
| Normal | Default | Ready state |

### Idle Column Color Thresholds

The idle time column uses color to indicate urgency:

| Duration | Color | Meaning |
|----------|-------|---------|
| `< 1m` | Green/Dimmed | Healthy activity |
| `1-5m` | Dimmed | Still okay |
| `5-30m` | Yellow | Warning - check on it |
| `> 30m` | Red | Likely stuck/abandoned |

### Implementation

```rust
fn idle_duration_color(duration: Duration) -> Color {
    let minutes = duration.as_secs() / 60;
    match minutes {
        0 => Color::DarkGray,      // < 1m: dimmed (healthy)
        1..=4 => Color::DarkGray,  // 1-5m: still okay
        5..=29 => Color::Yellow,   // 5-30m: warning
        _ => Color::Red,           // > 30m: likely stuck
    }
}
```

### Typography State Mapping

| Session State | Typography Treatment |
|---------------|---------------------|
| Running + recent activity | ACTIVE (shimmer) |
| Running + task complete | DONE (bold pulse) |
| Running + waiting | IDLE (normal) |
| Paused | PAUSED (muted) |
| Error | Error color + normal text |

---

## Diff Colors

For diff view, standard git colors apply:

| Element | Color |
|---------|-------|
| Additions | Green (`#22C55E`) |
| Deletions | Red (`#EF4444`) |
| Context | Gray (`#808080`) |
| Hunk headers | Blue (`#3B82F6`) |

These are the exception to "grayscale only" because diff coloring is a universal convention users expect.

---

## Typography

### No Decorative Elements

| Instead of | Use |
|------------|-----|
| Spinner `\|/-` | Static text `loading...` |
| Arrow `>` | Blue background |
| Brackets `[ ]` | Blue background |
| Progress bar | Percentage text |
| Colored bullets | Whitespace indentation |

### Text Styles

| Style | Usage |
|-------|-------|
| Normal | Idle sessions, baseline content |
| Dimmed | Model column, secondary info |
| Muted (50%) | Paused sessions |
| Bold + Pulse | Done/completed state |
| Shimmer | Active/processing state |

---

## Terminal Compatibility

### Requirements

- Minimum: 256 colors (for `#007AFF` approximation)
- Recommended: TrueColor for exact blue
- Fallback: ANSI blue for 16-color terminals

### Light/Dark Agnostic

By inheriting terminal default for most text:
- Dark terminal: Light text on dark background
- Light terminal: Dark text on light background
- Selection blue works on both

### No Background Colors (Except Selection)

The viewport and HUD use transparent backgrounds, inheriting from the terminal. Only the selected row has a background color.

---

## Accessibility

### Contrast Ratios

- Selection (`#007AFF` bg, white fg): 4.5:1 minimum contrast
- Gray text on dark bg: Verify 4.5:1 contrast
- Gray text on light bg: May need adjustment
- Idle warning colors maintain readability

### Color Blindness

Since we use only one accent color (blue) and status is conveyed through brightness:
- Deuteranopia: Blue is safe, brightness changes visible
- Protanopia: Blue is safe, brightness changes visible
- Tritanopia: Blue may appear cyan, still distinguishable

### Reduced Motion

For users who prefer reduced motion:
- Shimmer effect can be disabled (falls back to static bright text)
- Pulse effect can be disabled (falls back to static bold)
- Check `prefers-reduced-motion` media query or provide config option

```rust
pub struct AnimationConfig {
    pub shimmer_enabled: bool,
    pub pulse_enabled: bool,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            shimmer_enabled: true,
            pulse_enabled: true,
        }
    }
}
```

---

## Implementation Summary

```rust
// src/theme/colors.rs

pub mod colors {
    use ratatui::style::Color;

    // Selection (the only accent)
    pub const SELECTION_BG: Color = Color::Rgb(0, 122, 255);  // #007AFF
    pub const SELECTION_FG: Color = Color::White;

    // Grayscale
    pub const TEXT_DIMMED: Color = Color::Gray;
    pub const TEXT_MUTED: Color = Color::DarkGray;  // 50% opacity effect
    pub const SEPARATOR: Color = Color::DarkGray;

    // Status colors (for session name)
    pub const STATUS_ACTIVE: Color = Color::Cyan;
    pub const STATUS_ERROR: Color = Color::Red;

    // Idle column thresholds
    pub const IDLE_HEALTHY: Color = Color::DarkGray;
    pub const IDLE_WARNING: Color = Color::Yellow;
    pub const IDLE_CRITICAL: Color = Color::Red;

    // Diff (standard git colors)
    pub const DIFF_ADD: Color = Color::Rgb(34, 197, 94);    // #22C55E
    pub const DIFF_DEL: Color = Color::Rgb(239, 68, 68);    // #EF4444
    pub const DIFF_HUNK: Color = Color::Rgb(59, 130, 246);  // #3B82F6
}

pub mod styles {
    use ratatui::style::Style;
    use super::colors;

    pub fn selected() -> Style {
        Style::new()
            .bg(colors::SELECTION_BG)
            .fg(colors::SELECTION_FG)
    }

    pub fn normal() -> Style {
        Style::default()  // Inherits terminal colors
    }

    pub fn dimmed() -> Style {
        Style::new().fg(colors::TEXT_DIMMED)
    }

    pub fn muted() -> Style {
        Style::new().fg(colors::TEXT_MUTED)  // For PAUSED state
    }

    pub fn model_column() -> Style {
        Style::new().fg(colors::TEXT_DIMMED)  // Always dimmed
    }

    pub fn idle_column(duration_minutes: u64) -> Style {
        let color = match duration_minutes {
            0..=4 => colors::IDLE_HEALTHY,
            5..=29 => colors::IDLE_WARNING,
            _ => colors::IDLE_CRITICAL,
        };
        Style::new().fg(color)
    }

    pub fn session_status(is_active: bool, is_error: bool) -> Style {
        if is_error {
            Style::new().fg(colors::STATUS_ERROR)
        } else if is_active {
            Style::new().fg(colors::STATUS_ACTIVE)
        } else {
            Style::default()
        }
    }
}
```

