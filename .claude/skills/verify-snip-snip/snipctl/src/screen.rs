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

/// The actual painted image transform, reported only in verification runs.
/// Native window geometry supplies the screen origin; no toolbar/theme assumptions.
pub struct Layout {
    pub origin: (f32, f32),
    /// Screen units per image pixel, including zoom.
    pub scale: f32,
    pub image: (u32, u32),
}

struct PaintedView {
    image: (u32, u32),
    origin: (f32, f32),
    scale: f32,
    device_scale: f32,
    content: (f32, f32),
}

impl PaintedView {
    fn from_log(log: &str) -> Result<Self> {
        // A concurrent stderr write may leave the last line incomplete.
        let complete = log.rsplit_once('\n').map_or("", |(complete, _)| complete);
        let line = complete
            .lines()
            .rev()
            .find_map(|line| line.strip_prefix("snipctl-view: "))
            .context("no painted view reported; rebuild the app and launch with snipctl")?;
        let values = line.split_whitespace().collect::<Vec<_>>();
        anyhow::ensure!(
            values.len() == 8,
            "incomplete painted view report; retry after the frame settles"
        );
        let image = (values[0].parse()?, values[1].parse()?);
        let numbers = values[2..]
            .iter()
            .map(|v| v.parse::<f32>())
            .collect::<Result<Vec<_>, _>>()?;
        anyhow::ensure!(
            image.0 > 0
                && image.1 > 0
                && numbers.iter().all(|v| v.is_finite())
                && numbers[2..].iter().all(|v| *v > 0.),
            "invalid painted view report"
        );
        Ok(Self {
            image,
            origin: (numbers[0], numbers[1]),
            scale: numbers[2],
            device_scale: numbers[3],
            content: (numbers[4], numbers[5]),
        })
    }
}

impl Layout {
    pub fn of(run: &Run, window: &xcap::Window) -> Result<Self> {
        let view = PaintedView::from_log(&run.app_log())?;
        let units_per_logical = platform::screen_units_per_logical(view.device_scale);
        let content = platform::content_rect(window, view.content)?;
        anyhow::ensure!(
            (content.w - view.content.0 * units_per_logical).abs() <= 2.
                && (content.h - view.content.1 * units_per_logical).abs() <= 2.,
            "window resized before its painted view settled; retry"
        );
        Ok(Self {
            origin: (
                content.x + view.origin.0 * units_per_logical,
                content.y + view.origin.1 * units_per_logical,
            ),
            scale: view.scale * units_per_logical,
            image: view.image,
        })
    }

    pub fn to_screen(&self, (x, y): (f32, f32)) -> (i32, i32) {
        (
            (self.origin.0 + x * self.scale).round() as i32,
            (self.origin.1 + y * self.scale).round() as i32,
        )
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

/// Compare decoded RGBA pixels, allowing PNG encoding/metadata to differ.
pub fn compare(a: &Path, b: &Path) -> Result<String> {
    let a_image = image::open(a)
        .with_context(|| format!("reading {}", a.display()))?
        .to_rgba8();
    let b_image = image::open(b)
        .with_context(|| format!("reading {}", b.display()))?
        .to_rgba8();
    anyhow::ensure!(
        a_image.dimensions() == b_image.dimensions(),
        "image dimensions differ: {} {:?}, {} {:?}",
        a.display(),
        a_image.dimensions(),
        b.display(),
        b_image.dimensions()
    );
    let different = a_image
        .pixels()
        .zip(b_image.pixels())
        .filter(|(a, b)| a != b)
        .count();
    anyhow::ensure!(
        different == 0,
        "{different} pixels differ: {} vs {}",
        a.display(),
        b.display()
    );
    Ok(format!(
        "identical decoded pixels: {} vs {} ({}x{})",
        a.display(),
        b.display(),
        a_image.width(),
        a_image.height()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn painted_view_uses_latest_complete_report_and_rejects_invalid_geometry() {
        let view = PaintedView::from_log(
            "snipctl-view: 600 300 16 60 1 1 900 600\nsnipctl-view: 1199 599 -40 80 0.75 2 800 500\nsnipctl-view: 1199",
        )
        .unwrap();
        assert_eq!(view.image, (1199, 599));
        assert_eq!(view.content, (800., 500.));
        assert_eq!(view.origin, (-40., 80.));
        assert_eq!(view.scale, 0.75);
        assert_eq!(view.device_scale, 2.);
        for log in [
            "",
            "snipctl-view: 600 300\n",
            "snipctl-view: 600 300 0 0 NaN 2 800 500\n",
            "snipctl-view: 0 300 0 0 1 1 800 500\n",
        ] {
            assert!(PaintedView::from_log(log).is_err());
        }
    }

    #[test]
    fn comparison_checks_pixels_dimensions_and_alpha_across_image_formats() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("rgb.png");
        let b = dir.path().join("rgba.png");
        image::RgbImage::from_pixel(2, 2, image::Rgb([20, 30, 40]))
            .save(&a)
            .unwrap();
        let mut rgba = RgbaImage::from_pixel(2, 2, image::Rgba([20, 30, 40, 255]));
        rgba.save(&b).unwrap();
        compare(&a, &b).unwrap();
        rgba.put_pixel(1, 1, image::Rgba([20, 30, 40, 254]));
        rgba.save(&b).unwrap();
        assert!(
            compare(&a, &b)
                .unwrap_err()
                .to_string()
                .contains("1 pixels differ")
        );
        image::RgbImage::new(3, 2).save(&b).unwrap();
        assert!(
            compare(&a, &b)
                .unwrap_err()
                .to_string()
                .contains("dimensions differ")
        );
        assert!(compare(&a, &dir.path().join("missing.png")).is_err());
    }
}
