//! The message box, like the TUI's: a cyan frame titled " Message ", a dim pro tip while empty.

use std::time::Instant;

use eframe::egui::{self, Align2, Id, Key, RichText, Stroke, TextEdit};

use crate::tips::Tips;
use crate::theme;

const MAX_ROWS: usize = 8;

pub struct InputBox {
    pub text: String,
    tips: Tips,
    /// Sent prompts, oldest first, and where Up/Down browsing is. `draft` is what was typed before browsing.
    history: Vec<String>,
    browsing: Option<usize>,
    draft: String,
    move_cursor_to_end: bool,
    /// Where the box was drawn last frame, for the slash menu.
    pub rect: egui::Rect,
}

pub enum InputAction {
    Send(String),
    None,
}

fn id() -> Id {
    Id::new("message input")
}

/// Takes the first Enter press without Shift out of the input events, so the text edit never sees it.
fn take_plain_enter(ctx: &egui::Context) -> bool {
    ctx.input_mut(|input| {
        let position = input.events.iter().position(|event| {
            matches!(event, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift)
        });
        position.map(|position| input.events.remove(position)).is_some()
    })
}

fn take_key(ctx: &egui::Context, key: Key) -> bool {
    ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key))
}

impl InputBox {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            tips: Tips::new(Instant::now()),
            history: Vec::new(),
            browsing: None,
            draft: String::new(),
            move_cursor_to_end: false,
            rect: egui::Rect::NOTHING,
        }
    }

    pub fn set_tips(&mut self, tips: Vec<String>) {
        self.tips.set(tips, Instant::now());
    }

    pub fn remember(&mut self, prompt: &str) {
        if self.history.last().map(String::as_str) != Some(prompt) {
            self.history.push(prompt.to_string());
        }
        self.browsing = None;
    }

    fn browse(&mut self, up: bool) {
        let next = match (self.browsing, up) {
            (None, true) if !self.history.is_empty() => {
                self.draft = std::mem::take(&mut self.text);
                Some(self.history.len() - 1)
            }
            (Some(index), true) => Some(index.saturating_sub(1)),
            (Some(index), false) if index + 1 < self.history.len() => Some(index + 1),
            (Some(_), false) => {
                self.text = std::mem::take(&mut self.draft);
                self.browsing = None;
                self.move_cursor_to_end = true;
                return;
            }
            _ => return,
        };
        if let Some(index) = next {
            self.text = self.history[index].clone();
            self.browsing = next;
            self.move_cursor_to_end = true;
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> InputAction {
        let ctx = ui.ctx().clone();
        let focused = ctx.memory(|memory| memory.has_focus(id()));
        let mut action = InputAction::None;
        if focused {
            if take_plain_enter(&ctx) && !self.text.trim().is_empty() {
                action = InputAction::Send(std::mem::take(&mut self.text));
            }
            // Tab completes a typed `/word` to the first command that matches.
            if let Some(name) = crate::slash::resolve(&self.text) {
                if take_key(&ctx, Key::Tab) {
                    self.text = name.to_string();
                    self.move_cursor_to_end = true;
                }
            }
            if !self.text.contains('\n') {
                if take_key(&ctx, Key::ArrowUp) {
                    self.browse(true);
                } else if take_key(&ctx, Key::ArrowDown) {
                    self.browse(false);
                }
            }
            if ctx.input(|input| !input.events.is_empty()) {
                self.tips.restart_idle_delay(Instant::now());
            }
        }

        let tip = self.tips.visible_text(Instant::now());
        let rows = self.text.lines().count().max(1) + usize::from(self.text.ends_with('\n'));
        let frame = egui::Frame::NONE
            .stroke(Stroke::new(1.0, theme::CYAN))
            .fill(theme::SURFACE)
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(12, 10));
        let output = frame.show(ui, |ui| {
            let mut edit = TextEdit::multiline(&mut self.text)
                .id(id())
                .frame(false)
                .desired_width(f32::INFINITY)
                .desired_rows(rows.min(MAX_ROWS))
                .hint_text(RichText::new(tip.as_str()).color(theme::DARK_GRAY));
            edit = edit.lock_focus(true);
            let response = edit.show(ui);
            if std::mem::take(&mut self.move_cursor_to_end) {
                let mut state = response.state.clone();
                let end = egui::text::CCursor::new(self.text.chars().count());
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ui.ctx(), id());
            }
        });
        // Focus on start and after sending, so typing goes straight to the box.
        if ctx.memory(|memory| memory.focused().is_none()) {
            ctx.memory_mut(|memory| memory.request_focus(id()));
        }
        // The title cuts the top border, like ratatui's block title.
        let rect = output.response.rect;
        self.rect = rect;
        let painter = ui.painter();
        let title_pos = rect.left_top() + egui::vec2(10.0, 0.0);
        let galley = painter.layout_no_wrap(" Message ".to_string(), theme::small(), theme::CYAN);
        let title_rect = Align2::LEFT_CENTER.anchor_size(title_pos, galley.size());
        // Above the border the window shows, below it the box's own fill: cut the line cleanly.
        let (upper, lower) = title_rect.split_top_bottom_at_fraction(0.5);
        painter.rect_filled(upper, 0.0, theme::BACKGROUND);
        painter.rect_filled(lower, 0.0, theme::SURFACE);
        painter.galley(title_rect.min, galley, theme::CYAN);
        if self.text.is_empty() {
            if let Some(wait) = self.tips.next_wake(Instant::now()) {
                ctx.request_repaint_after(wait);
            }
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, input: &mut InputBox, keys: &[Key]) {
        let events = keys
            .iter()
            .map(|key| egui::Event::Key { key: *key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE })
            .collect();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                input.show(ui);
            });
        });
    }

    #[test]
    fn tab_completes_a_partial_slash_command() {
        let ctx = egui::Context::default();
        let mut input = InputBox::new();
        input.text = "/mc".to_string();
        frame(&ctx, &mut input, &[]);
        frame(&ctx, &mut input, &[Key::Tab]);
        assert_eq!(input.text, "/mcp");
    }

    #[test]
    fn tab_leaves_other_text_alone() {
        let ctx = egui::Context::default();
        let mut input = InputBox::new();
        input.text = "/Users/me/x.rs".to_string();
        frame(&ctx, &mut input, &[]);
        frame(&ctx, &mut input, &[Key::Tab]);
        assert!(input.text.starts_with("/Users/me/x.rs"));
    }
}
