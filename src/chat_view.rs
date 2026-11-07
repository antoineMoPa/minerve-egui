//! The chat, labeled and colored like the TUI's (`chat_lines` in `src/tui/draw.rs`), spaced
//! like a reading column: a role label, an accent bar beside each message, one line per tool call.

use eframe::egui::{self, text::LayoutJob, Color32, FontFamily, FontId, RichText, Stroke, TextFormat};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::theme;

/// Longest tool output shown whole, like the TUI's `MAX_OUTPUT_LEN`.
const MAX_OUTPUT_LEN: usize = 500;
/// Role label of the `/goal` evaluator's prompts in the transcript.
const GOAL_EVALUATOR_ROLE: &str = "goal evaluator";
/// Space between the accent bar and the text.
const BAR_OFFSET: f32 = 12.0;
/// Space between consecutive tool rows.
const TOOL_ROW_GAP: f32 = 3.0;

pub struct ChatView {
    markdown_cache: CommonMarkCache,
}

fn label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).font(theme::small()).color(color));
    ui.add_space(4.0);
}

fn is_tool(role: &str) -> bool {
    !matches!(role, "user" | "minerve" | GOAL_EVALUATOR_ROLE)
}

/// `▸ name  N lines · first line`, dim arrow and bright name, with the key to expand on the right.
fn tool_row(ui: &mut egui::Ui, name: &str, content: &str) {
    let font = FontId::new(12.0, FontFamily::Monospace);
    let format = |color| TextFormat { font_id: font.clone(), color, ..Default::default() };
    let lines = content.lines().count();
    let first = content.lines().next().unwrap_or_default();
    let mut job = LayoutJob::default();
    job.append("▸ ", 0.0, format(theme::DIM));
    job.append(name, 0.0, format(theme::NAME));
    job.append(&format!("  {lines} line{} · {first}", if lines == 1 { "" } else { "s" }), 0.0, format(theme::DARK_GRAY));
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    ui.horizontal(|ui| {
        ui.label(job);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("Ctrl+O").font(theme::small()).color(theme::DIM));
        });
    });
}

impl ChatView {
    pub fn new() -> Self {
        Self { markdown_cache: CommonMarkCache::default() }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, transcript: &[(String, String)], notices: &[String], pending_shell: &[String], tools_expanded: bool) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let visible: Vec<&(String, String)> = transcript.iter().filter(|(role, _)| role != "system").collect();
            for (index, (role, content)) in visible.iter().enumerate() {
                if is_tool(role) && !tools_expanded {
                    tool_row(ui, role, content);
                    // Consecutive tool rows sit close; the group is separated like any message.
                    let next_is_tool = visible.get(index + 1).is_some_and(|(next, _)| is_tool(next));
                    ui.add_space(if next_is_tool { TOOL_ROW_GAP } else { theme::MESSAGE_GAP });
                    continue;
                }
                let (name, color) = match role.as_str() {
                    "user" => ("You", theme::GREEN),
                    "minerve" => ("Minerve", theme::CYAN),
                    GOAL_EVALUATOR_ROLE => ("Goal evaluator", theme::MAGENTA),
                    other => (other, theme::NAME),
                };
                self.message(ui, name, color, |ui, cache| {
                    if role == "minerve" {
                        CommonMarkViewer::new().show(ui, cache, content.trim());
                    } else if content.chars().count() > MAX_OUTPUT_LEN {
                        let head: String = content.chars().take(MAX_OUTPUT_LEN).collect();
                        ui.label(format!("{head}\n...[truncated]"));
                    } else {
                        ui.label(content.as_str());
                    }
                });
            }
            // Shell output queued with `!command`: sent with the next message.
            for output in pending_shell {
                self.message(ui, "Shell (queued, sent with your next message)", theme::YELLOW, |ui, cache| {
                    CommonMarkViewer::new().show(ui, cache, output);
                });
            }
            // Results of local commands. Not sent to the model.
            for notice in notices {
                self.message(ui, "Command", theme::CYAN, |ui, _| {
                    ui.label(RichText::new(notice.as_str()).color(theme::GRAY));
                });
            }
        });
    }

    /// A label, the body, and an accent bar in the role's color beside both.
    fn message(&mut self, ui: &mut egui::Ui, name: &str, color: Color32, body: impl FnOnce(&mut egui::Ui, &mut CommonMarkCache)) {
        let left = ui.max_rect().left();
        let top = ui.cursor().top();
        label(ui, name, color);
        body(ui, &mut self.markdown_cache);
        let bottom = ui.cursor().top();
        ui.painter().vline(left - BAR_OFFSET, top..=bottom, Stroke::new(2.0, color.gamma_multiply(0.55)));
        ui.add_space(theme::MESSAGE_GAP);
    }
}
