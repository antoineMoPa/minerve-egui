//! The GUI's protocol types against a real `minerve --serve`: every event it sends must parse
//! (none may fall into `Unknown`), and every command the GUI can send must be accepted.
//! Needs a minerve with `--serve`: `MINERVE_BIN`, else `../minerve/target/debug/minerve`, else PATH.
//! Skipped (loudly) when none is found, because CI builds this repo alone.

#[path = "../src/protocol.rs"]
#[allow(dead_code)]
mod protocol;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use protocol::{Command as Cmd, Event};

fn minerve() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MINERVE_BIN") {
        return Some(PathBuf::from(path));
    }
    let sibling = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../minerve/target/debug/minerve");
    if sibling.is_file() {
        return Some(sibling);
    }
    let output = Command::new("which").arg("minerve").output().ok()?;
    let path = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

#[test]
fn every_event_parses_and_every_command_is_accepted() {
    let Some(minerve) = minerve() else {
        eprintln!("SKIPPED: no minerve found (set MINERVE_BIN)");
        return;
    };
    let dir = std::env::temp_dir().join(format!("minerve-gui-contract-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let socket_path = dir.join("s.sock");
    let listener = UnixListener::bind(&socket_path).unwrap();
    let mut child = Command::new(&minerve)
        .arg("--serve")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", &dir)
        .env("MINERVE_CONTROL_SOCKET", &socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (stream, _) = listener.accept().unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);

    let read = |reader: &mut BufReader<_>| -> Event {
        let mut line = String::new();
        reader.read_line(&mut line).expect("read from minerve");
        let event: Event = serde_json::from_str(&line).unwrap_or_else(|err| panic!("unreadable {line:?}: {err}"));
        assert_ne!(event, Event::Unknown, "the GUI does not know this event: {line}");
        event
    };

    let Event::Hello { features, .. } = read(&mut reader) else { panic!("first event is not hello") };
    assert!(features.iter().any(|feature| feature == "serve"));

    // Safe commands only: nothing that spends tokens, opens a browser or touches real config.
    let commands = [
        Cmd::GetIntro {},
        Cmd::ListModels {},
        Cmd::ListSessions {},
        Cmd::GetApiKeys {},
        Cmd::ListMcp {},
        Cmd::MessageAppsStatus {},
        Cmd::McpRemove { name: "nope".into() },
        Cmd::MessageAppsSave { bot_name: "Bad Name".into(), app_token: None, bot_token: None },
        Cmd::NewSession {},
        Cmd::Cancel {},
    ];
    for command in &commands {
        writeln!(writer, "{}", serde_json::to_string(command).unwrap()).unwrap();
    }
    // Replies arrive in any order; read until the last one we know to expect, parsing all.
    let (mut intro, mut models, mut sessions, mut keys, mut mcp, mut apps) = (false, false, false, false, false, false);
    for _ in 0..60 {
        match read(&mut reader) {
            Event::Intro { .. } => intro = true,
            Event::Models { .. } => models = true,
            Event::Sessions { .. } => sessions = true,
            Event::ApiKeys { .. } => keys = true,
            Event::McpServers { .. } => mcp = true,
            Event::MessageApps { .. } => apps = true,
            _ => {}
        }
        if intro && models && sessions && keys && mcp && apps {
            break;
        }
    }
    assert!(intro && models && sessions && keys && mcp && apps, "missing replies: {intro} {models} {sessions} {keys} {mcp} {apps}");
    assert!(child.try_wait().unwrap().is_none(), "minerve died on a command the GUI sends");

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
