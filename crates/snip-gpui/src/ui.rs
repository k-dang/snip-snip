//! Colors and controls shared by the Settings and Library windows. The editor
//! keeps its own palette but uses `line` for text tests can find.

use gpui::{Div, ElementId, SharedString, Stateful, div, prelude::*, rgb};

pub const SURFACE: u32 = 0x20242a;
pub const TEXT: u32 = 0xf1f3f5;
pub const MUTED: u32 = 0x9aa3ad;
pub const BORDER: u32 = 0x58616e;
pub const FOCUS: u32 = 0x71b7ff;
pub const ERROR: u32 = 0xffb8a8;
pub const SUCCESS: u32 = 0xa8d9c0;

/// A line of text that tests can find by its content.
pub fn line(text: impl Into<SharedString>) -> Div {
    let text = text.into();
    div()
        .debug_selector({
            let text = text.clone();
            move || text.to_string()
        })
        .child(text)
}

/// A bordered button that tests can find by its label; callers add `on_click`.
pub fn button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(ElementId::Name(id.into()))
        .debug_selector({
            let label = label.to_string();
            move || label
        })
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(BORDER))
        .cursor_pointer()
        .child(label)
}
