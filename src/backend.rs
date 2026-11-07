//! The minerve behind the window: a `minerve --serve` child that connects to a socket we listen on.

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

pub enum BackendEvent {
    Event(Event),
    /// The socket closed or a line could not be read; the reason is for the user.
    Disconnected(String),
}

pub struct Backend {
    writer: Mutex<UnixStream>,
    child: Child,
    socket_path: PathBuf,
    pub events: Receiver<BackendEvent>,
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
    pub fn start(resume: Option<&str>, minerve: Option<&Path>, ctx: eframe::egui::Context) -> Result<Self, String> {
        std::fs::create_dir_all(minerve_dir()).map_err(|err| format!("cannot create ~/.minerve: {err}"))?;
        let socket_path = minerve_dir().join(format!("gui-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path)
            .map_err(|err| format!("cannot listen on {}: {err}", socket_path.display()))?;
        listener.set_nonblocking(true).map_err(|err| err.to_string())?;

        let executable = minerve_executable(minerve);
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path())
            .map_err(|err| format!("cannot open {}: {err}", log_path().display()))?;
        let mut command = Process::new(&executable);
        command
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
            for line in BufReader::new(reader).lines() {
                let event = line
                    .map_err(|err| err.to_string())
                    .and_then(|line| serde_json::from_str::<Event>(&line).map_err(|err| format!("unreadable line ({err}): {line}")));
                match event {
                    Ok(event) => {
                        let _ = tx.send(BackendEvent::Event(event));
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
        });
        Ok(Self { writer: Mutex::new(stream), child, socket_path, events })
    }

    pub fn send(&self, command: &Command) {
        let mut line = serde_json::to_vec(command).expect("commands always serialize");
        line.push(b'\n');
        // A minerve that went away shows up as `Disconnected`.
        let _ = self.writer.lock().unwrap().write_all(&line);
    }
}

fn fail(child: &mut Child, socket_path: &Path, reason: String) -> String {
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(socket_path);
    format!("{reason}\n{}", last_log_lines(20))
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.writer.lock().unwrap().shutdown(std::net::Shutdown::Both);
        let deadline = Instant::now() + CLEAN_EXIT_TIMEOUT;
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
