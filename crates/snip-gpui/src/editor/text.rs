//! Text editing sessions: the caret, keyboard editing, and OS text input.
//! Typed text, IME composition, and dead keys arrive through GPUI's input
//! handler, which the canvas registers only while a label is being edited.

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, UTF16Selection, Window, px, size};
use snip_core::{Annotation, Color, Document, Point, Renderer, Text};
use unicode_segmentation::UnicodeSegmentation;

use super::Editor;

/// The text annotation receiving keystrokes. Offsets are bytes into its
/// content, on grapheme boundaries.
pub(super) struct TextSession {
    index: usize,
    caret: usize,
    /// Text the IME is still composing, kept in the content so it renders.
    marked: Option<Range<usize>>,
    /// The image x that consecutive Up/Down moves aim for.
    goal_x: Option<f32>,
}

/// The caret and IME composition underline, in image pixels.
pub(super) struct TextOverlay {
    /// Caret top and height.
    pub(super) caret: (Point, f32),
    /// Underline start and width, under the text being composed.
    pub(super) composition: Option<(Point, f32)>,
    pub(super) color: Color,
}

/// Keyboard edits that don't insert text; typed text uses the input handler.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TextEdit {
    DeleteBackward,
    DeleteForward,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

fn as_text(annotation: &Annotation) -> &Text {
    match annotation {
        Annotation::Text(text) => text,
        _ => unreachable!("only text annotations are edited"),
    }
}

fn as_text_mut(annotation: &mut Annotation) -> &mut Text {
    match annotation {
        Annotation::Text(text) => text,
        _ => unreachable!("only text annotations are edited"),
    }
}

/// The edited label and its session, if text is being edited. Takes the
/// fields rather than the editor so the renderer stays borrowable.
fn edited_text<'a>(
    editing: &'a mut Option<TextSession>,
    doc: &'a mut Document,
) -> Option<(&'a mut TextSession, &'a mut Text)> {
    let session = editing.as_mut()?;
    let text = as_text_mut(&mut doc.annotations[session.index]);
    Some((session, text))
}

/// Replaces `range` (UTF-16), or else the composition, or else inserts at the
/// caret, leaving the caret after the new text. Line endings become `\n` and
/// other control characters are dropped. Returns the new text's byte range.
fn splice(
    session: &mut TextSession,
    text: &mut Text,
    range: Option<Range<usize>>,
    new_text: &str,
) -> Range<usize> {
    let new_text: String = new_text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|&c| c == '\n' || !c.is_control())
        .collect();
    let range = range
        .map(|r| from_utf16(&text.content, &r))
        .or(session.marked.take())
        .unwrap_or(session.caret..session.caret);
    text.content.replace_range(range.clone(), &new_text);
    session.caret = range.start + new_text.len();
    session.goal_x = None;
    range.start..session.caret
}

/// The top-left, width, and height of `range`'s carets in image pixels.
/// Compositions are a single line; a range spanning lines collapses to its
/// start.
fn caret_span(renderer: &mut Renderer, text: &Text, range: Range<usize>) -> (Point, f32, f32) {
    let (start, height) = renderer.text_caret(text, range.start);
    let (end, _) = renderer.text_caret(text, range.end);
    if end.y == start.y {
        (
            Point::new(start.x.min(end.x), start.y),
            (end.x - start.x).abs(),
            height,
        )
    } else {
        (start, 0.0, height)
    }
}

impl Editor {
    pub(super) fn editing_index(&self) -> Option<usize> {
        self.editing.as_ref().map(|session| session.index)
    }

    /// Starts editing the text annotation at `index`, with the caret nearest
    /// `p`. Restarting the current session just moves its caret.
    pub(super) fn start_text(&mut self, index: usize, p: Point, window: &mut Window) {
        let caret = self
            .renderer
            .text_index_at(as_text(&self.doc.annotations[index]), p);
        self.editing = Some(TextSession {
            index,
            caret,
            marked: None,
            goal_x: None,
        });
        window.invalidate_character_coordinates();
    }

    /// The topmost text annotation under `p`, if any.
    pub(super) fn text_at(&mut self, p: Point) -> Option<usize> {
        let renderer = &mut self.renderer;
        self.doc.annotations.iter().rposition(|a| match a {
            Annotation::Text(t) => renderer.text_box(t).contains(p),
            _ => false,
        })
    }

    /// Handles a key that edits the session without typing a character, and
    /// returns whether it did. Typed characters arrive through the input
    /// handler instead.
    pub(super) fn text_key_down(&mut self, key: &str, window: &mut Window) -> bool {
        let edit = match key {
            "escape" => {
                self.finish_text(window);
                return true;
            }
            // Enter commits an IME composition, or else starts a new line.
            "enter" => {
                match self.editing.as_mut() {
                    Some(session) if session.marked.is_some() => session.marked = None,
                    _ => self.replace_text(None, "\n", window),
                }
                return true;
            }
            "backspace" => TextEdit::DeleteBackward,
            "delete" => TextEdit::DeleteForward,
            "left" => TextEdit::Left,
            "right" => TextEdit::Right,
            "up" => TextEdit::Up,
            "down" => TextEdit::Down,
            "home" => TextEdit::Home,
            "end" => TextEdit::End,
            _ => return false,
        };
        self.edit_text(edit, window);
        true
    }

    /// Applies a caret movement or deletion to the edited label. Either one
    /// ends an IME composition, keeping the text composed so far.
    fn edit_text(&mut self, edit: TextEdit, window: &mut Window) {
        let renderer = &mut self.renderer;
        let Some((session, text)) = edited_text(&mut self.editing, &mut self.doc) else {
            return;
        };
        let content = &text.content;
        let caret = session.caret;
        let previous = content[..caret]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i);
        let next = content[caret..]
            .graphemes(true)
            .next()
            .map_or(caret, |g| caret + g.len());
        let unchanged = caret..caret;
        let (moved, removed, goal_x) = match edit {
            TextEdit::DeleteBackward => (previous, previous..caret, None),
            TextEdit::DeleteForward => (caret, caret..next, None),
            TextEdit::Left => (previous, unchanged, None),
            TextEdit::Right => (next, unchanged, None),
            TextEdit::Home => (content[..caret].rfind('\n').map_or(0, |i| i + 1), unchanged, None),
            TextEdit::End => (
                content[caret..].find('\n').map_or(content.len(), |i| caret + i),
                unchanged,
                None,
            ),
            // Aim at the middle of the adjacent line; past the first or last
            // line that lands at the label's start or end.
            TextEdit::Up | TextEdit::Down => {
                let (top, height) = renderer.text_caret(text, caret);
                let x = session.goal_x.unwrap_or(top.x);
                let rows = if edit == TextEdit::Down { 1.5 } else { -0.5 };
                let target = renderer.text_index_at(text, Point::new(x, top.y + rows * height));
                (target, unchanged, Some(x))
            }
        };
        session.caret = moved;
        session.marked = None;
        session.goal_x = goal_x;
        if !removed.is_empty() {
            text.content.replace_range(removed, "");
            self.rerender(window);
        }
        window.invalidate_character_coordinates();
    }

    /// Splices `new_text` into the edited label and shows the result.
    fn replace_text(&mut self, range: Option<Range<usize>>, new_text: &str, window: &mut Window) {
        if let Some((session, text)) = edited_text(&mut self.editing, &mut self.doc) {
            splice(session, text, range, new_text);
            self.rerender(window);
            window.invalidate_character_coordinates();
        }
    }

    /// Ends text editing, committing any IME composition and dropping the
    /// label if it was left blank. Returns the label's index if it was kept.
    pub(super) fn finish_text(&mut self, window: &mut Window) -> Option<usize> {
        let index = self.editing.take()?.index;
        let kept = !as_text(&self.doc.annotations[index]).content.trim().is_empty();
        if !kept {
            self.doc.annotations.remove(index);
            self.selected = None;
            self.rerender(window);
        }
        self.history.commit(&self.doc);
        self.update_cursor(window);
        kept.then_some(index)
    }

    /// What the canvas paints for the edited label.
    pub(super) fn text_overlay(&mut self) -> Option<TextOverlay> {
        let renderer = &mut self.renderer;
        let (session, text) = edited_text(&mut self.editing, &mut self.doc)?;
        let composition = session.marked.clone().map(|range| {
            let (top_left, width, height) = caret_span(renderer, text, range);
            (Point::new(top_left.x, top_left.y + height), width)
        });
        Some(TextOverlay {
            caret: renderer.text_caret(text, session.caret),
            composition,
            color: text.color,
        })
    }
}

/// The OS's view of the edited label. Ranges cross this boundary in UTF-16
/// code units and are converted to byte offsets at once.
impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let (_, text) = edited_text(&mut self.editing, &mut self.doc)?;
        let range = from_utf16(&text.content, &range);
        *adjusted_range = Some(to_utf16(&text.content, &range));
        Some(text.content[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let (session, text) = edited_text(&mut self.editing, &mut self.doc)?;
        let caret = utf16_offset(&text.content, session.caret);
        Some(UTF16Selection {
            range: caret..caret,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let session = self.editing.as_ref()?;
        let text = as_text(&self.doc.annotations[session.index]);
        Some(to_utf16(&text.content, session.marked.as_ref()?))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(session) = self.editing.as_mut() {
            session.marked = None;
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text(range, text, window);
        cx.notify();
    }

    /// Inserts composing text; `selection` (UTF-16, within `text`) places the
    /// caret inside it.
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((session, label)) = edited_text(&mut self.editing, &mut self.doc) else {
            return;
        };
        let inserted = splice(session, label, range, text);
        if !inserted.is_empty() {
            if let Some(selection) = selection {
                session.caret = inserted.start + from_utf16(&label.content[inserted.clone()], &selection).end;
            }
            session.marked = Some(inserted);
        }
        self.rerender(window);
        window.invalidate_character_coordinates();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let viewport = self.viewport.get()?;
        let (_, text) = edited_text(&mut self.editing, &mut self.doc)?;
        let range = from_utf16(&text.content, &range);
        let (top_left, width, height) = caret_span(&mut self.renderer, text, range);
        Some(Bounds::new(
            viewport.to_window(top_left),
            size(px(width * viewport.scale), px(height * viewport.scale)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let viewport = self.viewport.get()?;
        let (_, text) = edited_text(&mut self.editing, &mut self.doc)?;
        let caret = self.renderer.text_index_at(text, viewport.to_image(point));
        Some(utf16_offset(&text.content, caret))
    }
}

/// Converts a UTF-16 range in `s` to bytes, clamped to character boundaries.
fn from_utf16(s: &str, range: &Range<usize>) -> Range<usize> {
    let byte = |target: usize| {
        let mut units = 0;
        for (i, c) in s.char_indices() {
            if units >= target {
                return i;
            }
            units += c.len_utf16();
        }
        s.len()
    };
    byte(range.start)..byte(range.end)
}

/// The UTF-16 offset of byte `byte` in `s`.
fn utf16_offset(s: &str, byte: usize) -> usize {
    s[..byte].encode_utf16().count()
}

fn to_utf16(s: &str, range: &Range<usize>) -> Range<usize> {
    utf16_offset(s, range.start)..utf16_offset(s, range.end)
}
