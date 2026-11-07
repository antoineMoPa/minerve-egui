//! The `/message-apps` window: Slack setup, connection state and linking your account.

use eframe::egui::{self, Align2, RichText};

use crate::modals::ModalAction;
use crate::protocol::{Command, MessageAppsStatus};
use crate::theme;

pub struct AppsModal {
    status: MessageAppsStatus,
    /// The setup form: bot name, app token, bot token. Shown until Slack is set up, or on Edit.
    form: Option<[String; 3]>,
}

impl AppsModal {
    pub fn new(status: MessageAppsStatus) -> Self {
        let form = (status.state == "not_set_up").then(|| [status.bot_name.clone(), String::new(), String::new()]);
        Self { status, form }
    }

    pub fn update(&mut self, status: MessageAppsStatus) {
        if status.state != "not_set_up" && self.status.state == "not_set_up" && self.form.is_some() {
            self.form = None;
        }
        self.status = status;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> ModalAction {
        let mut action = ModalAction::None;
        let mut close = false;
        egui::Window::new(RichText::new(" Message apps ").color(theme::c().accent))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([600.0, 320.0])
            .show(ctx, |ui| {
                self.show_state(ui);
                ui.separator();
                if let Some(form) = self.form.as_mut() {
                    show_form(ui, form, &self.status, &mut action);
                } else {
                    self.show_actions(ui, &mut action);
                }
                ui.add_space(4.0);
                if ui.button(" Close ").clicked() {
                    close = true;
                }
            });
        if close {
            ModalAction::Close
        } else {
            action
        }
    }

    fn show_state(&self, ui: &mut egui::Ui) {
        let (label, color) = match self.status.state.as_str() {
            "connected" => ("connected", theme::c().green),
            "connecting" => ("connecting...", theme::c().gray),
            "failed" => ("failed", theme::c().magenta),
            "disconnected" => ("disconnected", theme::c().dark_gray),
            _ => ("not set up", theme::c().dark_gray),
        };
        ui.horizontal(|ui| {
            ui.label("Slack:");
            ui.label(RichText::new(label).color(color));
        });
        if !self.status.detail.is_empty() {
            ui.label(RichText::new(&self.status.detail).color(if self.status.state == "failed" { theme::c().magenta } else { theme::c().gray }));
        }
        if !self.status.members.is_empty() {
            ui.label(RichText::new(format!("linked: {}", self.status.members.join(", "))).color(theme::c().dark_gray));
        }
    }

    fn show_actions(&mut self, ui: &mut egui::Ui, action: &mut ModalAction) {
        let connected = matches!(self.status.state.as_str(), "connected" | "connecting");
        match self.status.link.as_str() {
            "waiting" => {
                ui.label(RichText::new(format!("Send this code to the bot in Slack: {}", self.status.link_detail)).strong());
                ui.label(RichText::new(format!("expires in {}s", self.status.link_seconds_left)).color(theme::c().dark_gray));
                if ui.button(" Cancel link ").clicked() {
                    *action = ModalAction::Send(Command::MessageAppsCancelLink {});
                }
            }
            "linked" => {
                ui.label(RichText::new(format!("Linked {}.", self.status.link_detail)).color(theme::c().green));
            }
            _ => {}
        }
        ui.horizontal(|ui| {
            if connected {
                if ui.button(" Disconnect ").clicked() {
                    *action = ModalAction::Send(Command::MessageAppsStop {});
                }
                if self.status.link == "none" && ui.button(" Link my account ").clicked() {
                    *action = ModalAction::Send(Command::MessageAppsLink {});
                }
            } else if matches!(self.status.state.as_str(), "disconnected" | "failed") && ui.button(" Connect ").clicked() {
                *action = ModalAction::Send(Command::MessageAppsStart {});
            }
            if ui.button(" Edit setup ").clicked() {
                self.form = Some([self.status.bot_name.clone(), String::new(), String::new()]);
            }
        });
    }
}

fn show_form(ui: &mut egui::Ui, form: &mut [String; 3], status: &MessageAppsStatus, action: &mut ModalAction) {
    ui.label("Bot name (lowercase letters, digits, - _ .)");
    ui.add(egui::TextEdit::singleline(&mut form[0]).desired_width(f32::INFINITY));
    ui.horizontal(|ui| {
        if ui.button(" Copy Slack manifest + open Slack ").clicked() {
            *action = ModalAction::Send(Command::MessageAppsManifest { bot_name: form[0].clone() });
        }
        ui.label(RichText::new("create the app from the manifest, then paste its tokens").color(theme::c().dark_gray));
    });
    let saved = if status.has_tokens { " (saved: leave empty to keep)" } else { "" };
    ui.label(format!("App token, xapp-…{saved}"));
    ui.add(egui::TextEdit::singleline(&mut form[1]).password(true).desired_width(f32::INFINITY));
    ui.label(format!("Bot token, xoxb-…{saved}"));
    ui.add(egui::TextEdit::singleline(&mut form[2]).password(true).desired_width(f32::INFINITY));
    if ui.button(" Save and connect ").clicked() {
        let some = |text: &str| Some(text.trim().to_string()).filter(|text| !text.is_empty());
        *action = ModalAction::Send(Command::MessageAppsSave { bot_name: form[0].trim().to_string(), app_token: some(&form[1]), bot_token: some(&form[2]) });
    }
}
