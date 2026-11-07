//! The message box: the text, then a row with the model pill and the send / stop button. A dim pro tip while empty.

use std::time::Instant;

use eframe::egui::{self, Id, Key, RichText, Stroke, TextEdit};

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
    /// Clipboard images attached to the prompt being typed, by their `[Image #N]` token.
    images: Vec<(String, std::path::PathBuf)>,
    /// Why the last Ctrl+V did not attach an image.
    pub paste_error: Option<String>,
    /// Where the box was drawn last frame, for the slash menu.
    pub rect: egui::Rect,
}

pub enum InputAction {
    Send(String),
    /// The model pill was clicked: open `/model`.
    OpenModels,
    /// The stop button, like Esc.
    Cancel,
    None,
}

/// A small flat pill: dim label, bright value, hairline border.
fn pill(ui: &mut egui::Ui, label: &str, value: &str) -> egui::Response {
    let mut job = egui::text::LayoutJob::default();
    let format = |color| egui::TextFormat { font_id: theme::small(), color, ..Default::default() };
    job.append(&format!("{label} "), 0.0, format(theme::DARK_GRAY));
    job.append(value, 0.0, format(theme::GRAY));
    let button = egui::Button::new(job)
        .fill(theme::BACKGROUND)
        .stroke(Stroke::new(1.0, theme::HAIRLINE))
        .corner_radius(4)
        .min_size(egui::vec2(0.0, 22.0));
    ui.add(button).on_hover_cursor(egui::CursorIcon::PointingHand)
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
            images: Vec::new(),
            paste_error: None,
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

    /// Images whose token is still in the sent `text`. Starts a new prompt's list.
    pub fn take_images(&mut self, text: &str) -> Vec<std::path::PathBuf> {
        std::mem::take(&mut self.images)
            .into_iter()
            .filter(|(token, _)| text.contains(token.as_str()))
            .map(|(_, path)| path)
            .collect()
    }

    /// Ctrl+V / Cmd+V with an image on the clipboard: saves it and adds an `[Image #N]` token.
    fn paste_clipboard_image(&mut self) {
        match crate::clipboard_image::save_clipboard_image() {
            Ok(Some(path)) => {
                let token = format!("[Image #{}]", self.images.len() + 1);
                self.text.push_str(&token);
                self.images.push((token, path));
                self.move_cursor_to_end = true;
                self.paste_error = None;
            }
            Ok(None) => self.paste_error = Some("no image on the clipboard".to_string()),
            Err(err) => self.paste_error = Some(err),
        }
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

    /// `model` is empty until minerve said which one runs. `working` turns send into stop.
    pub fn show(&mut self, ui: &mut egui::Ui, model: &str, working: bool) -> InputAction {
        let ctx = ui.ctx().clone();
        let focused = ctx.memory(|memory| memory.has_focus(id()));
        let mut action = InputAction::None;
        if focused {
            if take_plain_enter(&ctx) && !self.text.trim().is_empty() {
                action = InputAction::Send(std::mem::take(&mut self.text));
            }
            // Ctrl+V (and Cmd+V, which egui turns into a text paste only when the clipboard has text).
            let paste_key = ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::CTRL, Key::V) || input.consume_key(egui::Modifiers::COMMAND, Key::V)
            });
            let empty_paste = ctx.input(|input| input.events.iter().any(|e| matches!(e, egui::Event::Paste(t) if t.is_empty())));
            if paste_key || empty_paste {
                self.paste_clipboard_image();
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
        let border = if focused { theme::CYAN } else { theme::SEP };
        let frame = egui::Frame::NONE
            .stroke(Stroke::new(1.0, border))
            .fill(theme::SURFACE)
            .corner_radius(8)
            .inner_margin(egui::Margin { left: 14, right: 8, top: 12, bottom: 8 });
        let output = frame.show(ui, |ui| {
            let mut edit = TextEdit::multiline(&mut self.text)
                .id(id())
                .frame(egui::Frame::NONE)
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
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if !model.is_empty() && pill(ui, "model", model).on_hover_text("Change model (/model)").clicked() {
                    action = InputAction::OpenModels;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (text, color, fill) = if working {
                        ("\u{25a0} stop", theme::MAGENTA, theme::SURFACE)
                    } else {
                        ("send \u{21b5}", theme::BACKGROUND, theme::CYAN)
                    };
                    let enabled = working || !self.text.trim().is_empty();
                    let button = egui::Button::new(RichText::new(text).font(theme::small()).color(color).strong())
                        .fill(if enabled { fill } else { theme::HAIRLINE })
                        .stroke(Stroke::new(1.0, if working { theme::MAGENTA } else { egui::Color32::TRANSPARENT }))
                        .corner_radius(4)
                        .min_size(egui::vec2(0.0, 22.0));
                    if ui.add_enabled(enabled, button).clicked() {
                        action = if working { InputAction::Cancel } else { InputAction::Send(std::mem::take(&mut self.text)) };
                    }
                });
            });
        });
        // Focus on start and after sending, so typing goes straight to the box.
        if ctx.memory(|memory| memory.focused().is_none()) {
            ctx.memory_mut(|memory| memory.request_focus(id()));
        }
        self.rect = output.response.rect;
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
        // No renderer here: drop the font texture upload egui asks for.
        ctx.run_ui(raw, |ui| {
            input.show(ui, "", false);
        })
        .textures_delta
        .clear();
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
