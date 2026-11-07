//! Reads an image off the system clipboard into `~/.minerve/images`, where minerve reads attached files.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// `Ok(None)` when the clipboard holds no image.
pub fn save_clipboard_image() -> Result<Option<PathBuf>, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|err| format!("cannot open the clipboard: {err}"))?;
    let data = match clipboard.get_image() {
        Ok(data) => data,
        Err(arboard::Error::ContentNotAvailable) => return Ok(None),
        Err(err) => return Err(format!("cannot read the clipboard image: {err}")),
    };
    let dir = dirs::home_dir().ok_or("no home directory")?.join(".minerve").join("images");
    std::fs::create_dir_all(&dir).map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = dir.join(format!("gui-{nanos}.png"));
    let image = image::RgbaImage::from_raw(data.width as u32, data.height as u32, data.bytes.into_owned())
        .ok_or("the clipboard image has an unexpected size")?;
    // Same cap as minerve's own downscale, so big screenshots stay cheap to send.
    let image = image::DynamicImage::ImageRgba8(image);
    let image = if image.width().max(image.height()) > 1568 { image.resize(1568, 1568, image::imageops::FilterType::Triangle) } else { image };
    image.save(&path).map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    Ok(Some(path))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    /// Puts a real PNG on the clipboard with osascript, then reads it back. Overwrites the clipboard.
    #[test]
    #[ignore = "overwrites the system clipboard"]
    fn reads_a_png_from_the_clipboard() {
        let png = std::env::temp_dir().join("minerve-gui-clip-test.png");
        image::RgbaImage::from_pixel(4, 3, image::Rgba([255, 0, 0, 255])).save(&png).unwrap();
        let script = format!("set the clipboard to (read (POSIX file \"{}\") as «class PNGf»)", png.display());
        assert!(std::process::Command::new("osascript").args(["-e", &script]).status().unwrap().success());
        let saved = super::save_clipboard_image().unwrap().expect("image");
        let back = image::open(&saved).unwrap();
        assert_eq!((back.width(), back.height()), (4, 3));
        std::fs::remove_file(saved).ok();
    }
}
