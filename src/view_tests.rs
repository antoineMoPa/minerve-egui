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
fn messages_sent_while_a_turn_runs_wait_and_go_together_when_it_ends() {
    let ctx = egui::Context::default();
    let (mut view, sent) = attach_view_to_recording_minerve(&ctx);
    view.submit("one".to_string());
    view.submit("two".to_string());
    view.submit("three".to_string());
    assert_eq!(list_submitted_texts(&sent), ["one"]);
    assert_eq!(view.queue.list_texts(), ["two", "three"]);

    end_turn(&mut view);
    assert_eq!(list_submitted_texts(&sent), ["one", "two\n\nthree"]);
    assert!(view.queue.is_empty());
    assert!(view.working, "the queued messages started a turn");

    end_turn(&mut view);
    assert_eq!(list_submitted_texts(&sent).len(), 2);
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

fn count_cancels(sent: &Arc<Mutex<Vec<String>>>) -> usize {
    sent.lock()
        .unwrap()
        .iter()
        .filter(|line| line.contains(r#""type":"cancel""#))
        .count()
}

#[test]
fn a_stop_cancels_the_turn_and_no_queued_message_starts_another() {
    let ctx = egui::Context::default();
    let (mut view, sent) = attach_view_to_recording_minerve(&ctx);
    view.submit("one".to_string());
    view.submit("two".to_string());
    view.submit("three".to_string());
    view.input.text = "draft".to_string();

    view.stop_running_turn(&ctx);
    assert_eq!(count_cancels(&sent), 1);
    assert!(view.queue.is_empty());
    assert_eq!(view.input.text, "two\nthree\ndraft");

    end_turn(&mut view);
    assert_eq!(list_submitted_texts(&sent), ["one"]);
    assert!(!view.working, "the stop left no turn running");
}

fn count_unqueues(sent: &Arc<Mutex<Vec<String>>>) -> usize {
    sent.lock()
        .unwrap()
        .iter()
        .filter(|line| line.contains(r#""type":"unqueue_newest""#))
        .count()
}

/// With the `queue` feature, minerve adds a message to the running turn: it is sent at once.
#[test]
fn a_message_sent_while_a_turn_runs_goes_to_minerve_at_once_and_shows_until_the_model_gets_it() {
    let ctx = egui::Context::default();
    let (mut view, sent) = attach_view_to_recording_minerve(&ctx);
    view.features.push("queue".to_string());
    view.submit("one".to_string());
    view.submit("two".to_string());
    view.submit("three".to_string());
    assert_eq!(list_submitted_texts(&sent), ["one", "two", "three"]);
    assert_eq!(view.queue.list_texts(), ["two", "three"]);

    view.apply(Event::QueuedSent { count: 1 });
    assert_eq!(view.queue.list_texts(), ["three"]);

    // Taken back, it leaves minerve's queue too.
    assert!(view.take_back_newest_queued_message(&ctx));
    assert_eq!(count_unqueues(&sent), 1);
    assert_eq!(view.input.text, "three");

    end_turn(&mut view);
    assert_eq!(
        list_submitted_texts(&sent).len(),
        3,
        "nothing is sent twice"
    );
    assert!(!view.working);
}

#[test]
fn a_stop_takes_the_queued_messages_out_of_minerve_before_it_cancels() {
    let ctx = egui::Context::default();
    let (mut view, sent) = attach_view_to_recording_minerve(&ctx);
    view.features.push("queue".to_string());
    view.submit("one".to_string());
    view.submit("two".to_string());
    view.submit("three".to_string());

    view.stop_running_turn(&ctx);
    assert_eq!(view.input.text, "two\nthree");
    let sent = sent.lock().unwrap();
    let kinds: Vec<&str> = sent
        .iter()
        .rev()
        .take(3)
        .map(|line| line.split('"').nth(3).unwrap())
        .collect();
    assert_eq!(kinds, ["cancel", "unqueue_newest", "unqueue_newest"]);
}

/// Where the text `label` was drawn in the frame's shapes, if it was.
fn find_drawn_text(shapes: &[egui::epaint::ClippedShape], label: &str) -> Option<egui::Rect> {
    shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.galley.rect.translate(text.pos.to_vec2()))
        }
        _ => None,
    })
}

fn render_view(
    ctx: &egui::Context,
    view: &mut MinerveView,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1000.0, 700.0),
        )),
        events,
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| view.show(ui));
    output.textures_delta.clear();
    output.shapes
}

#[test]
fn the_skip_button_shows_8s_into_a_tool_call_and_skips_it() {
    let ctx = egui::Context::default();
    let (mut view, sent) = attach_view_to_recording_minerve(&ctx);
    view.features.push("skip_tool_call".to_string());
    view.submit("build it".to_string());

    let shapes = render_view(&ctx, &mut view, vec![]);
    assert!(find_drawn_text(&shapes, "skip this step").is_none());

    view.apply(Event::ToolCallSkippable { skippable: true });
    let shapes = render_view(&ctx, &mut view, vec![]);
    let button = find_drawn_text(&shapes, "skip this step").expect("no skip button");
    let at = button.center();
    render_view(
        &ctx,
        &mut view,
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    render_view(
        &ctx,
        &mut view,
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    let skips = sent
        .lock()
        .unwrap()
        .iter()
        .filter(|line| line.contains(r#""type":"skip_tool_call""#))
        .count();
    assert_eq!(skips, 1);
    let shapes = render_view(&ctx, &mut view, vec![]);
    assert!(find_drawn_text(&shapes, "skip this step").is_none());
}
