//! Document editing and interaction rules, independent of window events and painting.
//! Editing owns mutable document state; callers can only read the document.

use snip_core::{
    Annotation, Arrow, Color, Document, History, Point, Rect, Rectangle, Renderer, RgbaImage, Stroke, Text,
};

use crop::CropGesture;
pub(super) use crop::crop_handles;
use text::{TextOverlay, TextSession};

mod crop;
#[cfg(test)]
mod tests;
mod text;

pub(super) const INK: Color = Color::rgb(0xE5, 0x48, 0x4D);
/// Logical pixels, converted to image pixels when a label is created.
const TEXT_SIZE: f32 = 20.;
/// Ignore accidental arrow clicks/jitter, measured at screenshot resolution.
const MIN_ARROW_LENGTH: f32 = 3.;
const HIT_TOLERANCE: f32 = 4.;
const HANDLE_HIT_RADIUS: f32 = 6.;
/// A click's small pointer jitter must not become a move or resize.
const DRAG_THRESHOLD: f32 = 3.;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tool {
    Select,
    Pencil,
    Arrow,
    Rectangle,
    Text,
    Crop,
}

/// An unfinished document edit, always in screenshot pixels.
enum Gesture {
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
pub(super) enum Handle {
    ArrowTail,
    ArrowHead,
    RectangleCorner { opposite: Point },
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

/// The document's pointer affordance; the window maps it to a native cursor.
pub(super) enum PointerHint {
    Arrow,
    Crosshair,
    Text,
    Move,
    Resize { handle: Handle, position: Point },
}

pub(super) struct Overlay {
    pub live_annotation: Option<Annotation>,
    pub crop: Option<Rect>,
    pub crop_handles: bool,
    pub text: Option<TextOverlay>,
    pub selection: Option<SelectionOverlay>,
}

pub(super) struct SelectionOverlay {
    pub bounds: snip_core::Bounds,
    pub handles: Vec<Point>,
}

pub(super) struct Editing {
    doc: Document,
    history: History,
    renderer: Renderer,
    tool: Tool,
    /// Drawing defaults stay independent of selection and document history.
    ink_color: Color,
    /// Logical pixels; converted to screenshot pixels when drawing or restyling.
    ink_width: f32,
    selected: Option<usize>,
    gesture: Option<Gesture>,
    editing: Option<TextSession>,
    // Only changes to annotation pixels require compositing again. Crop,
    // selection, and caret changes are painted by the window as overlays.
    preview_dirty: bool,
}

impl Editing {
    pub fn new(image: RgbaImage) -> Self {
        let doc = Document::new(image);
        Self {
            history: History::new(&doc),
            doc,
            renderer: Renderer::new(),
            tool: Tool::Pencil,
            ink_color: INK,
            ink_width: 3.,
            selected: None,
            gesture: None,
            editing: None,
            preview_dirty: false,
        }
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }
    pub fn tool(&self) -> Tool {
        self.tool
    }
    pub fn ink_width(&self) -> f32 {
        self.ink_width
    }
    pub fn has_gesture(&self) -> bool {
        self.gesture.is_some()
    }
    pub fn is_cropping(&self) -> bool {
        matches!(self.gesture, Some(Gesture::Crop(_)))
    }
    pub fn is_editing_text(&self) -> bool {
        self.editing.is_some()
    }
    pub fn has_selection(&self) -> bool {
        self.selected.is_some()
    }
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Resolve unfinished input before another document operation. Text is
    /// committed; unfinished pointer edits are restored without an undo step.
    fn settle(&mut self) {
        self.cancel_gesture();
        self.finish_text();
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.settle();
        self.tool = tool;
        if tool != Tool::Select {
            self.selected = None;
        }
    }

    pub fn take_preview(&mut self) -> Option<RgbaImage> {
        if !std::mem::take(&mut self.preview_dirty) {
            return None;
        }
        Some(self.renderer.composite(&self.doc))
    }

    pub fn pointer_down(&mut self, p: Point, view_scale: f32, device_scale: f32, double_click: bool) {
        let (width, height) = self.doc.image.dimensions();
        let inside = (0.0..width as f32).contains(&p.x) && (0.0..height as f32).contains(&p.y);
        // A click inside the label being edited only moves its caret.
        if let Some(index) = self.editing_index()
            && self.text_at(p) == Some(index)
        {
            self.start_text(index, p);
            return;
        }
        self.settle();
        if self.tool != Tool::Select {
            self.selected = None;
        }
        let ink_width = self.ink_width * device_scale;
        match self.tool {
            Tool::Select => {
                if let Some((position, handle)) = self.handle_at(p, view_scale) {
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
                                .annotation_at(&self.doc, p, HIT_TOLERANCE / view_scale)
                        })
                        .flatten();
                    if let Some(index) = self.selected {
                        if double_click && matches!(self.doc.annotations[index], Annotation::Text(_)) {
                            self.start_text(index, p);
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
            Tool::Crop => self.begin_crop(p, view_scale, inside),
            Tool::Text if inside => {
                let index = self.text_at(p).unwrap_or_else(|| {
                    self.doc.annotations.push(Annotation::Text(Text {
                        origin: p,
                        content: String::new(),
                        size: TEXT_SIZE * device_scale,
                        color: self.ink_color,
                    }));
                    self.doc.annotations.len() - 1
                });
                self.start_text(index, p);
            }
            _ => {}
        }
    }

    pub fn update_gesture(&mut self, p: Point, shift: bool, scale: f32) {
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
                    self.preview_dirty = true;
                }
            }
            Some(Gesture::Crop(gesture)) => {
                self.doc.crop = gesture.update(p, self.doc.image.dimensions(), scale);
            }
            _ => {}
        }
    }

    pub fn finish_gesture(&mut self) {
        match self.gesture.take() {
            Some(Gesture::Annotation(annotation)) => {
                if valid_annotation(&annotation) {
                    self.doc.annotations.push(annotation);
                    self.history.commit(&self.doc);
                    self.preview_dirty = true;
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
                    self.preview_dirty = true;
                } else {
                    self.history.commit(&self.doc);
                }
            }
            None => {}
        }
    }

    pub fn cancel_gesture(&mut self) {
        match self.gesture.take() {
            Some(Gesture::Crop(gesture)) => self.doc.crop = gesture.previous(),
            Some(Gesture::Edit { index, original, .. }) if self.doc.annotations[index] != original => {
                self.doc.annotations[index] = original;
                self.preview_dirty = true;
            }
            _ => {}
        }
    }

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

    pub fn pointer_hint(&mut self, p: Point, scale: f32) -> PointerHint {
        if self.editing.is_some() {
            PointerHint::Text
        } else if self.tool == Tool::Crop {
            self.crop_hint(p, scale)
        } else if self.tool != Tool::Select {
            PointerHint::Crosshair
        } else {
            match &self.gesture {
                Some(Gesture::Edit {
                    operation: EditOperation::Move,
                    ..
                }) => PointerHint::Move,
                Some(Gesture::Edit {
                    operation: EditOperation::Resize { handle, .. },
                    ..
                }) => PointerHint::Resize {
                    handle: *handle,
                    position: p,
                },
                _ => {
                    if let Some((position, handle)) = self.handle_at(p, scale) {
                        PointerHint::Resize { handle, position }
                    } else {
                        let (w, h) = self.doc.image.dimensions();
                        let inside = (0.0..w as f32).contains(&p.x) && (0.0..h as f32).contains(&p.y);
                        if inside
                            && self
                                .renderer
                                .annotation_at(&self.doc, p, HIT_TOLERANCE / scale)
                                .is_some()
                        {
                            PointerHint::Move
                        } else {
                            PointerHint::Arrow
                        }
                    }
                }
            }
        }
    }

    pub fn restore_history(&mut self, redo: bool) {
        self.settle();
        // Finishing changed text commits it and clears redo, just like any
        // new edit. An unchanged session preserves the redo branch.
        let changed = if redo {
            self.history.redo(&mut self.doc)
        } else {
            self.history.undo(&mut self.doc)
        };
        if changed {
            self.preview_dirty = true;
        }
        self.selected = None;
    }

    pub fn overlay(&mut self) -> Overlay {
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
        Overlay {
            live_annotation,
            crop: self.doc.crop,
            crop_handles: self.tool == Tool::Crop,
            text,
            selection,
        }
    }

    pub fn displayed_style(&self, device_scale: f32) -> (Color, Option<f32>) {
        let annotation = self
            .editing_index()
            .or(self.selected)
            .and_then(|i| self.doc.annotations.get(i));
        match annotation {
            Some(Annotation::Stroke(a)) => (a.color, Some(a.width / device_scale)),
            Some(Annotation::Arrow(a)) => (a.color, Some(a.width / device_scale)),
            Some(Annotation::Rectangle(a)) => (a.color, Some(a.width / device_scale)),
            Some(Annotation::Text(a)) => (a.color, None),
            None => (
                self.ink_color,
                (self.tool != Tool::Text).then_some(self.ink_width),
            ),
        }
    }

    fn prepare_style_change(&mut self) {
        self.cancel_gesture();
        if let Some(label) = self.finish_text() {
            self.selected = Some(label);
        }
    }

    pub fn set_color(&mut self, color: Color) {
        self.prepare_style_change();
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
                self.preview_dirty = true;
            }
        }
    }

    pub fn set_width(&mut self, width: f32, device_scale: f32) {
        self.prepare_style_change();
        self.ink_width = width;
        if let Some(annotation) = self.selected.and_then(|i| self.doc.annotations.get_mut(i)) {
            let current = match annotation {
                Annotation::Stroke(a) => &mut a.width,
                Annotation::Arrow(a) => &mut a.width,
                Annotation::Rectangle(a) => &mut a.width,
                Annotation::Text(_) => return,
            };
            let image_width = width * device_scale;
            if *current != image_width {
                *current = image_width;
                self.history.commit(&self.doc);
                self.preview_dirty = true;
            }
        }
    }

    pub fn delete_selected(&mut self) {
        self.cancel_gesture();
        if let Some(index) = self.selected.take() {
            self.doc.annotations.remove(index);
            self.history.commit(&self.doc);
            self.preview_dirty = true;
        }
    }

    pub fn clear_crop(&mut self) {
        self.settle();
        self.doc.crop = None;
        self.history.commit(&self.doc);
    }

    pub fn export(&mut self) -> RgbaImage {
        self.settle();
        self.renderer.export(&self.doc)
    }
}
