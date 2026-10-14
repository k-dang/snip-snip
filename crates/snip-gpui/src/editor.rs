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
use snip_core::{
    Annotation, Arrow, Color, Document, History, Point, Rect, Rectangle, Renderer, RgbaImage, Stroke, Text,
};

use crop::CropGesture;
use text::{TextOverlay, TextSession};
use view::{CanvasView, Pan, ViewPreset, Viewport};

use crate::{capture::to_render_image, drag};

mod crop;
mod export;
mod navigation;
mod text;
mod toolbar;
mod view;
// GPUI's rendered-element selectors are only available in debug builds.
#[cfg(all(test, debug_assertions))]
mod tests;

const INK: Color = Color::rgb(0xE5, 0x48, 0x4D);
const PALETTE: [(&str, Color); 5] = [
    ("Red", INK),
    ("Yellow", Color::rgb(0xF5, 0xD9, 0x0A)),
    ("Green", Color::rgb(0x30, 0xA4, 0x6C)),
    ("Blue", Color::rgb(0x00, 0x90, 0xFF)),
    ("Black", Color::rgb(0x00, 0x00, 0x00)),
];
const WIDTHS: [(&str, f32); 3] = [("Thin", 1.0), ("Medium", 3.0), ("Thick", 6.0)];
/// Text size in logical pixels; converted to image pixels with the editor
/// window's scale factor when the annotation is created.
const TEXT_SIZE: f32 = 20.0;
/// Ignore accidental arrow clicks/jitter, measured at screenshot resolution.
const MIN_ARROW_LENGTH: f32 = 3.0;
const CANVAS_PADDING: f32 = 16.0;
static VERIFY_VIEW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
const HIT_TOLERANCE: f32 = 4.0;
const HANDLE_SIZE: f32 = 8.0;
const HANDLE_HIT_RADIUS: f32 = 6.0;
/// A click's small pointer jitter must not become a move or resize.
const DRAG_THRESHOLD: f32 = 3.0;

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    Pencil,
    Arrow,
    Rectangle,
    Text,
    Crop,
}

/// An annotation gesture in image pixels, or navigation in window coordinates.
enum Gesture {
    Pan(Pan),
    Annotation(Annotation),
    Crop(CropGesture),
    Edit {
        index: usize,
        original: Annotation,
        grab: Point,
        started: bool,
        operation: EditOperation,
    },
}

#[derive(Clone, Copy)]
enum Handle {
    ArrowTail,
    ArrowHead,
    RectangleCorner { opposite: Point },
}

impl Handle {
    fn cursor(self, position: Point) -> CursorStyle {
        match self {
            Self::ArrowTail | Self::ArrowHead => CursorStyle::Crosshair,
            Self::RectangleCorner { opposite } => {
                if (position.x - opposite.x) * (position.y - opposite.y) >= 0.0 {
                    CursorStyle::ResizeUpLeftDownRight
                } else {
                    CursorStyle::ResizeUpRightDownLeft
                }
            }
        }
    }
}

enum EditOperation {
    Move,
    Resize { handle: Handle, position: Point },
}

/// Handles use geometric endpoints/corners, rather than expanded ink bounds.
fn annotation_handles(annotation: &Annotation) -> Vec<(Point, Handle)> {
    match annotation {
        Annotation::Arrow(arrow) => vec![(arrow.tail, Handle::ArrowTail), (arrow.head, Handle::ArrowHead)],
        Annotation::Rectangle(rectangle) => {
            let corners = rectangle.corners();
            corners
                .iter()
                .enumerate()
                .map(|(i, &position)| {
                    (
                        position,
                        Handle::RectangleCorner {
                            opposite: corners[(i + 2) % 4],
                        },
                    )
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn valid_annotation(annotation: &Annotation) -> bool {
    match annotation {
        Annotation::Arrow(arrow) => {
            (arrow.head.x - arrow.tail.x).hypot(arrow.head.y - arrow.tail.y) >= MIN_ARROW_LENGTH
        }
        Annotation::Rectangle(rectangle) => !rectangle.is_empty(),
        _ => true,
    }
}

struct Editor {
    doc: Document,
    history: History,
    renderer: Renderer,
    /// `renderer.composite(&doc)` as a texture; refreshed by `rerender`.
    preview: Arc<RenderImage>,
    tool: Tool,
    /// Shared drawing defaults, independent of selection and document history.
    ink_color: Color,
    /// Logical pixels; annotations store widths converted to image pixels.
    ink_width: f32,
    selected: Option<usize>,
    cursor: CursorStyle,
    gesture: Option<Gesture>,
    space_held: bool,
    editing: Option<TextSession>,
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
        let doc = Document::new(image);
        Self {
            preview: to_render_image(&doc.image),
            history: History::new(&doc),
            doc,
            renderer: Renderer::new(),
            tool: Tool::Pencil,
            ink_color: INK,
            ink_width: 3.0,
            selected: None,
            cursor: CursorStyle::Crosshair,
            gesture: None,
            space_held: false,
            editing: None,
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

    /// Re-renders the preview from the document, including live geometry edits.
    fn rerender(&mut self, window: &mut Window) {
        let started = Instant::now();
        let composite = self.renderer.composite(&self.doc);
        let rendered = started.elapsed();
        let preview = to_render_image(&composite);
        // The window's GPU atlas keeps every image it has painted until the
        // window closes, so drop the replaced one.
        window
            .drop_image(std::mem::replace(&mut self.preview, preview))
            .ok();
        eprintln!(
            "editor: rerender {rendered:?} (+ upload prep {:?})",
            started.elapsed() - rendered
        );
    }

    fn set_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        self.popup = None;
        self.cancel_gesture(window);
        self.finish_text(window);
        self.tool = tool;
        if tool != Tool::Select {
            self.selected = None;
        }
        self.update_cursor(window);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(viewport) = self.viewport.get() else {
            return;
        };
        window.focus(&self.focus, cx);
        if self.space_held && self.editing.is_none() && self.gesture.is_none() {
            self.gesture = self.viewport.begin_pan(event.position).map(Gesture::Pan);
            self.update_cursor(window);
            cx.stop_propagation();
            return cx.notify();
        }
        let p = viewport.to_image(event.position);
        let (width, height) = self.doc.image.dimensions();
        let inside = (0.0..width as f32).contains(&p.x) && (0.0..height as f32).contains(&p.y);
        // A click inside the label being edited only moves its caret.
        if let Some(index) = self.editing_index()
            && self.text_at(p) == Some(index)
        {
            self.start_text(index, p, window);
            return cx.notify();
        }
        self.cancel_gesture(window);
        self.finish_text(window);
        if self.tool != Tool::Select {
            self.selected = None;
        }
        let ink_width = self.ink_width * window.scale_factor();
        match self.tool {
            Tool::Select => {
                if let Some((position, handle)) = self.handle_at(p, viewport.scale) {
                    let index = self.selected.unwrap();
                    self.gesture = Some(Gesture::Edit {
                        index,
                        original: self.doc.annotations[index].clone(),
                        grab: p,
                        started: false,
                        operation: EditOperation::Resize { handle, position },
                    });
                } else {
                    self.selected = inside
                        .then(|| {
                            self.renderer
                                .annotation_at(&self.doc, p, HIT_TOLERANCE / viewport.scale)
                        })
                        .flatten();
                    if let Some(index) = self.selected {
                        if event.click_count == 2
                            && matches!(self.doc.annotations[index], Annotation::Text(_))
                        {
                            self.start_text(index, p, window);
                        } else {
                            self.gesture = Some(Gesture::Edit {
                                index,
                                original: self.doc.annotations[index].clone(),
                                grab: p,
                                started: false,
                                operation: EditOperation::Move,
                            });
                        }
                    }
                }
            }
            Tool::Pencil if inside => {
                self.gesture = Some(Gesture::Annotation(Annotation::Stroke(Stroke {
                    points: vec![p],
                    color: self.ink_color,
                    width: ink_width,
                })))
            }
            Tool::Arrow if inside => {
                self.gesture = Some(Gesture::Annotation(Annotation::Arrow(Arrow {
                    tail: p,
                    head: p,
                    color: self.ink_color,
                    width: ink_width,
                })))
            }
            Tool::Rectangle if inside => {
                self.gesture = Some(Gesture::Annotation(Annotation::Rectangle(Rectangle {
                    start: p,
                    end: p,
                    color: self.ink_color,
                    width: ink_width,
                })))
            }
            Tool::Crop => self.begin_crop(p, viewport.scale, inside),
            Tool::Text if inside => {
                let index = self.text_at(p).unwrap_or_else(|| {
                    self.doc.annotations.push(Annotation::Text(Text {
                        origin: p,
                        content: String::new(),
                        size: TEXT_SIZE * window.scale_factor(),
                        color: self.ink_color,
                    }));
                    self.doc.annotations.len() - 1
                });
                self.start_text(index, p, window);
            }
            _ => {}
        }
        self.update_cursor(window);
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(viewport) = self.viewport.get() else {
            return;
        };
        let p = viewport.to_image(event.position);
        if let Some(Gesture::Pan(pan)) = &self.gesture {
            self.viewport.pan(pan, event.position);
        } else {
            self.update_gesture(p, event.modifiers.shift, window);
        }
        let cursor_changed = self.update_cursor(window);
        if self.gesture.is_some() || cursor_changed {
            cx.notify();
        }
    }

    fn on_modifiers_changed(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.gesture.is_some()
            && let Some(viewport) = self.viewport.get()
        {
            self.update_gesture(
                viewport.to_image(window.mouse_position()),
                event.modifiers.shift,
                window,
            );
            self.update_cursor(window);
            cx.notify();
        }
    }

    fn update_gesture(&mut self, p: Point, shift: bool, window: &mut Window) {
        match &mut self.gesture {
            Some(Gesture::Annotation(Annotation::Stroke(stroke))) => {
                if stroke.points.last() != Some(&p) {
                    stroke.points.push(p);
                }
            }
            Some(Gesture::Annotation(Annotation::Arrow(arrow))) => {
                arrow.head = if shift { p.snap_45(arrow.tail) } else { p };
            }
            Some(Gesture::Annotation(Annotation::Rectangle(rectangle))) => {
                rectangle.end = if shift {
                    p.square_corner(rectangle.start)
                } else {
                    p
                };
            }
            Some(Gesture::Edit {
                index,
                original,
                grab,
                started,
                operation,
            }) => {
                let offset = Point::new(p.x - grab.x, p.y - grab.y);
                let scale = self.viewport.get().map_or(1.0, |v| v.scale);
                *started |= offset.x.hypot(offset.y) * scale >= DRAG_THRESHOLD;
                if !*started {
                    return;
                }
                let mut annotation = original.clone();
                match operation {
                    EditOperation::Move => annotation.translate(offset),
                    EditOperation::Resize { handle, position } => {
                        let corner = Point::new(position.x + offset.x, position.y + offset.y);
                        match (&mut annotation, handle) {
                            (Annotation::Arrow(arrow), Handle::ArrowTail) => {
                                arrow.tail = if shift { corner.snap_45(arrow.head) } else { corner };
                            }
                            (Annotation::Arrow(arrow), Handle::ArrowHead) => {
                                arrow.head = if shift { corner.snap_45(arrow.tail) } else { corner };
                            }
                            (Annotation::Rectangle(rectangle), Handle::RectangleCorner { opposite }) => {
                                let corner = if shift {
                                    corner.square_corner(*opposite)
                                } else {
                                    corner
                                };
                                // Preserve endpoint ordering so an out-and-back
                                // resize is a no-op in document history too.
                                if rectangle.start.x == position.x {
                                    rectangle.start.x = corner.x;
                                }
                                if rectangle.start.y == position.y {
                                    rectangle.start.y = corner.y;
                                }
                                if rectangle.end.x == position.x {
                                    rectangle.end.x = corner.x;
                                }
                                if rectangle.end.y == position.y {
                                    rectangle.end.y = corner.y;
                                }
                            }
                            _ => unreachable!("handles match the original annotation"),
                        }
                    }
                }
                if annotation != self.doc.annotations[*index] {
                    self.doc.annotations[*index] = annotation;
                    self.rerender(window);
                }
            }
            Some(Gesture::Crop(gesture)) => {
                let scale = self.viewport.get().map_or(1.0, |v| v.scale);
                self.doc.crop = gesture.update(p, self.doc.image.dimensions(), scale);
            }
            _ => {}
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(viewport) = self.viewport.get() {
            self.update_gesture(viewport.to_image(event.position), event.modifiers.shift, window);
        }
        match self.gesture.take() {
            Some(Gesture::Pan(pan)) => self.viewport.pan(&pan, event.position),
            Some(Gesture::Annotation(annotation)) => {
                if valid_annotation(&annotation) {
                    self.doc.annotations.push(annotation);
                    self.history.commit(&self.doc);
                    self.rerender(window);
                }
            }
            Some(Gesture::Crop(gesture)) => {
                if self.doc.crop.is_none() {
                    self.doc.crop = gesture.previous();
                }
                self.history.commit(&self.doc);
            }
            Some(Gesture::Edit { index, original, .. }) => {
                if !valid_annotation(&self.doc.annotations[index]) {
                    self.doc.annotations[index] = original;
                    self.rerender(window);
                } else {
                    self.history.commit(&self.doc);
                }
            }
            None => return,
        }
        self.update_cursor(window);
        cx.notify();
    }

    /// Live geometry/crop changes are restored without touching history.
    fn cancel_gesture(&mut self, window: &mut Window) {
        match self.gesture.take() {
            Some(Gesture::Pan(pan)) => self.viewport.cancel_pan(pan),
            Some(Gesture::Crop(gesture)) => self.doc.crop = gesture.previous(),
            Some(Gesture::Edit { index, original, .. }) if self.doc.annotations[index] != original => {
                self.doc.annotations[index] = original;
                self.rerender(window);
            }
            _ => {}
        }
    }

    /// The closest visible handle wins; equal distances preserve handle order.
    fn handle_at(&self, p: Point, scale: f32) -> Option<(Point, Handle)> {
        let annotation = self.selected.and_then(|i| self.doc.annotations.get(i))?;
        let radius = HANDLE_HIT_RADIUS / scale;
        annotation_handles(annotation)
            .into_iter()
            .filter_map(|(position, handle)| {
                let distance = (position.x - p.x).hypot(position.y - p.y);
                (distance <= radius).then_some((position, handle, distance))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(position, handle, _)| (position, handle))
    }

    fn update_cursor(&mut self, window: &Window) -> bool {
        let cursor = if self.editing.is_some() {
            CursorStyle::IBeam
        } else if matches!(self.gesture, Some(Gesture::Pan(_))) {
            CursorStyle::ClosedHand
        } else if self.space_held && self.gesture.is_none() {
            CursorStyle::OpenHand
        } else if self.tool == Tool::Crop {
            self.crop_cursor(window)
        } else if self.tool != Tool::Select {
            CursorStyle::Crosshair
        } else if let Some(viewport) = self.viewport.get() {
            let p = viewport.to_image(window.mouse_position());
            match &self.gesture {
                Some(Gesture::Edit {
                    operation: EditOperation::Move,
                    ..
                }) => CursorStyle::ClosedHand,
                Some(Gesture::Edit {
                    operation: EditOperation::Resize { handle, .. },
                    ..
                }) => handle.cursor(p),
                _ => {
                    if let Some((position, handle)) = self.handle_at(p, viewport.scale) {
                        handle.cursor(position)
                    } else {
                        let (w, h) = self.doc.image.dimensions();
                        let inside = (0.0..w as f32).contains(&p.x) && (0.0..h as f32).contains(&p.y);
                        if inside
                            && self
                                .renderer
                                .annotation_at(&self.doc, p, HIT_TOLERANCE / viewport.scale)
                                .is_some()
                        {
                            CursorStyle::OpenHand
                        } else {
                            CursorStyle::Arrow
                        }
                    }
                }
            }
        } else {
            CursorStyle::Arrow
        };
        let changed = self.cursor != cursor;
        self.cursor = cursor;
        changed
    }

    fn restore_history(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_gesture(window);
        self.finish_text(window);
        // Finishing changed text commits it and clears redo, just like any
        // new edit. An unchanged session preserves the redo branch.
        let changed = if redo {
            self.history.redo(&mut self.doc)
        } else {
            self.history.undo(&mut self.doc)
        };
        if changed {
            self.rerender(window);
        }
        self.selected = None;
        self.update_cursor(window);
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

        if self.editing.is_some() {
            // Unhandled keys propagate so typed characters reach the input handler.
            if self.text_key_down(&keystroke.key, window) {
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
            "escape" if self.gesture.is_some() => {
                self.cancel_gesture(window);
                self.update_cursor(window);
                cx.notify();
            }
            "escape" if self.selected.is_some() => {
                self.selected = None;
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
                self.cancel_gesture(window);
                if let Some(index) = self.selected.take() {
                    self.doc.annotations.remove(index);
                    self.history.commit(&self.doc);
                    self.rerender(window);
                }
                self.update_cursor(window);
                cx.notify();
            }
            _ => {}
        }
    }

    fn export(&mut self, window: &mut Window) -> RgbaImage {
        self.popup = None;
        self.cancel_gesture(window);
        self.finish_text(window);
        self.update_cursor(window);
        let started = Instant::now();
        let image = self.renderer.export(&self.doc);
        eprintln!("editor: export {:?}", started.elapsed());
        image
    }

    /// What the canvas paints this frame, in image pixels.
    fn overlay(&mut self) -> CanvasOverlay {
        let live_annotation = match &self.gesture {
            Some(Gesture::Annotation(annotation)) => Some(annotation.clone()),
            _ => None,
        };
        let text = self.text_overlay();
        let selection = self
            .selected
            .and_then(|i| self.doc.annotations.get(i))
            .map(|annotation| SelectionOverlay {
                bounds: self.renderer.bounds(annotation),
                handles: annotation_handles(annotation)
                    .into_iter()
                    .map(|(position, _)| position)
                    .collect(),
            });
        CanvasOverlay {
            live_annotation,
            crop: self.doc.crop,
            crop_handles: self.tool == Tool::Crop,
            text,
            selection,
        }
    }
}

struct CanvasOverlay {
    live_annotation: Option<Annotation>,
    crop: Option<Rect>,
    crop_handles: bool,
    text: Option<TextOverlay>,
    selection: Option<SelectionOverlay>,
}

struct SelectionOverlay {
    bounds: snip_core::Bounds,
    handles: Vec<Point>,
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = self.preview.clone();
        let image_size = self.doc.image.dimensions();
        let viewport_cell = self.viewport.clone();
        let layout_viewport = self.viewport.clone();
        let crop_dimensions = if matches!(self.gesture, Some(Gesture::Crop(_))) {
            self.doc.crop
        } else {
            None
        };
        let overlay = self.overlay();
        let device_scale = window.scale_factor();
        let this = cx.entity();
        let editing = self.editing.is_some();
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
        let annotation = self
            .editing_index()
            .or(self.selected)
            .and_then(|i| self.doc.annotations.get(i));
        match annotation {
            Some(Annotation::Stroke(a)) => (a.color, Some(a.width / window.scale_factor())),
            Some(Annotation::Arrow(a)) => (a.color, Some(a.width / window.scale_factor())),
            Some(Annotation::Rectangle(a)) => (a.color, Some(a.width / window.scale_factor())),
            Some(Annotation::Text(a)) => (a.color, None),
            None => (
                self.ink_color,
                (self.tool != Tool::Text).then_some(self.ink_width),
            ),
        }
    }

    /// Resolve unfinished input before a toolbar release can commit a restyle.
    fn prepare_style_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_gesture(window);
        if let Some(label) = self.finish_text(window) {
            self.selected = Some(label);
        }
        window.focus(&self.focus, cx);
        self.update_cursor(window);
        cx.notify();
    }

    fn set_color(&mut self, color: Color, window: &mut Window, cx: &mut Context<Self>) {
        self.ink_color = color;
        if let Some(annotation) = self.selected.and_then(|i| self.doc.annotations.get_mut(i)) {
            let current = match annotation {
                Annotation::Stroke(a) => &mut a.color,
                Annotation::Arrow(a) => &mut a.color,
                Annotation::Rectangle(a) => &mut a.color,
                Annotation::Text(a) => &mut a.color,
            };
            if *current != color {
                *current = color;
                self.history.commit(&self.doc);
                self.rerender(window);
            }
        }
        cx.notify();
    }

    fn set_width(&mut self, width: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.ink_width = width;
        if let Some(annotation) = self.selected.and_then(|i| self.doc.annotations.get_mut(i)) {
            let current = match annotation {
                Annotation::Stroke(a) => &mut a.width,
                Annotation::Arrow(a) => &mut a.width,
                Annotation::Rectangle(a) => &mut a.width,
                Annotation::Text(_) => return,
            };
            let image_width = width * window.scale_factor();
            if *current != image_width {
                *current = image_width;
                self.history.commit(&self.doc);
                self.rerender(window);
            }
        }
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
            for (position, _) in crop::crop_handles(crop) {
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
