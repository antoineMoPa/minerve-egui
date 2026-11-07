//! `/tetris`: a small Tetris game. Pieces are four cells around a pivot; rotation turns the
//! offsets 90 degrees, with a few horizontal kicks so a piece can rotate against a wall.

use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, Key, Rect};

use super::{seed, ToyStatus};
use crate::theme;

const WIDTH: usize = 10;
const HEIGHT: usize = 20;
/// Points for clearing 0 to 4 lines at once, before multiplying by the level.
const LINE_POINTS: [u32; 5] = [0, 100, 300, 500, 800];
const LINES_PER_LEVEL: u32 = 10;
/// Horizontal offsets tried, in order, when a rotation does not fit where it is.
const ROTATION_KICKS: [i32; 5] = [0, -1, 1, -2, 2];
const CELL: f32 = 24.0;

/// Offsets of the four cells from the pivot, for each piece: I, O, T, S, Z, J, L.
const SHAPES: [[(i32, i32); 4]; 7] = [
    [(-1, 0), (0, 0), (1, 0), (2, 0)],
    [(0, 0), (1, 0), (0, 1), (1, 1)],
    [(-1, 0), (0, 0), (1, 0), (0, -1)],
    [(0, 0), (1, 0), (-1, 1), (0, 1)],
    [(-1, 0), (0, 0), (0, 1), (1, 1)],
    [(-1, 0), (0, 0), (1, 0), (-1, 1)],
    [(-1, 0), (0, 0), (1, 0), (1, 1)],
];

const COLORS: [Color32; 7] = [
    Color32::from_rgb(0x3d, 0xc8, 0xd8),
    Color32::from_rgb(0xe0, 0xc8, 0x40),
    Color32::from_rgb(0xd0, 0x6a, 0xd8),
    Color32::from_rgb(0x4e, 0xc9, 0x6a),
    Color32::from_rgb(0xe0, 0x50, 0x50),
    Color32::from_rgb(0x50, 0x70, 0xe8),
    Color32::from_rgb(255, 165, 0),
];

#[derive(Clone, Copy)]
struct Piece {
    kind: usize,
    x: i32,
    y: i32,
    offsets: [(i32, i32); 4],
}

impl Piece {
    fn spawn(kind: usize) -> Self {
        Self { kind, x: WIDTH as i32 / 2, y: 1, offsets: SHAPES[kind] }
    }

    fn cells(&self) -> [(i32, i32); 4] {
        self.offsets.map(|(dx, dy)| (self.x + dx, self.y + dy))
    }

    fn rotated(&self) -> Self {
        Self { offsets: self.offsets.map(|(dx, dy)| (-dy, dx)), ..*self }
    }
}

fn next_index(state: &mut u64, n: usize) -> usize {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state % n as u64) as usize
}

pub struct Tetris {
    board: [[Option<usize>; WIDTH]; HEIGHT],
    piece: Piece,
    next: usize,
    bag: Vec<usize>,
    rng: u64,
    score: u32,
    lines: u32,
    last_fall: Instant,
    over: bool,
}

impl Tetris {
    pub fn new() -> Self {
        let mut game = Self {
            board: [[None; WIDTH]; HEIGHT],
            piece: Piece::spawn(0),
            next: 0,
            bag: Vec::new(),
            rng: seed(),
            score: 0,
            lines: 0,
            last_fall: Instant::now(),
            over: false,
        };
        game.piece = Piece::spawn(game.take_from_bag());
        game.next = game.take_from_bag();
        game
    }

    fn take_from_bag(&mut self) -> usize {
        if self.bag.is_empty() {
            self.bag = (0..SHAPES.len()).collect();
            for i in (1..self.bag.len()).rev() {
                let j = next_index(&mut self.rng, i + 1);
                self.bag.swap(i, j);
            }
        }
        self.bag.pop().expect("bag was refilled above")
    }

    fn level(&self) -> u32 {
        1 + self.lines / LINES_PER_LEVEL
    }

    fn fall_interval(&self) -> Duration {
        Duration::from_millis(800u64.saturating_sub(70 * u64::from(self.level() - 1)).max(100))
    }

    fn fits(&self, piece: &Piece) -> bool {
        piece.cells().iter().all(|&(x, y)| {
            (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) && self.board[y as usize][x as usize].is_none()
        })
    }

    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        let moved = Piece { x: self.piece.x + dx, y: self.piece.y + dy, ..self.piece };
        let fits = self.fits(&moved);
        if fits {
            self.piece = moved;
        }
        fits
    }

    fn rotate(&mut self) {
        let turned = self.piece.rotated();
        for kick in ROTATION_KICKS {
            let candidate = Piece { x: turned.x + kick, ..turned };
            if self.fits(&candidate) {
                self.piece = candidate;
                return;
            }
        }
    }

    /// Writes the piece into the board, clears full rows, and spawns the next piece.
    fn lock(&mut self) {
        for (x, y) in self.piece.cells() {
            self.board[y as usize][x as usize] = Some(self.piece.kind);
        }
        let kept: Vec<_> = self.board.iter().copied().filter(|row| row.iter().any(|cell| cell.is_none())).collect();
        let cleared = HEIGHT - kept.len();
        let mut board = [[None; WIDTH]; HEIGHT];
        for (i, row) in kept.into_iter().enumerate() {
            board[cleared + i] = row;
        }
        self.board = board;
        self.score += LINE_POINTS[cleared] * self.level();
        self.lines += cleared as u32;
        self.piece = Piece::spawn(self.next);
        self.next = self.take_from_bag();
        if !self.fits(&self.piece) {
            self.over = true;
        }
    }

    fn hard_drop(&mut self) {
        while self.try_move(0, 1) {
            self.score += 2;
        }
        self.lock();
    }

    /// Gravity: moves the piece down once the fall interval has passed, locking it when it lands.
    fn update(&mut self, now: Instant) {
        if self.over || now.duration_since(self.last_fall) < self.fall_interval() {
            return;
        }
        self.last_fall = now;
        if !self.try_move(0, 1) {
            self.lock();
        }
    }

    /// One key press. Returns false when the player leaves the game.
    fn handle_key(&mut self, key: Key) -> bool {
        match key {
            Key::Escape | Key::Q => return false,
            Key::R if self.over => *self = Tetris::new(),
            _ if self.over => {}
            Key::ArrowLeft => {
                self.try_move(-1, 0);
            }
            Key::ArrowRight => {
                self.try_move(1, 0);
            }
            Key::ArrowDown => {
                if self.try_move(0, 1) {
                    self.score += 1;
                }
            }
            Key::ArrowUp | Key::X => self.rotate(),
            Key::Space => self.hard_drop(),
            _ => {}
        }
        true
    }

    /// Column of the falling piece's pivot, for tests.
    #[cfg(test)]
    pub fn piece_x(&self) -> i32 {
        self.piece.x
    }

    pub fn show(&mut self, ui: &mut egui::Ui, rect: Rect) -> ToyStatus {
        let keys: Vec<Key> = ui.ctx().input(|input| {
            input.events.iter().filter_map(|event| match event {
                egui::Event::Key { key, pressed: true, .. } => Some(*key),
                _ => None,
            }).collect()
        });
        for key in keys {
            if !self.handle_key(key) {
                return ToyStatus::Done;
            }
        }
        self.update(Instant::now());
        self.paint(ui, rect);
        ToyStatus::Running
    }

    fn paint(&self, ui: &egui::Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme::c().background);
        let cell = CELL.min(rect.height() / (HEIGHT as f32 + 2.0)).max(8.0);
        let board_size = egui::vec2(WIDTH as f32 * cell, HEIGHT as f32 * cell);
        let panel_width = cell * 8.0;
        let total = board_size.x + cell * 2.0 + panel_width;
        let origin = rect.center() - egui::vec2(total / 2.0, board_size.y / 2.0);
        let board = Rect::from_min_size(origin, board_size);
        painter.rect_stroke(board.expand(2.0), 0.0, egui::Stroke::new(1.0, theme::c().accent), egui::StrokeKind::Outside);

        let mut grid = self.board;
        for (x, y) in self.piece.cells() {
            grid[y as usize][x as usize] = Some(self.piece.kind);
        }
        for (row, cells) in grid.iter().enumerate() {
            for (col, kind) in cells.iter().enumerate() {
                let min = board.min + egui::vec2(col as f32 * cell, row as f32 * cell);
                let square = Rect::from_min_size(min, egui::vec2(cell, cell)).shrink(1.0);
                match kind {
                    Some(kind) => painter.rect_filled(square, 0.0, COLORS[*kind]),
                    None => painter.circle_filled(square.center(), 1.0, theme::c().dark_gray),
                };
            }
        }

        let panel = egui::pos2(board.right() + cell * 2.0, board.top());
        let font = theme::mono();
        let line = |index: f32, text: String, color: Color32| {
            painter.text(panel + egui::vec2(0.0, index * 18.0), Align2::LEFT_TOP, text, font.clone(), color);
        };
        line(0.0, "score".into(), theme::c().gray);
        line(1.0, self.score.to_string(), theme::c().bright);
        line(3.0, "lines".into(), theme::c().gray);
        line(4.0, self.lines.to_string(), theme::c().bright);
        line(6.0, "level".into(), theme::c().gray);
        line(7.0, self.level().to_string(), theme::c().bright);
        line(9.0, "next".into(), theme::c().gray);
        for (dx, dy) in SHAPES[self.next] {
            let min = panel + egui::vec2((dx + 1) as f32 * cell, 10.0 * 18.0 + (dy + 1) as f32 * cell);
            painter.rect_filled(Rect::from_min_size(min, egui::vec2(cell, cell)).shrink(1.0), 0.0, COLORS[self.next]);
        }
        let help_top = 10.0 * 18.0 + cell * 3.0;
        for (index, text) in ["←→ move  ↑ turn", "↓ soft  space drop", "esc/q leave"].iter().enumerate() {
            painter.text(panel + egui::vec2(0.0, help_top + index as f32 * 18.0), Align2::LEFT_TOP, *text, font.clone(), theme::c().dark_gray);
        }
        if self.over {
            let top = help_top + 4.0 * 18.0;
            painter.text(panel + egui::vec2(0.0, top), Align2::LEFT_TOP, "game over", font.clone(), theme::c().magenta);
            painter.text(panel + egui::vec2(0.0, top + 18.0), Align2::LEFT_TOP, "r to restart", font, theme::c().gray);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_with(kind: usize) -> Tetris {
        let mut game = Tetris::new();
        game.piece = Piece::spawn(kind);
        game
    }

    #[test]
    fn a_piece_stops_at_the_walls_and_the_floor() {
        let mut game = game_with(1);
        while game.try_move(-1, 0) {}
        assert!(game.piece.cells().iter().all(|&(x, _)| x >= 0));
        assert!(!game.try_move(-1, 0));
        while game.try_move(0, 1) {}
        assert!(game.piece.cells().iter().any(|&(_, y)| y == HEIGHT as i32 - 1));
    }

    #[test]
    fn a_locked_cell_blocks_a_piece() {
        let mut game = game_with(1);
        for (x, y) in game.piece.cells() {
            game.board[y as usize + 1][x as usize] = Some(0);
        }
        assert!(!game.try_move(0, 1));
    }

    #[test]
    fn rotation_kicks_off_a_wall() {
        let mut game = game_with(0);
        game.piece.x = 0;
        game.piece.y = 5;
        // Horizontal I at the left wall does not fit unturned at x = -1; a vertical one does.
        game.rotate();
        assert!(game.fits(&game.piece));
        assert_ne!(game.piece.offsets, SHAPES[0]);
    }

    #[test]
    fn full_rows_clear_and_score_by_level() {
        let mut game = game_with(0);
        // Two nearly full rows: the last has a gap under the flat I piece, the one above keeps a hole.
        for row in HEIGHT - 2..HEIGHT {
            for col in 0..WIDTH {
                game.board[row][col] = Some(1);
            }
        }
        game.board[HEIGHT - 1][3..7].fill(None);
        game.board[HEIGHT - 2][0] = None;
        game.piece = Piece { kind: 0, x: 4, y: HEIGHT as i32 - 1, offsets: SHAPES[0] };
        // The I covers columns 3..=6 of the last row: one row clears.
        game.lock();
        assert_eq!(game.lines, 1);
        assert_eq!(game.score, 100);
        // The row with the hole fell to the bottom.
        assert!(game.board[HEIGHT - 1][0].is_none() && game.board[HEIGHT - 1][1..].iter().all(|cell| cell.is_some()));
    }

    #[test]
    fn hard_drop_scores_two_per_row() {
        let mut game = game_with(1);
        let start = game.piece.y;
        game.hard_drop();
        assert!(game.score >= 2 && game.score % 2 == 0, "start {start}, score {}", game.score);
    }

    #[test]
    fn escape_and_q_leave() {
        let mut game = Tetris::new();
        assert!(!game.handle_key(Key::Escape));
        assert!(!game.handle_key(Key::Q));
        assert!(game.handle_key(Key::ArrowLeft));
    }
}
