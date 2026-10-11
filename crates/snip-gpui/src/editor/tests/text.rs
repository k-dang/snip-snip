//! Text labels: multi-line editing, the caret, and OS text input.

use gpui::EntityInputHandler;

use crate::editor::Editor;

use super::*;

#[gpui::test]
fn double_click_reedits_text_at_the_click_without_moving_or_creating_a_label(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("t");
    click(&mut cx, 30.0, 120.0);
    cx.simulate_input("Hello");
    cx.simulate_keystrokes("escape v");
    click(&mut cx, 31.0, 125.0);
    let position = image_point(&mut cx, 31.0, 125.0);
    cx.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 2,
        ..Default::default()
    });
    cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
    cx.simulate_input("var");
    cx.simulate_keystrokes("backspace escape");
    assert_image(saved(&mut cx), &expected(vec![label("vaHello")]));
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &expected(vec![label("Hello")]));
}

fn label(content: &str) -> Annotation {
    Annotation::Text(Text {
        origin: Point::new(30.0, 120.0),
        content: content.into(),
        size: 20.0,
        color: INK,
    })
}

#[gpui::test]
fn enter_starts_a_new_line_and_escape_finishes_the_label(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("t");
    click(&mut cx, 30.0, 120.0);
    cx.simulate_input("Hi");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("there");
    cx.simulate_keystrokes("escape");
    assert_image(saved(&mut cx), &expected(vec![label("Hi\nthere")]));
}

#[gpui::test]
fn the_caret_moves_between_characters_and_lines_and_edits_where_it_is(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("t");
    click(&mut cx, 30.0, 120.0);
    cx.simulate_input("Hlo");
    cx.simulate_keystrokes("left left");
    cx.simulate_input("el");
    cx.simulate_keystrokes("end enter");
    cx.simulate_input("wrld");
    cx.simulate_keystrokes("home right");
    cx.simulate_input("o");
    // Up from a line start reaches the line above's start; Down on the
    // last line reaches the end of the label.
    cx.simulate_keystrokes("home up");
    cx.simulate_input("¡");
    cx.simulate_keystrokes("down down");
    cx.simulate_input("!");
    cx.simulate_keystrokes("up end");
    cx.simulate_input("?");
    cx.simulate_keystrokes("escape");
    assert_image(saved(&mut cx), &expected(vec![label("¡Hello?\nworld!")]));

    // Backspace at a line start joins lines; Delete removes forward.
    cx.simulate_keystrokes("t");
    click(&mut cx, 31.0, 121.0);
    cx.simulate_keystrokes("down home backspace delete escape");
    assert_image(saved(&mut cx), &expected(vec![label("¡Hello?orld!")]));
}

#[gpui::test]
fn clicks_place_the_caret_and_keep_one_undo_step_per_session(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    let width = Renderer::new().text_size(&Text {
        origin: Point::new(0.0, 0.0),
        content: "¡Hello".into(),
        size: 20.0,
        color: INK,
    });
    cx.simulate_keystrokes("t");
    click(&mut cx, 30.0, 120.0);
    cx.simulate_input("Hello");
    // Clicking inside the edited label moves the caret without finishing.
    click(&mut cx, 31.0, 130.0);
    cx.simulate_input("¡");
    click(&mut cx, 30.0 + width.0 - 1.0, 130.0);
    cx.simulate_input("!");
    cx.simulate_keystrokes("escape");
    assert_image(saved(&mut cx), &expected(vec![label("¡Hello!")]));
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &expected(vec![]));
    command(&mut cx, "shift-z");

    // Re-editing with the Text tool starts at the clicked caret.
    click(&mut cx, 31.0, 130.0);
    cx.simulate_input("A");
    cx.simulate_keystrokes("escape");
    assert_image(saved(&mut cx), &expected(vec![label("A¡Hello!")]));
}

/// Calls the editor's input handler the way the OS text input system does.
pub(super) fn ime<R>(
    cx: &mut VisualTestContext,
    f: impl FnOnce(&mut Editor, &mut gpui::Window, &mut gpui::Context<Editor>) -> R,
) -> R {
    let editor = cx.update(|window, _| window.root::<Editor>().flatten().expect("editor window"));
    let result = cx.update(|window, cx| editor.update(cx, |editor, cx| f(editor, window, cx)));
    cx.run_until_parked();
    result
}

#[gpui::test]
fn ime_composition_shows_inline_and_commits_or_keeps_its_text(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("t");
    click(&mut cx, 30.0, 120.0);
    cx.simulate_input("A");
    ime(&mut cx, |e, w, cx| {
        e.replace_and_mark_text_in_range(None, "k", Some(1..1), w, cx)
    });
    ime(&mut cx, |e, w, cx| {
        e.replace_and_mark_text_in_range(None, "か", Some(1..1), w, cx)
    });
    assert_eq!(ime(&mut cx, |e, w, cx| e.marked_text_range(w, cx)), Some(1..2));
    assert_eq!(
        ime(&mut cx, |e, w, cx| e.text_for_range(0..2, &mut None, w, cx)),
        Some("Aか".into())
    );
    ime(&mut cx, |e, w, cx| e.replace_text_in_range(None, "漢", w, cx));
    assert_eq!(ime(&mut cx, |e, w, cx| e.marked_text_range(w, cx)), None);
    // A dead key composes, then the next key replaces it with the result.
    ime(&mut cx, |e, w, cx| {
        e.replace_and_mark_text_in_range(None, "´", None, w, cx)
    });
    ime(&mut cx, |e, w, cx| e.replace_text_in_range(None, "é", w, cx));
    // Clicking away mid-composition keeps the composing text.
    ime(&mut cx, |e, w, cx| {
        e.replace_and_mark_text_in_range(None, "x", None, w, cx)
    });
    click(&mut cx, 200.0, 40.0);
    cx.simulate_keystrokes("escape");
    assert_image(saved(&mut cx), &expected(vec![label("A漢éx")]));
}

#[gpui::test]
fn the_ime_window_is_anchored_at_the_caret_on_high_dpi_and_fitted_layouts(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    for (scale, window) in [
        (2.0, size(px(900.0), px(600.0))),
        (1.0, size(px(480.0), px(260.0))),
    ] {
        cx.simulate_scale_factor_change(scale);
        cx.simulate_resize(window);
        cx.run_until_parked();
        cx.simulate_keystrokes("t");
        click(&mut cx, 30.0, 120.0);
        cx.simulate_input("Hi");
        let text = Text {
            origin: Point::new(0.0, 0.0),
            content: "Hi".into(),
            size: 20.0 * scale,
            color: INK,
        };
        let (width, height) = Renderer::new().text_size(&text);
        let bounds = ime(&mut cx, |e, w, cx| {
            e.bounds_for_range(2..2, Default::default(), w, cx)
        })
        .expect("caret bounds");
        let top = image_point(&mut cx, 30.0 + width, 120.0);
        let bottom = image_point(&mut cx, 30.0 + width, 120.0 + height);
        assert!(
            (bounds.origin.x - top.x).abs() < px(0.01),
            "{bounds:?} at {top:?}"
        );
        assert!(
            (bounds.origin.y - top.y).abs() < px(0.01),
            "{bounds:?} at {top:?}"
        );
        assert!(
            (bounds.bottom() - bottom.y).abs() < px(0.01),
            "{bounds:?} to {bottom:?}"
        );
        cx.simulate_keystrokes("escape");
        command(&mut cx, "z");
    }
}

#[gpui::test]
fn a_blank_label_is_dropped_without_an_undo_step(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    cx.simulate_keystrokes("t");
    click(&mut cx, 30.0, 120.0);
    cx.simulate_input(" ");
    cx.simulate_keystrokes("enter escape");
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((40.0, 40.0), (100.0, 90.0))]),
    );
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &expected(vec![]));
}

#[gpui::test]
fn up_and_down_keep_the_caret_column_across_short_lines(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("t");
    click(&mut cx, 30.0, 120.0);
    cx.simulate_input("Hello");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("Hi");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("Hello");
    cx.simulate_keystrokes("up up");
    cx.simulate_input("!");
    cx.simulate_keystrokes("escape");
    assert_image(saved(&mut cx), &expected(vec![label("Hello!\nHi\nHello")]));
}
