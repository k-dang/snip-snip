//! The snip overlay: one borderless, topmost window per monitor showing that
//! monitor's frozen screenshot. Dragging selects a region of the frozen image,
//! so the selection is exactly what was on screen when the shortcut fired.

use std::{sync::Arc, time::Instant};

use gpui::{
    App, Bounds, Context, FocusHandle, Global, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    Pixels, Render, RenderImage, Window, WindowBounds, WindowHandle, WindowKind, WindowOptions, canvas, div,
    fill, hsla, outline, point, prelude::*, px,
};
use snip_core::{Point, Rect};

use crate::{
    capture::{self, MonitorCapture},
    drag, editor,
};

/// The overlay windows of the snip in progress. Present only while snipping,
/// which also makes a second shortcut press a no-op.
struct ActiveSnip {
    windows: Vec<WindowHandle<Overlay>>,
}

impl Global for ActiveSnip {}

/// Captures all monitors and opens an overlay on each.
pub fn start(cx: &mut App) {
    if cx.has_global::<ActiveSnip>() {
        return;
    }
    let started = Instant::now();
    let captures = match capture::capture_all(cx) {
        Ok(captures) if !captures.is_empty() => captures,
        Ok(_) => return eprintln!("snip: no capturable monitors"),
        Err(err) => return eprintln!("snip: capture failed: {err:#}"),
    };
    eprintln!(
        "snip: captured {} monitor(s) in {:?}",
        captures.len(),
        started.elapsed()
    );

    let mut windows = Vec::new();
    for capture in captures {
        let options = WindowOptions {
            window_bounds: Some(overlay_bounds(capture.bounds)),
            titlebar: None,
            focus: true,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            display_id: Some(capture.display_id),
            ..Default::default()
        };
        let opened = cx.open_window(options, |window, cx| {
            // AppKit keeps titled windows (every GPUI window has a title bar
            // style) below the menu bar. Simple fullscreen swaps in a
            // borderless style and sets the frame to the whole screen.
            if cfg!(target_os = "macos") {
                window.toggle_simple_fullscreen();
            }
            window.on_next_frame(move |_, _| {
                eprintln!("snip: overlay first frame at {:?}", started.elapsed());
            });
            cx.new(|cx| Overlay::new(capture, window, cx))
        });
        match opened {
            Ok(handle) => windows.push(handle),
            Err(err) => eprintln!("snip: failed to open overlay: {err:#}"),
        }
    }
    // A window opened from a background app isn't activated, so it gets no
    // keys. One overlay is enough: each one's Esc ends the whole snip.
    if let Some(first) = windows.first() {
        first.update(cx, |_, window, _| window.activate_window()).ok();
    }
    cx.set_global(ActiveSnip { windows });
}

/// On Windows, a titlebar-less GPUI window extends ~8 px past its bounds, so
/// adjacent overlays would steal each other's edge clicks; GPUI's fullscreen is
/// a borderless window exactly covering the monitor. On macOS fullscreen means
/// a new Space with an animation, so a `PopUp` at the display bounds is used
/// and then moved over the menu bar by simple fullscreen.
fn overlay_bounds(display: Bounds<Pixels>) -> WindowBounds {
    if cfg!(windows) {
        WindowBounds::Fullscreen(display)
    } else {
        WindowBounds::Windowed(display)
    }
}

/// Closes every overlay. Deferred so it never runs while one of the overlay
/// windows is mid-update.
fn finish(cx: &mut App) {
    cx.defer(|cx| {
        let Some(snip) = cx.try_global::<ActiveSnip>() else {
            return;
        };
        let windows = snip.windows.clone();
        cx.remove_global::<ActiveSnip>();
        for window in windows {
            window.update(cx, |_, window, _| window.remove_window()).ok();
        }
    });
}

struct Overlay {
    capture: MonitorCapture,
    texture: Arc<RenderImage>,
    /// Drag start and current position, in window coordinates.
    selection: Option<(gpui::Point<Pixels>, gpui::Point<Pixels>)>,
    focus: FocusHandle,
}

impl Overlay {
    fn new(capture: MonitorCapture, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        // An OS close (Alt+F4) of one overlay ends the whole snip, as Esc does.
        window.on_window_should_close(cx, |_, cx| {
            finish(cx);
            false
        });
        Self {
            texture: capture::to_render_image(&capture.image),
            capture,
            selection: None,
            focus,
        }
    }

    /// Screenshot pixels per logical pixel. The window covers the monitor
    /// exactly, so this is a pure ratio, independent of how the OS reports DPI.
    fn pixels_per_point(&self) -> f32 {
        self.capture.image.width() as f32 / f32::from(self.capture.bounds.size.width)
    }

    /// Window coordinates → screenshot pixels.
    fn to_image(&self, p: gpui::Point<Pixels>) -> Point {
        let scale = self.pixels_per_point();
        Point::new(f32::from(p.x) * scale, f32::from(p.y) * scale)
    }

    /// Extends the selection, stopping at the monitor's edges: a selection
    /// can't span monitors, but dragging past an edge reaches it exactly.
    fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, end)) = &mut self.selection {
            let size = window.viewport_size();
            *end = event
                .position
                .max(&point(px(0.), px(0.)))
                .min(&point(size.width, size.height));
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, cx: &mut Context<Self>) {
        let Some((start, end)) = self.selection.take() else {
            return;
        };
        // A click (or a tiny jitter) selects nothing; let the user try again.
        let drag = end - start;
        if f32::from(drag.x).abs() < 2.0 || f32::from(drag.y).abs() < 2.0 {
            return cx.notify();
        }
        // The cursor sits on a pixel; include the pixels under both ends.
        let (a, b) = (self.to_image(start), self.to_image(end));
        let low = Point::new(a.x.min(b.x).floor(), a.y.min(b.y).floor());
        let high = Point::new(a.x.max(b.x).floor() + 1.0, a.y.max(b.y).floor() + 1.0);
        let (width, height) = self.capture.image.dimensions();
        let Some(rect) = Rect::from_corners(low, high, width, height) else {
            return cx.notify();
        };
        let image = image::imageops::crop_imm(&self.capture.image, rect.x, rect.y, rect.width, rect.height)
            .to_image();
        editor::open(image, self.pixels_per_point(), cx);
        finish(cx);
    }
}

impl Render for Overlay {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let texture = self.texture.clone();
        let selection = self
            .selection
            .map(|(a, b)| Bounds::from_corners(a.min(&b), a.max(&b)));

        div()
            .size_full()
            .track_focus(&self.focus)
            .cursor_crosshair()
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    finish(cx);
                }
            }))
            .on_mouse_down(MouseButton::Right, cx.listener(|_, _, _, cx| finish(cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.selection = Some((event.position, event.position));
                    cx.notify();
                }),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        drag::track(window, this, Self::on_mouse_move, |this, _, cx| {
                            this.on_mouse_up(cx)
                        });
                        window
                            .paint_image(bounds, bounds, Default::default(), texture, 0, false)
                            .ok();
                        paint_dim_outside(window, bounds, selection);
                        if let Some(selection) = selection {
                            window.paint_quad(outline(selection, hsla(0., 0., 1., 0.9), Default::default()));
                        }
                    },
                )
                .size_full(),
            )
    }
}

/// Darkens everything except the selection, so the selected region reads as
/// "lit". With no selection the whole screen is dimmed.
fn paint_dim_outside(window: &mut Window, bounds: Bounds<Pixels>, hole: Option<Bounds<Pixels>>) {
    let dim = hsla(0., 0., 0., 0.35);
    let Some(hole) = hole else {
        return window.paint_quad(fill(bounds, dim));
    };
    let (l, t, r, b) = (bounds.left(), bounds.top(), bounds.right(), bounds.bottom());
    let (hl, ht, hr, hb) = (hole.left(), hole.top(), hole.right(), hole.bottom());
    for (a, z) in [
        (point(l, t), point(r, ht)),   // above
        (point(l, hb), point(r, b)),   // below
        (point(l, ht), point(hl, hb)), // left
        (point(hr, ht), point(r, hb)), // right
    ] {
        if z.x > a.x && z.y > a.y {
            window.paint_quad(fill(Bounds::from_corners(a, z), dim));
        }
    }
}
