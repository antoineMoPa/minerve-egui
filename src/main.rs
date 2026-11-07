//! minerve-gui: a window for minerve. The view itself is the library (`src/lib.rs`).

use std::path::PathBuf;

use eframe::egui;
use minerve_gui::{theme, MinerveView, Options};

fn parse_args() -> Options {
    let mut args = Options { screenshot_after: std::time::Duration::from_millis(1500), ..Default::default() };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("minerve-gui {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--help" | "-h" => {
                println!("minerve-gui [--resume <session id>] [--minerve <path>] [--submit <text>] [--prefill <text>] [--screenshot <png> [--screenshot-after-ms <n>]]");
                std::process::exit(0);
            }
            "--resume" => args.resume = iter.next(),
            "--minerve" => args.launcher.program = iter.next().map(PathBuf::from),
            "--submit" => args.submit = iter.next(),
            "--prefill" => args.prefill = iter.next(),
            "--screenshot" => args.screenshot = iter.next().map(PathBuf::from),
            "--screenshot-after-ms" => {
                let ms = iter.next().and_then(|value| value.parse().ok());
                args.screenshot_after = std::time::Duration::from_millis(ms.unwrap_or_else(|| {
                    eprintln!("minerve-gui: --screenshot-after-ms needs a number");
                    std::process::exit(2);
                }));
            }
            other => {
                eprintln!("minerve-gui: unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    args
}

fn main() -> eframe::Result {
    let args = parse_args();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("minerve")
        .with_inner_size([1000.0, 720.0])
        .with_min_inner_size([520.0, 360.0]);
    if args.screenshot.is_some() {
        // Headless: never shown, never focused.
        viewport = viewport.with_visible(false).with_active(false);
    }
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if args.screenshot.is_some() {
        // Without this macOS makes the process the active app, which takes the keyboard from
        // whatever the user is typing in. An accessory app has no dock icon and is not activated.
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_activation_policy(ActivationPolicy::Accessory);
            builder.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native(
        "minerve",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(Window(MinerveView::new(&cc.egui_ctx, args))))
        }),
    )
}

/// The standalone window: one view filling it.
struct Window(MinerveView);

impl eframe::App for Window {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::BACKGROUND.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.show(ui);
    }
}
