//! The minerve behind the view: a `minerve --serve` child that connects to a socket we listen on.
//! `Launcher` says which program that is: the installed `minerve`, or an app that links minerve
//! and serves it from its own executable (`moon __minerve --serve`).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::protocol::{Command, Event};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CLEAN_EXIT_TIMEOUT: Duration = Duration::from_secs(2);

/// How to start the minerve to talk to. `--serve` (and `--resume <id>`) are added after `args`.
#[derive(Clone, Debug, Default)]
pub struct Launcher {
    /// `None`: the `minerve` next to this executable, else in the install folders, else on PATH.
    pub program: Option<PathBuf>,
    /// Put before `--serve`, e.g. `["__minerve"]` for an app serving minerve as a subcommand.
    pub args: Vec<String>,
    /// The folder minerve works in (its tools run there). `None`: this process's.
    pub workdir: Option<PathBuf>,
}

/// A minerve the host app already runs and talks to, handed to `MinerveView::attached`: its
/// event lines (the control protocol's, one JSON object each, the host replaying `hello` and
/// the latest state first), and where command lines go. Ending the minerve is the host's.
pub struct Link {
    pub events: Receiver<String>,
    pub send: Box<dyn Fn(String) + Send + Sync>,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Link")
    }
}

/// Where command lines go.
enum Transport {
    /// The socket of the minerve we started, which we end.
    Socket { stream: Mutex<UnixStream>, child: Child, socket_path: PathBuf },
    /// The host's minerve.
    Link(Box<dyn Fn(String) + Send + Sync>),
}

/// Each view in this process listens on its own socket.
static NEXT_SOCKET: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub enum BackendEvent {
    Event(Event),
    /// The socket closed or a line could not be read; the reason is for the user.
    Disconnected(String),
}

pub struct Backend {
    transport: Transport,
    pub events: Receiver<BackendEvent>,
}

/// Reads event lines, handing each to `tx` and waking the UI. Ends with `Disconnected`.
fn forward_lines(lines: impl Iterator<Item = Result<String, String>>, tx: mpsc::Sender<BackendEvent>, ctx: eframe::egui::Context) {
    for line in lines {
        let event = line.and_then(|line| {
            serde_json::from_str::<Event>(&line).map_err(|err| format!("unreadable line ({err}): {line}"))
        });
        match event {
            Ok(event) => {
                // The view is gone: stop, which lets go of the host's end too.
                if tx.send(BackendEvent::Event(event)).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
            Err(reason) => {
                let _ = tx.send(BackendEvent::Disconnected(reason));
                ctx.request_repaint();
                return;
            }
        }
    }
    let _ = tx.send(BackendEvent::Disconnected("minerve closed the connection".into()));
    ctx.request_repaint();
}

fn minerve_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(std::env::temp_dir).join(".minerve")
}

pub fn log_path() -> PathBuf {
    minerve_dir().join("gui-minerve.log")
}

/// `--minerve`, else the `minerve` next to this executable, else `minerve` from PATH.
fn minerve_executable(explicit: Option<&Path>) -> PathBuf {
    if let Some(path) = explicit {
        return path.to_path_buf();
    }
    let sibling = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("minerve")));
    match sibling {
        Some(path) if path.is_file() => path,
        _ => well_known_install_dirs().into_iter().map(|dir| dir.join("minerve")).find(|path| path.is_file()).unwrap_or_else(|| PathBuf::from("minerve")),
    }
}

/// Where the installers put minerve; a GUI launched outside a shell may not have these on PATH.
fn well_known_install_dirs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return Vec::new() };
    vec![home.join(".local/bin"), home.join(".cargo/bin")]
}

fn last_log_lines(count: usize) -> String {
    let text = std::fs::read_to_string(log_path()).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

impl Backend {
    /// Starts minerve and waits for it to connect. `resume` is a session id for `--resume`.
    pub fn start(resume: Option<&str>, launcher: &Launcher, ctx: eframe::egui::Context) -> Result<Self, String> {
        std::fs::create_dir_all(minerve_dir()).map_err(|err| format!("cannot create ~/.minerve: {err}"))?;
        let index = NEXT_SOCKET.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let socket_path = minerve_dir().join(format!("gui-{}-{index}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path)
            .map_err(|err| format!("cannot listen on {}: {err}", socket_path.display()))?;
        listener.set_nonblocking(true).map_err(|err| err.to_string())?;

        let executable = minerve_executable(launcher.program.as_deref());
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path())
            .map_err(|err| format!("cannot open {}: {err}", log_path().display()))?;
        let mut command = Process::new(&executable);
        if let Some(workdir) = &launcher.workdir {
            command.current_dir(workdir);
        }
        command
            .args(&launcher.args)
            .arg("--serve")
            .env("MINERVE_CONTROL_SOCKET", &socket_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        if let Some(id) = resume {
            command.arg("--resume").arg(id);
        }
        let mut child = command.spawn().map_err(|err| {
            let _ = std::fs::remove_file(&socket_path);
            if err.kind() == std::io::ErrorKind::NotFound {
                format!("cannot find {} ({err}). Install minerve (curl -fsSL https://raw.githubusercontent.com/antoineMoPa/minerve/main/install.sh | sh) or pass --minerve <path>.", executable.display())
            } else {
                format!("cannot start {}: {err}", executable.display())
            }
        })?;

        let deadline = Instant::now() + CONNECT_TIMEOUT;
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(fail(&mut child, &socket_path, format!("accept failed: {err}"))),
            }
            if let Ok(Some(status)) = child.try_wait() {
                return Err(fail(&mut child, &socket_path, format!("minerve exited early ({status}). minerve-gui needs a minerve that has --serve: update minerve.")));
            }
            if Instant::now() > deadline {
                return Err(fail(&mut child, &socket_path, "minerve did not connect in 10 s".into()));
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        // Connected: the path is no longer needed, and a crash must not leave it behind.
        let _ = std::fs::remove_file(&socket_path);
        stream.set_nonblocking(false).map_err(|err| err.to_string())?;
        let reader = stream.try_clone().map_err(|err| err.to_string())?;
        let (tx, events) = mpsc::channel();
        std::thread::spawn(move || {
            forward_lines(BufReader::new(reader).lines().map(|line| line.map_err(|err| err.to_string())), tx, ctx)
        });
        Ok(Self { transport: Transport::Socket { stream: Mutex::new(stream), child, socket_path }, events })
    }

    /// Talks to the host's minerve over `link`.
    pub fn attach(link: Link, ctx: eframe::egui::Context) -> Self {
        let Link { events: lines, send } = link;
        let (tx, events) = mpsc::channel();
        std::thread::spawn(move || forward_lines(lines.into_iter().map(Ok), tx, ctx));
        Self { transport: Transport::Link(send), events }
    }

    pub fn send(&self, command: &Command) {
        let line = serde_json::to_string(command).expect("commands always serialize");
        // A minerve that went away shows up as `Disconnected`.
        match &self.transport {
            Transport::Socket { stream, .. } => {
                let _ = stream.lock().unwrap().write_all(format!("{line}\n").as_bytes());
            }
            Transport::Link(send) => send(line),
        }
    }
}

fn fail(child: &mut Child, socket_path: &Path, reason: String) -> String {
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(socket_path);
    format!("{reason}\n{}", last_log_lines(20))
}

impl Drop for Backend {
    /// Ends the minerve we started. The host's goes on: it is the host's to end.
    fn drop(&mut self) {
        let Transport::Socket { stream, child, socket_path } = &mut self.transport else { return };
        let _ = stream.lock().unwrap().shutdown(std::net::Shutdown::Both);
        let deadline = Instant::now() + CLEAN_EXIT_TIMEOUT;
        while Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if !matches!(child.try_wait(), Ok(Some(_))) {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(socket_path);
    }
}
