//! The look of the TUI in a window: monospace, square corners, thin strokes, and the same
//! colors the TUI uses for each part of the chat (see `src/tui/draw.rs`).

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle};

/// The colors of the view. `Colors::default()` is the TUI's dark look; an app that embeds the
/// view hands it its own (`MinerveView::set_colors`) so minerve sits in its palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    /// Dark widgets (egui's dark visuals) or light ones.
    pub dark: bool,
    /// Window and panel background.
    pub background: Color32,
    /// Fill of the message box, a step off the background.
    pub surface: Color32,
    /// The tinted status bar along the bottom edge.
    pub bar: Color32,
    /// The faintest lines.
    pub hairline: Color32,
    /// Separators between the parts of a status line.
    pub sep: Color32,
    /// Arrows and other marks that should barely show.
    pub dim: Color32,
    /// Labels: dimmer than what they label.
    pub dark_gray: Color32,
    /// Secondary text and values.
    pub gray: Color32,
    /// Names of tools.
    pub name: Color32,
    /// The chat's text.
    pub prose: Color32,
    /// What stands out most: the cost, a pressed widget.
    pub bright: Color32,
    pub code_background: Color32,
    pub selection: Color32,
    /// The frame of the message box, the user's name, links: cyan in the TUI.
    pub accent: Color32,
    /// Status and errors.
    pub magenta: Color32,
    pub green: Color32,
    /// The queued shell output block.
    pub yellow: Color32,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            dark: true,
            background: Color32::from_rgb(0x0f, 0x14, 0x17),
            surface: Color32::from_rgb(0x13, 0x1a, 0x1e),
            bar: Color32::from_rgb(31, 42, 48),
            hairline: Color32::from_rgb(0x1d, 0x25, 0x2a),
            sep: Color32::from_rgb(0x3d, 0x46, 0x4c),
            dim: Color32::from_rgb(0x4f, 0x58, 0x5e),
            dark_gray: Color32::from_rgb(0x7a, 0x83, 0x8a),
            gray: Color32::from_rgb(0xb0, 0xb4, 0xb8),
            name: Color32::from_rgb(0xa9, 0xb1, 0xb6),
            prose: Color32::from_rgb(0xc9, 0xcf, 0xd3),
            bright: Color32::WHITE,
            code_background: Color32::from_rgb(0x1d, 0x27, 0x2c),
            selection: Color32::from_rgb(0x1d, 0x4a, 0x52),
            accent: Color32::from_rgb(0x3d, 0xc8, 0xd8),
            magenta: Color32::from_rgb(0xd0, 0x6a, 0xd8),
            green: Color32::from_rgb(0x4e, 0xc9, 0x6a),
            yellow: Color32::from_rgb(0xe0, 0xb8, 0x4a),
        }
    }
}

thread_local! {
    /// The colors of the view being drawn: set by `MinerveView::show` for its frame, so the
    /// drawing code reads them without being handed them.
    static CURRENT: std::cell::Cell<Colors> = std::cell::Cell::new(Colors::default());
}

/// The colors of the view being drawn.
pub fn c() -> Colors {
    CURRENT.with(std::cell::Cell::get)
}

pub(crate) fn set(colors: Colors) {
    CURRENT.with(|current| current.set(colors));
}

pub const FONT_SIZE: f32 = 14.0;
/// Margins of the chat column and the panels under it.
pub const CHAT_MARGIN: egui::Margin = egui::Margin { left: 24, right: 20, top: 16, bottom: 0 };
pub const PANEL_MARGIN: i8 = 16;
/// Space between two messages.
pub const MESSAGE_GAP: f32 = 22.0;
/// Where macOS keeps the system sans, used for the chat prose when it is there.
const SYSTEM_SANS: &str = "/System/Library/Fonts/SFNS.ttf";

pub fn mono() -> FontId {
    FontId::new(FONT_SIZE, FontFamily::Monospace)
}

/// Small mono text: role labels, tool rows, usage, status and the hint.
pub fn small() -> FontId {
    FontId::new(12.0, FontFamily::Monospace)
}

/// The system sans for prose when this machine has one; the built-in proportional font otherwise.
fn install_fonts(ctx: &egui::Context) {
    let Ok(bytes) = std::fs::read(SYSTEM_SANS) else { return };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("system-sans".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "system-sans".to_owned());
    ctx.set_fonts(fonts);
}

/// The whole app's look, for the standalone window: dark theme, system sans, and `style` everywhere.
pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    install_fonts(ctx);
    ctx.all_styles_mut(style);
}

/// minerve's look on one style. An app that embeds `MinerveView` keeps its own fonts and
/// theme; the view applies this to its own `Ui` only.
pub fn style(style: &mut egui::Style) {
    let c = c();
    {
        style.text_styles.insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Heading, FontId::new(18.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Button, FontId::new(13.0, FontFamily::Monospace));
        style.text_styles.insert(TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace));
        style.text_styles.insert(TextStyle::Small, small());
        style.spacing.scroll = egui::style::ScrollStyle::floating();
        style.spacing.scroll.bar_width = 6.0;
        let visuals = &mut style.visuals;
        *visuals = if c.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        visuals.panel_fill = c.background;
        visuals.window_fill = c.background;
        visuals.extreme_bg_color = c.background;
        visuals.override_text_color = Some(c.prose);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, c.bright);
        visuals.code_bg_color = c.code_background;
        visuals.window_corner_radius = egui::CornerRadius::ZERO;
        visuals.menu_corner_radius = egui::CornerRadius::ZERO;
        visuals.selection.bg_fill = c.selection;
        visuals.selection.stroke = Stroke::new(1.0, c.accent);
        visuals.text_cursor.stroke = Stroke::new(2.0, c.accent);
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::ZERO;
        }
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, c.accent);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, c.bright);
        style.spacing.item_spacing = egui::vec2(0.0, 0.0);
    }
}
