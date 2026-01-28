# Color Tokens: Minimal Palette

> Grayscale + one accent. That's it.

This file defines the complete color palette for the Pilot's Seat design. Import these tokens rather than defining colors inline.

---

## Rust Implementation

```rust
// src/theme/colors.rs

use ratatui::style::{Color, Style};

// ============================================================================
// SELECTION (The One Accent)
// ============================================================================

pub mod selection {
    use ratatui::style::Color;

    /// Rubystone Red - the ONLY accent color in the entire UI
    pub const BACKGROUND: Color = Color::Rgb(243, 3, 126);  // #F3037E

    /// White text on blue background
    pub const FOREGROUND: Color = Color::White;
}

// ============================================================================
// GRAYSCALE
// ============================================================================

pub mod text {
    use ratatui::style::Color;

    /// Normal text - inherits terminal default
    /// Use Color::Reset or Style::default() to get this
    pub const DEFAULT: Color = Color::Reset;

    /// Dimmed text for secondary information
    pub const DIMMED: Color = Color::Gray;  // #808080 equivalent
}

pub mod ui {
    use ratatui::style::Color;

    /// Separator line between viewport and HUD
    pub const SEPARATOR: Color = Color::DarkGray;  // #404040 equivalent
}

// ============================================================================
// DIFF COLORS (Standard Git Convention)
// ============================================================================

pub mod diff {
    use ratatui::style::Color;

    /// Added lines
    pub const ADDITION: Color = Color::Rgb(34, 197, 94);    // #22C55E (green)

    /// Removed lines
    pub const DELETION: Color = Color::Rgb(239, 68, 68);    // #EF4444 (red)

    /// Context lines
    pub const CONTEXT: Color = Color::Gray;

    /// Hunk headers (@@ ... @@)
    pub const HUNK: Color = Color::Rgb(59, 130, 246);       // #3B82F6 (blue)
}

// ============================================================================
// STYLE PRESETS
// ============================================================================

pub mod styles {
    use ratatui::style::Style;
    use super::*;

    /// Selected row in the HUD
    pub fn selected() -> Style {
        Style::new()
            .bg(selection::BACKGROUND)
            .fg(selection::FOREGROUND)
    }

    /// Normal text (inherits terminal colors)
    pub fn normal() -> Style {
        Style::default()
    }

    /// Dimmed text for secondary information
    pub fn dimmed() -> Style {
        Style::new().fg(text::DIMMED)
    }

    /// Separator line
    pub fn separator() -> Style {
        Style::new().fg(ui::SEPARATOR)
    }

    /// Diff addition
    pub fn diff_add() -> Style {
        Style::new().fg(diff::ADDITION)
    }

    /// Diff deletion
    pub fn diff_del() -> Style {
        Style::new().fg(diff::DELETION)
    }

    /// Diff hunk header
    pub fn diff_hunk() -> Style {
        Style::new().fg(diff::HUNK)
    }

    /// Diff context
    pub fn diff_context() -> Style {
        Style::new().fg(diff::CONTEXT)
    }
}
```

---

## Color Reference Table

### Selection (The One Accent)

| Token | Hex | RGB | Usage |
|-------|-----|-----|-------|
| `selection::BACKGROUND` | `#F3037E` | (243, 3, 126) | Selected row background |
| `selection::FOREGROUND` | `#FFFFFF` | (255, 255, 255) | Text on selection |

### Grayscale

| Token | Value | Usage |
|-------|-------|-------|
| `text::DEFAULT` | (terminal fg) | Normal text |
| `text::DIMMED` | Gray | Secondary info |
| `ui::SEPARATOR` | DarkGray | Separator line |

### Diff Colors

| Token | Hex | Usage |
|-------|-----|-------|
| `diff::ADDITION` | `#22C55E` | Added lines |
| `diff::DELETION` | `#EF4444` | Removed lines |
| `diff::CONTEXT` | Gray | Context lines |
| `diff::HUNK` | `#3B82F6` | Hunk headers |

---

## Style Reference

| Style | Background | Foreground | Usage |
|-------|------------|------------|-------|
| `selected()` | `#F3037E` | White | Selected HUD row |
| `normal()` | - | (default) | Active content |
| `dimmed()` | - | Gray | Inactive content |
| `separator()` | - | DarkGray | Separator |

---

## Usage Guidelines

### Do

- Import colors from this module
- Use `styles::selected()` for selection
- Use `Style::default()` for normal text
- Use `styles::dimmed()` for secondary info

### Don't

- Define colors inline
- Add new accent colors
- Use semantic status colors (green=good, red=bad)
- Use background colors except for selection

### Example

```rust
use crate::theme::{colors, styles};

// Good - using style presets
let row_style = if is_selected {
    styles::selected()
} else {
    styles::dimmed()
};

// Good - using color constant
let separator = Span::styled(line, styles::separator());

// Bad - inline color definition
let style = Style::new().bg(Color::Rgb(139, 92, 246));  // Don't add new colors!
```

---

## Terminal Compatibility

### TrueColor (24-bit)

Exact colors as specified:
- `#F3037E` selection
- `#22C55E` additions
- `#EF4444` deletions
- `#3B82F6` hunk headers

### 256-color

Close approximations:
- Selection: Color256(33) (blue)
- Add: Color256(41) (green)
- Del: Color256(203) (red)
- Hunk: Color256(33) (blue)

### 16-color Fallback

Basic ANSI:
- Selection: Color::Blue
- Add: Color::Green
- Del: Color::Red
- Dimmed: Color::Gray

---

## Why So Minimal?

The previous color system had 50+ color tokens and 20+ style presets. This was:

1. **Complex** - Hard to maintain consistency
2. **Slow** - Animation calculations every frame
3. **Fragile** - TrueColor required, broke on some terminals
4. **Noisy** - Chromatic motion was distracting

The Pilot's Seat needs only:
- 1 accent color (selection)
- 2 grayscale levels (normal, dimmed)
- 4 diff colors (universal convention)

Total: **7 colors** vs 50+
