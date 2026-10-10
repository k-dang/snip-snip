//! The editor window. The preview is the core renderer's own output, uploaded
//! as a texture after every committed edit, so the preview and the exported
//! image cannot drift apart. Only in-progress gestures (a stroke being drawn,
//! the crop being dragged, the text caret) are painted by GPUI on top.

use std::{
    borrow::Cow,
    cell::Cell,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use gpui::{
    App, Bounds, Context, FocusHandle, Hsla, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    PathBuilder, Pixels, Render, RenderImage, SharedString, TitlebarOptions, Window, WindowBounds,
    WindowOptions, canvas, div, fill, hsla, outline, point, prelude::*, px, rgb, size,
};
use snip_core::{Annotation, Color, Document, Point, Rect, Renderer, RgbaImage, Stroke, Text};

use crate::capture::to_render_image;

const INK: Color = Color::rgb(0xE5, 0x48, 0x4D);
/// Pencil width and text size in logical pixels; converted to image pixels
/// with the editor window's scale factor when the annotation is created.
const PENCIL_WIDTH: f32 = 3.0;
const TEXT_SIZE: f32 = 20.0;
const CANVAS_PADDING: f32 = 16.0;

/// Opens an editor window for a freshly snipped image. `scale` is the source
/// monitor's image pixels per logical pixel, so the window opens at the size
/// the region had on screen.
pub fn open(image: RgbaImage, scale: f32, cx: &mut App) {
    let content = size(
        px(image.width() as f32 / scale + 2.0 * CANVAS_PADDING),
        px(image.height() as f32 / scale + 2.0 * CANVAS_PADDING + 44.0),
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
    Pencil,
    Text,
    Crop,
}

/// A pointer gesture in progress, in image pixels.
enum Gesture {
    Stroke(Vec<Point>),
    Crop(Point),
}

/// Where the image is drawn inside the window, recorded during paint so
/// pointer events can be mapped back to image pixels.
#[derive(Clone, Copy)]
struct Viewport {
    origin: gpui::Point<Pixels>,
    /// Logical pixels per image pixel.
    scale: f32,
}

impl Viewport {
    fn to_image(self, p: gpui::Point<Pixels>) -> Point {
        Point::new(
            f32::from(p.x - self.origin.x) / self.scale,
            f32::from(p.y - self.origin.y) / self.scale,
        )
    }

    fn to_window(self, p: Point) -> gpui::Point<Pixels> {
        self.origin + point(px(p.x * self.scale), px(p.y * self.scale))
    }

    fn rect_to_window(self, r: Rect) -> Bounds<Pixels> {
        Bounds::from_corners(
            self.to_window(Point::new(r.x as f32, r.y as f32)),
            self.to_window(Point::new((r.x + r.width) as f32, (r.y + r.height) as f32)),
        )
    }
}

struct Editor {
    doc: Document,
    renderer: Renderer,
    /// `renderer.composite(&doc)` as a texture; refreshed by `rerender`.
    preview: Arc<RenderImage>,
    tool: Tool,
    gesture: Option<Gesture>,
    /// Index of the text annotation receiving keystrokes.
    editing: Option<usize>,
    viewport: Rc<Cell<Option<Viewport>>>,
    focus: FocusHandle,
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
        let status = format!("{} × {} px", image.width(), image.height()).into();
        Self {
            preview: to_render_image(&image),
            doc: Document::new(image),
            renderer: Renderer::new(),
            tool: Tool::Pencil,
            gesture: None,
            editing: None,
            viewport: Rc::default(),
            focus,
            status,
        }
    }

    /// Re-renders the preview from the document after a committed edit.
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
        self.finish_text(window);
        self.tool = tool;
        cx.notify();
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(viewport) = self.viewport.get() else {
            return;
        };
        window.focus(&self.focus, cx);
        let p = viewport.to_image(event.position);
        let (width, height) = self.doc.image.dimensions();
        let inside = (0.0..width as f32).contains(&p.x) && (0.0..height as f32).contains(&p.y);
        match self.tool {
            Tool::Pencil if inside => self.gesture = Some(Gesture::Stroke(vec![p])),
            Tool::Crop => {
                self.doc.crop = None;
                self.gesture = Some(Gesture::Crop(p));
            }
            Tool::Text if inside => {
                self.finish_text(window);
                self.editing = Some(self.text_at(p).unwrap_or_else(|| {
                    self.doc.annotations.push(Annotation::Text(Text {
                        origin: p,
                        content: String::new(),
                        size: TEXT_SIZE * window.scale_factor(),
                        color: INK,
                    }));
                    self.doc.annotations.len() - 1
                }));
            }
            _ => {}
        }
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(viewport) = self.viewport.get() else {
            return;
        };
        let p = viewport.to_image(event.position);
        match &mut self.gesture {
            Some(Gesture::Stroke(points)) => points.push(p),
            Some(Gesture::Crop(start)) => {
                let (width, height) = self.doc.image.dimensions();
                self.doc.crop = Rect::from_corners(*start, p, width, height);
            }
            None => return,
        }
        cx.notify();
    }

    fn on_mouse_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Gesture::Stroke(points)) = self.gesture.take() {
            self.doc.annotations.push(Annotation::Stroke(Stroke {
                points,
                color: INK,
                width: PENCIL_WIDTH * window.scale_factor(),
            }));
            self.rerender(window);
        }
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let shortcut = keystroke.modifiers.secondary();
        match (shortcut, keystroke.key.as_str()) {
            (true, "c") => return self.copy(window, cx),
            (true, "s") => return self.save(window, cx),
            _ => {}
        }

        if let Some(Annotation::Text(text)) = self.editing.and_then(|i| self.doc.annotations.get_mut(i)) {
            match keystroke.key.as_str() {
                "escape" | "enter" => self.finish_text(window),
                "backspace" => {
                    text.content.pop();
                    self.rerender(window);
                }
                _ => match &keystroke.key_char {
                    Some(typed) if !shortcut => {
                        text.content.push_str(typed);
                        self.rerender(window);
                    }
                    _ => return,
                },
            }
            return cx.notify();
        }

        match keystroke.key.as_str() {
            "escape" => window.remove_window(),
            "p" => self.set_tool(Tool::Pencil, window, cx),
            "t" => self.set_tool(Tool::Text, window, cx),
            "c" => self.set_tool(Tool::Crop, window, cx),
            _ => {}
        }
    }

    /// Ends text editing, dropping the label if it was left empty.
    fn finish_text(&mut self, window: &mut Window) {
        let Some(index) = self.editing.take() else { return };
        if matches!(&self.doc.annotations[index], Annotation::Text(t) if t.content.is_empty()) {
            self.doc.annotations.remove(index);
            self.rerender(window);
        }
    }

    /// The topmost text annotation under `p`, if any.
    fn text_at(&mut self, p: Point) -> Option<usize> {
        let renderer = &mut self.renderer;
        self.doc.annotations.iter().rposition(|a| match a {
            Annotation::Text(t) => {
                let (w, h) = renderer.text_size(t);
                (t.origin.x..=t.origin.x + w).contains(&p.x) && (t.origin.y..=t.origin.y + h).contains(&p.y)
            }
            Annotation::Stroke(_) => false,
        })
    }

    fn export(&mut self, window: &mut Window) -> RgbaImage {
        self.finish_text(window);
        let started = Instant::now();
        let image = self.renderer.export(&self.doc);
        eprintln!("editor: export {:?}", started.elapsed());
        image
    }

    fn copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let image = self.export(window);
        let data = arboard::ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: Cow::Owned(image.into_raw()),
        };
        self.status = match arboard::Clipboard::new().and_then(|mut c| c.set_image(data)) {
            Ok(()) => "Copied to clipboard".into(),
            Err(err) => format!("Copy failed: {err}").into(),
        };
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let image = self.export(window);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let directory = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let chosen = cx.prompt_for_new_path(&directory, Some(&format!("snip-{stamp}.png")));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else { return };
            let written = cx
                .background_executor()
                .spawn(async move { std::fs::write(&path, snip_core::encode_png(&image)).map(|()| path) })
                .await;
            this.update(cx, |this, cx| {
                this.status = match written {
                    Ok(path) => format!("Saved {}", path.display()).into(),
                    Err(err) => format!("Save failed: {err}").into(),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// What the canvas paints this frame, in image pixels.
    fn overlay(&mut self) -> CanvasOverlay {
        let live_stroke = match &self.gesture {
            Some(Gesture::Stroke(points)) => Some(points.clone()),
            _ => None,
        };
        let caret = self.editing.and_then(|i| match &self.doc.annotations[i] {
            Annotation::Text(t) => Some((t.clone(), self.renderer.text_size(t))),
            Annotation::Stroke(_) => None,
        });
        CanvasOverlay {
            live_stroke,
            crop: self.doc.crop,
            caret: caret.map(|(t, (w, h))| (Point::new(t.origin.x + w, t.origin.y), h)),
        }
    }
}

struct CanvasOverlay {
    live_stroke: Option<Vec<Point>>,
    crop: Option<Rect>,
    /// Caret top and height, in image pixels.
    caret: Option<(Point, f32)>,
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = self.preview.clone();
        let image_size = self.doc.image.dimensions();
        let viewport_cell = self.viewport.clone();
        let overlay = self.overlay();
        let device_scale = window.scale_factor();
        let live_width = PENCIL_WIDTH * device_scale;

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1E1E20))
            .text_color(rgb(0xE6E6E6))
            .text_sm()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, window, cx| this.on_key_down(event, window, cx)))
            .child(
                div()
                    .h(px(44.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(0x333336))
                    .child(self.tool_button("Pencil", "P", Tool::Pencil, cx))
                    .child(self.tool_button("Text", "T", Tool::Text, cx))
                    .child(self.tool_button("Crop", "C", Tool::Crop, cx))
                    .child(div().flex_1())
                    .child(div().text_color(rgb(0x9A9AA0)).child(self.status.clone()))
                    .child(action_button("Copy", cx, |this, window, cx| {
                        this.copy(window, cx)
                    }))
                    .child(action_button("Save…", cx, |this, window, cx| {
                        this.save(window, cx)
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .cursor_crosshair()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event, window, cx| this.on_mouse_down(event, window, cx)),
                    )
                    .on_mouse_move(cx.listener(|this, event, _, cx| this.on_mouse_move(event, cx)))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.on_mouse_up(window, cx)),
                    )
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                let viewport = fit(bounds, image_size, device_scale);
                                viewport_cell.set(Some(viewport));
                                paint_canvas(window, viewport, image_size, preview, overlay, live_width);
                            },
                        )
                        .size_full(),
                    ),
            )
    }
}

impl Editor {
    fn tool_button(
        &self,
        label: &'static str,
        key: &'static str,
        tool: Tool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.tool == tool;
        button(label)
            .when(selected, |b| b.bg(rgb(0x3A3A3F)).border_color(rgb(0x55555C)))
            .child(div().text_color(rgb(0x7A7A80)).text_xs().child(key))
            .on_click(cx.listener(move |this, _, window, cx| this.set_tool(tool, window, cx)))
    }
}

fn action_button(
    label: &'static str,
    cx: &mut Context<Editor>,
    action: impl Fn(&mut Editor, &mut Window, &mut Context<Editor>) + 'static,
) -> impl IntoElement {
    button(label).on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
}

fn button(label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h(px(28.))
        .px_2p5()
        .flex()
        .items_center()
        .gap_1p5()
        .rounded_md()
        .border_1()
        .border_color(gpui::transparent_black())
        .hover(|s| s.bg(rgb(0x2C2C30)))
        .child(label)
}

/// Fits the image inside `bounds` with padding, centered, never larger than
/// one image pixel per device pixel.
fn fit(bounds: Bounds<Pixels>, (w, h): (u32, u32), device_scale: f32) -> Viewport {
    let avail_w = f32::from(bounds.size.width) - 2.0 * CANVAS_PADDING;
    let avail_h = f32::from(bounds.size.height) - 2.0 * CANVAS_PADDING;
    let scale = (avail_w / w as f32)
        .min(avail_h / h as f32)
        .min(1.0 / device_scale)
        .max(0.01);
    let shown = size(px(w as f32 * scale), px(h as f32 * scale));
    Viewport {
        origin: bounds.origin
            + point(
                (bounds.size.width - shown.width) / 2.,
                (bounds.size.height - shown.height) / 2.,
            ),
        scale,
    }
}

fn paint_canvas(
    window: &mut Window,
    viewport: Viewport,
    (w, h): (u32, u32),
    preview: Arc<RenderImage>,
    overlay: CanvasOverlay,
    live_width: f32,
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

    if let Some(points) = overlay.live_stroke {
        paint_live_stroke(window, viewport, &points, live_width);
    }

    if let Some((top, height)) = overlay.caret {
        let top = viewport.to_window(top);
        window.paint_quad(fill(
            Bounds::new(top, size(px(1.5), px(height * viewport.scale))),
            to_hsla(INK),
        ));
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
    }
}

/// The stroke being drawn, painted by GPUI until mouse-up commits it to the
/// document. `width` is in image pixels, like the committed stroke's.
fn paint_live_stroke(window: &mut Window, viewport: Viewport, points: &[Point], width: f32) {
    let width = px(width * viewport.scale);
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
        window.paint_path(path, to_hsla(INK));
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
