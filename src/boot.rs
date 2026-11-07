//! The boot clip, drawn from the `intro` event minerve sends (frames, fps, caption).
//! Frame index follows elapsed time, so a slow redraw skips frames instead of slowing the clip.
//! While the clip runs a dotted ring surrounds it; when it ends the dots erode away and a dash
//! sweep erases the caption.

use std::time::{Duration, Instant};

/// How long the dotted ring takes to fully disappear after the clip stops.
const ERODE: Duration = Duration::from_millis(900);
/// Redraw interval while the ring erodes (no frame boundaries to wait for).
const ERODE_TICK: Duration = Duration::from_millis(33);

/// What a boot cell is, so the view can pick a color for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootShade {
    Dot,
    Video,
    /// The caption, or a blank that erases it.
    Caption,
    /// The dashes sweeping in on the caption.
    Dash,
}

/// One character to paint, `x` columns and `y` rows from the top left of the boot area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootCell {
    pub x: i32,
    pub y: i32,
    pub ch: char,
    pub shade: BootShade,
}

pub struct BootClip {
    caption: String,
    frames: Vec<String>,
    frame_micros: u64,
    /// Width and height in cells of the first frame (all frames share one size).
    size: (u16, u16),
    started: Instant,
}

impl BootClip {
    /// `None` when there are no frames or `fps` is zero.
    pub fn new(fps: u32, caption: String, frames: Vec<String>) -> Option<Self> {
        let first = frames.first()?;
        if fps == 0 {
            return None;
        }
        let width = first.lines().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
        let height = first.lines().count() as u16;
        if width == 0 || height == 0 {
            return None;
        }
        Some(Self {
            caption,
            frames,
            frame_micros: (1_000_000 / u64::from(fps)).max(1),
            size: (width, height),
            started: Instant::now(),
        })
    }

    fn frame_index(&self) -> usize {
        (self.started.elapsed().as_micros() / self.frame_micros as u128) as usize
    }

    /// When the clip ended: the end of the last frame.
    fn ended_at(&self) -> Option<Instant> {
        let natural_end = self.started + Duration::from_micros(self.frames.len() as u64 * self.frame_micros);
        (Instant::now() >= natural_end).then_some(natural_end)
    }

    pub fn is_playing(&self) -> bool {
        self.ended_at().is_none()
    }

    fn current_frame(&self) -> Option<&str> {
        if !self.is_playing() {
            return None;
        }
        self.frames.get(self.frame_index()).map(String::as_str)
    }

    /// Erosion progress of the ring after the clip has ended: 0.0 just after, 1.0 when gone.
    fn erosion(&self) -> Option<f32> {
        let ended = self.ended_at()?;
        Some((ended.elapsed().as_secs_f32() / ERODE.as_secs_f32()).min(1.0))
    }

    /// Progress of the dash sweep. It starts at the midpoint of the clip and ends together with the erosion.
    fn sweep_progress(&self) -> Option<f32> {
        let total = self.frames.len() as u64 * self.frame_micros;
        let start = self.started + Duration::from_micros(total / 2);
        let end = self.started + Duration::from_micros(total) + ERODE;
        let now = Instant::now();
        if now < start {
            return None;
        }
        Some(((now - start).as_secs_f32() / (end - start).as_secs_f32()).min(1.0))
    }

    /// True once the sweep has completed: the boot screen can be dropped.
    pub fn is_finished(&self) -> bool {
        self.sweep_progress().is_some_and(|p| p >= 1.0)
    }

    /// How long until the next redraw is needed.
    pub fn time_to_next_frame(&self) -> Duration {
        if !self.is_playing() {
            return ERODE_TICK;
        }
        let next = (self.frame_index() as u64 + 1) * self.frame_micros;
        let elapsed = self.started.elapsed().as_micros() as u64;
        Duration::from_micros(next.saturating_sub(elapsed))
    }

    /// The cells to paint now in an area of `cols` x `rows`, in paint order (a later cell covers an
    /// earlier one at the same place). A blank character is a cell that erases.
    pub fn cells(&self, cols: i32, rows: i32) -> Vec<BootCell> {
        let erosion = self.erosion();
        let sweep = self.sweep_progress();
        let text = self.current_frame();
        let (width, height) = self.size;
        let mut cells = Vec::new();

        // Fit the video into the area, keeping its aspect ratio. Reserve 2 columns on each side
        // for the ring, and 4 rows for the ring (1 above and below) plus the caption block
        // (1 blank row, then the caption). Work in i32 so the ring can extend past the area.
        let (aw, ah) = (cols, rows);
        let (sw, sh) = (width as i32, height as i32);
        // 0.8 shrinks the fitted video by 20%, leaving more of the dot pattern visible around it.
        let scale = 0.8 * ((aw - 4).max(1) as f32 / sw as f32).min((ah - 4).max(1) as f32 / sh as f32);
        let w = ((sw as f32 * scale) as i32).max(1);
        let h = ((sh as f32 * scale) as i32).max(1);
        let vx = (aw - w) / 2;
        let vy = ((ah - h - 4).max(0)) / 2 + 1;
        let in_area = |x: i32, y: i32| x >= 0 && x < aw && y >= 0 && y < ah;

        // Distance from the video border, in cells. Vertical distance is doubled because a
        // terminal cell is about twice as tall as it is wide. Cells closer than EDGE_BLEND to the
        // border are the blend band.
        const EDGE_BLEND: i32 = 7;
        let edge_distance = |c: i32, r: i32| (c.min(w - 1 - c)).min(2 * r.min(h - 1 - r));
        // Stable pseudo-random value in [0, 1) per cell.
        let cell_hash = |x: i32, y: i32| {
            let h = ((x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663)) % 1000;
            h as f32 / 1000.0
        };

        // Sparse dot grid: one dot every 3 columns and every 2 rows, across the whole area, including
        // behind the video. The video's blank cells show these dots, so the pattern runs seamlessly
        // into the video. Erosion hides dots unevenly.
        for y in 0..ah {
            for x in 0..aw {
                if x % 3 != 0 || y % 2 != 0 {
                    continue;
                }
                if erosion.is_none_or(|e| cell_hash(x, y) >= e) {
                    cells.push(BootCell { x, y, ch: '\u{b7}', shade: BootShade::Dot });
                }
            }
        }

        // Video frame, centered inside the ring. Blank characters are transparent.
        if let Some(text) = text {
            // Nearest-neighbour resample of the source characters to the fitted size.
            let src: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
            for r in 0..h {
                let sr = (r * sh / h) as usize;
                for c in 0..w {
                    let sc = (c * sw / w) as usize;
                    let ch = src.get(sr).and_then(|row| row.get(sc)).copied().unwrap_or(' ');
                    let (x, y) = (vx + c, vy + r);
                    // Video cells in the blend band show with a chance rising from the border
                    // inward, so the edge dissolves into the dots.
                    let d = edge_distance(c, r);
                    let show = d >= EDGE_BLEND || cell_hash(x, y) < (d as f32 + 0.5) / EDGE_BLEND as f32;
                    if show && ch != ' ' && in_area(x, y) {
                        cells.push(BootCell { x, y, ch, shade: BootShade::Video });
                    }
                }
            }
        }

        // Caption centered under the video, one blank line below the ring. It stays for the
        // whole boot screen (also after the video stops) until the dash sweep erases it.
        let caption_y = vy + h + 2;
        let caption_len = self.caption.chars().count() as i32;
        let cap_l = vx + (w - caption_len) / 2;
        let cap_r = cap_l + caption_len - 1;
        for (col, ch) in self.caption.chars().enumerate() {
            let x = cap_l + col as i32;
            if in_area(x, caption_y) {
                cells.push(BootCell { x, y: caption_y, ch, shade: BootShade::Caption });
            }
        }

        // Dashes frame the caption from the start. After the dots erode, they move in along the
        // caption row toward its center. Every caption cell they pass is blanked; they vanish at
        // the center.
        let dash_l = cap_l - 2;
        let dash_r = cap_r + 2;
        let mid = (cap_l + cap_r) / 2;
        let p = sweep.unwrap_or(0.0);
        let lf = dash_l + ((mid - dash_l) as f32 * p) as i32;
        let rf = dash_r - ((dash_r - mid) as f32 * p) as i32;
        if sweep.is_some() {
            for x in (dash_l..lf).chain(rf + 1..=dash_r) {
                if in_area(x, caption_y) {
                    cells.push(BootCell { x, y: caption_y, ch: ' ', shade: BootShade::Caption });
                }
            }
        }
        for x in [lf, rf] {
            if in_area(x, caption_y) {
                cells.push(BootCell { x, y: caption_y, ch: '-', shade: BootShade::Dash });
            }
        }
        cells
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip() -> BootClip {
        let frame = "  ####  \n ###### \n  ####  ".to_string();
        BootClip::new(24, "loading ancient knowledge".into(), vec![frame.clone(), frame]).unwrap()
    }

    #[test]
    fn cells_stay_inside_the_area_and_show_the_caption() {
        let boot = clip();
        let cells = boot.cells(80, 30);
        assert!(!cells.is_empty());
        assert!(cells.iter().all(|c| (0..80).contains(&c.x) && (0..30).contains(&c.y)));
        let caption_row = cells.iter().find(|c| c.shade == BootShade::Caption).unwrap().y;
        let mut caption: Vec<&BootCell> =
            cells.iter().filter(|c| c.shade == BootShade::Caption && c.y == caption_row).collect();
        caption.sort_by_key(|c| c.x);
        let text: String = caption.iter().map(|c| c.ch).collect();
        assert_eq!(text, "loading ancient knowledge");
    }

    #[test]
    fn no_frames_or_no_fps_means_no_clip() {
        assert!(BootClip::new(24, String::new(), vec![]).is_none());
        assert!(BootClip::new(0, String::new(), vec!["x".into()]).is_none());
    }
}
