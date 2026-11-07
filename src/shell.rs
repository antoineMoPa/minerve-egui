//! `!command`: runs a shell command in the folder minerve was started in and keeps its output
//! for the next message, like the terminal app. minerve is not involved.

use std::sync::mpsc::{self, Receiver};

/// Runs `command` on its own thread; the receiver gets the finished block, ready to send.
pub fn spawn(command: String, ctx: eframe::egui::Context) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(block(&command, &run(&command)));
        ctx.request_repaint();
    });
    rx
}

fn run(command: &str) -> String {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(std::process::Stdio::null())
        .output()
        .map(|out| {
            if out.status.success() {
                String::from_utf8_lossy(&out.stdout).to_string()
            } else {
                format!("[Error] {}", String::from_utf8_lossy(&out.stderr))
            }
        })
        .unwrap_or_else(|err| format!("[Error] {err}"))
}

fn block(command: &str, output: &str) -> String {
    format!("```\n$ {}\n{}\n```", command, output.trim_end())
}

/// The message to send: the queued outputs ahead of what was typed, as the terminal app does.
pub fn with_context(outputs: &[String], message: &str) -> String {
    if outputs.is_empty() {
        return message.to_string();
    }
    format!("Output of commands I ran in the shell:\n```\n{}\n```\n\n{}", outputs.join("\n\n"), message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_runs_and_its_output_is_fenced() {
        assert_eq!(block("echo hi", &run("echo hi")), "```\n$ echo hi\nhi\n```");
    }

    #[test]
    fn a_failing_command_reports_its_error() {
        assert!(run("ls /definitely/not/here").starts_with("[Error]"));
    }

    #[test]
    fn queued_output_goes_ahead_of_the_message() {
        assert_eq!(with_context(&[], "hi"), "hi");
        let text = with_context(&["```\n$ ls\na\n```".to_string()], "what is this?");
        assert!(text.starts_with("Output of commands I ran in the shell:"));
        assert!(text.ends_with("what is this?"));
    }
}
