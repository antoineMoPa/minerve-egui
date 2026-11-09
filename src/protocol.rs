//! The control socket protocol, GUI side. Mirrors `src/control_socket.rs` of minerve.
//! Lines are JSON objects told apart by `type`. Events this app does not know are `Unknown`.

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Working,
    WaitingForInput,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Hello {
        version: u32,
        #[serde(default)]
        minerve_version: String,
        #[serde(default)]
        features: Vec<String>,
        /// The folder minerve works in. Empty from a minerve too old to say.
        #[serde(default)]
        cwd: String,
    },
    Session { session_id: String },
    State { state: AgentState },
    /// The chat as (role, content), the way the TUI shows it.
    Transcript { messages: Vec<(String, String)> },
    Status { status: String },
    Notice { notice: String },
    Usage { summary: String },
    /// The boot clip (plain-text frames, blank = transparent) and the pro tips, answering `get_intro`.
    Intro { fps: u32, caption: String, frames: Vec<String>, tips: Vec<String> },
    /// Answers `list_models`: every keyed provider's models, and the recent and current ones.
    Models { current: String, recent: Vec<String>, lists: Vec<ModelList> },
    /// Answers `list_sessions`, most recent first.
    Sessions { sessions: Vec<SessionInfo> },
    /// Which provider keys are set (never their values).
    ApiKeys { openrouter: bool, openai: bool, anthropic: bool },
    McpServers { servers: Vec<McpServer> },
    MessageApps { slack: MessageAppsStatus },
    /// The Slack app manifest to paste into Slack, answering `message_apps_manifest`.
    MessageAppsManifest { manifest: String, url: String },
    #[serde(other)]
    Unknown,
}

/// Slack in `/message-apps`. `state`: not_set_up, disconnected, connecting, connected or failed.
/// `link`: none, waiting (`link_detail` is the code to send the bot) or linked (`link_detail` is the member).
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct MessageAppsStatus {
    pub state: String,
    pub detail: String,
    pub bot_name: String,
    pub has_tokens: bool,
    pub members: Vec<String>,
    pub link: String,
    pub link_detail: String,
    pub link_seconds_left: u64,
}

/// `state` is connecting, connected, failed, needs_login, logging_in or disconnected.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct McpServer {
    pub name: String,
    pub target: String,
    pub source: String,
    pub state: String,
    pub tools: usize,
    pub detail: String,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ModelList {
    pub provider: String,
    pub models: Vec<String>,
    pub error: Option<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct SessionInfo {
    pub id: String,
    /// Seconds since the Unix epoch.
    pub modified: u64,
    pub first_prompt: String,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    Submit {
        text: String,
        /// Image files for the prompt; sent only when minerve lists the `images` feature.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        images: Vec<std::path::PathBuf>,
    },
    Cancel {},
    NewSession {},
    GetIntro {},
    Goal { goal: String },
    /// `/budget <arg>`: minerve answers with a notice.
    Budget { arg: String },
    ListModels {},
    SetModel { model: String },
    ListSessions {},
    ResumeSession { id: String },
    GetApiKeys {},
    MessageAppsStatus {},
    MessageAppsSave { bot_name: String, app_token: Option<String>, bot_token: Option<String> },
    MessageAppsStart {},
    MessageAppsStop {},
    MessageAppsLink {},
    MessageAppsCancelLink {},
    MessageAppsManifest { bot_name: String },
    ListMcp {},
    McpConnect { name: String },
    McpDisconnect { name: String },
    McpLogin { name: String },
    McpRemove { name: String },
    McpAdd { name: String, command_or_url: String, env_or_headers: String },
    SetApiKeys { openrouter: Option<String>, openai: Option<String>, anthropic: Option<String> },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Event {
        serde_json::from_str(line).unwrap()
    }

    #[test]
    fn events_parse_from_the_lines_minerve_sends() {
        assert_eq!(
            parse(r#"{"type":"hello","version":1}"#),
            Event::Hello { version: 1, minerve_version: String::new(), features: vec![], cwd: String::new() }
        );
        assert_eq!(
            parse(r#"{"type":"hello","version":1,"minerve_version":"0.8.0","features":["serve","intro"]}"#),
            Event::Hello { version: 1, minerve_version: "0.8.0".into(), features: vec!["serve".into(), "intro".into()], cwd: String::new() }
        );
        assert_eq!(
            parse(r#"{"type":"intro","fps":24,"caption":"c","frames":["f"],"tips":["t"]}"#),
            Event::Intro { fps: 24, caption: "c".into(), frames: vec!["f".into()], tips: vec!["t".into()] }
        );
        assert_eq!(
            parse(r#"{"type":"state","state":"waiting_for_input"}"#),
            Event::State { state: AgentState::WaitingForInput }
        );
        assert_eq!(
            parse(r#"{"type":"transcript","messages":[["user","hi"]]}"#),
            Event::Transcript { messages: vec![("user".into(), "hi".into())] }
        );
        assert_eq!(parse(r#"{"type":"status","status":""}"#), Event::Status { status: String::new() });
        assert_eq!(parse(r#"{"type":"notice","notice":"n"}"#), Event::Notice { notice: "n".into() });
        assert_eq!(parse(r#"{"type":"usage","summary":"s"}"#), Event::Usage { summary: "s".into() });
    }

    #[test]
    fn events_of_other_kinds_are_unknown() {
        assert_eq!(parse(r#"{"type":"message","message":{"role":"user"}}"#), Event::Unknown);
        assert_eq!(parse(r#"{"type":"history","session_id":"x","messages":[]}"#), Event::Unknown);
    }

    #[test]
    fn commands_serialize_to_the_lines_minerve_reads() {
        let line = |c: &Command| serde_json::to_string(c).unwrap();
        assert_eq!(line(&Command::Submit { text: "hi".into(), images: Vec::new() }), r#"{"type":"submit","text":"hi"}"#);
        assert_eq!(line(&Command::Cancel {}), r#"{"type":"cancel"}"#);
        assert_eq!(line(&Command::NewSession {}), r#"{"type":"new_session"}"#);
        assert_eq!(line(&Command::Goal { goal: "x".into() }), r#"{"type":"goal","goal":"x"}"#);
        assert_eq!(line(&Command::Budget { arg: "2".into() }), r#"{"type":"budget","arg":"2"}"#);
        assert_eq!(line(&Command::ListModels {}), r#"{"type":"list_models"}"#);
        assert_eq!(line(&Command::MessageAppsManifest { bot_name: "b".into() }), r#"{"type":"message_apps_manifest","bot_name":"b"}"#);
        assert_eq!(line(&Command::McpRemove { name: "a".into() }), r#"{"type":"mcp_remove","name":"a"}"#);
        assert_eq!(line(&Command::ResumeSession { id: "a".into() }), r#"{"type":"resume_session","id":"a"}"#);
        assert_eq!(
            line(&Command::SetApiKeys { openrouter: Some("k".into()), openai: None, anthropic: None }),
            r#"{"type":"set_api_keys","openrouter":"k","openai":null,"anthropic":null}"#
        );
        assert_eq!(line(&Command::GetIntro {}), r#"{"type":"get_intro"}"#);
    }
}
