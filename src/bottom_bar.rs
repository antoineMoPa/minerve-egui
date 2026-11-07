//! The lines under the chat: usage summary and key hint, with labels dimmer than values.

use eframe::egui::{text::LayoutJob, Color32, TextFormat};

use crate::theme;

fn format(color: Color32) -> TextFormat {
    TextFormat { font_id: theme::small(), color, ..Default::default() }
}

/// `Sent: 7.5k | Cost: $0.0010` as dim labels, bright values and `·` between.
pub fn usage_job(summary: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (index, part) in summary.split(" | ").enumerate() {
        if index > 0 {
            job.append("  ·  ", 0.0, format(theme::SEP));
        }
        match part.split_once(": ") {
            Some((label, value)) => {
                job.append(&format!("{label} "), 0.0, format(theme::DARK_GRAY));
                job.append(value, 0.0, format(theme::GRAY));
            }
            None => job.append(part, 0.0, format(theme::GRAY)),
        }
    }
    job
}

/// `Enter send · Esc cancel turn` with the keys bright and what they do dim.
pub fn hint_job(hint: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (index, part) in hint.split(" · ").enumerate() {
        if index > 0 {
            job.append("  ·  ", 0.0, format(theme::SEP));
        }
        let (key, what) = part.split_once(' ').unwrap_or((part, ""));
        job.append(key, 0.0, format(theme::NAME));
        if !what.is_empty() {
            job.append(&format!(" {what}"), 0.0, format(theme::DARK_GRAY));
        }
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_keeps_every_word_and_swaps_the_separator() {
        let job = usage_job("Sent: 7.5k (0% cached) | Cost: $0.0010");
        assert_eq!(job.text, "Sent 7.5k (0% cached)  ·  Cost $0.0010");
    }

    #[test]
    fn hint_splits_the_key_from_what_it_does() {
        let job = hint_job("Enter send · Ctrl+O tool output");
        assert_eq!(job.text, "Enter send  ·  Ctrl+O tool output");
    }
}
