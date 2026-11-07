//! The slash commands, and the menu that completes them while you type, like the terminal app's.

use eframe::egui::{self, Align2, RichText};

use crate::theme;

/// Name and one-line description, in the order the terminal app lists them (`/help` is ours).
pub const COMMANDS: [(&str, &str); 12] = [
    ("/about", "describe Minerve"),
    ("/model", "switch models and providers"),
    ("/goal", "keep working until an evaluator model judges the goal reached: /goal <goal>"),
    ("/resume", "continue a saved session"),
    ("/clear", "start a new conversation from fresh"),
    ("/apikeys", "set the OpenRouter, OpenAI and Anthropic keys"),
    ("/mcp", "manage MCP servers: add, connect, disconnect, remove"),
    ("/message-apps", "talk to minerve from Slack: set up, status, link your account"),
    ("/bootscreen", "replay the boot screen"),
    ("/matrix", "falling letters; any key to leave"),
    ("/tetris", "play a mini Tetris; esc to leave"),
    ("/help", "list the commands"),
];

/// Commands whose name starts with the input. Empty unless the input is a single `/word`:
/// a pasted path or a command with arguments is not completed.
pub fn matches(input: &str) -> Vec<(&'static str, &'static str)> {
    if !input.starts_with('/') || input.contains(char::is_whitespace) {
        return Vec::new();
    }
    COMMANDS.iter().filter(|(name, _)| name.starts_with(input)).copied().collect()
}

/// The command a partial `/word` stands for: the first match, as Enter runs it.
pub fn resolve(input: &str) -> Option<&'static str> {
    matches(input).first().map(|(name, _)| *name)
}

/// The menu of matches, floating just above the message box (`input_rect`). Returns the
/// command clicked, to put in the box as Tab would.
pub fn show_menu(ctx: &egui::Context, id: egui::Id, input_text: &str, input_rect: egui::Rect) -> Option<&'static str> {
    let found = matches(input_text);
    if found.is_empty() {
        return None;
    }
    let mut picked = None;
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .pivot(Align2::LEFT_BOTTOM)
        .fixed_pos(input_rect.left_top() - egui::vec2(0.0, 6.0))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(theme::c().surface)
                .stroke(egui::Stroke::new(1.0, theme::c().sep))
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    for (index, (name, description)) in found.iter().enumerate() {
                        // Reserved before the row, so the hover fill is drawn under its text.
                        let fill = ui.painter().add(egui::Shape::Noop);
                        let row = ui.horizontal(|ui| {
                            let name_color = if index == 0 { theme::c().accent } else { theme::c().name };
                            ui.label(RichText::new(format!("{name:<14}")).font(theme::small()).color(name_color));
                            ui.label(RichText::new(*description).font(theme::small()).color(theme::c().dark_gray));
                        });
                        let response = ui
                            .interact(row.response.rect, id.with(*name), egui::Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if response.hovered() {
                            let rect = row.response.rect.expand(2.0);
                            ui.painter().set(fill, egui::Shape::rect_filled(rect, 2.0, theme::c().hairline));
                        }
                        if response.clicked() {
                            picked = Some(*name);
                        }
                    }
                    ui.label(RichText::new("Tab completes · click picks").font(theme::small()).color(theme::c().dim));
                });
        });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_command_lists_its_matches_in_order() {
        let names: Vec<&str> = matches("/m").iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["/model", "/mcp", "/message-apps", "/matrix"]);
        assert_eq!(resolve("/m"), Some("/model"));
        assert_eq!(resolve("/mc"), Some("/mcp"));
    }

    #[test]
    fn paths_and_arguments_are_not_completed() {
        assert!(matches("/Users/me/x.rs").is_empty());
        assert!(matches("/goal fix it").is_empty());
        assert!(matches("hello").is_empty());
        assert!(matches("").is_empty());
    }

    #[test]
    fn a_bare_slash_lists_everything() {
        assert_eq!(matches("/").len(), COMMANDS.len());
    }
}
