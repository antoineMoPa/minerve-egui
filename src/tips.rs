//! Placeholder tips shown dimmed in the empty message box, from the `intro` event.
//! Each tip types out left to right, holds, then the next tip takes its place.

use std::time::{Duration, Instant};

/// Delay between each character while a tip is typing out.
const TYPING_STEP: Duration = Duration::from_millis(35);
/// How long a fully typed tip stays on screen before the next one starts.
const HOLD: Duration = Duration::from_secs(6);
/// Quiet time after the last key before a tip starts typing, so tips do not flicker while the user types.
const IDLE_DELAY: Duration = Duration::from_secs(2);

pub struct Tips {
    tips: Vec<String>,
    index: usize,
    shown_at: Instant,
}

impl Tips {
    /// No tips until `set` is called.
    pub fn new(now: Instant) -> Self {
        Self { tips: Vec::new(), index: 0, shown_at: now + IDLE_DELAY }
    }

    /// Replaces the tips, starting on one picked by the current time so a returning user sees a different one.
    pub fn set(&mut self, tips: Vec<String>, now: Instant) {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0);
        self.index = if tips.is_empty() { 0 } else { seconds as usize % tips.len() };
        self.tips = tips;
        self.shown_at = now + IDLE_DELAY;
    }

    /// Hides the tip until `IDLE_DELAY` passes without another call. Called on every key.
    pub fn restart_idle_delay(&mut self, now: Instant) {
        self.shown_at = now + IDLE_DELAY;
    }

    /// When the text next changes, so the window repaints only then. `None`: nothing will change.
    pub fn next_wake(&self, now: Instant) -> Option<Duration> {
        let tip = self.tips.get(self.index)?;
        let typing_time = TYPING_STEP * (format!("Pro tip: {tip}").chars().count() as u32);
        if now < self.shown_at {
            return Some(self.shown_at - now);
        }
        let elapsed = now - self.shown_at;
        if elapsed < typing_time {
            Some(TYPING_STEP)
        } else {
            Some((typing_time + HOLD).saturating_sub(elapsed).max(Duration::from_millis(1)))
        }
    }

    /// The text to draw at `now`, with the "Pro tip:" prefix. Moves to the next tip once the current one has held.
    pub fn visible_text(&mut self, now: Instant) -> String {
        let Some(tip) = self.tips.get(self.index) else {
            return String::new();
        };
        let full = format!("Pro tip: {tip}");
        let char_count = full.chars().count();
        let typing_time = TYPING_STEP * char_count as u32;

        let elapsed = now.saturating_duration_since(self.shown_at);
        if elapsed >= typing_time + HOLD {
            self.index = (self.index + 1) % self.tips.len();
            self.shown_at = now;
            return String::new();
        }

        let revealed = (elapsed.as_millis() / TYPING_STEP.as_millis()) as usize;
        full.chars().take(revealed.min(char_count)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tips(now: Instant) -> Tips {
        let mut tips = Tips::new(now);
        tips.set(vec!["first tip.".into(), "second tip.".into()], now);
        tips
    }

    #[test]
    fn no_tips_shows_nothing() {
        let now = Instant::now();
        assert_eq!(Tips::new(now).visible_text(now + IDLE_DELAY + HOLD), "");
    }

    #[test]
    fn every_visible_text_is_a_prefix_of_a_tip() {
        let start = Instant::now();
        let mut tips = tips(start);
        let start = start + IDLE_DELAY;
        for step in 0..500 {
            let shown = tips.visible_text(start + TYPING_STEP * step);
            assert!(
                ["Pro tip: first tip.", "Pro tip: second tip."].iter().any(|text| text.starts_with(&shown)),
                "{shown:?} is not a prefix of any tip"
            );
        }
    }

    #[test]
    fn types_left_to_right_then_moves_to_next_tip() {
        let start = Instant::now();
        let mut tips = tips(start);
        let first_index = tips.index;
        let start = start + IDLE_DELAY;

        assert_eq!(tips.visible_text(start), "");
        assert_eq!(tips.visible_text(start + TYPING_STEP * 3), "Pro");

        let full = format!("Pro tip: {}", tips.tips[first_index]);
        let done = start + TYPING_STEP * full.chars().count() as u32;
        assert_eq!(tips.visible_text(done + HOLD / 2), full);

        assert_eq!(tips.visible_text(done + HOLD), "");
        assert_ne!(tips.index, first_index);
    }

    #[test]
    fn stays_hidden_until_typing_stops_for_the_idle_delay() {
        let start = Instant::now();
        let mut tips = tips(start);
        tips.restart_idle_delay(start + IDLE_DELAY / 2);
        assert_eq!(tips.visible_text(start + IDLE_DELAY + TYPING_STEP * 5), "");
        assert_eq!(tips.visible_text(start + IDLE_DELAY / 2 + IDLE_DELAY + TYPING_STEP * 3), "Pro");
    }
}
