# Color Scheme — aztui

> Defines the application's color palette and semantic color assignments.
> All colors are specified as hex values. The Ratatui `Color::Rgb(r, g, b)`
> constructor accepts these directly. A 256-color fallback is provided for
> terminals without truecolor support.

---

## Design Intent

The palette is Azure-adjacent: it draws from the blue family that Azure is
known for, but shifts toward cooler, more muted tones that are comfortable
for long terminal sessions. High-saturation blues cause eye strain on dark
backgrounds, so the primaries are desaturated and the backgrounds are warm
dark grays rather than pure black.

The scheme assumes a **dark background** as the default (most terminal users
and ops teams run dark themes). A light variant is out of scope for now but
the architecture supports it via `theme.rs` if needed later.

---

## Palette

### Core Colors

| Role            | Name          | Hex       | RGB             | 256-color | Preview |
|-----------------|---------------|-----------|-----------------|-----------|---------|
| Background      | `base`        | `#1a1b26` | `26, 27, 38`    | `234`     | <span style="background:#1a1b26;color:#1a1b26">░░░░</span> |
| Surface         | `surface`     | `#24263a` | `36, 38, 58`    | `236`     | <span style="background:#24263a;color:#24263a">░░░░</span> |
| Overlay         | `overlay`     | `#2f3146` | `47, 49, 70`    | `237`     | <span style="background:#2f3146;color:#2f3146">░░░░</span> |
| Muted           | `muted`       | `#565a6e` | `86, 90, 110`   | `242`     | <span style="background:#565a6e;color:#565a6e">░░░░</span> |
| Subtle          | `subtle`      | `#8b8fa3` | `139, 143, 163` | `248`     | <span style="background:#8b8fa3;color:#8b8fa3">░░░░</span> |
| Text            | `text`        | `#c8cad8` | `200, 202, 216` | `252`     | <span style="background:#c8cad8;color:#c8cad8">░░░░</span> |
| Bright text     | `bright`      | `#e8e9f0` | `232, 233, 240` | `255`     | <span style="background:#e8e9f0;color:#e8e9f0">░░░░</span> |

### Accent Colors

| Role            | Name          | Hex       | RGB             | 256-color | Preview |
|-----------------|---------------|-----------|-----------------|-----------|---------|
| Primary         | `azure`       | `#0078d4` | `0, 120, 212`   | `32`      | <span style="background:#0078d4;color:#0078d4">░░░░</span> |
| Primary bright  | `azure_light` | `#4da6ff` | `77, 166, 255`  | `75`      | <span style="background:#4da6ff;color:#4da6ff">░░░░</span> |
| Secondary       | `teal`        | `#2ec4b6` | `46, 196, 182`  | `43`      | <span style="background:#2ec4b6;color:#2ec4b6">░░░░</span> |
| Success         | `green`       | `#59c990` | `89, 201, 144`  | `78`      | <span style="background:#59c990;color:#59c990">░░░░</span> |
| Warning         | `amber`       | `#e0a526` | `224, 165, 38`  | `178`     | <span style="background:#e0a526;color:#e0a526">░░░░</span> |
| Error           | `red`         | `#e05263` | `224, 82, 99`   | `167`     | <span style="background:#e05263;color:#e05263">░░░░</span> |
| Disabled        | `dimmed`      | `#4a4d5e` | `74, 77, 94`    | `240`     | <span style="background:#4a4d5e;color:#4a4d5e">░░░░</span> |

---

## Semantic Assignments

### Backgrounds

| Component                      | Color                          |
|--------------------------------|--------------------------------|
| Main application background    | `base`                         |
| Content area (list panels)     | `surface`                      |
| Modal overlay background       | `overlay`                      |
| Status bar background          | `surface`                      |
| Error notification bar         | `red` at 15% opacity over `surface` (or `#3a2029` solid fallback) |
| Search input background        | `overlay`                      |
| Selected/highlighted row       | `azure` at 20% opacity over `surface` (or `#1e2d42` solid fallback) |

### Text

| Component                      | Color                          |
|--------------------------------|--------------------------------|
| Primary body text              | `text`                         |
| Headings / titles              | `bright`                       |
| Tenant section headers         | `azure_light`                  |
| Subscription names             | `text`                         |
| Subscription IDs (GUID)        | `subtle`                       |
| Disabled/warned subscription   | `dimmed`                       |
| Search input text              | `bright`                       |
| Search placeholder text        | `muted`                        |
| Status bar text                | `subtle`                       |
| Active context in status bar   | `teal`                         |
| Key hints in status bar        | `muted`                        |
| Error notification text        | `red`                          |
| Success message text           | `green`                        |
| Spinner / in-progress text     | `amber`                        |

### Borders

| Component                      | Color                          |
|--------------------------------|--------------------------------|
| Content area border            | `muted`                        |
| Content area border (focused)  | `azure`                        |
| Modal border                   | `azure_light`                  |
| Status bar separator line      | `muted`                        |
| Confirmation dialog border     | `amber`                        |
| Error detail modal border      | `red`                          |
| Search input border            | `muted`                        |
| Search input border (focused)  | `azure_light`                  |

### Interactive Elements

| Component                      | Color                          |
|--------------------------------|--------------------------------|
| Cursor / selected row marker   | `azure`                        |
| Cursor row text                | `bright`                       |
| Active subscription checkmark  | `green`                        |
| Recent context marker (○)      | `teal`                         |
| Quick switch match highlight   | `azure_light` (bold)           |
| Fuzzy match character highlight| `amber`                        |
| Confirm button [Yes] focused   | `azure` bg, `bright` text      |
| Confirm button [No] focused    | `muted` bg, `text` text        |
| Password dots                  | `azure_light`                  |

### Data Display (Phase 3+)

| Component                      | Color                          |
|--------------------------------|--------------------------------|
| Resource type column           | `subtle`                       |
| Location column                | `muted`                        |
| Tag keys                       | `teal`                         |
| Tag values                     | `text`                         |
| Breadcrumb separators (▸)      | `muted`                        |
| Breadcrumb active segment      | `azure_light`                  |
| Breadcrumb inactive segments   | `subtle`                       |

### Cost Explorer (Phase 4)

| Component                      | Color                          |
|--------------------------------|--------------------------------|
| Cost total amount              | `bright`                       |
| Cost bar filled portion (█)    | `azure`                        |
| Cost bar empty portion (░)     | `overlay`                      |
| Percentage column              | `subtle`                       |
| Period selector arrows          | `azure_light`                  |
| Cost increase vs prior period  | `red`                          |
| Cost decrease vs prior period  | `green`                        |
| Currency symbol                | `muted`                        |

### Sparklines & Metrics (Future)

| Component                      | Color                          |
|--------------------------------|--------------------------------|
| Sparkline normal range         | `azure`                        |
| Sparkline high/alert range     | `amber`                        |
| Sparkline critical range       | `red`                          |
| Metric label                   | `subtle`                       |
| Metric value                   | `bright`                       |
| Metric unit                    | `muted`                        |

---

## Implementation Notes

### theme.rs structure

```rust
pub struct Theme {
    // Core
    pub base: Color,
    pub surface: Color,
    pub overlay: Color,
    pub muted: Color,
    pub subtle: Color,
    pub text: Color,
    pub bright: Color,

    // Accents
    pub azure: Color,
    pub azure_light: Color,
    pub teal: Color,
    pub green: Color,
    pub amber: Color,
    pub red: Color,
    pub dimmed: Color,
}

impl Theme {
    pub fn default_dark() -> Self {
        Self {
            base:        Color::Rgb(26, 27, 38),
            surface:     Color::Rgb(36, 38, 58),
            overlay:     Color::Rgb(47, 49, 70),
            muted:       Color::Rgb(86, 90, 110),
            subtle:      Color::Rgb(139, 143, 163),
            text:        Color::Rgb(200, 202, 216),
            bright:      Color::Rgb(232, 233, 240),
            azure:       Color::Rgb(0, 120, 212),
            azure_light: Color::Rgb(77, 166, 255),
            teal:        Color::Rgb(46, 196, 182),
            green:       Color::Rgb(89, 201, 144),
            amber:       Color::Rgb(224, 165, 38),
            red:         Color::Rgb(224, 82, 99),
            dimmed:      Color::Rgb(74, 77, 94),
        }
    }

    /// Fallback for terminals without truecolor.
    pub fn default_256() -> Self {
        Self {
            base:        Color::Indexed(234),
            surface:     Color::Indexed(236),
            overlay:     Color::Indexed(237),
            muted:       Color::Indexed(242),
            subtle:      Color::Indexed(248),
            text:        Color::Indexed(252),
            bright:      Color::Indexed(255),
            azure:       Color::Indexed(32),
            azure_light: Color::Indexed(75),
            teal:        Color::Indexed(43),
            green:       Color::Indexed(78),
            amber:       Color::Indexed(178),
            red:         Color::Indexed(167),
            dimmed:      Color::Indexed(240),
        }
    }
}
```

### Truecolor detection

At startup, check for truecolor support via the `COLORTERM` environment
variable (values `truecolor` or `24bit`). Fall back to the 256-color theme
if not detected. This should be handled in `main.rs` during terminal setup
and stored in `AppConfig` or passed to the `Theme` constructor.

### Accessibility considerations

- All text colors maintain a minimum contrast ratio of 4.5:1 against their
  background (WCAG AA).
- Error and warning states never rely on color alone — they also use symbols
  (⚠, ✗) and text labels.
- The `dimmed` color for disabled items is still distinguishable from the
  background; it does not disappear.

### Future: user-customizable themes

The `Theme` struct is designed so that a `[ui.colors]` section could be added
to `config.toml` in the future, allowing partial overrides of individual
colors. This is not a Phase 1 concern but the structure supports it.