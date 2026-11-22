//! The editor draws its own title bar: a drag strip across the top of the
//! canvas and, on Windows, the caption buttons. macOS draws its native
//! traffic lights over the strip.

use gpui::{App, Div, IntoElement, MouseButton, Window, WindowControlArea, div, prelude::*, px, rgb};

use super::theme::{HOVER, TEXT};

pub(super) const HEIGHT: f32 = 32.;

pub(super) fn render(window: &mut Window, cx: &mut App) -> impl IntoElement {
    // macOS: a press on the strip arms the move and the first drag starts it,
    // so double-clicks still arrive and drags from the canvas never move the window.
    let armed = cfg!(target_os = "macos").then(|| window.use_keyed_state("title-bar-drag", cx, |_, _| false));
    div()
        .id("editor-title-bar")
        // Presses here must not draw on or pan an image under the strip.
        .occlude()
        // Windows moves the window through its own hit test.
        .window_control_area(WindowControlArea::Drag)
        .when_some(armed, |bar, armed| {
            let (down, out, up, moved) = (armed.clone(), armed.clone(), armed.clone(), armed);
            bar.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                down.update(cx, |armed, _| *armed = true);
            })
            .on_mouse_down_out(move |_, _, cx| {
                out.update(cx, |armed, _| *armed = false);
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                up.update(cx, |armed, _| *armed = false);
            })
            .on_mouse_move(move |event, window, cx| {
                if event.pressed_button == Some(MouseButton::Left) && *moved.read(cx) {
                    moved.update(cx, |armed, _| *armed = false);
                    window.start_window_move();
                }
            })
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
        })
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .justify_end()
        .when(cfg!(windows), |bar| bar.child(captions(window)))
}

/// Windows caption buttons. The OS handles their clicks, hover-to-snap
/// layouts included, through the window's hit test.
fn captions(window: &Window) -> Div {
    let button = |id: &'static str, glyph: &'static str, area: WindowControlArea| {
        div()
            .id(id)
            // Blocks the drag strip underneath from claiming the hit test.
            .occlude()
            .window_control_area(area)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            // Ships with Windows 10 and 11; same glyphs as Segoe Fluent Icons.
            .font_family("Segoe MDL2 Assets")
            .text_size(px(10.))
            .text_color(rgb(TEXT))
            .map(|b| match area {
                WindowControlArea::Close => b.hover(|s| s.bg(rgb(0xC42B1C)).text_color(rgb(0xFFFFFF))),
                _ => b.hover(|s| s.bg(rgb(HOVER))),
            })
            .child(glyph)
    };
    div()
        .flex()
        .h_full()
        .flex_shrink_0()
        .child(button("caption-minimize", "\u{E921}", WindowControlArea::Min))
        .child(button(
            "caption-maximize",
            if window.is_maximized() {
                "\u{E923}"
            } else {
                "\u{E922}"
            },
            WindowControlArea::Max,
        ))
        .child(button("caption-close", "\u{E8BB}", WindowControlArea::Close))
}
