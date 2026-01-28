# UI Design: The Pilot's Seat

> Remove all TUI clutter. Rely entirely on whitespace, color, and position.

**Related docs:**
- [Theme Design](./theme-design.md) - Grayscale + accent color specification
- [Color Tokens](./color-tokens.md) - Minimal semantic color definitions

---

## Design Philosophy

> "You are the pilot. The interface is your windshield."

The UI follows these principles:

1. **Zero chrome** - No box drawing characters, no ASCII art borders, no decorative labels
2. **Whitespace as structure** - Position and spacing create hierarchy
3. **Color as meaning** - Grayscale for text, one accent color for selection
4. **Scrolloff navigation** - Selection stays centered; content flows past you
5. **Terminal native** - Viewport looks exactly like a raw terminal window

---

## Layout Structure

```
+---------------------------------------------------------------------------+
|                                                                           |
|  VIEWPORT (flexible height)                                               |
|  Raw tmux capture - edge-to-edge, zero UI chrome                          |
|  This is the "Windshield" - pure terminal content                         |
|                                                                           |
+---------------------------------------------------------------------------|  <- SEPARATOR (1 line)
|  backend      add-user-api                        gpt4       45m         |  <- HUD line 1
|  backend      refactor-db                         gemini     2h          |  <- HUD line 2
| [frontend     fix-auth                            claude     12m       ] |  <- SELECTED (centered)
|  backend      sql-migration                       codex      5m          |  <- HUD line 4
|  frontend     write-tests                         local      1m          |  <- HUD line 5
+---------------------------------------------------------------------------+
```

Three regions:
1. **Viewport** - Flexible height, raw tmux capture, edge-to-edge with no margins
2. **Separator** - Single horizontal line divider anchoring the eye
3. **HUD** - Fixed height (5 lines), scrollable session list with scrolloff

---

## The Three Regions

### 1. Viewport (Top)

The viewport is pure terminal output, exactly as if you ran the command yourself:

- **Flexible height**: Takes all available space above the HUD
- **Edge-to-edge**: No margins, no padding, no borders
- **Raw content**: Preserves ANSI colors from the underlying tmux pane
- **Zero chrome**: Looks indistinguishable from a native terminal window

This is the "windshield" - you see exactly what the agent sees.

### 2. Separator (Divider)

A single horizontal line separating viewport from HUD:

- **1 line tall**: Uses Unicode horizontal line character or dashes
- **Full width**: Spans the entire terminal width
- **Subtle color**: Gray or terminal default, not distracting
- **Visual anchor**: Prevents visual bleed between regions

```
----------------------------------------------------------------------------------------------------
```

### 3. HUD (Bottom)

Fixed-height session list with centered selection:

- **Fixed height**: Exactly 5 lines (configurable)
- **Scrolloff behavior**: Selected item always centered at line 3
- **Column layout**: Name | Status | Activity | Time
- **Selection highlight**: Blue background on selected row

---

## The Scrolloff Mechanism

**Critical UX feature**: The selected item is ALWAYS vertically centered in the HUD.

### How It Works

If HUD is 5 lines tall, selection stays pinned to line 3 (center):
- When pressing Down, the LIST moves up (not the cursor)
- The selection highlight stays stationary; content flows past it
- **Feel**: "You are stationary; the list flows past you"

### Example Navigation

```
Initial state (5 sessions, selected = 0):
  session-1 (selected, at center)
  session-2
  session-3
  session-4
  session-5

After pressing Down (selected = 1):
  session-1 (shifted up)
  session-2 (now selected, still at center)
  session-3
  session-4
  session-5

After pressing Down again (selected = 2):
  session-1
  session-2
  session-3 (now selected, still at center)
  session-4
  session-5
```

### Edge Cases

At the start/end of the list, the selection may not be perfectly centered:
- At start: Cannot scroll past beginning, selection appears higher
- At end: Cannot scroll past end, selection appears lower

### No Manual Scroll Controls

Navigation IS scrolling:
- **Removed**: Ctrl+u/d, PageUp/PageDown, manual scroll actions
- **j/k or arrows**: Move selection AND scroll the list
- Simpler mental model, fewer key bindings to remember

---

## HUD Column Layout

Four columns with strict alignment. Status is communicated via **typography and color**, not a separate column.

| Column | Width | Content | Styling |
|--------|-------|---------|---------|
| PROJECT | ~12ch | Repository name | **Secondary** - dimmed |
| SESSION | Flex | Session title (becomes branch like `user/fix-auth`) | **Primary focus** - bright. Color = status indicator |
| AGENT | ~8ch | Agent brain | **Secondary** - dimmed dark grey |
| IDLE | ~6ch | Time since last activity | Color-coded by duration |

### Project Column

The repository/project name where this session is working:

- Examples: `my-app`, `backend`, `frontend-web`
- Dimmed styling - secondary information
- Useful when managing sessions across multiple repos

### Session Column (Status via Color)

The session name IS the status light. No separate status column needed.

- **Active/Thinking**: Cyan or Blue
- **Error**: Red
- **Normal**: Default text color

### Model Column

- Examples: `claude`, `gpt4`, `gemini`, `codex`, `local`
- Always dimmed - secondary information
- "You only check this if wondering why agent is being smart/dumb"

### Idle Column (The "Stuck" Detector)

Color thresholds:
- `< 1m`: Green or dimmed (healthy)
- `> 5m`: Yellow (warning)
- `> 30m`: Red (likely abandoned/crashed)

Format: `2s`, `4m`, `1h`

### Example Row

```
  my-app       fix-auth                              claude     2s
  ^-- project  ^-- session (flex)                    ^-- model  ^-- idle
```

---

## The Idleness Metric

**What it is**: How long since the agent last typed a key, updated a file, or emitted a log.

**Why it matters**:
- `Idle: 2s` - It's actively working
- `Idle: 15m` - Might be stuck/looping/waiting for you

**Implementation**: Query tmux activity time or check timestamp of last captured log line.

The idle time replaces the old "duration" concept. Instead of showing how long a session has existed, we show how long since the agent last did something observable. This is far more useful for spotting stuck or abandoned agents.

---

## Swiss-Style Typography System

Status is communicated through **text treatment**, not icons or spinners. This creates a sophisticated, high-end print design aesthetic.

### ACTIVE (High Motion)
- **Treatment**: Shimmering - wave of white intensity moves left-to-right across characters
- **Vibe**: Feels alive and processing

### DONE (High Weight)
- **Treatment**: Bold and Pulsing - solid white/bold text that slowly breathes (dims/brightens)
- **Vibe**: Feels substantial and resolved, waiting for you

### IDLE (Baseline)
- **Treatment**: Normal text - standard weight, standard opacity, no animation
- **Vibe**: Ready and listening, "present"

### PAUSED (Low Presence)
- **Treatment**: Muted - 50% opacity, no animation, recedes into background
- **Vibe**: Dormant, takes no visual priority

This approach uses "grayscale intensity and typographic manipulation" - the most sophisticated approach. It feels like high-end print design or "Swiss Style" typography, avoiding the "video game" look of colorful spinners.

---

## Color Scheme

**Requirement**: Must work on BOTH light and dark terminals.

### Grayscale + One Accent

- **Default text**: Terminal default (inherits from user's theme)
- **Dimmed text**: Gray (for less important info, model column)
- **Selection background**: `#007AFF` (iMessage blue) - the ONLY primary color
- **Selection text**: White on blue background

### Status via Session Name Color

The session name color indicates status:
- **Active/Thinking**: Cyan or Blue - agent is working
- **Error**: Red - something went wrong
- **Normal**: Default text color - ready state

### Idle Column Color Thresholds

The idle time uses color to indicate urgency:
- **< 1m**: Green or dimmed (healthy activity)
- **> 5m**: Yellow (warning - check on it)
- **> 30m**: Red (likely stuck/abandoned)

### Typography-Based Status

Instead of colored indicators, status is conveyed through text treatment:
- **ACTIVE**: Shimmer animation (wave of brightness)
- **DONE**: Bold with slow pulse/breathing
- **IDLE**: Normal text, no effects
- **PAUSED**: Muted to 50% opacity

See [Theme Design](./theme-design.md) for color token definitions and [Animation System](./animation-system.md) for typography effects.

---

## Interaction Modes

### 1. List Mode (Default)

The normal state when browsing sessions:
- Viewport shows selected session's tmux output
- HUD shows session list with centered selection
- All key bindings active

### 2. Attached Mode

Full terminal immersion (press Enter to enter):
- HUD and Separator vanish completely
- Viewport expands to 100% of terminal height
- Full tmux immersion, passthrough input
- Press Esc to detach and return to List Mode

### 3. Input Mode

Simple text input (press `n` for new session):
- HUD area clears
- Shows simple prompt: `New Agent Session: _`
- No popup overlays, no modal dialogs
- Press Enter to submit, Esc to cancel

---

## Example Render

### Basic Layout

```
  > npm install @types/react --save-dev

  added 1 package, and audited 1452 packages in 3s

  found 0 vulnerabilities

  agent@box:~/frontend-web$ _



----------------------------------------------------------------------------------------------------
  backend      add-user-api                         gpt4       2s
  backend      refactor-db                          gemini     12s
  frontend     fix-auth                             claude     0s     <- Selected (blue bg)
  backend      sql-migration                        codex      15m
  frontend     write-tests                          local      1h
```

### With Typography Effects Applied

```
----------------------------------------------------------------------------------------------------
  [SHIMMER]     frontend     fix-auth                   claude     4s
  [NORMAL]      backend      refactor-db                gpt4       15m
  [MUTED]       backend      add-user-api               codex      2h
```

Note: The `[BOLD/Pulse]`, `[SHIMMER]`, etc. annotations indicate the typography treatment applied to each row based on session state. See [Animation System](./animation-system.md) for implementation details.

---

## Key Bindings

### Navigation

| Key | Action |
|-----|--------|
| `j` / Down | Select next session (scrolls list) |
| `k` / Up | Select previous session (scrolls list) |

### Session Management

| Key | Action |
|-----|--------|
| `Enter` | Attach to selected session |
| `n` | Create new session |
| `d` | Delete selected session |
| `p` | Pause/resume selected session |
| `P` | Push session changes |

### View

| Key | Action |
|-----|--------|
| `Tab` | Toggle preview/diff mode |
| `?` | Toggle help |
| `q` / `Esc` | Quit |

### Removed Bindings

The following are intentionally NOT implemented:
- `Ctrl+u/d` - No manual scrolling
- `PageUp/PageDown` - No manual scrolling
- Any scroll-related keys

---

## What's NOT in UI

Following the Pilot's Seat philosophy:

- No box drawing characters (`+`, `-`, `|`, corners)
- No ASCII art borders
- No decorative labels or titles
- No status bar with static text
- No spinner symbols (`|/-\`)
- No arrow cursors (`>`, `>>>`)
- No bracket selection (`[ ]`)
- No progress bar blocks (`[====    ]`)
- No colored status indicators (use text instead)
- No manual scroll controls

**Whitespace, color, and position do the work.**

---

## Implementation Notes

### Viewport Rendering

```rust
impl App {
    fn render_viewport(&self, frame: &mut Frame, area: Rect) {
        // Get raw tmux capture for selected session
        let content = self.selected_preview().unwrap_or_default();

        // Render edge-to-edge, no block, no borders
        let paragraph = Paragraph::new(content);
        frame.render_widget(paragraph, area);
    }
}
```

### Separator Rendering

```rust
impl App {
    fn render_separator(&self, frame: &mut Frame, area: Rect) {
        // Single line of dashes
        let line = "-".repeat(area.width as usize);
        let paragraph = Paragraph::new(line)
            .style(Style::new().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
    }
}
```

### HUD Rendering with Scrolloff

```rust
impl App {
    fn render_hud(&self, frame: &mut Frame, area: Rect) {
        let hud_height = area.height as usize;
        let center = hud_height / 2;

        // Calculate which sessions to show (scrolloff)
        let start = self.selected.saturating_sub(center);
        let end = (start + hud_height).min(self.sessions.len());
        let start = end.saturating_sub(hud_height);

        for (i, session) in self.sessions[start..end].iter().enumerate() {
            let y = area.y + i as u16;
            let is_selected = start + i == self.selected;

            // Get idle duration for color coding
            let idle_duration = self.get_idle_duration(session.id);
            let idle_color = idle_duration_color(idle_duration);

            let base_style = if is_selected {
                Style::new().bg(Color::Rgb(0, 122, 255)).fg(Color::White)
            } else {
                Style::new().fg(Color::Gray)
            };

            // Format: PROJECT | SESSION | AGENT | IDLE
            let row = format_session_row(session, idle_duration, idle_color);
            let span = Span::styled(row, base_style);
            frame.render_widget(Paragraph::new(span), Rect::new(area.x, y, area.width, 1));
        }
    }

    fn get_idle_duration(&self, session_id: SessionId) -> Duration {
        self.last_activity
            .get(&session_id)
            .map(|instant| instant.elapsed())
            .unwrap_or(Duration::ZERO)
    }
}

fn idle_duration_color(duration: Duration) -> Color {
    let minutes = duration.as_secs() / 60;
    match minutes {
        0 => Color::DarkGray,      // < 1m: dimmed (healthy)
        1..=4 => Color::DarkGray,  // 1-5m: still okay
        5..=29 => Color::Yellow,   // 5-30m: warning
        _ => Color::Red,           // > 30m: likely stuck
    }
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
```

---

## File Organization

```
zen/src/
  app.rs              # Application state (no scroll_offset)
  event.rs            # Actions (no ScrollUp/ScrollDown)
  ui.rs               # Render functions (viewport, separator, HUD)
```

Target: Clean, minimal rendering code focused on the three regions.
