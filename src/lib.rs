//! minerve-gui as a library: `MinerveView` is minerve's chat, drawn in any egui `Ui`.
//! It starts a `minerve --serve` (see `Launcher`) and talks to it over a Unix socket (see
//! `protocol` and `backend`). The look follows the TUI (see `theme`).
//! Nothing is shared with minerve's source: the boot clip and the tips arrive in the `intro` event.
//!
//! ```ignore
//! let mut view = minerve_gui::MinerveView::new(ctx, minerve_gui::Options::default());
//! view.show(ui);
//! ```

mod backend;
mod boot;
mod bottom_bar;
mod boot_view;
mod chat_view;
mod clipboard_image;
mod input_box;
mod modal_apps;
mod modals;
mod protocol;
mod screenshot;
mod shell;
mod slash;
mod tips;
mod toys;
pub mod theme;
mod view;

pub use backend::{Launcher, Link};
pub use theme::Colors;
pub use view::{MinerveView, Options};
