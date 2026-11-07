//! The tinted bar along the window's bottom edge: cwd and usage.

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
                // The cost is what people look for: brightest.
                let color = if label == "Cost" { theme::WHITE } else { theme::GRAY };
                job.append(value, 0.0, format(color));
            }
            None => job.append(part, 0.0, format(theme::GRAY)),
        }
    }
    job
}

/// `~/prog/minerve` for the bar's left side.
pub fn tilde_cwd() -> String {
    let Ok(dir) = std::env::current_dir() else { return String::new() };
    match dirs::home_dir().and_then(|home| dir.strip_prefix(&home).ok().map(|rest| rest.to_path_buf())) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", rest.display()),
        None => dir.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_keeps_every_word_and_swaps_the_separator() {
        let job = usage_job("Sent: 7.5k (0% cached) | Cost: $0.0010");
        assert_eq!(job.text, "Sent 7.5k (0% cached)  ·  Cost $0.0010");
    }
}
