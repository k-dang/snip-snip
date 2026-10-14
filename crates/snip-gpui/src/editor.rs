//! The editor window. The preview is the core renderer's own output, uploaded
//! as a texture after edits, including live moves and resizes, so the preview
//! and exported image cannot drift apart. New ink, crop, caret, and selection
//! controls are painted by GPUI on top.

use std::{cell::Cell, rc::Rc, sync::Arc, time::Instant};

use gpui::{
    App, Bounds, Context, CursorStyle, ElementInputHandler, FocusHandle, Hsla, KeyDownEvent,
    ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Render,
    RenderImage, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions, canvas, div, fill, hsla,
    outline, point, prelude::*, px, rgb, size,
};
use snip_core::{Annotation, Color, Point, Rect, RgbaImage, Stroke};

use editing::{Editing, Handle, INK, Overlay as CanvasOverlay, PointerHint, Tool};
use view::{CanvasView, Pan, ViewPreset, Viewport};

use crate::{capture::to_render_image, drag};

mod crop;
mod editing;
mod export;
mod navigation;
mod text;
mod toolbar;
mod view;
// GPUI's rendered-element selectors are only available in debug builds.
#[cfg(all(test, debug_assertions))]
mod tests;

const PALETTE: [(&str, Color); 5] = [
    ("Red", INK),
    ("Yellow", Color::rgb(0xF5, 0xD9, 0x0A)),
    ("Green", Color::rgb(0x30, 0xA4, 0x6C)),
    ("Blue", Color::rgb(0x00, 0x90, 0xFF)),
    ("Black", Color::rgb(0x00, 0x00, 0x00)),
];
const WIDTHS: [(&str, f32); 3] = [("Thin", 1.0), ("Medium", 3.0), ("Thick", 6.0)];
const CANVAS_PADDING: f32 = 16.0;
static VERIFY_VIEW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
const HANDLE_SIZE: f32 = 8.0;

/// Opens an editor window for a freshly snipped image. `scale` is the source
/// monitor's image pixels per logical pixel, so the window opens at the size
/// the region had on screen.
pub fn open(image: RgbaImage, scale: f32, cx: &mut App) {
    let content = size(
        px(image.width() as f32 / scale + 2.0 * CANVAS_PADDING),
        px(image.height() as f32 / scale + 2.0 * CANVAS_PADDING + toolbar::HEIGHT + toolbar::STATUS_HEIGHT),
    );
    let max = cx
        .primary_display()
        .map(|d| d.bounds().size)
        .unwrap_or(size(px(1280.), px(800.)));
    let window_size = size(
        content.width.max(px(480.)).min(max.width * 0.85),
        content.height.max(px(320.)).min(max.height * 0.85),
    );
    let options = WindowOptions {
        window_min_size: Some(size(px(480.), px(320.))),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, window_size, cx))),
        titlebar: Some(TitlebarOptions {
            title: Some("snip-snip".into()),
            ..Default::default()
        }),
        focus: true,
        ..Default::default()
    };
    // A background app's new window opens behind the frontmost app on macOS;
    // the app itself must be activated to bring it forward.
    cx.activate(true);
    let opened = cx.open_window(options, |window, cx| {
        window.activate_window();
        cx.new(|cx| Editor::new(image, window, cx))
    });
    if let Err(err) = opened {
        eprintln!("editor: failed to open: {err:#}");
    }
}

struct Editor {
    model: Editing,
    /// The editing module's composite as a texture; updated by `refresh`.
    preview: Arc<RenderImage>,
    cursor: CursorStyle,
    pan: Option<Pan>,
    space_held: bool,
    viewport: Rc<CanvasView>,
    focus: FocusHandle,
    popup: Option<toolbar::Popup>,
    /// Mouse-opened popovers return to the canvas; keyboard-opened ones to their trigger.
    popup_return_focus: FocusHandle,
    menu_focus: [FocusHandle; 3],
    menu_bounds: [Rc<Cell<Bounds<gpui::Pixels>>>; 3],
    popup_focus: FocusHandle,
    status: SharedString,
}

impl Editor {
    fn new(image: RgbaImage, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        // GPUI on Windows leaks a window closed by the OS (title bar, Alt+F4):
        // its drag-drop registration is revoked after the HWND is gone. One
        // closed with `remove_window` is freed, so OS closes are redirected.
        window.on_window_should_close(cx, |window, _| {
            window.remove_window();
            false
        });
        cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.release_space(window, cx);
            }
        })
        .detach();
        let status = format!("{} × {} px", image.width(), image.height()).into();
        let preview = to_render_image(&image);
        Self {
            preview,
            model: Editing::new(image),
            cursor: CursorStyle::Crosshair,
            pan: None,
            space_held: false,
            viewport: Rc::default(),
            popup_return_focus: focus.clone(),
            focus,
            popup: None,
            menu_focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            menu_bounds: std::array::from_fn(|_| Rc::new(Cell::new(Bounds::default()))),
            popup_focus: cx.focus_handle(),
            status,
        }
    }

    /// Synchronize pixels and input affordances after an editing operation.
    fn refresh(&mut self, window: &mut Window) -> bool {
        let started = Instant::now();
        if let Some(composite) = self.model.take_preview() {
            let rendered = started.elapsed();
            let preview = to_render_image(&composite);
            // The GPU atlas retains painted images until the window closes.
            window
                .drop_image(std::mem::replace(&mut self.preview, preview))
                .ok();
            eprintln!(
                "editor: rerender {rendered:?} (+ upload prep {:?})",
                started.elapsed() - rendered
            );
        }
        if self.model.is_editing_text() {
            window.invalidate_character_coordinates();
        }
        self.update_cursor(window)
    }

    fn set_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        self.popup = None;
        self.cancel_pan();
        self.model.set_tool(tool);
        self.refresh(window);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(viewport) = self.viewport.get() else {
            return;
        };
        window.focus(&self.focus, cx);
        if self.space_held && !self.model.is_editing_text() && !self.has_gesture() {
            self.pan = self.viewport.begin_pan(event.position);
            self.update_cursor(window);
            cx.stop_propagation();
            return cx.notify();
        }
        self.cancel_pan();
        self.model.pointer_down(
            viewport.to_image(event.position),
            viewport.scale,
            window.scale_factor(),
            event.click_count == 2,
        );
        self.refresh(window);
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(viewport) = self.viewport.get() else {
            return;
        };
        if let Some(pan) = &self.pan {
            self.viewport.pan(pan, event.position);
        } else {
            self.model.update_gesture(
                viewport.to_image(event.position),
                event.modifiers.shift,
                viewport.scale,
            );
        }
        let cursor_changed = self.refresh(window);
        if self.has_gesture() || cursor_changed {
            cx.notify();
        }
    }

    fn on_modifiers_changed(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.model.has_gesture()
            && let Some(viewport) = self.viewport.get()
        {
            self.model.update_gesture(
                viewport.to_image(window.mouse_position()),
                event.modifiers.shift,
                viewport.scale,
            );
            self.refresh(window);
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pan) = self.pan.take() {
            self.viewport.pan(&pan, event.position);
        } else if self.model.has_gesture() {
            if let Some(viewport) = self.viewport.get() {
                self.model.update_gesture(
                    viewport.to_image(event.position),
                    event.modifiers.shift,
                    viewport.scale,
                );
            }
            self.model.finish_gesture();
        } else {
            return;
        }
        self.refresh(window);
        cx.notify();
    }

    fn has_gesture(&self) -> bool {
        self.pan.is_some() || self.model.has_gesture()
    }

    fn cancel_pan(&mut self) {
        if let Some(pan) = self.pan.take() {
            self.viewport.cancel_pan(pan);
        }
    }

    fn cancel_gesture(&mut self, window: &mut Window) {
        self.cancel_pan();
        self.model.cancel_gesture();
        self.refresh(window);
    }

    fn update_cursor(&mut self, window: &Window) -> bool {
        let cursor = if self.model.is_editing_text() {
            CursorStyle::IBeam
        } else if self.pan.is_some() {
            CursorStyle::ClosedHand
        } else if self.space_held && !self.model.has_gesture() {
            CursorStyle::OpenHand
        } else if let Some(viewport) = self.viewport.get() {
            match self
                .model
                .pointer_hint(viewport.to_image(window.mouse_position()), viewport.scale)
            {
                PointerHint::Arrow => CursorStyle::Arrow,
                PointerHint::Crosshair => CursorStyle::Crosshair,
                PointerHint::Text => CursorStyle::IBeam,
                PointerHint::Move => {
                    if self.model.has_gesture() {
                        CursorStyle::ClosedHand
                    } else {
                        CursorStyle::OpenHand
                    }
                }
                PointerHint::Resize { handle, position } => handle_cursor(handle, position),
            }
        } else if self.model.tool() != Tool::Select {
            CursorStyle::Crosshair
        } else {
            CursorStyle::Arrow
        };
        let changed = self.cursor != cursor;
        self.cursor = cursor;
        changed
    }

    fn restore_history(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_pan();
        self.model.restore_history(redo);
        self.refresh(window);
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.key == "escape" && self.popup.is_some() {
            self.dismiss_popup(window, cx);
            cx.stop_propagation();
            return;
        }
        if keystroke.key == "tab" {
            if keystroke.modifiers.shift {
                window.focus_prev(cx);
            } else {
                window.focus_next(cx);
            }
            cx.stop_propagation();
            return;
        }
        let shortcut = keystroke.modifiers.secondary();
        match (shortcut, keystroke.key.as_str()) {
            (true, "0") => return self.set_view(ViewPreset::Fit, window, cx),
            (true, "1") => return self.set_view(ViewPreset::ActualSize, window, cx),
            (true, "z") => return self.restore_history(keystroke.modifiers.shift, window, cx),
            (true, "y") if cfg!(windows) && keystroke.modifiers.control => {
                return self.restore_history(true, window, cx);
            }
            (true, "c") => return self.copy(keystroke.modifiers.shift, window, cx),
            (true, "s") => return self.save(window, cx),
            _ => {}
        }

        if self.model.is_editing_text() {
            // Unhandled keys propagate so typed characters reach the input handler.
            if self.model.text_key_down(&keystroke.key) {
                self.refresh(window);
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }

        if keystroke.key == "space" && keystroke.modifiers == gpui::Modifiers::none() {
            self.space_held = true;
            self.update_cursor(window);
            cx.stop_propagation();
            return cx.notify();
        }

        if keystroke.key != "escape"
            && (keystroke.modifiers.control
                || keystroke.modifiers.platform
                || keystroke.modifiers.alt
                || keystroke.modifiers.shift)
        {
            return;
        }

        match keystroke.key.as_str() {
            "escape" if self.has_gesture() => {
                self.cancel_gesture(window);
                cx.notify();
            }
            "escape" if self.model.has_selection() => {
                self.model.clear_selection();
                self.update_cursor(window);
                cx.notify();
            }
            "escape" => window.remove_window(),
            "v" => self.set_tool(Tool::Select, window, cx),
            "p" => self.set_tool(Tool::Pencil, window, cx),
            "a" => self.set_tool(Tool::Arrow, window, cx),
            "r" => self.set_tool(Tool::Rectangle, window, cx),
            "t" => self.set_tool(Tool::Text, window, cx),
            "c" => self.set_tool(Tool::Crop, window, cx),
            "delete" | "backspace" => {
                self.cancel_pan();
                self.model.delete_selected();
                self.refresh(window);
                cx.notify();
            }
            _ => {}
        }
    }

    fn export(&mut self, window: &mut Window) -> RgbaImage {
        self.popup = None;
        self.cancel_pan();
        let started = Instant::now();
        let image = self.model.export();
        self.refresh(window);
        eprintln!("editor: export {:?}", started.elapsed());
        image
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = self.preview.clone();
        let image_size = self.model.document().image.dimensions();
        let viewport_cell = self.viewport.clone();
        let layout_viewport = self.viewport.clone();
        let crop_dimensions = if self.model.is_cropping() {
            self.model.document().crop
        } else {
            None
        };
        let overlay = self.model.overlay();
        let device_scale = window.scale_factor();
        let this = cx.entity();
        let editing = self.model.is_editing_text();
        let focus = self.focus.clone();
        let painted_zoom_label = self.viewport.zoom_label();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1E1E20))
            .text_color(rgb(0xE6E6E6))
            .text_sm()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, window, cx| this.on_key_down(event, window, cx)))
            .on_key_up(cx.listener(Self::on_key_up))
            .on_modifiers_changed(
                cx.listener(|this, event, window, cx| this.on_modifiers_changed(event, window, cx)),
            )
            .child(self.toolbar(window, cx))
            .child(
                div()
                    .debug_selector(|| "editor-canvas".into())
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .cursor(self.cursor)
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .on_pinch(cx.listener(Self::on_pinch))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event, window, cx| this.on_mouse_down(event, window, cx)),
                    )
                    .child(
                        canvas(
                            move |bounds, window, cx| {
                                let viewport = layout_viewport.resolve(bounds, image_size, device_scale);
                                let dimensions = viewport.and_then(|viewport| {
                                    crop_dimensions.map(|crop| {
                                        crop::prepaint_dimensions(crop, viewport, bounds, window, cx)
                                    })
                                });
                                (viewport, dimensions)
                            },
                            move |bounds, (viewport, dimensions), window, cx| {
                                if editing {
                                    window.handle_input(
                                        &focus,
                                        ElementInputHandler::new(bounds, this.clone()),
                                        cx,
                                    );
                                }
                                drag::track(window, this.clone(), Self::on_mouse_move, Self::on_mouse_up);
                                if let Some(viewport) = viewport {
                                    paint_canvas(window, viewport, image_size, preview, overlay);
                                    // Verification observes the painted transform; input still goes through the OS.
                                    if *VERIFY_VIEW
                                        .get_or_init(|| std::env::var_os("SNIP_SNIP_VERIFY").is_some())
                                    {
                                        let content = window.viewport_size();
                                        eprintln!(
                                            "snipctl-view: {} {} {} {} {} {} {} {}",
                                            image_size.0,
                                            image_size.1,
                                            f32::from(viewport.origin.x),
                                            f32::from(viewport.origin.y),
                                            viewport.scale,
                                            window.scale_factor(),
                                            f32::from(content.width),
                                            f32::from(content.height),
                                        );
                                    }
                                    if let Some(mut dimensions) = dimensions {
                                        dimensions.paint(window, cx);
                                    }
                                    let view_changed = viewport_cell.record_paint(viewport);
                                    if editing && view_changed {
                                        window.invalidate_character_coordinates();
                                    }
                                    if viewport_cell.zoom_label() != painted_zoom_label {
                                        this.update(cx, |_, cx| cx.notify());
                                    }
                                }
                            },
                        )
                        .size_full(),
                    ),
            )
            .child(self.status_line(window))
    }
}

impl Editor {
    /// Selection displays its own style without replacing drawing defaults.
    fn displayed_style(&self, window: &Window) -> (Color, Option<f32>) {
        self.model.displayed_style(window.scale_factor())
    }

    fn set_color(&mut self, color: Color, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_pan();
        self.model.set_color(color);
        self.refresh(window);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn set_width(&mut self, width: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_pan();
        self.model.set_width(width, window.scale_factor());
        self.refresh(window);
        window.focus(&self.focus, cx);
        cx.notify();
    }
}

fn paint_canvas(
    window: &mut Window,
    viewport: Viewport,
    (w, h): (u32, u32),
    preview: Arc<RenderImage>,
    overlay: CanvasOverlay,
) {
    let image_bounds = viewport.rect_to_window(Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    });
    window
        .paint_image(image_bounds, image_bounds, Default::default(), preview, 0, false)
        .ok();
    // Keeps the image edge visible when the screenshot is as dark as the editor.
    window.paint_quad(outline(
        image_bounds.dilate(px(1.)),
        hsla(0., 0., 1., 0.12),
        Default::default(),
    ));

    if let Some(annotation) = overlay.live_annotation {
        window.with_content_mask(Some(gpui::ContentMask { bounds: image_bounds }), |window| {
            paint_live_annotation(window, viewport, &annotation);
        });
    }

    if let Some(text) = overlay.text {
        let (top, height) = text.caret;
        window.paint_quad(fill(
            Bounds::new(
                viewport.to_window(top),
                size(px(1.5), px(height * viewport.scale)),
            ),
            to_hsla(text.color),
        ));
        if let Some((start, width)) = text.composition {
            window.paint_quad(fill(
                Bounds::new(
                    viewport.to_window(start),
                    size(px(width * viewport.scale), px(1.5)),
                ),
                to_hsla(text.color),
            ));
        }
    }

    if let Some(crop) = overlay.crop {
        let hole = viewport.rect_to_window(crop);
        let dim = hsla(0., 0., 0., 0.55);
        let (l, t, r, b) = (
            image_bounds.left(),
            image_bounds.top(),
            image_bounds.right(),
            image_bounds.bottom(),
        );
        for (a, z) in [
            (point(l, t), point(r, hole.top())),
            (point(l, hole.bottom()), point(r, b)),
            (point(l, hole.top()), point(hole.left(), hole.bottom())),
            (point(hole.right(), hole.top()), point(r, hole.bottom())),
        ] {
            if z.x > a.x && z.y > a.y {
                window.paint_quad(fill(Bounds::from_corners(a, z), dim));
            }
        }
        window.paint_quad(outline(hole, hsla(0., 0., 1., 0.9), Default::default()));
        if overlay.crop_handles {
            for (position, _) in editing::crop_handles(crop) {
                paint_handle(window, viewport.to_window(position));
            }
        }
    }

    if let Some(selection) = overlay.selection {
        let bounds = Bounds::from_corners(
            viewport.to_window(selection.bounds.min),
            viewport.to_window(selection.bounds.max),
        );
        // A dark outer edge keeps the controls visible on light screenshots.
        window.paint_quad(outline(
            bounds.dilate(px(1.)),
            hsla(0., 0., 0., 0.85),
            Default::default(),
        ));
        window.paint_quad(outline(bounds, hsla(0.58, 0.9, 0.65, 1.), Default::default()));
        for position in selection.handles {
            paint_handle(window, viewport.to_window(position));
        }
    }
}

fn paint_handle(window: &mut Window, center: gpui::Point<gpui::Pixels>) {
    let handle = Bounds::new(
        center - point(px(HANDLE_SIZE / 2.), px(HANDLE_SIZE / 2.)),
        size(px(HANDLE_SIZE), px(HANDLE_SIZE)),
    );
    window.paint_quad(outline(
        handle.dilate(px(1.)),
        hsla(0., 0., 0., 0.85),
        Default::default(),
    ));
    window.paint_quad(fill(handle, hsla(0., 0., 1., 1.)));
    window.paint_quad(outline(handle, hsla(0.58, 0.9, 0.45, 1.), Default::default()));
}

/// The stroke being drawn, painted by GPUI until mouse-up commits it to the
/// document. Its width is in image pixels, like the committed stroke's.
fn paint_live_stroke(window: &mut Window, viewport: Viewport, stroke: &Stroke) {
    let points = &stroke.points;
    let width = px(stroke.width * viewport.scale);
    let mut path = PathBuilder::stroke(width);
    path.move_to(viewport.to_window(points[0]));
    for p in &points[1..] {
        path.line_to(viewport.to_window(*p));
    }
    if points.len() == 1 {
        // Degenerate path: nudge so a click shows a dot while held.
        path.line_to(viewport.to_window(points[0]) + point(px(0.01), px(0.)));
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, to_hsla(stroke.color));
    }
}

/// Live shapes use the core's geometry and matching butt caps/miter joins.
/// The committed preview and all exports use the core rasterizer itself.
fn paint_live_annotation(window: &mut Window, viewport: Viewport, annotation: &Annotation) {
    match annotation {
        Annotation::Stroke(stroke) => paint_live_stroke(window, viewport, stroke),
        Annotation::Arrow(arrow) => {
            let Some(geometry) = arrow.geometry() else { return };
            let mut shaft = PathBuilder::stroke(px(arrow.width * viewport.scale));
            shaft.move_to(viewport.to_window(geometry.shaft[0]));
            shaft.line_to(viewport.to_window(geometry.shaft[1]));
            if let Ok(path) = shaft.build() {
                window.paint_path(path, to_hsla(arrow.color));
            }
            let mut head = PathBuilder::fill();
            head.move_to(viewport.to_window(geometry.head[0]));
            for point in &geometry.head[1..] {
                head.line_to(viewport.to_window(*point));
            }
            head.close();
            if let Ok(path) = head.build() {
                window.paint_path(path, to_hsla(arrow.color));
            }
        }
        Annotation::Rectangle(rectangle) => {
            if rectangle.is_empty() {
                return;
            }
            let corners = rectangle.corners();
            let mut path = PathBuilder::stroke(px(rectangle.width * viewport.scale));
            path.move_to(viewport.to_window(corners[0]));
            for point in &corners[1..] {
                path.line_to(viewport.to_window(*point));
            }
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, to_hsla(rectangle.color));
            }
        }
        Annotation::Text(_) => {}
    }
}

fn to_hsla(c: Color) -> Hsla {
    gpui::Rgba {
        r: c.r as f32 / 255.,
        g: c.g as f32 / 255.,
        b: c.b as f32 / 255.,
        a: c.a as f32 / 255.,
    }
    .into()
}

fn handle_cursor(handle: Handle, position: Point) -> CursorStyle {
    match handle {
        Handle::ArrowTail | Handle::ArrowHead => CursorStyle::Crosshair,
        Handle::RectangleCorner { opposite } => {
            if (position.x - opposite.x) * (position.y - opposite.y) >= 0. {
                CursorStyle::ResizeUpLeftDownRight
            } else {
                CursorStyle::ResizeUpRightDownLeft
            }
        }
    }
}
