//! `/matrix` and `/tetris`: small toys that replace the chat area until the person leaves.
//! They live in this window only; minerve knows nothing about them.

mod matrix;
mod tetris;

use eframe::egui;

use matrix::MatrixRain;
use tetris::Tetris;

#[derive(PartialEq, Eq, Debug)]
pub enum ToyStatus {
    Running,
    Done,
}

pub enum Toy {
    Matrix(MatrixRain),
    Tetris(Box<Tetris>),
}

impl Toy {
    pub fn matrix() -> Self {
        Toy::Matrix(MatrixRain::new())
    }

    pub fn tetris() -> Self {
        Toy::Tetris(Box::new(Tetris::new()))
    }

    /// Reads its keys and paints over `rect`.
    pub fn show(&mut self, ui: &mut egui::Ui, rect: egui::Rect) -> ToyStatus {
        let status = match self {
            Toy::Matrix(rain) => rain.show(ui, rect),
            Toy::Tetris(game) => game.show(ui, rect),
        };
        if status == ToyStatus::Running {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
        }
        status
    }
}

/// Xorshift64 step, returning a float in [0, 1). Avoids a crate.
pub(crate) fn next_unit(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 40) as f32 / (1u64 << 24) as f32
}

pub(crate) fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
        | 1
}

/// True when any key went down this frame.
pub(crate) fn any_key_pressed(ctx: &egui::Context) -> bool {
    ctx.input(|input| input.events.iter().any(|event| matches!(event, egui::Event::Key { pressed: true, .. })))
}

/// Runs one egui frame of `toy` with these keys pressed, the way the window would.
#[cfg(test)]
pub(crate) fn run_frame(ctx: &egui::Context, toy: &mut Toy, keys: &[egui::Key]) -> ToyStatus {
    let events = keys
        .iter()
        .map(|key| egui::Event::Key { key: *key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE })
        .collect();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 720.0))),
        events,
        ..Default::default()
    };
    let mut status = ToyStatus::Running;
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = ui.max_rect();
            status = toy.show(ui, rect);
        });
    });
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tetris_answers_keys_and_esc_leaves() {
        let ctx = egui::Context::default();
        let mut toy = Toy::tetris();
        let Toy::Tetris(game) = &toy else { unreachable!() };
        let before = game.piece_x();
        assert_eq!(run_frame(&ctx, &mut toy, &[egui::Key::ArrowLeft]), ToyStatus::Running);
        let Toy::Tetris(game) = &toy else { unreachable!() };
        assert_eq!(game.piece_x(), before - 1);
        assert_eq!(run_frame(&ctx, &mut toy, &[egui::Key::ArrowRight]), ToyStatus::Running);
        let Toy::Tetris(game) = &toy else { unreachable!() };
        assert_eq!(game.piece_x(), before);
        assert_eq!(run_frame(&ctx, &mut toy, &[egui::Key::Space]), ToyStatus::Running);
        assert_eq!(run_frame(&ctx, &mut toy, &[egui::Key::Escape]), ToyStatus::Done);
    }

    #[test]
    fn matrix_runs_until_a_key_then_fades_out_and_leaves() {
        let ctx = egui::Context::default();
        let mut toy = Toy::matrix();
        assert_eq!(run_frame(&ctx, &mut toy, &[]), ToyStatus::Running);
        assert_eq!(run_frame(&ctx, &mut toy, &[egui::Key::A]), ToyStatus::Running, "fades out first");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if run_frame(&ctx, &mut toy, &[]) == ToyStatus::Done {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "matrix never ended after a key");
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }
}
