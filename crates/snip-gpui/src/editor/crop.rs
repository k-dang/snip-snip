//! Crop dimensions and toolbar controls. Geometry belongs to Editing.
use super::{Editor, Viewport, toolbar::control};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Context, Pixels, Window, div, point, prelude::*, px, rgb, size,
};
use snip_core::Rect;

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
        let enabled = self.model.document().crop.is_some();
        control("Clear crop", enabled, cx, |this, window, cx, _| {
            this.cancel_pan();
            this.model.clear_crop();
            this.popup = None;
            this.refresh(window);
            window.focus(&this.focus, cx);
            cx.notify();
        })
        .w_full()
        .px_2()
        .when(enabled, |b| b.hover(|s| s.bg(rgb(super::toolbar::HOVER))))
        .child("Clear crop")
    }
}
