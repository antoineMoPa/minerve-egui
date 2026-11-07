//! The look of the TUI in a window: monospace, square corners, thin strokes, and the same
//! colors the TUI uses for each part of the chat (see `src/tui/draw.rs`).

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle};

pub const GREEN: Color32 = Color32::from_rgb(0x4e, 0xc9, 0x6a);
pub const CYAN: Color32 = Color32::from_rgb(0x3d, 0xc8, 0xd8);
pub const MAGENTA: Color32 = Color32::from_rgb(0xd0, 0x6a, 0xd8);
pub const GRAY: Color32 = Color32::from_rgb(0xb0, 0xb4, 0xb8);
pub const DARK_GRAY: Color32 = Color32::from_rgb(0x7a, 0x83, 0x8a);
/// Arrows and other marks that should barely show.
pub const DIM: Color32 = Color32::from_rgb(0x4f, 0x58, 0x5e);
/// Separators between the parts of a status line.
pub const SEP: Color32 = Color32::from_rgb(0x3d, 0x46, 0x4c);
/// The thin line above the usage summary.
pub const HAIRLINE: Color32 = Color32::from_rgb(0x1d, 0x25, 0x2a);
/// Fill of the message box, a step above the background.
pub const SURFACE: Color32 = Color32::from_rgb(0x13, 0x1a, 0x1e);
/// The queued shell output block.
pub const YELLOW: Color32 = Color32::from_rgb(0xe0, 0xb8, 0x4a);
/// Names of tools and the keys in the hint line.
pub const NAME: Color32 = Color32::from_rgb(0xa9, 0xb1, 0xb6);
const PROSE: Color32 = Color32::from_rgb(0xc9, 0xcf, 0xd3);
const CODE_BACKGROUND: Color32 = Color32::from_rgb(0x1d, 0x27, 0x2c);
pub const WHITE: Color32 = Color32::WHITE;
/// Window and panel background.
pub const BACKGROUND: Color32 = Color32::from_rgb(0x0f, 0x14, 0x17);
const SELECTION: Color32 = Color32::from_rgb(0x1d, 0x4a, 0x52);

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

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    install_fonts(ctx);
    ctx.all_styles_mut(|style| {
        style.text_styles.insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Heading, FontId::new(18.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Button, FontId::new(13.0, FontFamily::Monospace));
        style.text_styles.insert(TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace));
        style.text_styles.insert(TextStyle::Small, small());
        style.spacing.scroll = egui::style::ScrollStyle::floating();
        style.spacing.scroll.bar_width = 6.0;
        let visuals = &mut style.visuals;
        *visuals = egui::Visuals::dark();
        visuals.panel_fill = BACKGROUND;
        visuals.window_fill = BACKGROUND;
        visuals.extreme_bg_color = BACKGROUND;
        visuals.override_text_color = Some(PROSE);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, WHITE);
        visuals.code_bg_color = CODE_BACKGROUND;
        visuals.window_corner_radius = egui::CornerRadius::ZERO;
        visuals.menu_corner_radius = egui::CornerRadius::ZERO;
        visuals.selection.bg_fill = SELECTION;
        visuals.selection.stroke = Stroke::new(1.0, CYAN);
        visuals.text_cursor.stroke = Stroke::new(2.0, CYAN);
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::ZERO;
        }
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, CYAN);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, WHITE);
        style.spacing.item_spacing = egui::vec2(0.0, 0.0);
    });
}
