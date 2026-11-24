//! Messages sent while a turn runs: they wait here, oldest first, and one is sent each time a
//! turn ends, like the terminal app's queue. Esc takes the newest one back into the message box.

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

    /// The message to send now that a turn ended.
    pub fn take_oldest_message(&mut self) -> Option<QueuedMessage> {
        self.messages.pop_front()
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
    fn the_oldest_is_sent_first_and_the_newest_is_taken_back() {
        let mut queue = MessageQueue::default();
        for text in ["one", "two", "three"] {
            queue.push_message(message(text));
        }
        assert_eq!(queue.list_texts(), ["one", "two", "three"]);
        assert_eq!(queue.take_newest_message(), Some(message("three")));
        assert_eq!(queue.take_oldest_message(), Some(message("one")));
        assert_eq!(queue.len(), 1);
        queue.clear_messages();
        assert!(queue.is_empty());
    }
}
