//! `--screenshot <file.png>`: the window takes its own picture, saves it and exits.
//! The window is created hidden and unfocused (see `main`), so a test run never steals focus.
//! `--screenshot-after-ms <n>` waits before the picture (default 1500, mid-boot clip).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

pub struct Screenshot {
    path: PathBuf,
    at: Instant,
    requested: bool,
}

impl Screenshot {
    pub fn new(path: PathBuf, after: Duration) -> Self {
        Self { path, at: Instant::now() + after, requested: false }
    }

    /// Asks for the picture once the wait is over, saves it when it arrives, then exits.
    pub fn tick(&mut self, ctx: &egui::Context) {
        if !self.requested {
            if Instant::now() < self.at {
                ctx.request_repaint_after(Duration::from_millis(50));
                return;
            }
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let image = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        match image {
            Some(image) => {
                let saved = image::save_buffer(
                    &self.path,
                    image.as_raw(),
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                );
                match saved {
                    Ok(()) => println!("{}", self.path.display()),
                    Err(err) => {
                        eprintln!("minerve-gui: cannot save {}: {err}", self.path.display());
                        std::process::exit(1);
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            None => ctx.request_repaint_after(Duration::from_millis(50)),
        }
    }
}
