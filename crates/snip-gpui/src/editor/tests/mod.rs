//! Drive the rendered editor through GPUI events and observe saved PNGs.
//! The OS file picker and text input system are simulated; no private editor state is inspected.

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, Pixels, TestAppContext, VisualTestContext, point, px, size,
};
use image::Rgba;
use snip_core::{Annotation, Arrow, Color, Document, Point, Rectangle, Renderer, RgbaImage, Stroke, Text};

const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
const INK: Color = Color::rgb(0xE5, 0x48, 0x4D);

fn editor(cx: &mut TestAppContext) -> VisualTestContext {
    save_flow::install(cx);
    let window = cx.update(|cx| {
        super::open(RgbaImage::from_pixel(320, 200, WHITE), 1.0, cx);
        cx.windows()[0]
    });
    let cx = VisualTestContext::from_window(window, cx);
    cx.simulate_scale_factor_change(1.0);
    // Keep image coordinates exact for pixel assertions; fitted layouts have
    // their own cases below, including the editor's minimum window size.
    cx.simulate_resize(size(px(900.0), px(600.0)));
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

fn press_keys(cx: &mut VisualTestContext, keys: &str) {
    for key in keys.split_whitespace() {
        cx.simulate_keystrokes(key);
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
    }
}

fn toolbar(cx: &mut VisualTestContext, label: &'static str) {
    if cx.debug_bounds(label).is_none() {
        let trigger = match label {
            "Red" | "Yellow" | "Green" | "Blue" | "Black" | "Thin" | "Medium" | "Thick" | "Clear crop" => {
                "Ink and stroke"
            }
            "Fit" | "100%" => "Zoom",
            "Copy" | "Copy & Close" | "Save…" => "Export",
            _ => panic!("missing toolbar control {label}"),
        };
        toolbar(cx, trigger);
    }
    let position = cx.debug_bounds(label).expect("rendered toolbar control").center();
    cx.simulate_mouse_move(position, None, Modifiers::none());
    cx.simulate_click(position, Modifiers::none());
    if !matches!(label, "Ink and stroke" | "Zoom" | "Export" | "Copy & Close")
        && cx.debug_bounds("toolbar-popover").is_some()
    {
        // Finish choosing options before drawing; the popover intentionally
        // receives pointer input over the part of the canvas it covers.
        let outside = cx.debug_bounds("editor-toolbar").unwrap().origin + point(px(1.), px(1.));
        cx.simulate_click(outside, Modifiers::none());
    }
}

#[gpui::test]
fn style_is_shared_across_drawing_tools_and_saved_at_image_resolution(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    toolbar(&mut cx, "Blue");
    toolbar(&mut cx, "Thick");
    drag(&mut cx, (20.0, 30.0), (80.0, 30.0), Modifiers::none());
    cx.simulate_keystrokes("a");
    drag(&mut cx, (100.0, 30.0), (160.0, 30.0), Modifiers::none());
    cx.simulate_keystrokes("r");
    drag(&mut cx, (180.0, 30.0), (240.0, 70.0), Modifiers::none());
    cx.simulate_keystrokes("t");
    click(&mut cx, 20.0, 120.0);
    cx.simulate_input("Blue");
    cx.simulate_keystrokes("escape");

    let blue = Color::rgb(0x00, 0x90, 0xFF);
    let image = saved(&mut cx);
    assert_image(
        image.clone(),
        &expected(vec![
            Annotation::Stroke(Stroke {
                points: vec![Point::new(20.0, 30.0), Point::new(80.0, 30.0)],
                color: blue,
                width: 6.0,
            }),
            Annotation::Arrow(Arrow {
                tail: Point::new(100.0, 30.0),
                head: Point::new(160.0, 30.0),
                color: blue,
                width: 6.0,
            }),
            Annotation::Rectangle(Rectangle {
                start: Point::new(180.0, 30.0),
                end: Point::new(240.0, 70.0),
                color: blue,
                width: 6.0,
            }),
            Annotation::Text(Text {
                origin: Point::new(20.0, 120.0),
                content: "Blue".into(),
                size: 20.0,
                color: blue,
            }),
        ]),
    );
    assert_eq!(*image.get_pixel(50, 32), Rgba([0x00, 0x90, 0xFF, 255]));
    assert_eq!(*image.get_pixel(50, 34), WHITE);
}

#[gpui::test]
fn a_selected_annotation_restyles_one_property_per_undo_and_sets_drawing_defaults(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    cx.simulate_keystrokes("a");
    drag(&mut cx, (150.0, 40.0), (230.0, 40.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 70.0, 40.0);
    toolbar(&mut cx, "Green");
    toolbar(&mut cx, "Thin");
    let green = Color::rgb(0x30, 0xA4, 0x6C);
    let styled = |color, width| {
        expected(vec![
            Annotation::Rectangle(Rectangle {
                start: Point::new(40.0, 40.0),
                end: Point::new(100.0, 90.0),
                color,
                width,
            }),
            arrow((150.0, 40.0), (230.0, 40.0)),
        ])
    };
    assert_image(saved(&mut cx), &styled(green, 1.0));
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &styled(green, 3.0));
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &styled(INK, 3.0));
    command(&mut cx, "shift-z");
    assert_image(saved(&mut cx), &styled(green, 3.0));
    command(&mut cx, "shift-z");
    assert_image(saved(&mut cx), &styled(green, 1.0));
    command(&mut cx, "z");
    cx.simulate_keystrokes("p");
    drag(&mut cx, (40.0, 140.5), (100.0, 140.5), Modifiers::none());
    let image = saved(&mut cx);
    assert_eq!(*image.get_pixel(70, 140), Rgba([0x30, 0xA4, 0x6C, 255]));
    assert_eq!(*image.get_pixel(70, 142), WHITE);
}

#[gpui::test]
fn text_color_finishes_the_session_and_restyles_separately_without_applying_width(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    toolbar(&mut cx, "Thick");
    cx.simulate_keystrokes("t");
    click(&mut cx, 30.0, 120.0);
    cx.simulate_input("Hello");
    toolbar(&mut cx, "Thin");
    // Disabled width must not finish the active session or consume input.
    cx.simulate_input("!");
    toolbar(&mut cx, "Blue");
    toolbar(&mut cx, "Green");
    toolbar(&mut cx, "Medium");
    let label = |color| {
        expected(vec![Annotation::Text(Text {
            origin: Point::new(30.0, 120.0),
            content: "Hello!".into(),
            size: 20.0,
            color,
        })])
    };
    assert_image(saved(&mut cx), &label(Color::rgb(0x30, 0xA4, 0x6C)));
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &label(Color::rgb(0x00, 0x90, 0xFF)));
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &label(INK));
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &expected(vec![]));
    command(&mut cx, "shift-z");
    cx.simulate_keystrokes("p");
    drag(&mut cx, (30.0, 40.0), (100.0, 40.0), Modifiers::none());
    let image = saved(&mut cx);
    assert_eq!(*image.get_pixel(60, 42), Rgba([0x30, 0xA4, 0x6C, 255]));
    assert_eq!(*image.get_pixel(60, 44), WHITE);
    cx.simulate_keystrokes("t");
    click(&mut cx, 200.0, 120.0);
    toolbar(&mut cx, "Black");
    // An empty text session leaves no annotation or history entry.
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &label(INK));
    cx.simulate_keystrokes("v");
    click(&mut cx, 50.0, 125.0);
    toolbar(&mut cx, "Black");
    assert_image(saved(&mut cx), &label(Color::rgb(0, 0, 0)));
}

#[gpui::test]
fn palette_and_width_presets_produce_their_declared_pixels(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    for (color, rgba, width, y, inside, outside) in [
        ("Red", [0xE5, 0x48, 0x4D, 255], "Thin", 20.5, 20, 21),
        ("Yellow", [0xF5, 0xD9, 0x0A, 255], "Medium", 50.5, 51, 52),
        ("Green", [0x30, 0xA4, 0x6C, 255], "Thick", 80.5, 82, 84),
        ("Blue", [0x00, 0x90, 0xFF, 255], "Thin", 110.5, 110, 111),
        ("Black", [0, 0, 0, 255], "Medium", 140.5, 141, 142),
    ] {
        toolbar(&mut cx, color);
        toolbar(&mut cx, width);
        drag(&mut cx, (30.0, y), (100.0, y), Modifiers::none());
        let image = saved(&mut cx);
        assert_eq!(*image.get_pixel(60, inside), Rgba(rgba), "{color} {width}");
        assert_eq!(*image.get_pixel(60, outside), WHITE, "{color} {width}");
    }
}

#[gpui::test]
fn selection_and_no_op_style_choices_preserve_defaults_and_redo(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    toolbar(&mut cx, "Blue");
    toolbar(&mut cx, "Thick");
    cx.simulate_keystrokes("v");
    click(&mut cx, 70.0, 40.0);
    click(&mut cx, 200.0, 150.0);
    cx.simulate_keystrokes("p");
    drag(&mut cx, (150.0, 40.0), (230.0, 40.0), Modifiers::none());
    let blue_stroke = Annotation::Stroke(Stroke {
        points: vec![Point::new(150.0, 40.0), Point::new(230.0, 40.0)],
        color: Color::rgb(0x00, 0x90, 0xFF),
        width: 6.0,
    });
    let original = expected(vec![rectangle((40.0, 40.0), (100.0, 90.0)), blue_stroke]);
    assert_image(saved(&mut cx), &original);
    command(&mut cx, "z");
    toolbar(&mut cx, "Green");
    toolbar(&mut cx, "Thin");
    cx.simulate_keystrokes("v");
    click(&mut cx, 70.0, 40.0);
    toolbar(&mut cx, "Red");
    toolbar(&mut cx, "Medium");
    command(&mut cx, "shift-z");
    assert_image(saved(&mut cx), &original);
    cx.simulate_keystrokes("p");
    drag(&mut cx, (150.0, 140.0), (230.0, 140.0), Modifiers::none());
    let image = saved(&mut cx);
    assert_eq!(*image.get_pixel(180, 140), Rgba([0xE5, 0x48, 0x4D, 255]));
    assert_eq!(*image.get_pixel(180, 142), WHITE);
}

#[gpui::test]
fn toolbar_stays_in_one_row_and_popovers_fit_at_minimum_window_size(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("c");
    for scale in [1.0, 2.0] {
        cx.simulate_scale_factor_change(scale);
        cx.simulate_resize(size(px(480.0), px(320.0)));
        cx.run_until_parked();
        let bar = cx.debug_bounds("editor-toolbar").unwrap();
        assert_eq!(bar.size.height, px(52.));
        let canvas = cx.debug_bounds("editor-canvas").unwrap();
        assert!(canvas.size.height > px(200.));
        let row_y = cx.debug_bounds("Select").unwrap().center().y;
        let mut controls = Vec::new();
        for label in [
            "Select",
            "Pencil",
            "Arrow",
            "Rectangle",
            "Text",
            "Crop",
            "Ink and stroke",
            "Zoom",
            "Export",
        ] {
            let bounds = cx.debug_bounds(label).expect(label);
            assert!(
                bounds.left() >= bar.left() && bounds.right() <= bar.right(),
                "{label} fits horizontally"
            );
            assert_eq!(bounds.center().y, row_y);
            for (other, previous) in &controls {
                assert!(!bounds.intersects(previous), "{label} overlaps {other}");
            }
            controls.push((label, bounds));
        }
        for (trigger, items) in [
            (
                "Ink and stroke",
                &[
                    "Red",
                    "Yellow",
                    "Green",
                    "Blue",
                    "Black",
                    "Thin",
                    "Medium",
                    "Thick",
                    "Clear crop",
                ][..],
            ),
            ("Zoom", &["Fit", "100%"][..]),
            ("Export", &["Copy", "Copy & Close", "Save…"][..]),
        ] {
            toolbar(&mut cx, trigger);
            let popup = cx.debug_bounds("toolbar-popover").unwrap();
            assert!(popup.left() >= px(0.) && popup.right() <= px(480.));
            assert!(
                popup.top() >= bar.bottom() && popup.bottom() <= px(320.),
                "{trigger}: popup {popup:?}, bar {bar:?}"
            );
            for &label in items {
                let bounds = cx.debug_bounds(label).expect(label);
                assert!(
                    bounds.left() >= popup.left() && bounds.right() <= popup.right(),
                    "{label} inside popup"
                );
            }
            cx.simulate_keystrokes("escape");
            assert!(cx.debug_bounds("toolbar-popover").is_none());
        }
    }
}

#[gpui::test]
fn menus_dismiss_before_escape_closes_the_editor_and_allow_keyboard_export(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    // Six tool buttons precede Ink, Zoom, and Export in tab order.
    press_keys(&mut cx, "tab tab tab tab tab tab tab enter");
    assert!(cx.debug_bounds("Red").is_some());
    press_keys(&mut cx, "escape");
    assert!(cx.debug_bounds("Red").is_none());
    assert!(cx.debug_bounds("editor-canvas").is_some());
    press_keys(&mut cx, "tab space");
    assert!(cx.debug_bounds("Fit").is_some());
    press_keys(&mut cx, "escape tab enter tab enter");
    assert!(cx.debug_bounds("Copied to clipboard").is_some());
    assert!(cx.debug_bounds("toolbar-popover").is_none());
    toolbar(&mut cx, "Ink and stroke");
    toolbar(&mut cx, "Ink and stroke");
    assert!(
        cx.debug_bounds("toolbar-popover").is_none(),
        "the trigger toggles its own popup"
    );
    toolbar(&mut cx, "Ink and stroke");
    click(&mut cx, 150., 150.);
    assert!(cx.debug_bounds("toolbar-popover").is_none());
}

#[gpui::test]
fn dismissing_a_mouse_opened_popover_preserves_live_text_input(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("t");
    click(&mut cx, 20., 120.);
    cx.simulate_input("A");
    toolbar(&mut cx, "Ink and stroke");
    cx.simulate_keystrokes("escape");
    cx.simulate_input("B");
    assert_image(
        saved(&mut cx),
        &expected(vec![Annotation::Text(Text {
            origin: Point::new(20., 120.),
            content: "AB".into(),
            size: 20.,
            color: INK,
        })]),
    );
}

#[gpui::test]
fn a_new_text_session_clears_the_previous_style_target(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("t");
    click(&mut cx, 30.0, 120.0);
    cx.simulate_input("First");
    toolbar(&mut cx, "Blue");
    click(&mut cx, 170.0, 120.0);
    cx.simulate_input("Second");
    cx.simulate_keystrokes("escape");
    toolbar(&mut cx, "Green");
    let blue = Color::rgb(0x00, 0x90, 0xFF);
    assert_image(
        saved(&mut cx),
        &expected(vec![
            Annotation::Text(Text {
                origin: Point::new(30.0, 120.0),
                content: "First".into(),
                size: 20.0,
                color: blue,
            }),
            Annotation::Text(Text {
                origin: Point::new(170.0, 120.0),
                content: "Second".into(),
                size: 20.0,
                color: blue,
            }),
        ]),
    );
}

#[gpui::test]
fn high_dpi_restyles_preserve_geometry_and_crop_for_all_line_tools(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_scale_factor_change(2.0);
    cx.simulate_resize(size(px(480.0), px(320.0)));
    cx.run_until_parked();
    toolbar(&mut cx, "Thick");
    drag(&mut cx, (40.0, 40.0), (100.0, 40.0), Modifiers::none());
    let image = saved(&mut cx);
    assert_eq!(*image.get_pixel(70, 45), Rgba([0xE5, 0x48, 0x4D, 255]));
    assert_eq!(*image.get_pixel(70, 47), WHITE);
    cx.simulate_keystrokes("a");
    drag(&mut cx, (40.0, 90.0), (100.0, 90.0), Modifiers::none());
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 140.0), (100.0, 180.0), Modifiers::none());
    cx.simulate_keystrokes("c");
    drag(&mut cx, (20.0, 20.0), (120.0, 190.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    for y in [40.0, 90.0, 140.0] {
        click(&mut cx, 70.0, y);
        toolbar(&mut cx, "Yellow");
        toolbar(&mut cx, "Thin");
    }
    let yellow = Color::rgb(0xF5, 0xD9, 0x0A);
    let marks = expected(vec![
        Annotation::Stroke(Stroke {
            points: vec![Point::new(40.0, 40.0), Point::new(100.0, 40.0)],
            color: yellow,
            width: 2.0,
        }),
        Annotation::Arrow(Arrow {
            tail: Point::new(40.0, 90.0),
            head: Point::new(100.0, 90.0),
            color: yellow,
            width: 2.0,
        }),
        Annotation::Rectangle(Rectangle {
            start: Point::new(40.0, 140.0),
            end: Point::new(100.0, 180.0),
            color: yellow,
            width: 2.0,
        }),
    ]);
    assert_image(
        saved(&mut cx),
        &image::imageops::crop_imm(&marks, 20, 20, 100, 170).to_image(),
    );
}

#[gpui::test]
fn style_clicks_cancel_unfinished_moves_and_drawing_before_committing(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (40.0, 40.0), (100.0, 90.0), Modifiers::none());
    cx.simulate_keystrokes("v");
    let from = image_point(&mut cx, 70.0, 40.0);
    let to = image_point(&mut cx, 90.0, 60.0);
    cx.simulate_mouse_move(from, None, Modifiers::none());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    toolbar(&mut cx, "Blue");
    assert_image(
        saved(&mut cx),
        &expected(vec![Annotation::Rectangle(Rectangle {
            start: Point::new(40.0, 40.0),
            end: Point::new(100.0, 90.0),
            color: Color::rgb(0x00, 0x90, 0xFF),
            width: 3.0,
        })]),
    );
    command(&mut cx, "z");
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((40.0, 40.0), (100.0, 90.0))]),
    );
    cx.simulate_keystrokes("p");
    let from = image_point(&mut cx, 170.0, 140.0);
    let to = image_point(&mut cx, 220.0, 140.0);
    cx.simulate_mouse_move(from, None, Modifiers::none());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    toolbar(&mut cx, "Thick");
    command(&mut cx, "shift-z");
    let image = saved(&mut cx);
    assert_eq!(*image.get_pixel(70, 40), Rgba([0x00, 0x90, 0xFF, 255]));
    assert_eq!(*image.get_pixel(200, 140), WHITE);
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

mod crop;
mod save_flow;
mod text;
mod zoom;
