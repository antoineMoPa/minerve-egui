//! `/matrix`: falling letters over the chat area. Any key starts the fade-out.

use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, Rect};

use super::{any_key_pressed, next_unit, seed, ToyStatus};
use crate::theme;

/// Characters the falling streams are made of.
const GLYPHS: &str = "123456789abcdefghijklmnopqrstuvwxyzαβγδεζηθικλμνξοπρστυφχψωΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩ";
const FADE: Duration = Duration::from_millis(500);

/// One falling column of letters.
#[derive(Clone, Copy, Debug)]
pub struct Stream {
    /// Row of the leading (brightest) letter. Negative while above the area.
    head: f32,
    /// Rows per second.
    speed: f32,
    /// Trail length in rows, head included. 0 draws nothing.
    length: u16,
}

impl Stream {
    fn random(rng: &mut u64, height: u16) -> Self {
        let height = height.max(1) as f32;
        Self { head: -next_unit(rng) * height, speed: 8.0 + next_unit(rng) * 20.0, length: 4 + (next_unit(rng) * height * 0.6) as u16 }
    }

    /// Moves down by `dt` seconds. A stream past the bottom restarts above the top, or ends when `stopping`.
    fn step(&mut self, dt: f32, height: u16, stopping: bool, rng: &mut u64) {
        self.head += self.speed * dt;
        if self.head - self.length as f32 > height as f32 {
            if stopping {
                self.length = 0;
            } else {
                *self = Stream::random(rng, height);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    FadingIn,
    Running,
    FadingOut,
}

pub struct MatrixRain {
    cols: u16,
    rows: u16,
    streams: Vec<Stream>,
    glyphs: Vec<char>,
    rng: u64,
    last_tick: Instant,
    phase: Phase,
    phase_started: Instant,
    /// Brightness when the fade-out began, so a stop during the fade-in continues from there.
    stop_level: f32,
}

impl MatrixRain {
    pub fn new() -> Self {
        Self {
            cols: 0,
            rows: 0,
            streams: Vec::new(),
            glyphs: GLYPHS.chars().collect(),
            rng: seed(),
            last_tick: Instant::now(),
            phase: Phase::FadingIn,
            phase_started: Instant::now(),
            stop_level: 1.0,
        }
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.streams = (0..cols).map(|_| Stream::random(&mut self.rng, rows)).collect();
    }

    fn stop(&mut self) {
        if self.phase != Phase::FadingOut {
            self.stop_level = self.level();
            self.phase = Phase::FadingOut;
            self.phase_started = Instant::now();
        }
    }

    /// Brightness in [0, 1] that scales every color.
    fn level(&self) -> f32 {
        let t = (self.phase_started.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.0);
        match self.phase {
            Phase::FadingIn => t,
            Phase::Running => 1.0,
            Phase::FadingOut => self.stop_level * (1.0 - t),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, rect: Rect) -> ToyStatus {
        if any_key_pressed(ui.ctx()) {
            self.stop();
        }
        if self.phase == Phase::FadingIn && self.phase_started.elapsed() >= FADE {
            self.phase = Phase::Running;
        }
        if self.phase == Phase::FadingOut && self.level() <= 0.0 {
            return ToyStatus::Done;
        }
        let font = theme::mono();
        let (glyph_width, row_height) = ui.fonts_mut(|fonts| (fonts.glyph_width(&font, 'x'), fonts.row_height(&font)));
        let (cols, rows) = ((rect.width() / glyph_width) as u16, (rect.height() / row_height) as u16);
        if cols == 0 || rows == 0 {
            return ToyStatus::Running;
        }
        if (cols, rows) != (self.cols, self.rows) {
            self.resize(cols, rows);
        }
        // Capped so a long pause does not jump the streams.
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32().min(0.2);
        self.last_tick = now;
        let stopping = self.phase == Phase::FadingOut;
        for stream in &mut self.streams {
            stream.step(dt, rows, stopping, &mut self.rng);
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::BLACK);
        let level = self.level();
        let scale = |value: f32| (value * level) as u8;
        for (col, stream) in self.streams.iter().enumerate() {
            for row in 0..rows {
                let distance = stream.head.floor() - row as f32;
                if distance < 0.0 || distance >= stream.length as f32 {
                    continue;
                }
                // Letters flicker: each drawn cell picks a new glyph.
                let glyph = self.glyphs[(next_unit(&mut self.rng) * self.glyphs.len() as f32) as usize % self.glyphs.len()];
                let color = if distance < 1.0 {
                    Color32::from_rgb(scale(255.0), scale(255.0), scale(255.0))
                } else {
                    Color32::from_rgb(0, scale(60.0 + 195.0 * (1.0 - distance / stream.length as f32)), 0)
                };
                let pos = rect.min + egui::vec2(col as f32 * glyph_width, row as f32 * row_height);
                painter.text(pos, Align2::LEFT_TOP, glyph, font.clone(), color);
            }
        }
        ToyStatus::Running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_moves_down_with_time() {
        let mut rng = 1;
        let mut stream = Stream { head: 0.0, speed: 10.0, length: 5 };
        stream.step(0.5, 20, false, &mut rng);
        assert_eq!(stream.head, 5.0);
    }

    #[test]
    fn a_stream_past_the_bottom_restarts_above_or_ends() {
        let mut rng = 7;
        let past = Stream { head: 40.0, speed: 10.0, length: 5 };
        let mut again = past;
        again.step(0.1, 20, false, &mut rng);
        assert!(again.head <= 0.0 && again.length >= 4);
        let mut ended = past;
        ended.step(0.1, 20, true, &mut rng);
        assert_eq!(ended.length, 0);
    }
}
