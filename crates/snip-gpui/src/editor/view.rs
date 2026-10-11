//! The editor's view of the document, independent of its content and history.

use std::cell::Cell;

use gpui::{Bounds, Pixels, point, px};
use snip_core::{Point, Rect};

use super::CANVAS_PADDING as PADDING;

#[derive(Clone, Copy)]
pub(super) enum ViewPreset {
    Fit,
    ActualSize,
}

/// Logical window coordinates per screenshot pixel, shared by painting and input.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Viewport {
    pub origin: gpui::Point<Pixels>,
    pub scale: f32,
}

impl Viewport {
    pub fn to_image(self, p: gpui::Point<Pixels>) -> Point {
        Point::new(
            f32::from(p.x - self.origin.x) / self.scale,
            f32::from(p.y - self.origin.y) / self.scale,
        )
    }

    pub fn to_window(self, p: Point) -> gpui::Point<Pixels> {
        self.origin + point(px(p.x * self.scale), px(p.y * self.scale))
    }

    pub fn rect_to_window(self, r: Rect) -> Bounds<Pixels> {
        Bounds::from_corners(
            self.to_window(Point::new(r.x as f32, r.y as f32)),
            self.to_window(Point::new((r.x + r.width) as f32, (r.y + r.height) as f32)),
        )
    }
}

#[derive(Clone, Copy)]
struct Layout {
    bounds: Bounds<Pixels>,
    image: (u32, u32),
    device_scale: f32,
}

impl Layout {
    fn fit_scale(self) -> f32 {
        ((f32::from(self.bounds.size.width) - 2. * PADDING) / self.image.0 as f32)
            .min((f32::from(self.bounds.size.height) - 2. * PADDING) / self.image.1 as f32)
            .min(1. / self.device_scale)
    }

    fn centered(self, scale: f32, center: Point) -> Viewport {
        Viewport {
            origin: self.bounds.center() - point(px(center.x * scale), px(center.y * scale)),
            scale,
        }
    }

    fn constrain(self, viewport: &mut Viewport) {
        fn axis(origin: Pixels, length: f32, start: Pixels, available: Pixels) -> Pixels {
            if px(length) <= available - px(2. * PADDING) {
                start + (available - px(length)) / 2.
            } else {
                origin.clamp(start + available - px(PADDING + length), start + px(PADDING))
            }
        }
        viewport.origin.x = axis(
            viewport.origin.x,
            self.image.0 as f32 * viewport.scale,
            self.bounds.left(),
            self.bounds.size.width,
        );
        viewport.origin.y = axis(
            viewport.origin.y,
            self.image.1 as f32 * viewport.scale,
            self.bounds.top(),
            self.bounds.size.height,
        );
    }
}

/// A manual view keeps physical magnification and the image point at canvas center.
#[derive(Clone, Copy)]
struct ManualView {
    zoom: f32,
    center: Point,
}

/// The original view and pointer position of a temporary pan gesture.
pub(super) struct Pan {
    grab: gpui::Point<Pixels>,
    viewport: Viewport,
    manual: Option<ManualView>,
}

/// Paint resolves canvas bounds; navigation updates the same transform immediately.
#[derive(Default)]
pub(super) struct CanvasView {
    layout: Cell<Option<(Layout, Viewport)>>,
    manual: Cell<Option<ManualView>>,
    painted: Cell<Option<Viewport>>,
}

impl CanvasView {
    pub fn get(&self) -> Option<Viewport> {
        self.layout.get().map(|(_, viewport)| viewport)
    }

    /// Track the displayed transform separately from navigation's immediate input updates.
    pub fn record_paint(&self, viewport: Viewport) -> bool {
        self.painted.replace(Some(viewport)) != Some(viewport)
    }

    pub fn zoom_label(&self) -> String {
        let percent = self.layout.get().map_or(100., |(layout, viewport)| {
            viewport.scale * layout.device_scale * 100.
        });
        if percent < 1. {
            format!("{percent:.1}%")
        } else {
            format!("{percent:.0}%")
        }
    }

    pub fn reset(&self, preset: ViewPreset) {
        let Some((layout, _)) = self.layout.get() else {
            return;
        };
        self.manual.set(match preset {
            ViewPreset::Fit => None,
            ViewPreset::ActualSize => Some(ManualView {
                zoom: 1.,
                center: Point::new(layout.image.0 as f32 / 2., layout.image.1 as f32 / 2.),
            }),
        });
        self.resolve(layout.bounds, layout.image, layout.device_scale);
    }

    pub fn begin_pan(&self, grab: gpui::Point<Pixels>) -> Option<Pan> {
        self.get().map(|viewport| Pan {
            grab,
            viewport,
            manual: self.manual.get(),
        })
    }

    pub fn pan(&self, pan: &Pan, position: gpui::Point<Pixels>) {
        let Some((layout, previous)) = self.layout.get() else {
            return;
        };
        let mut viewport = Viewport {
            origin: pan.viewport.origin + (position - pan.grab),
            scale: previous.scale,
        };
        layout.constrain(&mut viewport);
        let manual = self.manual.get().is_some() || viewport.origin != previous.origin;
        self.commit(layout, viewport, manual);
    }

    pub fn cancel_pan(&self, pan: Pan) {
        self.manual.set(pan.manual);
        if let Some((layout, _)) = self.layout.get() {
            self.resolve(layout.bounds, layout.image, layout.device_scale);
        }
    }

    pub fn resolve(&self, bounds: Bounds<Pixels>, image: (u32, u32), device_scale: f32) -> Option<Viewport> {
        if bounds.size.width <= px(2. * PADDING) || bounds.size.height <= px(2. * PADDING) {
            self.layout.set(None);
            return None;
        }
        let layout = Layout {
            bounds,
            image,
            device_scale,
        };
        let (scale, center) = match self.manual.get() {
            Some(view) => (view.zoom / device_scale, view.center),
            None => (
                layout.fit_scale(),
                Point::new(image.0 as f32 / 2., image.1 as f32 / 2.),
            ),
        };
        let mut viewport = layout.centered(scale, center);
        layout.constrain(&mut viewport);
        self.commit(layout, viewport, self.manual.get().is_some());
        Some(viewport)
    }

    fn commit(&self, layout: Layout, viewport: Viewport, manual: bool) {
        self.layout.set(Some((layout, viewport)));
        self.manual.set(manual.then(|| ManualView {
            zoom: viewport.scale * layout.device_scale,
            center: viewport.to_image(layout.bounds.center()),
        }));
    }

    pub fn zoom(&self, position: gpui::Point<Pixels>, factor: f32) -> bool {
        let Some((layout, old)) = self.layout.get() else {
            return false;
        };
        if !layout.bounds.contains(&position) || !factor.is_finite() || factor <= 0. || factor == 1. {
            return false;
        }
        let zoom = (old.scale * layout.device_scale * factor)
            .clamp((layout.fit_scale() * layout.device_scale).min(0.1), 8.);
        let scale = zoom / layout.device_scale;
        let anchor = old.to_image(position);
        let mut viewport = Viewport {
            origin: position - point(px(anchor.x * scale), px(anchor.y * scale)),
            scale,
        };
        layout.constrain(&mut viewport);
        self.commit(layout, viewport, true);
        true
    }
}
