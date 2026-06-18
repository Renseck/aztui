use ratatui:: style::{Color, Modifier, Style};

/// The application color theme. All colors are specified as [`Color::Rgb`] for
/// true-color terminals, with a 256-color fallback via [`Theme"::default_256`].
/// 
/// See `docs/color-scheme.md` for the full design intent and semantic assignments.
#[derive(Debug, Clone)]
pub struct Theme {
    // Core backgrounds
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
    /// True-color theme matching `docs/color-scheme.md`.
    pub fn default_dark() -> Self {
        Self {
            base: Color::Rgb(26, 27, 38),
            surface: Color::Rgb(36, 38, 58),
            overlay: Color::Rgb(47, 49, 70),
            muted: Color::Rgb(86, 90, 110),
            subtle: Color::Rgb(139, 143, 163),
            text: Color::Rgb(200, 202, 216),
            bright: Color::Rgb(232, 233, 240),
            azure: Color::Rgb(0, 120, 212),
            azure_light: Color::Rgb(77, 166, 255),
            teal: Color::Rgb(46, 196, 182),
            green: Color::Rgb(89, 201, 144),
            amber: Color::Rgb(224, 165, 38),
            red: Color::Rgb(224, 82, 99),
            dimmed: Color::Rgb(74, 77, 94),
        }
    }
    
    /* ========================================================================================== */
    /// 256-color fallback for terminals without true-color support.
    pub fn default_256() -> Self {
        Self {
            base: Color::Indexed(234),
            surface: Color::Indexed(236),
            overlay: Color::Indexed(237),
            muted: Color::Indexed(242),
            subtle: Color::Indexed(248),
            text: Color::Indexed(252),
            bright: Color::Indexed(255),
            azure: Color::Indexed(32),
            azure_light: Color::Indexed(75),
            teal: Color::Indexed(43),
            green: Color::Indexed(78),
            amber: Color::Indexed(178),
            red: Color::Indexed(167),
            dimmed: Color::Indexed(240),
        }
    }

    /* ========================================================================================== */
    /// Detects true-color support via the `COLORTERM` environment variable.
    pub fn detect() -> Self {
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        if colorterm == "truecolor" || colorterm == "24bit" {
            Self::default_dark()
        } else {
            Self::default_256()
        }
    }

    /* ========================================================================================== */
    /*                                   Semantic style helpers                                   */
    /* ========================================================================================== */

    pub fn base_style(&self) -> Style {
        Style::default().bg(self.base).fg(self.text)
    }

    pub fn surface_style(&self) -> Style {
        Style::default().bg(self.surface).fg(self.text)
    }

    /// Style for the currently selected list row.
    pub fn selected_style(&self) -> Style {
        // ? Add this to Theme?
        Style::default()
            .bg(Color::Rgb(30, 45, 66)) 
            .fg(self.bright)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for the active (az-pointede-at) context row.
    pub fn active_context_style(&self) -> Style {
        Style::default().fg(self.green).add_modifier(Modifier::BOLD)
    }

    /// Style for disabled/warned subscription rows.
    pub fn dimmed_style(&self) -> Style {
        Style::default().fg(self.dimmed)
    }
    
    /// Style for tenant section headers.
    pub fn tenant_header_style(&self) -> Style {
        Style::default()
            .fg(self.azure_light)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for the resource-type label of a VM row (distinct + actionable).
    pub fn vm_type_style(&self) -> Style {
        Style::default().fg(self.teal).add_modifier(Modifier::BOLD)
    }

    /// Style for the status bar.
    pub fn status_bar_style(&self) -> Style {
        Style::default().bg(self.surface).fg(self.subtle)
    }

    /// Style for active context in status bar.
    pub fn active_context_indicator_style(&self) -> Style {
        Style::default().fg(self.teal).add_modifier(Modifier::BOLD)
    }

    /// Style for in-progress spinner/operation text.
    pub fn spinner_style(&self) -> Style {
        Style::default().fg(self.amber)
    }

    /// Style for error text.
    pub fn error_style(&self) -> Style {
        Style::default().fg(self.red)
    }

    /// Style for modal borders.
    pub fn modal_border_style(&self) -> Style {
        Style::default().fg(self.azure_light)
    }

    /// Style for confirmation modal border.
    pub fn confirm_border_style(&self) -> Style {
        Style::default().fg(self.amber)
    }

    /// Style for error modal border.
    pub fn error_border_style(&self) -> Style {
        Style::default().fg(self.red)
    }

    /// Style for search input border when focused.
    pub fn search_focused_style(&self) -> Style {
        Style::default().fg(self.azure_light)
    }

    /// Style for content area border when focused.
    pub fn content_focused_style(&self) -> Style {
        Style::default().fg(self.azure)
    }

    /// Style for content area border when unfocused.
    pub fn content_border_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    /// Style for key hints.
    pub fn hint_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    /// Brighter text style for headings.
    pub fn heading_style(&self) -> Style {
        Style::default().fg(self.bright).add_modifier(Modifier::BOLD)
    }

    /// Style for highlighted fuzzy-match characters (see docs/color-scheme.md).
    pub fn match_style(&self) -> Style {
        Style::default().fg(self.amber).add_modifier(Modifier::BOLD)
    }
}