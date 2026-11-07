//! The window: chat, usage line, status line, message box and hint line, in the TUI's order.

use eframe::egui::{self, RichText};

use crate::backend::{self, Backend, BackendEvent};
use crate::boot::BootClip;
use crate::bottom_bar;
use crate::chat_view::ChatView;
use crate::screenshot::Screenshot;
use crate::slash;
use crate::input_box::{InputAction, InputBox};
use crate::modals::{KeysModal, McpModal, Modal, ModalAction, ModelModal, SessionModal};
use crate::protocol::{AgentState, Command, Event};
use crate::toys::{Toy, ToyStatus};
use crate::{boot_view, theme, Args};

const HINT: &str = "Enter send · Shift+Enter newline · Esc cancel turn · Up/Down history · Ctrl+O tool output · /clear new chat · /help";
/// ASCII: the braille spinner is not in the mono font.
const SPINNER: [char; 4] = ['|', '/', '-', '\\'];
const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Awaiting {
    Models,
    Sessions,
    Keys,
    Mcp,
    Apps,
}

pub struct MinerveApp {
    args: Args,
    backend: Option<Backend>,
    /// minerve is starting on another thread (up to 10 s), so the window stays responsive.
    starting: Option<std::sync::mpsc::Receiver<Result<Backend, String>>>,
    /// Why there is no working minerve; shown instead of the chat.
    error: Option<String>,
    transcript: Vec<(String, String)>,
    notices: Vec<String>,
    status: String,
    usage: String,
    working: bool,
    tools_expanded: bool,
    boot: Option<BootClip>,
    /// The clip as minerve sent it, kept so /bootscreen can replay it.
    intro: Option<(u32, String, Vec<String>)>,
    /// The running session, so Restart resumes it instead of starting a new chat.
    session_id: Option<String>,
    modal: Option<Modal>,
    /// `/matrix` or `/tetris`, replacing the chat while it runs.
    toy: Option<Toy>,
    /// `--submit`, sent once `hello` told us what this minerve can do.
    startup_submit: Option<String>,
    /// `!command` runs in flight, and their finished outputs waiting for the next message.
    ctx: egui::Context,
    shell_jobs: Vec<(String, std::sync::mpsc::Receiver<String>)>,
    pending_shell: Vec<String>,
    /// MCP connections change in the background, so the open manager asks again every second.
    last_mcp_poll: std::time::Instant,
    /// Text for the clipboard, copied on the next frame (events have no egui context).
    pending_clipboard: Option<String>,
    /// A `/model`, `/resume` or `/apikeys` asked for: its modal opens when minerve answers.
    awaiting: Option<Awaiting>,
    /// No boot clip: resuming, MINERVE_NO_BOOT, or the user already pressed a key.
    skip_boot: bool,
    /// What this minerve told us in `hello`: commands it accepts beyond the original ones.
    features: Vec<String>,
    chat: ChatView,
    input: InputBox,
    screenshot: Option<Screenshot>,
}

impl MinerveApp {
    pub fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        theme::apply(&cc.egui_ctx);
        // The boot clip is for fresh sessions only, like the TUI's. It starts when minerve sends it.
        let skip_boot = args.resume.is_some() || std::env::var_os("MINERVE_NO_BOOT").is_some();
        let screenshot = args.screenshot.clone().map(|path| Screenshot::new(path, args.screenshot_after));
        let mut app = Self {
            args,
            backend: None,
            starting: None,
            error: None,
            transcript: Vec::new(),
            notices: Vec::new(),
            status: String::new(),
            usage: String::new(),
            working: false,
            tools_expanded: false,
            boot: None,
            intro: None,
            session_id: None,
            modal: None,
            toy: None,
            startup_submit: None,
            ctx: cc.egui_ctx.clone(),
            shell_jobs: Vec::new(),
            pending_shell: Vec::new(),
            last_mcp_poll: std::time::Instant::now(),
            pending_clipboard: None,
            awaiting: None,
            skip_boot,
            features: Vec::new(),
            chat: ChatView::new(),
            input: InputBox::new(),
            screenshot,
        };
        app.start_backend(&cc.egui_ctx);
        app.startup_submit = app.args.submit.clone();
        if let Some(text) = app.args.prefill.clone() {
            app.input.text = text;
        }
        app
    }

    fn start_backend(&mut self, ctx: &egui::Context) {
        self.backend = None;
        self.error = None;
        self.status = "starting minerve...".to_string();
        let resume = self.session_id.clone().or_else(|| self.args.resume.clone());
        let minerve = self.args.minerve.clone();
        let ctx = ctx.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let started = Backend::start(resume.as_deref(), minerve.as_deref(), ctx.clone());
            let _ = tx.send(started);
            ctx.request_repaint();
        });
        self.starting = Some(rx);
    }

    fn poll_shell(&mut self) {
        let mut finished = Vec::new();
        self.shell_jobs.retain(|(command, rx)| match rx.try_recv() {
            Ok(block) => {
                finished.push((command.clone(), block));
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
        for (command, block) in finished {
            self.pending_shell.push(block);
            self.status = format!("queued output of `{command}` for the next message");
        }
    }

    /// Takes the backend once the starting thread is done.
    fn poll_starting(&mut self) {
        let Some(rx) = &self.starting else { return };
        match rx.try_recv() {
            Ok(Ok(backend)) => self.backend = Some(backend),
            Ok(Err(error)) => {
                self.status.clear();
                self.error = Some(error);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.error = Some("minerve did not start".to_string()),
        }
        self.starting = None;
    }

    fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|name| name == feature)
    }

    fn send(&self, command: Command) {
        if let Some(backend) = &self.backend {
            backend.send(&command);
        }
    }

    fn drain_events(&mut self) {
        self.poll_starting();
        self.poll_shell();
        let Some(backend) = &self.backend else { return };
        let events: Vec<BackendEvent> = backend.events.try_iter().collect();
        for event in events {
            match event {
                BackendEvent::Event(event) => self.apply(event),
                BackendEvent::Disconnected(reason) => {
                    self.error = Some(reason);
                    self.backend = None;
                    self.working = false;
                    return;
                }
            }
        }
    }

    fn apply(&mut self, event: Event) {
        match event {
            Event::Hello { version, .. } if version != PROTOCOL_VERSION => {
                self.error = Some(format!(
                    "minerve speaks version {version} of the control protocol and minerve-gui reads {PROTOCOL_VERSION}. \
                     Install matching versions of both."
                ));
            }
            Event::Hello { minerve_version, features, .. } => {
                if !features.iter().any(|feature| feature == "serve") {
                    self.error = Some(format!(
                        "this minerve ({minerve_version}) is too old for minerve-gui: update minerve."
                    ));
                    self.backend = None;
                    return;
                }
                self.features = features;
                self.status.clear();
                if self.has_feature("intro") {
                    self.send(Command::GetIntro {});
                }
                // With an intro coming, wait for it: /bootscreen needs the clip.
                if !self.has_feature("intro") {
                    if let Some(text) = self.startup_submit.take() {
                        self.submit(text);
                    }
                }
            }
            Event::Intro { fps, caption, frames, tips } => {
                self.input.set_tips(tips);
                if !self.skip_boot {
                    self.boot = BootClip::new(fps, caption.clone(), frames.clone());
                }
                self.intro = Some((fps, caption, frames));
                if let Some(text) = self.startup_submit.take() {
                    self.submit(text);
                }
            }
            Event::State { state } => self.working = state == AgentState::Working,
            Event::Transcript { messages } => self.transcript = messages,
            Event::Status { status } => self.status = status,
            Event::Notice { notice } => self.notices.push(notice),
            Event::Usage { summary } => self.usage = summary,
            Event::Session { session_id } => self.session_id = Some(session_id),
            Event::Models { current, recent, lists } => {
                if self.awaiting.take() == Some(Awaiting::Models) {
                    self.modal = Some(Modal::Models(ModelModal::new(current, recent, lists)));
                }
            }
            Event::Sessions { sessions } => {
                if self.awaiting.take() == Some(Awaiting::Sessions) {
                    self.modal = Some(Modal::Sessions(SessionModal::new(sessions)));
                }
            }
            Event::ApiKeys { openrouter, openai, anthropic } => {
                if self.awaiting.take() == Some(Awaiting::Keys) {
                    self.modal = Some(Modal::Keys(KeysModal::new(openrouter, openai, anthropic)));
                }
            }
            Event::McpServers { servers } => match &mut self.modal {
                Some(Modal::Mcp(modal)) => modal.update(servers),
                _ if self.awaiting.take() == Some(Awaiting::Mcp) => self.modal = Some(Modal::Mcp(McpModal::new(servers))),
                _ => {}
            },
            Event::MessageApps { slack } => match &mut self.modal {
                Some(Modal::Apps(modal)) => modal.update(slack),
                _ if self.awaiting.take() == Some(Awaiting::Apps) => {
                    self.modal = Some(Modal::Apps(crate::modal_apps::AppsModal::new(slack)))
                }
                _ => {}
            },
            Event::MessageAppsManifest { manifest, url } => {
                self.pending_clipboard = Some(manifest);
                self.notices.push(format!("Slack manifest copied (also in ~/.minerve/slack-manifest.json). Opened {url}"));
            }
            Event::Unknown => {}
        }
    }

    fn submit(&mut self, text: String) {
        let trimmed = text.trim();
        self.notices.clear();
        // `!command` runs here and is kept for the next message; minerve is not asked.
        if let Some(command) = trimmed.strip_prefix('!') {
            let command = command.trim();
            if !command.is_empty() {
                self.input.remember(trimmed);
                self.status = format!("running `{command}`...");
                self.shell_jobs.push((command.to_string(), crate::shell::spawn(command.to_string(), self.ctx.clone())));
            }
            return;
        }
        // Enter on a partial `/word` runs the first command that matches, like the terminal app.
        let completed = crate::slash::resolve(trimmed);
        let trimmed = completed.unwrap_or(trimmed);
        if self.backend.is_none() {
            let why = if self.starting.is_some() { "minerve is still starting" } else { "minerve is not running: use Restart minerve" };
            self.notices.push(why.to_string());
            self.input.text = text;
            return;
        }
        // `/Users/me/x.rs fix this` is a prompt with a path, not a command.
        let first_word = trimmed.split_whitespace().next().unwrap_or("");
        let is_command = first_word.starts_with('/') && !first_word[1..].contains('/');
        if !is_command {
            if self.working {
                self.status = "a turn is running: wait for it, or press Esc to cancel it".to_string();
                self.input.text = text;
                return;
            }
            self.input.remember(trimmed);
            let message = crate::shell::with_context(&self.pending_shell, trimmed);
            self.pending_shell.clear();
            self.send(Command::Submit { text: message });
            return;
        }
        if trimmed == "/clear" {
            self.pending_shell.clear();
            if self.has_feature("new_session") {
                self.send(Command::NewSession {});
            } else {
                self.notices.push("this minerve cannot start a new session from minerve-gui: update minerve".to_string());
            }
        } else if let Some(command) = self.server_command(trimmed) {
            self.send(command);
        } else if trimmed == "/bootscreen" {
            match &self.intro {
                Some((fps, caption, frames)) => self.boot = BootClip::new(*fps, caption.clone(), frames.clone()),
                None => self.notices.push("this minerve did not send a boot screen: update minerve".to_string()),
            }
        } else if trimmed == "/matrix" {
            self.toy = Some(Toy::matrix());
        } else if trimmed == "/tetris" {
            self.toy = Some(Toy::tetris());
        } else if trimmed == "/about" {
            self.notices.push("minerve-gui: a window for minerve. It runs `minerve --serve` and talks to it over the control socket.".to_string());
        } else if trimmed == "/help" {
            self.notices.push("Available: /model /resume /goal <goal> /apikeys /mcp /message-apps /clear /bootscreen /matrix /tetris /about /help.".to_string());
        } else {
            self.notices.push(format!("{first_word} is not in minerve-gui yet. Type /help; the terminal app (minerve) has it."));
        }
        self.input.remember(trimmed);
    }

    /// The command for a slash command minerve serves, when this minerve lists its feature.
    /// Sets what to open once minerve answers.
    fn server_command(&mut self, text: &str) -> Option<Command> {
        let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        let (feature, command, awaiting) = match name {
            "/goal" if !rest.trim().is_empty() => ("goal", Command::Goal { goal: rest.trim().to_string() }, None),
            "/model" => ("models", Command::ListModels {}, Some(Awaiting::Models)),
            "/resume" => ("sessions", Command::ListSessions {}, Some(Awaiting::Sessions)),
            "/message-apps" => ("message_apps", Command::MessageAppsStatus {}, Some(Awaiting::Apps)),
            "/mcp" => ("mcp", Command::ListMcp {}, Some(Awaiting::Mcp)),
            "/apikeys" => ("api_keys", Command::GetApiKeys {}, Some(Awaiting::Keys)),
            _ => return None,
        };
        if !self.has_feature(feature) {
            self.notices.push(format!("this minerve cannot do {name} from minerve-gui: update minerve"));
            return None;
        }
        self.awaiting = awaiting;
        Some(command)
    }

    fn run_modal(&mut self, ctx: &egui::Context) {
        if let Some(text) = self.pending_clipboard.take() {
            ctx.copy_text(text);
        }
        let poll = match self.modal {
            Some(Modal::Mcp(_)) => Some(Command::ListMcp {}),
            Some(Modal::Apps(_)) => Some(Command::MessageAppsStatus {}),
            _ => None,
        };
        if let Some(poll) = poll {
            if self.last_mcp_poll.elapsed() >= std::time::Duration::from_secs(1) {
                self.last_mcp_poll = std::time::Instant::now();
                self.send(poll);
            }
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        let Some(modal) = self.modal.as_mut() else { return };
        match modal.show(ctx) {
            ModalAction::None => {}
            ModalAction::Close => self.modal = None,
            ModalAction::Send(command) => self.send(command),
            ModalAction::SetModel(model) => {
                self.modal = None;
                self.send(Command::SetModel { model });
            }
            ModalAction::Resume(id) => {
                self.modal = None;
                self.pending_shell.clear();
                self.send(Command::ResumeSession { id });
            }
            ModalAction::SaveKeys { openrouter, openai, anthropic } => {
                self.modal = None;
                let some = |key: String| Some(key).filter(|key| !key.trim().is_empty());
                self.send(Command::SetApiKeys { openrouter: some(openrouter), openai: some(openai), anthropic: some(anthropic) });
            }
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let (escape, tools, any_key) = ctx.input(|input| {
            (
                input.key_pressed(egui::Key::Escape),
                input.modifiers.ctrl && input.key_pressed(egui::Key::O),
                input.events.iter().any(|event| matches!(event, egui::Event::Key { pressed: true, .. } | egui::Event::PointerButton { pressed: true, .. })),
            )
        });
        // The first key or click ends the boot clip.
        let typing = any_key && self.screenshot.is_none();
        let dismissed_boot = typing && self.boot.is_some();
        if typing {
            // Also stops a clip that arrives later: the user is already typing.
            self.boot = None;
            self.skip_boot = true;
        }
        if escape && !dismissed_boot && self.modal.is_none() && self.toy.is_none() && self.working && self.has_feature("cancel") {
            self.send(Command::Cancel {});
        }
        if tools {
            self.tools_expanded = !self.tools_expanded;
        }
    }

    fn show_error(&mut self, ui: &mut egui::Ui, error: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() / 3.0).max(20.0));
            ui.label(RichText::new("minerve is not running").color(theme::MAGENTA).strong());
            ui.add_space(theme::FONT_SIZE);
            ui.label(error);
            ui.add_space(theme::FONT_SIZE);
            ui.label(RichText::new(format!("log: {}", backend::log_path().display())).color(theme::DARK_GRAY));
            ui.add_space(theme::FONT_SIZE);
            if ui.button(" Restart minerve ").clicked() {
                self.start_backend(&ui.ctx().clone());
            }
        });
    }
}

impl eframe::App for MinerveApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::BACKGROUND.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.handle_keys(ctx);
        if let Some(screenshot) = self.screenshot.as_mut() {
            screenshot.tick(ctx);
        }
        if self.boot.as_ref().is_some_and(BootClip::is_finished) {
            self.boot = None;
        }
        let panel = |top: i8, bottom: i8| {
            egui::Frame::NONE.fill(theme::BACKGROUND).inner_margin(egui::Margin { left: theme::PANEL_MARGIN, right: theme::PANEL_MARGIN, top, bottom })
        };
        let no_frame = egui::Frame::NONE.fill(theme::BACKGROUND).inner_margin(theme::CHAT_MARGIN);

        egui::TopBottomPanel::bottom("hint").frame(panel(6, 10)).show_separator_line(false).show(ctx, |ui| {
            ui.label(bottom_bar::hint_job(HINT));
        });
        let mut submitted = None;
        egui::TopBottomPanel::bottom("input").frame(panel(10, 0)).show_separator_line(false).show(ctx, |ui| {
            // A toy reads the keys itself.
            if self.toy.is_none() {
                if let InputAction::Send(text) = self.input.show(ui) {
                    submitted = Some(text);
                }
            }
        });
        egui::TopBottomPanel::bottom("status").frame(panel(0, 0)).show_separator_line(false).show(ctx, |ui| {
            let status = if self.working {
                let frame = (ctx.input(|input| input.time) * 8.0) as usize % SPINNER.len();
                ctx.request_repaint_after(std::time::Duration::from_millis(125));
                format!("{} {}", SPINNER[frame], if self.status.is_empty() { "working..." } else { &self.status })
            } else {
                self.status.clone()
            };
            ui.label(RichText::new(status).font(theme::small()).color(theme::MAGENTA));
        });
        egui::TopBottomPanel::bottom("usage").frame(panel(9, 2)).show_separator_line(false).show(ctx, |ui| {
            let line = ui.max_rect().top() - 9.0;
            ui.painter().hline(ui.max_rect().x_range(), line, egui::Stroke::new(1.0, theme::HAIRLINE));
            ui.label(bottom_bar::usage_job(&self.usage));
        });

        egui::CentralPanel::default().frame(no_frame).show(ctx, |ui| {
            if let Some(error) = self.error.clone() {
                self.show_error(ui, &error);
                return;
            }
            // The terminal views (boot clip, matrix, tetris) run to the window's left, right and
            // top edges; only the chat keeps its reading margins.
            let content = ui.available_rect_before_wrap();
            let window = ctx.screen_rect();
            let rect = egui::Rect::from_min_max(window.min, egui::pos2(window.max.x, content.max.y));
            if let Some(toy) = self.toy.as_mut() {
                if toy.show(ui, rect) == ToyStatus::Done {
                    self.toy = None;
                }
                return;
            }
            self.chat.show(ui, &self.transcript, &self.notices, &self.pending_shell, self.tools_expanded);
            if let Some(boot) = &self.boot {
                boot_view::paint(ui, rect, boot);
            }
        });
        if self.modal.is_none() && self.toy.is_none() {
            slash::show_menu(ctx, &self.input.text, self.input.rect);
        }
        self.run_modal(ctx);
        if let Some(text) = submitted {
            self.submit(text);
        }
    }
}
