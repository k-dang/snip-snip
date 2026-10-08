use std::sync::Arc;

use anyhow::{Context as _, Result};
use gpui::{App, Bounds, DisplayId, Pixels, RenderImage, point, px, size};
use snip_core::RgbaImage;

/// One monitor's frozen screenshot, paired with the GPUI display it came from.
pub struct MonitorCapture {
    pub display_id: DisplayId,
    /// The display's bounds in GPUI's logical coordinates; the overlay window
    /// is opened at exactly these bounds.
    pub bounds: Bounds<Pixels>,
    pub image: RgbaImage,
}

/// Captures every monitor. Monitors that can't be matched to a GPUI display
/// are skipped (and logged) rather than failing the whole snip.
pub fn capture_all(cx: &App) -> Result<Vec<MonitorCapture>> {
    let displays = cx.displays();
    let mut captures = Vec::new();
    for monitor in xcap::Monitor::all().context("listing monitors")? {
        let logical = logical_bounds(&monitor)?;
        let Some(display) = displays.iter().find(|d| approx_eq(d.bounds(), logical)) else {
            eprintln!("no GPUI display matches monitor at {logical:?}; skipping");
            continue;
        };
        captures.push(MonitorCapture {
            display_id: display.id(),
            bounds: display.bounds(),
            image: monitor.capture_image().context("capturing monitor")?,
        });
    }
    Ok(captures)
}

/// A monitor's bounds in GPUI's coordinate space.
///
/// xcap reports physical pixels on Windows and points on macOS. GPUI's
/// Windows displays divide each monitor's physical rect by *that monitor's*
/// scale factor, so with mixed DPI the logical origins are not a single
/// continuous space. Matching that formula exactly is what lets us pair them.
fn logical_bounds(monitor: &xcap::Monitor) -> Result<Bounds<Pixels>> {
    let scale = if cfg!(target_os = "windows") {
        monitor.scale_factor()?
    } else {
        1.0
    };
    Ok(Bounds {
        origin: point(px(monitor.x()? as f32 / scale), px(monitor.y()? as f32 / scale)),
        size: size(
            px(monitor.width()? as f32 / scale),
            px(monitor.height()? as f32 / scale),
        ),
    })
}

fn approx_eq(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    let close = |a: Pixels, b: Pixels| (f32::from(a) - f32::from(b)).abs() <= 1.0;
    close(a.origin.x, b.origin.x)
        && close(a.origin.y, b.origin.y)
        && close(a.size.width, b.size.width)
        && close(a.size.height, b.size.height)
}

/// Converts an opaque RGBA image into the BGRA texture GPUI paints.
pub fn to_render_image(image: &RgbaImage) -> Arc<RenderImage> {
    let mut bgra = image.clone();
    for pixel in bgra.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![image::Frame::new(bgra)]))
}
