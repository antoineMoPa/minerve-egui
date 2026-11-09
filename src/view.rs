//! `MinerveView`: chat, status line, message box, and the tinted bar with cwd and usage.
//! It draws inside whatever `Ui` it is given: the whole window in minerve-gui, a pane in an app
//! that embeds it.

use eframe::egui::{self, RichText};

use crate::backend::{self, Backend, BackendEvent, Launcher, Link};
use crate::boot::BootClip;
use crate::bottom_bar;
use crate::chat_view::ChatView;
use crate::screenshot::Screenshot;
use crate::slash;
use crate::input_box::{InputAction, InputBox};
use crate::modals::{KeysModal, McpModal, Modal, ModalAction, ModelModal, SessionModal};
use crate::protocol::{AgentState, Command, Event};
use crate::toys::{Toy, ToyStatus};
use crate::{boot_view, theme};

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

/// What a `MinerveView` starts with.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// A session id to resume instead of starting a new chat.
    pub resume: Option<String>,
    pub launcher: Launcher,
    /// Send this prompt as soon as minerve is up.
    pub submit: Option<String>,
    /// Start with this typed in the message box.
    pub prefill: Option<String>,
    /// `--screenshot <png>` of minerve-gui: save a picture of the window and exit.
    pub screenshot: Option<std::path::PathBuf>,
    pub screenshot_after: std::time::Duration,
    /// No boot clip: a view put back on a chat that is already going.
    pub skip_boot: bool,
    /// One pane of an app among others: the box takes the keyboard when clicked rather than
    /// whenever nothing has it.
    pub embedded: bool,
}

/// Each view in a process gets its own panel ids.
static NEXT_VIEW: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub struct MinerveView {
    id: egui::Id,
    /// Talking to the host's minerve (`attached`): it cannot be restarted from here.
    attached: bool,
    /// Where minerve works, as its `hello` said: the bar's left side.
    cwd: Option<std::path::PathBuf>,
    colors: theme::Colors,
    args: Options,
    backend: Option<Backend>,
    /// minerve is starting on another thread (up to 10 s), so the window stays responsive.
    starting: Option<std::sync::mpsc::Receiver<Result<Backend, String>>>,
    /// Why there is no working minerve; shown instead of the chat.
    error: Option<String>,
    transcript: Vec<(String, String)>,
    notices: Vec<String>,
    status: String,
    usage: String,
    /// The running model, for the pill in the message box. Empty until minerve said.
    model: String,
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

impl MinerveView {
    /// Talks to a minerve the host app runs (`moon` runs `__minerve --serve` for its tasks), over
    /// `link`. Nothing is started, and nothing is ended when the view goes: the host owns it.
    /// `options.launcher` is not used.
    pub fn attached(ctx: &egui::Context, link: Link, options: Options) -> Self {
        let mut view = Self::new_unstarted(ctx, options);
        view.attached = true;
        view.status.clear();
        view.backend = Some(Backend::attach(link, ctx.clone()));
        view
    }

    /// Starts minerve in the background; `show` draws the view. Leaves the app's style alone:
    /// minerve-gui calls `theme::apply` for the whole window.
    pub fn new(ctx: &egui::Context, args: Options) -> Self {
        let mut view = Self::new_unstarted(ctx, args);
        view.start_backend(ctx);
        view
    }

    fn new_unstarted(ctx: &egui::Context, args: Options) -> Self {
        // The boot clip is for fresh sessions only, like the TUI's. It starts when minerve sends it.
        let skip_boot = args.skip_boot || args.resume.is_some() || std::env::var_os("MINERVE_NO_BOOT").is_some();
        let screenshot = args.screenshot.clone().map(|path| Screenshot::new(path, args.screenshot_after));
        let mut app = Self {
            attached: false,
            cwd: None,
            colors: theme::Colors::default(),
            id: egui::Id::new(("minerve-view", NEXT_VIEW.fetch_add(1, std::sync::atomic::Ordering::Relaxed))),
            args,
            backend: None,
            starting: None,
            error: None,
            transcript: Vec::new(),
            notices: Vec::new(),
            status: String::new(),
            usage: String::new(),
            model: String::new(),
            working: false,
            tools_expanded: false,
            boot: None,
            intro: None,
            session_id: None,
            modal: None,
            toy: None,
            startup_submit: None,
            ctx: ctx.clone(),
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
        app.input.id = app.id.with("message input");
        app.input.grabs_focus = !app.args.embedded;
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
        let launcher = self.args.launcher.clone();
        let ctx = ctx.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let started = Backend::start(resume.as_deref(), &launcher, ctx.clone());
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
            Event::Hello { minerve_version, features, cwd, .. } => {
                if !cwd.is_empty() {
                    self.cwd = Some(std::path::PathBuf::from(cwd));
                }
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
                // Only for the model pill: no modal opens, since nothing is awaited.
                if self.has_feature("models") {
                    self.send(Command::ListModels {});
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
                self.model = current.clone();
                // The startup ask for the pill awaits nothing: leave another modal's wait alone.
                if self.awaiting == Some(Awaiting::Models) {
                    self.awaiting = None;
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
            let images = self.input.take_images(trimmed);
            let images = if self.has_feature("images") { images } else { Vec::new() };
            self.send(Command::Submit { text: message, images });
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
            self.notices.push("Available: /model /resume /goal <goal> /budget <usd> /apikeys /mcp /message-apps /clear /bootscreen /matrix /tetris /about /help.".to_string());
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
            "/budget" => ("budget", Command::Budget { arg: rest.trim().to_string() }, None),
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
        let action = modal.show(ctx);
        self.act_on_modal(action);
        // The modal's own box had the keyboard: hand it back to the view's box, or (embedded in
        // moon) it lands on another pane.
        if self.modal.is_none() {
            ctx.memory_mut(|memory| memory.request_focus(self.input.id));
        }
    }

    fn act_on_modal(&mut self, action: ModalAction) {
        match action {
            ModalAction::None => {}
            ModalAction::Close => self.modal = None,
            ModalAction::Send(command) => self.send(command),
            ModalAction::SetModel(model) => {
                self.modal = None;
                self.model = model.clone();
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

    /// The view's keys, when they are meant for it: its box has the keyboard, or the pointer is
    /// over it. An app's other panes have their own Esc.
    fn handle_keys(&mut self, ctx: &egui::Context, view_rect: egui::Rect) {
        let focused = ctx.memory(|memory| memory.has_focus(self.input.id));
        let hovered = ctx.pointer_hover_pos().is_some_and(|pos| view_rect.contains(pos));
        if !(focused || hovered || self.input.grabs_focus) {
            return;
        }
        // Embedded: a click anywhere in the view gives its box the keyboard.
        if !self.input.grabs_focus && hovered && ctx.input(|input| input.pointer.any_pressed()) {
            ctx.memory_mut(|memory| memory.request_focus(self.input.id));
        }
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
            ui.label(RichText::new("minerve is not running").color(theme::c().magenta).strong());
            ui.add_space(theme::FONT_SIZE);
            ui.label(error);
            ui.add_space(theme::FONT_SIZE);
            if !self.attached {
                ui.label(RichText::new(format!("log: {}", backend::log_path().display())).color(theme::c().dark_gray));
            }
            ui.add_space(theme::FONT_SIZE);
            // The host's minerve is started again the host's way (moon: the card's resume).
            if !self.attached && ui.button(" Restart minerve ").clicked() {
                self.start_backend(&ui.ctx().clone());
            }
        });
    }
}

impl MinerveView {
    /// What is typed in the message box.
    pub fn draft(&self) -> &str {
        &self.input.text
    }

    /// Whether minerve went away or could not be reached: the view says why.
    pub fn has_failed(&self) -> bool {
        self.error.is_some()
    }

    /// Whether minerve is up and answering: it said hello and has not gone away.
    pub fn is_connected(&self) -> bool {
        self.backend.is_some() && !self.features.is_empty() && self.error.is_none()
    }

    /// The session minerve is in, once it said: what `Options::resume` takes to come back to it.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// The colors to draw with from the next frame: an embedding app's palette, which it can
    /// hand again on every frame (a light/dark switch).
    pub fn set_colors(&mut self, colors: theme::Colors) {
        self.colors = colors;
    }

    /// Draws the view in all of `ui`, styled like minerve whatever the app's style is.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        theme::set(self.colors);
        theme::style(ui.style_mut());
        let view_rect = ui.max_rect();
        ui.painter().rect_filled(view_rect, 0.0, theme::c().background);
        self.drain_events();
        self.handle_keys(ctx, view_rect);
        if let Some(screenshot) = self.screenshot.as_mut() {
            screenshot.tick(ctx);
        }
        if self.boot.as_ref().is_some_and(BootClip::is_finished) {
            self.boot = None;
        }
        let panel = |top: i8, bottom: i8| {
            egui::Frame::NONE.fill(theme::c().background).inner_margin(egui::Margin { left: theme::PANEL_MARGIN, right: theme::PANEL_MARGIN, top, bottom })
        };
        let no_frame = egui::Frame::NONE.fill(theme::c().background).inner_margin(theme::CHAT_MARGIN);

        egui::Panel::bottom(self.id.with("status_bar"))
            .frame(egui::Frame::NONE.fill(theme::c().bar).inner_margin(egui::Margin::symmetric(theme::PANEL_MARGIN, 5)))
            .show_separator_line(false)
            .show(ui, |ui| {
                let usage = if self.usage.is_empty() {
                    egui::text::LayoutJob::simple_singleline("no turns yet".into(), theme::small(), theme::c().dim)
                } else {
                    bottom_bar::usage_job(&self.usage)
                };
                let usage_width = ui.fonts_mut(|fonts| fonts.layout_job(usage.clone()).size().x);
                let cwd = bottom_bar::tilde_cwd(self.cwd.as_deref().or(self.args.launcher.workdir.as_deref()));
                ui.horizontal(|ui| {
                    // The usage keeps its room; the folder has what is left, cut short.
                    let room = ui.available_width() - usage_width - 16.0;
                    if room > 40.0 {
                        ui.allocate_ui(egui::vec2(room, ui.available_height()), |ui| {
                            ui.add(egui::Label::new(RichText::new(cwd).font(theme::small()).color(theme::c().dark_gray)).truncate());
                        });
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(usage);
                    });
                });
            });
        let mut submitted = None;
        let mut input_action = InputAction::None;
        egui::Panel::bottom(self.id.with("input")).frame(panel(4, 12)).show_separator_line(false).show(ui, |ui| {
            // A toy reads the keys itself.
            if self.toy.is_none() {
                input_action = self.input.show(ui, &self.model, self.working);
            }
        });
        if let Some(error) = self.input.paste_error.take() {
            self.status = error;
        }
        match input_action {
            InputAction::Send(text) => submitted = Some(text),
            InputAction::OpenModels => {
                if let Some(command) = self.server_command("/model") {
                    self.send(command);
                }
            }
            InputAction::Cancel => {
                if self.has_feature("cancel") {
                    self.send(Command::Cancel {});
                }
            }
            InputAction::None => {}
        }
        egui::Panel::bottom(self.id.with("status")).frame(panel(10, 2)).show_separator_line(false).show(ui, |ui| {
            if self.working {
                let frame = (ctx.input(|input| input.time) * 8.0) as usize % SPINNER.len();
                ctx.request_repaint_after(std::time::Duration::from_millis(125));
                let status = if self.status.is_empty() { "working..." } else { &self.status };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} {status}", SPINNER[frame])).font(theme::small()).color(theme::c().magenta));
                    ui.label(RichText::new("  ·  esc to stop").font(theme::small()).color(theme::c().dark_gray));
                });
            } else {
                ui.label(RichText::new(&self.status).font(theme::small()).color(theme::c().magenta));
            }
        });

        egui::CentralPanel::default().frame(no_frame).show(ui, |ui| {
            if let Some(error) = self.error.clone() {
                self.show_error(ui, &error);
                return;
            }
            // The terminal views (boot clip, matrix, tetris) run to the view's left, right and
            // top edges; only the chat keeps its reading margins.
            let content = ui.available_rect_before_wrap();
            let window = view_rect;
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
            if let Some(name) = slash::show_menu(ctx, self.id.with("slash menu"), &self.input.text, self.input.rect) {
                self.input.set_text(ctx, name);
            }
        }
        self.run_modal(ctx);
        if let Some(text) = submitted {
            self.submit(text);
        }
    }
}
