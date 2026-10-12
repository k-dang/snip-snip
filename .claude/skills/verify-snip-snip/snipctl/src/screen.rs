//! Finding snip-snip's windows, mapping image pixels to the screen, and the
//! pixel evidence: screenshots, the clipboard, and color counts.

use std::path::Path;

use anyhow::{Context as _, Result, bail};
use image::RgbaImage;

use crate::{platform, run::Run};

pub const EDITOR_TITLE: &str = "snip-snip";
pub const OVERLAY_TITLE: &str = "snip-snip capture";

/// Colors painted by the fixture, plus the editor's ink; `inspect` counts them.
pub const COLORS: [(&str, [u8; 3]); 3] = [
    ("blue", [0x2E, 0x86, 0xDE]),
    ("green", [0x20, 0xBF, 0x6B]),
    ("ink", [0xE5, 0x48, 0x4D]),
];

/// A rectangle in screen units.
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub fn windows_of(pid: u32) -> Result<Vec<xcap::Window>> {
    Ok(xcap::Window::all()?
        .into_iter()
        .filter(|w| w.pid().is_ok_and(|p| p == pid))
        .collect())
}

pub fn titled(pid: u32, title: &str) -> Result<Option<xcap::Window>> {
    Ok(windows_of(pid)?
        .into_iter()
        .find(|w| w.title().is_ok_and(|t| t == title)))
}

pub fn editor(pid: u32) -> Result<xcap::Window> {
    titled(pid, EDITOR_TITLE)?.context("no editor window is open")
}

/// Where the editor paints the image in Fit. Mirrors crates/snip-gpui/src/editor.rs:
/// 16 px canvas padding below the toolbar, the image centered and never more
/// than one image pixel per device pixel. All lengths there are logical px.
/// The toolbar wraps on narrow windows, so its height is measured instead.
pub struct Layout {
    pub origin: (f32, f32),
    /// Screen units per image pixel.
    pub fit: f32,
    pub image: (u32, u32),
}

impl Layout {
    pub fn of(run: &Run, window: &xcap::Window) -> Result<Self> {
        let scale = window.current_monitor()?.scale_factor()?;
        let (units_per_logical, _) = platform::units(scale);
        let content = platform::content_rect(window)?;
        let size = std::fs::read_to_string(run.path("image.size")).context("no snip recorded")?;
        let (w, h) = size.trim().split_once('x').context("bad image.size")?;
        let image: (u32, u32) = (w.parse()?, h.parse()?);

        let toolbar = toolbar_bottom(window)? - content.y;
        let canvas = Rect {
            x: content.x,
            y: content.y + toolbar,
            w: content.w,
            h: content.h - toolbar,
        };
        let logical_fit = ((canvas.w / units_per_logical - 32.0) / image.0 as f32)
            .min((canvas.h / units_per_logical - 32.0) / image.1 as f32)
            .min(1.0 / scale);
        let fit = logical_fit * units_per_logical;
        let origin = (
            canvas.x + (canvas.w - image.0 as f32 * fit) / 2.0,
            canvas.y + (canvas.h - image.1 as f32 * fit) / 2.0,
        );
        Ok(Self { origin, fit, image })
    }

    pub fn to_screen(&self, (x, y): (f32, f32)) -> (i32, i32) {
        (
            (self.origin.0 + x * self.fit).round() as i32,
            (self.origin.1 + y * self.fit).round() as i32,
        )
    }
}

/// The toolbar's bottom border color (`0x333336` in the editor).
const TOOLBAR_BORDER: [u8; 3] = [0x33, 0x33, 0x36];

/// The screen y just below the toolbar: the first window row that is mostly
/// the toolbar's 1 px bottom border.
fn toolbar_bottom(window: &xcap::Window) -> Result<f32> {
    #[cfg(windows)]
    {
        // Window captures can omit invisible frame pixels even when xcap's
        // reported bounds include them. Scan actual screen pixels instead.
        let monitor = window.current_monitor()?;
        let capture = monitor.capture_image()?;
        let content = platform::content_rect(window)?;
        let left = (content.x - monitor.x()? as f32).max(0.) as u32;
        let top = (content.y - monitor.y()? as f32).max(0.) as u32;
        let right = (left + content.w as u32).min(capture.width());
        let bottom = (top + content.h as u32).min(capture.height());
        let row = (top..bottom)
            .find(|&y| {
                let border = (left..right)
                    .filter(|&x| capture.get_pixel(x, y).0[..3] == TOOLBAR_BORDER)
                    .count();
                border as f32 >= 0.8 * (right - left) as f32
            })
            .context("no toolbar border in the editor screen pixels")?;
        Ok(monitor.y()? as f32 + (row + 1) as f32)
    }
    #[cfg(not(windows))]
    {
        let capture = window.capture_image()?;
        let per_unit = capture.width() as f32 / window.width()? as f32;
        let row = (0..capture.height())
            .find(|&y| {
                let border = (0..capture.width())
                    .filter(|&x| capture.get_pixel(x, y).0[..3] == TOOLBAR_BORDER)
                    .count();
                border as f32 >= 0.8 * capture.width() as f32
            })
            .context("no toolbar border in the editor window")?;
        Ok(window.y()? as f32 + (row + 1) as f32 / per_unit)
    }
}

/// Screenshots the editor window, or the primary monitor when none is open.
pub fn shot(pid: u32, path: &Path) -> Result<()> {
    let image = match titled(pid, EDITOR_TITLE)? {
        Some(window) => window.capture_image()?,
        None => {
            let monitors = xcap::Monitor::all()?;
            let primary = monitors.into_iter().find(|m| m.is_primary().unwrap_or(false));
            primary.context("no primary monitor")?.capture_image()?
        }
    };
    image.save(path)?;
    Ok(())
}

/// Writes the clipboard's image to `path` as PNG.
pub fn save_clipboard(path: &Path) -> Result<()> {
    let data = arboard::Clipboard::new()?
        .get_image()
        .context("clipboard has no image")?;
    let image = RgbaImage::from_raw(data.width as u32, data.height as u32, data.bytes.into_owned())
        .context("bad clipboard image")?;
    image.save(path)?;
    Ok(())
}

/// `<path>  <w>x<h>  blue=… green=… ink=…`, counting exact RGB matches.
pub fn inspect(path: &Path) -> Result<String> {
    if !path.exists() {
        bail!("{} does not exist", path.display());
    }
    let image = image::open(path)?.to_rgb8();
    let counts = COLORS.map(|(name, rgb)| (name, image.pixels().filter(|p| p.0 == rgb).count()));
    let counts = counts.map(|(name, n)| format!("{name}={n}")).join(" ");
    Ok(format!(
        "{}  {}x{}  {counts}",
        path.display(),
        image.width(),
        image.height()
    ))
}
