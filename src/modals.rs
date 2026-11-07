//! The windows behind `/model`, `/resume` and `/apikeys`. They only collect a choice: the app
//! turns it into a command for minerve.

use eframe::egui::{self, Align2, RichText};

use crate::protocol::{Command, McpServer, ModelList, SessionInfo};
use crate::theme;

/// What the person chose in a modal.
pub enum ModalAction {
    None,
    Close,
    SetModel(String),
    Resume(String),
    /// A command the modal built itself (the MCP manager).
    Send(Command),
    SaveKeys { openrouter: String, openai: String, anthropic: String },
}

pub enum Modal {
    Models(ModelModal),
    Sessions(SessionModal),
    Keys(KeysModal),
    Mcp(McpModal),
    Apps(crate::modal_apps::AppsModal),
}

impl Modal {
    pub fn show(&mut self, ctx: &egui::Context) -> ModalAction {
        let escape = ctx.input(|input| input.key_pressed(egui::Key::Escape));
        let action = match self {
            Modal::Models(modal) => modal.show(ctx),
            Modal::Sessions(modal) => modal.show(ctx),
            Modal::Keys(modal) => modal.show(ctx),
            Modal::Mcp(modal) => modal.show(ctx),
            Modal::Apps(modal) => modal.show(ctx),
        };
        match action {
            ModalAction::None if escape => ModalAction::Close,
            action => action,
        }
    }
}

fn window(title: &str) -> egui::Window<'_> {
    egui::Window::new(RichText::new(format!(" {title} ")).color(theme::CYAN))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([620.0, 420.0])
        .frame(egui::Frame::window(&egui::Style::default()).fill(theme::BACKGROUND).inner_margin(16.0))
}

/// The global theme zeroes item spacing (tight chat layout); dialogs need air.
fn roomy(ui: &mut egui::Ui) {
    let spacing = ui.spacing_mut();
    spacing.item_spacing = egui::vec2(8.0, 8.0);
    spacing.button_padding = egui::vec2(12.0, 5.0);
    spacing.interact_size.y = 26.0;
}

pub struct ModelModal {
    current: String,
    recent: Vec<String>,
    lists: Vec<ModelList>,
    filter: String,
    selected: usize,
}

impl ModelModal {
    pub fn new(current: String, recent: Vec<String>, lists: Vec<ModelList>) -> Self {
        Self { current, recent, lists, filter: String::new(), selected: 0 }
    }

    /// Recent models matching the filter, then every other matching model in provider order.
    fn rows(&self) -> Vec<(String, bool)> {
        let needle = self.filter.to_lowercase();
        let all: Vec<&String> = self.lists.iter().flat_map(|list| list.models.iter()).collect();
        let matches = |id: &str| id.to_lowercase().contains(&needle);
        let recent = self.recent.iter().filter(|id| all.contains(id) && matches(id)).map(|id| (id.clone(), true));
        let others = all.iter().filter(|id| !self.recent.contains(id) && matches(id)).map(|id| ((*id).clone(), false));
        recent.chain(others).collect()
    }

    fn show(&mut self, ctx: &egui::Context) -> ModalAction {
        let rows = self.rows();
        self.selected = self.selected.min(rows.len().saturating_sub(1));
        let (up, down, enter) = ctx.input(|input| {
            (input.key_pressed(egui::Key::ArrowUp), input.key_pressed(egui::Key::ArrowDown), input.key_pressed(egui::Key::Enter))
        });
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if down {
            self.selected = (self.selected + 1).min(rows.len().saturating_sub(1));
        }
        let mut action = ModalAction::None;
        window("Model").show(ctx, |ui| {
            roomy(ui);
            let filter = ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("type to filter").desired_width(f32::INFINITY));
            filter.request_focus();
            if filter.changed() {
                self.selected = 0;
            }
            for list in &self.lists {
                if let Some(error) = &list.error {
                    ui.label(RichText::new(format!("{}: {error}", list.provider)).color(theme::MAGENTA));
                }
            }
            ui.label(RichText::new("Enter choose · Esc close").color(theme::DARK_GRAY));
            egui::ScrollArea::vertical().max_height(310.0).show(ui, |ui| {
                for (index, (id, recent)) in rows.iter().enumerate() {
                    let marker = if *id == self.current { "* " } else { "  " };
                    let tag = if *recent { "  (recent)" } else { "" };
                    let label = RichText::new(format!("{marker}{id}{tag}"));
                    let response = ui.selectable_label(index == self.selected, label);
                    if index == self.selected && (up || down) {
                        response.scroll_to_me(None);
                    }
                    if response.clicked() {
                        action = ModalAction::SetModel(id.clone());
                    }
                }
                if rows.is_empty() {
                    ui.label(RichText::new("no models (is an API key set? try /apikeys)").color(theme::DARK_GRAY));
                }
            });
        });
        if enter {
            if let Some((id, _)) = rows.get(self.selected) {
                action = ModalAction::SetModel(id.clone());
            }
        }
        action
    }
}

pub struct SessionModal {
    sessions: Vec<SessionInfo>,
    selected: usize,
}

impl SessionModal {
    pub fn new(sessions: Vec<SessionInfo>) -> Self {
        // A chat with no prompt has nothing to resume.
        Self { sessions: sessions.into_iter().filter(|session| !session.first_prompt.is_empty()).collect(), selected: 0 }
    }

    fn show(&mut self, ctx: &egui::Context) -> ModalAction {
        let last = self.sessions.len().saturating_sub(1);
        let (up, down, enter) = ctx.input(|input| {
            (input.key_pressed(egui::Key::ArrowUp), input.key_pressed(egui::Key::ArrowDown), input.key_pressed(egui::Key::Enter))
        });
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if down {
            self.selected = (self.selected + 1).min(last);
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let mut action = ModalAction::None;
        window("Resume").show(ctx, |ui| {
            roomy(ui);
            ui.label(RichText::new("Enter resume · Esc close").color(theme::DARK_GRAY));
            egui::ScrollArea::vertical().max_height(330.0).show(ui, |ui| {
                for (index, session) in self.sessions.iter().enumerate() {
                    let label = format!("{:>8}  {}", age(now.saturating_sub(session.modified)), session.first_prompt);
                    let response = ui.selectable_label(index == self.selected, RichText::new(label));
                    if index == self.selected && (up || down) {
                        response.scroll_to_me(None);
                    }
                    if response.clicked() {
                        action = ModalAction::Resume(session.id.clone());
                    }
                }
                if self.sessions.is_empty() {
                    ui.label(RichText::new("no saved sessions yet").color(theme::DARK_GRAY));
                }
            });
        });
        if enter {
            if let Some(session) = self.sessions.get(self.selected) {
                action = ModalAction::Resume(session.id.clone());
            }
        }
        action
    }
}

fn age(seconds: u64) -> String {
    match seconds {
        0..=59 => "now".to_string(),
        60..=3599 => format!("{}m ago", seconds / 60),
        3600..=86399 => format!("{}h ago", seconds / 3600),
        _ => format!("{}d ago", seconds / 86400),
    }
}

pub struct KeysModal {
    is_set: [bool; 3],
    typed: [String; 3],
}

impl KeysModal {
    pub fn new(openrouter: bool, openai: bool, anthropic: bool) -> Self {
        Self { is_set: [openrouter, openai, anthropic], typed: Default::default() }
    }

    fn show(&mut self, ctx: &egui::Context) -> ModalAction {
        let mut action = ModalAction::None;
        let names = ["OpenRouter", "OpenAI", "Anthropic"];
        egui::Window::new(RichText::new(" API keys ").color(theme::CYAN))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([520.0, 220.0])
            .show(ctx, |ui| {
            roomy(ui);
                for (index, name) in names.iter().enumerate() {
                    ui.label(RichText::new(format!("{name} {}", if self.is_set[index] { "(set: leave empty to keep)" } else { "(not set)" })));
                    let edit = ui.add(egui::TextEdit::singleline(&mut self.typed[index]).password(true).desired_width(f32::INFINITY));
                    if index == 0 {
                        edit.request_focus();
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(" Save ").clicked() {
                        action = ModalAction::SaveKeys {
                            openrouter: self.typed[0].clone(),
                            openai: self.typed[1].clone(),
                            anthropic: self.typed[2].clone(),
                        };
                    }
                    if ui.button(" Cancel ").clicked() {
                        action = ModalAction::Close;
                    }
                });
                ui.label(RichText::new("Saved to ~/.minerve/keys.json").color(theme::DARK_GRAY));
            });
        action
    }
}

pub struct McpModal {
    servers: Vec<McpServer>,
    selected: usize,
    confirm_remove: bool,
    /// The "add a server" form: name, command or URL, env or headers.
    form: Option<[String; 3]>,
}

impl McpModal {
    pub fn new(servers: Vec<McpServer>) -> Self {
        Self { servers, selected: 0, confirm_remove: false, form: None }
    }

    pub fn update(&mut self, servers: Vec<McpServer>) {
        self.servers = servers;
        self.selected = self.selected.min(self.servers.len().saturating_sub(1));
    }

    fn show(&mut self, ctx: &egui::Context) -> ModalAction {
        let mut action = ModalAction::None;
        if self.form.is_some() {
            self.show_form(ctx, &mut action);
            return action;
        }
        window("MCP servers").show(ctx, |ui| {
            roomy(ui);
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for (index, server) in self.servers.iter().enumerate() {
                    let state = match server.state.as_str() {
                        "connected" => format!("connected, {} tools", server.tools),
                        other => other.replace('_', " "),
                    };
                    let label = format!("{:<22}{:<22}{}", server.name, state, server.source);
                    if ui.selectable_label(index == self.selected, RichText::new(label)).clicked() {
                        self.selected = index;
                        self.confirm_remove = false;
                    }
                }
                if self.servers.is_empty() {
                    ui.label(RichText::new("no MCP servers yet").color(theme::DARK_GRAY));
                }
            });
            ui.separator();
            if let Some(server) = self.servers.get(self.selected) {
                ui.label(RichText::new(&server.target).strong());
                if !server.detail.is_empty() {
                    let color = if server.state == "failed" { theme::MAGENTA } else { theme::GRAY };
                    ui.label(RichText::new(&server.detail).color(color));
                }
                let name = server.name.clone();
                ui.horizontal(|ui| {
                    match server.state.as_str() {
                        "connected" | "connecting" | "logging_in" => {
                            if ui.button(" Disconnect ").clicked() {
                                action = ModalAction::Send(Command::McpDisconnect { name: name.clone() });
                            }
                        }
                        "needs_login" => {
                            if ui.button(" Log in ").clicked() {
                                action = ModalAction::Send(Command::McpLogin { name: name.clone() });
                            }
                        }
                        _ => {
                            if ui.button(" Connect ").clicked() {
                                action = ModalAction::Send(Command::McpConnect { name: name.clone() });
                            }
                        }
                    }
                    if self.confirm_remove {
                        if ui.button(RichText::new(" Really remove ").color(theme::MAGENTA)).clicked() {
                            self.confirm_remove = false;
                            action = ModalAction::Send(Command::McpRemove { name: name.clone() });
                        }
                    } else if ui.button(" Remove ").clicked() {
                        self.confirm_remove = true;
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(" + Add a server ").clicked() {
                    self.form = Some(Default::default());
                }
                if ui.button(" Close ").clicked() {
                    action = ModalAction::Close;
                }
            });
        });
        action
    }

    fn show_form(&mut self, ctx: &egui::Context, action: &mut ModalAction) {
        let Some(form) = self.form.as_mut() else { return };
        let labels = ["Name", "Command or URL", "Env or headers (KEY=value, one per space)"];
        let mut done = false;
        let mut back = false;
        egui::Window::new(RichText::new(" Add MCP server ").color(theme::CYAN))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([560.0, 240.0])
            .frame(egui::Frame::window(&egui::Style::default()).fill(theme::BACKGROUND).inner_margin(16.0))
            .show(ctx, |ui| {
            roomy(ui);
                for (index, label) in labels.iter().enumerate() {
                    ui.label(*label);
                    let edit = ui.add(egui::TextEdit::singleline(&mut form[index]).desired_width(f32::INFINITY));
                    if index == 0 {
                        edit.request_focus();
                    }
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    done = ui.button(" Add ").clicked();
                    back = ui.button(" Back ").clicked();
                });
            });
        if done {
            let [name, command_or_url, env_or_headers] = std::mem::take(form);
            *action = ModalAction::Send(Command::McpAdd { name, command_or_url, env_or_headers });
            self.form = None;
        } else if back || ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.form = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modal() -> ModelModal {
        let list = ModelList { provider: "OpenRouter".into(), models: vec!["a/x".into(), "b/y".into(), "b/z".into()], error: None };
        ModelModal::new("a/x".into(), vec!["b/z".into()], vec![list])
    }

    #[test]
    fn recent_models_come_first_and_the_filter_narrows() {
        let mut modal = modal();
        let ids: Vec<String> = modal.rows().into_iter().map(|(id, _)| id).collect();
        assert_eq!(ids, ["b/z", "a/x", "b/y"]);
        modal.filter = "B/".into();
        let ids: Vec<String> = modal.rows().into_iter().map(|(id, _)| id).collect();
        assert_eq!(ids, ["b/z", "b/y"]);
    }

    #[test]
    fn ages_read_naturally() {
        assert_eq!(age(5), "now");
        assert_eq!(age(120), "2m ago");
        assert_eq!(age(7200), "2h ago");
        assert_eq!(age(172800), "2d ago");
    }
}
