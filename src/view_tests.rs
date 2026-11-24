//! The view against a host's minerve that only records the command lines it is sent.

use std::sync::{Arc, Mutex};

use super::*;

/// A view attached to a minerve that answers nothing, and the commands it received.
fn attach_view_to_recording_minerve(ctx: &egui::Context) -> (MinerveView, Arc<Mutex<Vec<String>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let recorded = sent.clone();
    // The sender is kept alive, or the view would see minerve go away.
    let (events_tx, events) = std::sync::mpsc::channel();
    std::mem::forget(events_tx);
    let link = Link {
        events,
        send: Box::new(move |line| recorded.lock().unwrap().push(line)),
    };
    let mut view = MinerveView::attached(ctx, link, Options::default());
    view.features = vec!["serve".to_string(), "cancel".to_string()];
    (view, sent)
}

fn list_submitted_texts(sent: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    sent.lock()
        .unwrap()
        .iter()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            (value["type"] == "submit").then(|| value["text"].as_str().unwrap().to_string())
        })
        .collect()
}

fn end_turn(view: &mut MinerveView) {
    view.apply(Event::State {
        state: AgentState::WaitingForInput,
    });
}

#[test]
fn messages_sent_while_a_turn_runs_wait_and_go_one_per_ended_turn() {
    let ctx = egui::Context::default();
    let (mut view, sent) = attach_view_to_recording_minerve(&ctx);
    view.submit("one".to_string());
    view.submit("two".to_string());
    view.submit("three".to_string());
    assert_eq!(list_submitted_texts(&sent), ["one"]);
    assert_eq!(view.queue.list_texts(), ["two", "three"]);

    end_turn(&mut view);
    assert_eq!(list_submitted_texts(&sent), ["one", "two"]);
    assert!(view.working, "the queued message started a turn");

    end_turn(&mut view);
    end_turn(&mut view);
    assert_eq!(list_submitted_texts(&sent), ["one", "two", "three"]);
    assert!(!view.working);
}

#[test]
fn the_newest_queued_message_goes_back_to_the_box_ahead_of_the_draft() {
    let ctx = egui::Context::default();
    let (mut view, sent) = attach_view_to_recording_minerve(&ctx);
    view.submit("one".to_string());
    view.submit("two".to_string());
    view.input.text = "draft".to_string();
    assert!(view.take_back_newest_queued_message(&ctx));
    assert_eq!(view.input.text, "two\ndraft");
    assert!(view.queue.is_empty());
    assert!(!view.take_back_newest_queued_message(&ctx));

    end_turn(&mut view);
    assert_eq!(list_submitted_texts(&sent), ["one"]);
}

#[test]
fn a_new_session_drops_the_queue() {
    let ctx = egui::Context::default();
    let (mut view, _sent) = attach_view_to_recording_minerve(&ctx);
    view.submit("one".to_string());
    view.submit("two".to_string());
    view.submit("/clear".to_string());
    assert!(view.queue.is_empty());
}
