//! minerve-gui: a window for minerve. It starts a `minerve --serve` and talks to it over a Unix
//! socket (see `protocol` and `backend`). The look follows the TUI (see `theme`).
//! Nothing is shared with minerve's source: the boot clip and the tips arrive in the `intro` event.


mod app;
mod backend;
mod boot;
mod bottom_bar;
mod boot_view;
mod chat_view;
mod input_box;
mod modal_apps;
mod modals;
mod protocol;
mod screenshot;
mod shell;
mod slash;
mod theme;
mod tips;
mod toys;

use std::path::PathBuf;

pub struct Args {
    pub resume: Option<String>,
    pub minerve: Option<PathBuf>,
    /// `--screenshot <png>`: save a picture of the window and exit, without showing the window.
    pub screenshot: Option<PathBuf>,
    pub screenshot_after: std::time::Duration,
    /// `--submit <text>`: send this prompt as soon as minerve is up (for headless checks).
    pub submit: Option<String>,
    /// `--prefill <text>`: start with this typed in the message box (for headless checks).
    pub prefill: Option<String>,
}

fn parse_args() -> Args {
    let mut args = Args {
        resume: None,
        minerve: None,
        screenshot: None,
        screenshot_after: std::time::Duration::from_millis(1500),
        submit: None,
        prefill: None,
    };
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
            "--minerve" => args.minerve = iter.next().map(PathBuf::from),
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
    eframe::run_native("minerve", options, Box::new(|cc| Ok(Box::new(app::MinerveApp::new(cc, args)))))
}
