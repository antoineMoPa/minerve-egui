//! Messages sent while a turn runs, oldest first, shown under the chat until the model gets them.
//! Esc takes the newest one back into the message box.
//!
//! minerve with the `queue` feature keeps them too: it adds them to the running turn after its
//! tool calls and says how many went (`forget_oldest_messages`). An older minerve refuses a
//! message during a turn, so they all go together as one when the turn ends.

use std::collections::VecDeque;
use std::path::PathBuf;

/// A message as it was typed, with its clipboard images by their `[Image #N]` token.
#[derive(Clone, Debug, PartialEq)]
pub struct QueuedMessage {
    pub text: String,
    pub images: Vec<(String, PathBuf)>,
}

#[derive(Default)]
pub struct MessageQueue {
    messages: VecDeque<QueuedMessage>,
}

impl MessageQueue {
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn push_message(&mut self, message: QueuedMessage) {
        self.messages.push_back(message);
    }

    /// The message to send now that a turn ended: every waiting message, oldest first, as one,
    /// so the model reads them all before it answers.
    pub fn take_all_messages_as_one(&mut self) -> Option<QueuedMessage> {
        let mut merged = self.messages.pop_front()?;
        for message in self.messages.drain(..) {
            merged.text.push_str("\n\n");
            merged.text.push_str(&message.text);
            merged.images.extend(message.images);
        }
        Some(merged)
    }

    /// Drops the `count` oldest messages: minerve sent them to the model.
    pub fn forget_oldest_messages(&mut self, count: usize) {
        self.messages.drain(..count.min(self.messages.len()));
    }

    /// The message Esc gives back to the message box.
    pub fn take_newest_message(&mut self) -> Option<QueuedMessage> {
        self.messages.pop_back()
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }

    /// What the chat shows under the transcript, oldest first.
    pub fn list_texts(&self) -> Vec<&str> {
        self.messages
            .iter()
            .map(|message| message.text.as_str())
            .collect()
    }
}

/// `2 queued`, for the status line.
pub fn describe_count(count: usize) -> String {
    format!("{count} queued")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(text: &str) -> QueuedMessage {
        QueuedMessage {
            text: text.to_string(),
            images: Vec::new(),
        }
    }

    #[test]
    fn all_messages_are_sent_as_one_and_the_newest_is_taken_back() {
        let mut queue = MessageQueue::default();
        for text in ["one", "two", "three"] {
            queue.push_message(message(text));
        }
        assert_eq!(queue.list_texts(), ["one", "two", "three"]);
        assert_eq!(queue.take_newest_message(), Some(message("three")));
        assert_eq!(
            queue.take_all_messages_as_one(),
            Some(message("one\n\ntwo"))
        );
        assert!(queue.is_empty());
        assert_eq!(queue.take_all_messages_as_one(), None);
    }

    #[test]
    fn the_images_of_merged_messages_keep_their_order() {
        let mut queue = MessageQueue::default();
        for (text, image) in [("one", "a.png"), ("two", "b.png")] {
            queue.push_message(QueuedMessage {
                text: text.to_string(),
                images: vec![("[Image #1]".to_string(), PathBuf::from(image))],
            });
        }
        let merged = queue.take_all_messages_as_one().unwrap();
        let paths: Vec<_> = merged.images.iter().map(|(_, path)| path.clone()).collect();
        assert_eq!(paths, [PathBuf::from("a.png"), PathBuf::from("b.png")]);
    }

    #[test]
    fn the_oldest_messages_are_forgotten_once_sent() {
        let mut queue = MessageQueue::default();
        for text in ["one", "two", "three"] {
            queue.push_message(message(text));
        }
        queue.forget_oldest_messages(2);
        assert_eq!(queue.list_texts(), ["three"]);
        // More than are left: one was taken back as the turn took it.
        queue.forget_oldest_messages(2);
        assert!(queue.is_empty());
    }

    #[test]
    fn a_cleared_queue_is_empty() {
        let mut queue = MessageQueue::default();
        queue.push_message(message("one"));
        assert_eq!(queue.len(), 1);
        queue.clear_messages();
        assert!(queue.is_empty());
    }
}
