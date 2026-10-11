//! Drive the rendered editor through GPUI events and observe saved PNGs.
//! Only the OS file picker is simulated; no private editor state is inspected.

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, Pixels, TestAppContext, VisualTestContext, point, px, size,
};
use image::Rgba;
use snip_core::{Annotation, Arrow, Color, Document, Point, Rectangle, Renderer, RgbaImage, Stroke, Text};

const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
const INK: Color = Color::rgb(0xE5, 0x48, 0x4D);

fn editor(cx: &mut TestAppContext) -> VisualTestContext {
    let window = cx.update(|cx| {
        super::open(RgbaImage::from_pixel(320, 200, WHITE), 1.0, cx);
        cx.windows()[0]
    });
    let cx = VisualTestContext::from_window(window, cx);
    cx.simulate_scale_factor_change(1.0);
    cx.run_until_parked();
    cx
}

// Locate fixture pixels in the public rendered canvas at its current fit/scale.
fn image_point(cx: &mut VisualTestContext, x: f32, y: f32) -> gpui::Point<Pixels> {
    let bounds = cx.debug_bounds("editor-canvas").expect("rendered canvas");
    let device_scale = cx.update(|w, _| w.scale_factor());
    let scale = ((f32::from(bounds.size.width) - 32.0) / 320.0)
        .min((f32::from(bounds.size.height) - 32.0) / 200.0)
        .min(1.0 / device_scale);
    bounds.center() + point(px((x - 160.0) * scale), px((y - 100.0) * scale))
}

fn drag(cx: &mut VisualTestContext, from: (f32, f32), to: (f32, f32), modifiers: Modifiers) {
    let from = image_point(cx, from.0, from.1);
    let to = image_point(cx, to.0, to.1);
    cx.simulate_mouse_move(from, None, modifiers);
    cx.simulate_mouse_down(from, MouseButton::Left, modifiers);
    cx.simulate_mouse_move(to, MouseButton::Left, modifiers);
    cx.simulate_mouse_up(to, MouseButton::Left, modifiers);
}

fn click(cx: &mut VisualTestContext, x: f32, y: f32) {
    let position = image_point(cx, x, y);
    cx.simulate_mouse_move(position, None, Modifiers::none());
    cx.simulate_click(position, Modifiers::none());
}

fn command(cx: &mut VisualTestContext, key: &str) {
    let modifier = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    cx.simulate_keystrokes(&format!("{modifier}-{key}"));
}

fn saved(cx: &mut VisualTestContext) -> RgbaImage {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("selection.png");
    command(cx, "s");
    cx.simulate_new_path_selection(|_| Some(path.clone()));
    cx.run_until_parked();
    image::open(path).expect("saved PNG").into_rgba8()
}

fn expected(annotations: Vec<Annotation>) -> RgbaImage {
    let mut doc = Document::new(RgbaImage::from_pixel(320, 200, WHITE));
    doc.annotations = annotations;
    Renderer::new().export(&doc)
}

#[track_caller]
fn assert_image(actual: RgbaImage, expected: &RgbaImage) {
    assert_eq!(actual.dimensions(), expected.dimensions());
    for (x, y, pixel) in actual.enumerate_pixels() {
        assert_eq!(pixel, expected.get_pixel(x, y), "exported pixel ({x}, {y})");
    }
}

fn rectangle(start: (f32, f32), end: (f32, f32)) -> Annotation {
    Annotation::Rectangle(Rectangle {
        start: Point::new(start.0, start.1),
        end: Point::new(end.0, end.1),
        color: INK,
        width: 3.0,
    })
}

fn arrow(tail: (f32, f32), head: (f32, f32)) -> Annotation {
    Annotation::Arrow(Arrow {
        tail: Point::new(tail.0, tail.1),
        head: Point::new(head.0, head.1),
        color: INK,
        width: 3.0,
    })
}

#[gpui::test]
fn selecting_moving_and_deleting_use_one_undo_step_each(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    let original = expected(vec![rectangle((40.0, 40.0), (100.0, 90.0))]);
    assert_image(saved(&mut cx), &original);

    let select = cx.debug_bounds("Select").expect("Select toolbar button").center();
    cx.simulate_mouse_move(select, None, Modifiers::none());
    cx.simulate_click(select, Modifiers::none());
    drag(&mut cx, (70.0, 40.0), (90.0, 60.0), Modifiers::none());
    let moved = expected(vec![rectangle((60.0, 60.0), (120.0, 110.0))]);
    assert_image(saved(&mut cx), &moved);

    command(&mut cx, "z");
    assert_image(saved(&mut cx), &original);
    command(&mut cx, "shift-z");
    assert_image(saved(&mut cx), &moved);

    click(&mut cx, 90.0, 60.0);
    cx.simulate_keystrokes("delete");
    assert_image(saved(&mut cx), &expected(vec![]));
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &moved);
}

#[gpui::test]
fn escape_cancels_a_drag_while_shift_is_held(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 70.0, 40.0);
    let from = image_point(&mut cx, 100.0, 90.0);
    let to = image_point(&mut cx, 130.0, 110.0);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::none()
    };
    cx.simulate_mouse_move(from, None, shift);
    cx.simulate_mouse_down(from, MouseButton::Left, shift);
    cx.simulate_mouse_move(to, MouseButton::Left, shift);
    cx.simulate_keystrokes("shift-escape");
    cx.simulate_mouse_up(to, MouseButton::Left, shift);
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((40.0, 40.0), (100.0, 90.0))]),
    );
}

#[gpui::test]
fn a_corner_drag_returning_to_its_start_preserves_redo(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    drag(&mut cx, (70.0, 40.0), (90.0, 60.0), Modifiers::none());
    command(&mut cx, "z");
    click(&mut cx, 70.0, 40.0);

    let corner = image_point(&mut cx, 40.0, 40.0);
    let away = image_point(&mut cx, 20.0, 20.0);
    cx.simulate_mouse_move(corner, None, Modifiers::none());
    cx.simulate_mouse_down(corner, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(away, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(corner, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(corner, MouseButton::Left, Modifiers::none());
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((40.0, 40.0), (100.0, 90.0))]),
    );
    command(&mut cx, "shift-z");
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((60.0, 60.0), (120.0, 110.0))]),
    );
}

#[gpui::test]
fn an_unchanged_shift_arrow_resize_preserves_redo(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("a");
    drag(&mut cx, (40.0, 40.0), (40.0, 100.0), Modifiers::none());
    cx.simulate_keystrokes("r");
    drag(&mut cx, (200.0, 120.0), (260.0, 160.0), Modifiers::none());
    command(&mut cx, "z");
    cx.simulate_keystrokes("v");
    click(&mut cx, 40.0, 70.0);

    let head = image_point(&mut cx, 40.0, 100.0);
    let away = image_point(&mut cx, 60.0, 120.0);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::none()
    };
    cx.simulate_mouse_move(head, None, shift);
    cx.simulate_mouse_down(head, MouseButton::Left, shift);
    cx.simulate_mouse_move(away, MouseButton::Left, shift);
    cx.simulate_mouse_move(head, MouseButton::Left, shift);
    cx.simulate_mouse_up(head, MouseButton::Left, shift);

    command(&mut cx, "shift-z");
    assert_image(
        saved(&mut cx),
        &expected(vec![
            arrow((40.0, 40.0), (40.0, 100.0)),
            rectangle((200.0, 120.0), (260.0, 160.0)),
        ]),
    );
    command(&mut cx, "z");
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &expected(vec![]));
}

#[gpui::test]
fn rectangle_handles_keep_the_opposite_corner_fixed_after_crossing(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    for (corner, crossed, opposite) in [
        ((40.0, 40.0), (120.0, 110.0), (100.0, 90.0)),
        ((100.0, 40.0), (20.0, 110.0), (40.0, 90.0)),
        ((100.0, 90.0), (20.0, 20.0), (40.0, 40.0)),
        ((40.0, 90.0), (120.0, 20.0), (100.0, 40.0)),
    ] {
        click(&mut cx, 70.0, 40.0);
        drag(&mut cx, corner, crossed, Modifiers::none());
        assert_image(saved(&mut cx), &expected(vec![rectangle(opposite, crossed)]));
        command(&mut cx, "z");
    }

    click(&mut cx, 70.0, 40.0);
    drag(&mut cx, (100.0, 90.0), (100.0, 40.0), Modifiers::none());
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((40.0, 40.0), (100.0, 90.0))]),
    );
}

#[gpui::test]
fn arrow_endpoints_and_shift_constraints_use_the_fixed_endpoint(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("a");
    let shift = Modifiers {
        shift: true,
        ..Modifiers::none()
    };
    drag(&mut cx, (40.0, 40.0), (64.0, 47.0), shift);
    assert_image(saved(&mut cx), &expected(vec![arrow((40.0, 40.0), (65.0, 40.0))]));

    cx.simulate_keystrokes("v");
    click(&mut cx, 50.0, 40.0);
    drag(&mut cx, (40.0, 40.0), (65.0, 65.0), shift);
    assert_image(saved(&mut cx), &expected(vec![arrow((65.0, 65.0), (65.0, 40.0))]));

    drag(&mut cx, (65.0, 40.0), (90.0, 65.0), Modifiers::none());
    assert_image(saved(&mut cx), &expected(vec![arrow((65.0, 65.0), (90.0, 65.0))]));
    drag(&mut cx, (90.0, 65.0), (65.0, 65.0), Modifiers::none());
    assert_image(saved(&mut cx), &expected(vec![arrow((65.0, 65.0), (90.0, 65.0))]));
}

#[gpui::test]
fn double_click_reedits_text_without_moving_or_creating_a_label(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("t");
    click(&mut cx, 30.0, 120.0);
    cx.simulate_input("Hello");
    cx.simulate_keystrokes("enter v");
    click(&mut cx, 50.0, 125.0);
    let position = image_point(&mut cx, 50.0, 125.0);
    cx.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 2,
        ..Default::default()
    });
    cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
    cx.simulate_input("var");
    cx.simulate_keystrokes("backspace escape");
    let label = |content: &str| {
        Annotation::Text(Text {
            origin: Point::new(30.0, 120.0),
            content: content.into(),
            size: 20.0,
            color: INK,
        })
    };
    assert_image(saved(&mut cx), &expected(vec![label("Hellova")]));
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &expected(vec![label("Hello")]));
}

#[gpui::test]
fn handles_keep_the_grab_offset_on_high_dpi_and_fitted_images(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 70.0, 40.0);
    cx.simulate_scale_factor_change(2.0);
    cx.run_until_parked();
    drag(&mut cx, (107.0, 90.0), (147.0, 110.0), Modifiers::none());
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((40.0, 40.0), (140.0, 110.0))]),
    );

    command(&mut cx, "z");
    cx.simulate_scale_factor_change(1.0);
    cx.simulate_resize(size(px(480.0), px(260.0)));
    cx.run_until_parked();
    click(&mut cx, 70.0, 40.0);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::none()
    };
    drag(&mut cx, (100.0, 90.0), (130.0, 110.0), shift);
    // At the reduced fit, float conversion can move geometry by a subpixel.
    let image = saved(&mut cx);
    assert_eq!(*image.get_pixel(40, 85), Rgba([0xE5, 0x48, 0x4D, 255]));
    assert_eq!(*image.get_pixel(130, 85), Rgba([0xE5, 0x48, 0x4D, 255]));
    assert_eq!(*image.get_pixel(85, 130), Rgba([0xE5, 0x48, 0x4D, 255]));
    assert_eq!(*image.get_pixel(85, 110), WHITE);
}

#[gpui::test]
fn unfinished_moves_cancel_on_export_and_tool_change_and_track_outside_the_image(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    let original = expected(vec![rectangle((40.0, 40.0), (100.0, 90.0))]);
    let from = image_point(&mut cx, 70.0, 40.0);
    let to = image_point(&mut cx, 90.0, 60.0);
    cx.simulate_mouse_move(from, None, Modifiers::none());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    assert_image(saved(&mut cx), &original);
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    assert_image(saved(&mut cx), &original);

    cx.simulate_mouse_move(from, None, Modifiers::none());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_keystrokes("p");
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    assert_image(saved(&mut cx), &original);

    cx.simulate_keystrokes("v");
    drag(&mut cx, (70.0, 40.0), (340.0, 40.0), Modifiers::none());
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((310.0, 40.0), (370.0, 90.0))]),
    );
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &original);
}

#[gpui::test]
fn pencil_marks_move_and_crop_remains_independent_of_selection(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    click(&mut cx, 30.0, 30.0);
    drag(&mut cx, (60.0, 60.0), (80.0, 80.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    drag(&mut cx, (30.0, 30.0), (50.0, 30.0), Modifiers::none());
    drag(&mut cx, (70.0, 70.0), (100.0, 70.0), Modifiers::none());
    let marks = expected(vec![
        Annotation::Stroke(Stroke {
            points: vec![Point::new(50.0, 30.0)],
            color: INK,
            width: 3.0,
        }),
        Annotation::Stroke(Stroke {
            points: vec![Point::new(90.0, 60.0), Point::new(110.0, 80.0)],
            color: INK,
            width: 3.0,
        }),
    ]);
    assert_image(saved(&mut cx), &marks);

    cx.simulate_keystrokes("c");
    drag(&mut cx, (20.0, 20.0), (120.0, 100.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 50.0, 30.0);
    assert_image(
        saved(&mut cx),
        &image::imageops::crop_imm(&marks, 20, 20, 100, 80).to_image(),
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(
        cx.cx.read(|app| app.windows().len()),
        1,
        "Esc only clears selection"
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(
        cx.cx.read(|app| app.windows().len()),
        0,
        "the next Esc closes the editor"
    );
}

#[gpui::test]
fn stationary_shift_changes_during_resize_still_commit_as_one_edit(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 70.0, 40.0);
    let from = image_point(&mut cx, 100.0, 90.0);
    let to = image_point(&mut cx, 130.0, 110.0);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::none()
    };
    cx.simulate_mouse_move(from, None, Modifiers::none());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_modifiers_change(shift);
    cx.simulate_modifiers_change(Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((40.0, 40.0), (130.0, 110.0))]),
    );
    command(&mut cx, "z");
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((40.0, 40.0), (100.0, 90.0))]),
    );
    command(&mut cx, "shift-z");

    click(&mut cx, 85.0, 40.0);
    let from = image_point(&mut cx, 130.0, 110.0);
    let to = image_point(&mut cx, 150.0, 120.0);
    cx.simulate_mouse_move(from, None, Modifiers::none());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_modifiers_change(shift);
    cx.simulate_mouse_up(to, MouseButton::Left, shift);
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((40.0, 40.0), (150.0, 150.0))]),
    );
    command(&mut cx, "z");
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((40.0, 40.0), (130.0, 110.0))]),
    );
}
