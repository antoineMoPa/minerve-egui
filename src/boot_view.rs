//! The boot screen in the window: the cells of `boot::BootClip::cells`,
//! drawn on a character grid of the monospace font.

use eframe::egui::{self, Align2, Color32, Rect};


use crate::boot::{BootClip, BootShade};
use crate::theme;

fn color(shade: BootShade) -> Color32 {
    match shade {
        BootShade::Dot => theme::c().dark_gray,
        BootShade::Video => theme::c().prose,
        BootShade::Caption => theme::c().gray,
        BootShade::Dash => theme::c().bright,
    }
}

pub fn paint(ui: &egui::Ui, rect: Rect, boot: &BootClip) {
    ui.painter().rect_filled(rect, 0.0, theme::c().background);
    let font = theme::mono();
    let (glyph_width, row_height) = ui.fonts_mut(|fonts| (fonts.glyph_width(&font, 'x'), fonts.row_height(&font)));
    let cols = (rect.width() / glyph_width).floor() as i32;
    let rows = (rect.height() / row_height).floor() as i32;
    if cols < 1 || rows < 1 {
        return;
    }
    // Later cells cover earlier ones, a blank erases.
    let mut grid: Vec<Option<(char, BootShade)>> = vec![None; (cols * rows) as usize];
    for cell in boot.cells(cols, rows) {
        grid[(cell.y * cols + cell.x) as usize] = (cell.ch != ' ').then_some((cell.ch, cell.shade));
    }
    let painter = ui.painter_at(rect);
    for row in 0..rows {
        // One text per run of cells with the same color; gaps are spaces, which line up in a monospace font.
        let mut col = 0;
        while col < cols {
            let Some((_, shade)) = grid[(row * cols + col) as usize] else {
                col += 1;
                continue;
            };
            let start = col;
            let mut text = String::new();
            while col < cols {
                match grid[(row * cols + col) as usize] {
                    Some((ch, s)) if s == shade => text.push(ch),
                    _ => break,
                }
                col += 1;
            }
            let pos = rect.min + egui::vec2(start as f32 * glyph_width, row as f32 * row_height);
            painter.text(pos, Align2::LEFT_TOP, text, font.clone(), color(shade));
        }
    }
    ui.ctx().request_repaint_after(boot.time_to_next_frame());
}
