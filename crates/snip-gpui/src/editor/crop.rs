//! Crop gestures retain their starting geometry so live adjustment never drifts.

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Context, CursorStyle, MouseButton, Pixels, Window, div, point,
    prelude::*, px, rgb, size,
};
use snip_core::{Point, Rect};

use super::{DRAG_THRESHOLD, Editor, Gesture, HANDLE_HIT_RADIUS, Handle, Viewport, button};

pub(super) enum CropGesture {
    Create {
        start: Point,
        previous: Option<Rect>,
    },
    Resize {
        original: Rect,
        position: Point,
        opposite: Point,
        grab: Point,
        started: bool,
    },
}

impl CropGesture {
    pub fn previous(&self) -> Option<Rect> {
        match self {
            Self::Create { previous, .. } => *previous,
            Self::Resize { original, .. } => Some(*original),
        }
    }

    pub fn cursor(&self) -> CursorStyle {
        match *self {
            Self::Create { .. } => CursorStyle::Crosshair,
            Self::Resize {
                position, opposite, ..
            } => Handle::RectangleCorner { opposite }.cursor(position),
        }
    }

    pub fn update(&mut self, p: Point, (width, height): (u32, u32), scale: f32) -> Option<Rect> {
        match self {
            Self::Create { start, .. } => {
                if start.x == p.x || start.y == p.y {
                    None
                } else {
                    Rect::from_corners(*start, p, width, height)
                }
            }
            Self::Resize {
                original,
                position,
                opposite,
                grab,
                started,
            } => {
                let offset = Point::new(p.x - grab.x, p.y - grab.y);
                *started |= offset.x.hypot(offset.y) * scale >= DRAG_THRESHOLD;
                if !*started {
                    return Some(*original);
                }
                // Round outward, and stop one pixel before the fixed corner.
                let boundary = |start: f32, fixed: f32, delta: f32, limit: u32| {
                    if start < fixed {
                        (start + delta).floor().clamp(0., fixed - 1.)
                    } else {
                        (start + delta).ceil().clamp(fixed + 1., limit as f32)
                    }
                };
                let corner = Point::new(
                    boundary(position.x, opposite.x, offset.x, width),
                    boundary(position.y, opposite.y, offset.y, height),
                );
                Rect::from_corners(corner, *opposite, width, height)
            }
        }
    }
}

pub(super) fn crop_handles(crop: Rect) -> [(Point, Handle); 4] {
    let (left, top) = (crop.x as f32, crop.y as f32);
    let (right, bottom) = ((crop.x + crop.width) as f32, (crop.y + crop.height) as f32);
    let corners = [
        Point::new(left, top),
        Point::new(right, top),
        Point::new(right, bottom),
        Point::new(left, bottom),
    ];
    std::array::from_fn(|i| {
        (
            corners[i],
            Handle::RectangleCorner {
                opposite: corners[(i + 2) % 4],
            },
        )
    })
}

/// Lay out the live label before painting, keeping it readable at canvas edges.
pub(super) fn prepaint_dimensions(
    crop: Rect,
    viewport: Viewport,
    canvas: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let label = format!("{} × {} px", crop.width, crop.height);
    let selector = label.clone();
    let mut element = div()
        .debug_selector(move || selector)
        .px(px(8.))
        .py(px(4.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(rgb(0xFFFFFF))
        .bg(rgb(0x121214))
        .border_1()
        .border_color(rgb(0x55555C))
        .rounded_md()
        .child(label)
        .into_any_element();
    let label_size = element.layout_as_root(
        size(AvailableSpace::MinContent, AvailableSpace::MinContent),
        window,
        cx,
    );
    let crop_bounds = viewport.rect_to_window(crop);
    let pointer = window.mouse_position();
    let corner = point(
        pointer.x.clamp(crop_bounds.left(), crop_bounds.right()),
        pointer.y.clamp(crop_bounds.top(), crop_bounds.bottom()),
    );
    let place = |corner: Pixels, length: Pixels, start: Pixels, end: Pixels| {
        let offset = px(12.);
        let preferred = if corner + offset + length <= end {
            corner + offset
        } else {
            corner - offset - length
        };
        preferred.clamp(start, (end - length).max(start))
    };
    let origin = point(
        place(corner.x, label_size.width, canvas.left(), canvas.right()),
        place(corner.y, label_size.height, canvas.top(), canvas.bottom()),
    );
    element.prepaint_at(origin, window, cx);
    element
}

impl Editor {
    pub(super) fn clear_crop_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.doc.crop.is_some();
        button("Clear crop")
            .when(!enabled, |button| button.opacity(0.4))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    if enabled {
                        this.cancel_gesture(window);
                        this.finish_text(window);
                        window.focus(&this.focus, cx);
                        this.update_cursor(window);
                        cx.notify();
                    }
                }),
            )
            .when(enabled, |button| {
                button.on_click(cx.listener(|this, _, window, cx| {
                    if this.doc.crop.take().is_some() {
                        this.history.commit(&this.doc);
                    }
                    this.update_cursor(window);
                    cx.notify();
                }))
            })
    }

    pub(super) fn crop_handle_at(&self, p: Point, scale: f32) -> Option<(Point, Handle)> {
        crop_handles(self.doc.crop?)
            .into_iter()
            .map(|(position, handle)| {
                let distance = (position.x - p.x).hypot(position.y - p.y);
                (position, handle, distance)
            })
            .filter(|(_, _, distance)| *distance <= HANDLE_HIT_RADIUS / scale)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(position, handle, _)| (position, handle))
    }

    pub(super) fn begin_crop(&mut self, p: Point, scale: f32, inside: bool) {
        if let Some((position, Handle::RectangleCorner { opposite })) = self.crop_handle_at(p, scale) {
            self.gesture = Some(Gesture::Crop(CropGesture::Resize {
                original: self.doc.crop.unwrap(),
                position,
                opposite,
                grab: p,
                started: false,
            }));
        } else if inside {
            let previous = self.doc.crop.take();
            self.gesture = Some(Gesture::Crop(CropGesture::Create { start: p, previous }));
        }
    }

    pub(super) fn crop_cursor(&self, window: &Window) -> CursorStyle {
        if let Some(Gesture::Crop(gesture)) = &self.gesture {
            gesture.cursor()
        } else if let Some(viewport) = self.viewport.get()
            && let Some((position, handle)) =
                self.crop_handle_at(viewport.to_image(window.mouse_position()), viewport.scale)
        {
            handle.cursor(position)
        } else {
            CursorStyle::Crosshair
        }
    }
}
