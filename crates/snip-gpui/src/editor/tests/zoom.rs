//! Navigation goes through platform input; observations are painted bounds and saved pixels.

use gpui::{
    Bounds, EntityInputHandler, KeyDownEvent, KeyUpEvent, Keystroke, PinchEvent, ScrollDelta,
    ScrollWheelEvent, hsla,
};

use super::*;

fn zoom_modifiers() -> Modifiers {
    Modifiers {
        control: !cfg!(target_os = "macos"),
        platform: cfg!(target_os = "macos"),
        ..Modifiers::none()
    }
}

/// GPUI Windows multiplies a native wheel notch by the OS's scroll-line setting.
#[allow(unsafe_code)]
fn native_notch() -> f32 {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{SPI_GETWHEELSCROLLLINES, SystemParametersInfoW};
        let mut lines = 0u32;
        // SAFETY: this read-only SPI action writes exactly one u32 to a live buffer.
        unsafe {
            SystemParametersInfoW(
                SPI_GETWHEELSCROLLLINES,
                0,
                Some((&mut lines as *mut u32).cast()),
                Default::default(),
            )
        }
        .expect("read native wheel settings for the event fixture");
        lines.max(1) as f32
    }
    #[cfg(not(windows))]
    {
        1.
    }
}

/// The visible screenshot edge, excluding its one-logical-pixel outline.
fn painted_image(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    painted_outline(cx, hsla(0., 0., 1., 0.12)).dilate(px(-1.))
}

fn painted_selection(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    painted_outline(cx, hsla(0.58, 0.9, 0.65, 1.))
}

fn painted_outline(cx: &mut VisualTestContext, color: gpui::Hsla) -> Bounds<Pixels> {
    cx.run_until_parked();
    cx.update(|w, _| {
        let quad = w
            .painted_quads()
            .into_iter()
            .find(|q| q.border_color == color)
            .expect("painted outline");
        let scale = w.scale_factor();
        Bounds::new(
            point(
                px(quad.bounds.origin.x.as_f32() / scale),
                px(quad.bounds.origin.y.as_f32() / scale),
            ),
            size(
                px(quad.bounds.size.width.as_f32() / scale),
                px(quad.bounds.size.height.as_f32() / scale),
            ),
        )
    })
}

fn window_drag(cx: &mut VisualTestContext, from: gpui::Point<Pixels>, to: gpui::Point<Pixels>) {
    cx.simulate_mouse_move(from, None, Modifiers::none());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
}

fn pinch(cx: &mut VisualTestContext, position: gpui::Point<Pixels>, delta: f32) {
    cx.simulate_mouse_move(position, None, Modifiers::none());
    cx.simulate_event(PinchEvent {
        position,
        delta,
        ..Default::default()
    });
}

fn assert_position(actual: gpui::Point<Pixels>, expected: gpui::Point<Pixels>) {
    // GPUI rounds painted quad edges to physical pixels.
    assert!(
        (actual.x - expected.x).abs() <= px(1.) && (actual.y - expected.y).abs() <= px(1.),
        "{actual:?} at {expected:?}"
    );
}

fn space(cx: &mut VisualTestContext, down: bool) {
    let keystroke = Keystroke::parse("space").unwrap();
    if down {
        cx.simulate_event(KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        });
    } else {
        cx.simulate_event(KeyUpEvent { keystroke });
    }
}

#[gpui::test]
fn wheel_zoom_changes_display_size_without_changing_annotation_pixels(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    cx.simulate_event(ScrollWheelEvent {
        position: center,
        delta: ScrollDelta::Lines(point(0., native_notch())),
        modifiers: zoom_modifiers(),
        ..Default::default()
    });
    let shown = painted_image(&mut cx);
    assert!((shown.size.width - px(352.)).abs() < px(0.01), "{shown:?}");
    assert!((shown.size.height - px(220.)).abs() < px(0.01), "{shown:?}");
    cx.simulate_keystrokes("r");
    window_drag(&mut cx, center, center + point(px(22.), px(22.)));
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((160., 100.), (180., 120.))]),
    );
}

#[gpui::test]
fn pinch_keeps_the_point_under_the_gesture_and_stops_at_zoom_limits(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (140., 80.), (180., 120.), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 160., 80.);
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    pinch(&mut cx, center, 3.);
    let shown = painted_selection(&mut cx);
    assert_eq!(shown.size, size(px(180.), px(180.)));
    assert_position(shown.center(), center);
    let anchor = center + point(px(100.), px(60.));
    pinch(&mut cx, anchor, 1.);
    let shown = painted_selection(&mut cx);
    assert_eq!(shown.size, size(px(360.), px(360.)));
    assert_position(shown.center(), center - point(px(100.), px(60.)));
    for delta in [1., 0., f32::NAN, -1.] {
        pinch(&mut cx, anchor, delta);
        assert_eq!(painted_selection(&mut cx), shown);
    }
    cx.simulate_keystrokes("r");
    window_drag(&mut cx, anchor, anchor + point(px(16.), px(16.)));
    assert_image(
        saved(&mut cx),
        &expected(vec![
            rectangle((140., 80.), (180., 120.)),
            rectangle((185., 115.), (187., 117.)),
        ]),
    );
    pinch(&mut cx, center, -0.999);
    assert!((painted_image(&mut cx).size.width - px(32.)).abs() < px(0.01));
}

#[gpui::test]
fn fit_and_actual_size_controls_survive_resize_and_display_scale_changes(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_scale_factor_change(2.);
    cx.run_until_parked();
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    pinch(&mut cx, center, 1.);
    assert_eq!(painted_image(&mut cx).size.width, px(320.));
    command(&mut cx, "1");
    assert_eq!(painted_image(&mut cx).size.width, px(160.));
    pinch(&mut cx, center, 1.);
    toolbar(&mut cx, "100%");
    assert_eq!(painted_image(&mut cx).size.width, px(160.));
    cx.simulate_scale_factor_change(1.);
    assert_eq!(painted_image(&mut cx).size.width, px(320.));
    cx.simulate_resize(size(px(480.), px(320.)));
    assert_eq!(painted_image(&mut cx).size.width, px(320.));
    command(&mut cx, "0");
    let fitted = painted_image(&mut cx);
    let canvas = cx.debug_bounds("editor-canvas").unwrap();
    assert!(fitted.size.height <= canvas.size.height - px(32.));
    assert_position(fitted.center(), canvas.center());
    cx.simulate_resize(size(px(900.), px(600.)));
    assert_eq!(painted_image(&mut cx).size.width, px(320.));
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    pinch(&mut cx, center, 1.);
    toolbar(&mut cx, "Fit");
    assert_eq!(painted_image(&mut cx).size.width, px(320.));
}

#[gpui::test]
fn space_pan_preserves_selection_and_its_next_drag_edits_at_the_new_position(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (140., 80.), (180., 120.), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 160., 80.);
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    pinch(&mut cx, center, 3.);
    let original = painted_selection(&mut cx);
    space(&mut cx, true);
    window_drag(&mut cx, center, center + point(px(60.), px(40.)));
    space(&mut cx, false);
    assert_position(
        painted_selection(&mut cx).center(),
        original.center() + point(px(60.), px(40.)),
    );
    let edge = center + point(px(60.), px(-40.));
    window_drag(&mut cx, edge, edge + point(px(40.), px(20.)));
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((150., 85.), (190., 125.))]),
    );
    command(&mut cx, "z");
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((140., 80.), (180., 120.))]),
    );
    space(&mut cx, true);
    window_drag(&mut cx, center, center - point(px(40.), px(20.)));
    space(&mut cx, false);
    command(&mut cx, "shift-z");
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((150., 85.), (190., 125.))]),
    );
}

#[gpui::test]
fn pan_ends_on_key_release_escape_outside_release_and_deactivation(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (140., 80.), (180., 120.), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 160., 80.);
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    pinch(&mut cx, center, 3.);
    let initial = painted_selection(&mut cx);
    space(&mut cx, true);
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        center + point(px(30.), px(20.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    space(&mut cx, false);
    cx.simulate_mouse_move(
        center + point(px(90.), px(60.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(center, MouseButton::Left, Modifiers::none());
    assert_position(
        painted_selection(&mut cx).center(),
        initial.center() + point(px(30.), px(20.)),
    );

    space(&mut cx, true);
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        center - point(px(40.), px(20.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_keystrokes("escape");
    cx.simulate_mouse_up(center, MouseButton::Left, Modifiers::none());
    assert_position(
        painted_selection(&mut cx).center(),
        initial.center() + point(px(30.), px(20.)),
    );
    space(&mut cx, false);

    space(&mut cx, true);
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        center - point(px(30.), px(20.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.deactivate_window();
    cx.update(|w, _| w.activate_window());
    cx.run_until_parked();
    let outside = point(px(-10.), px(-10.));
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    let edge = center - point(px(0.), px(80.));
    window_drag(&mut cx, edge, edge + point(px(40.), px(20.)));
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((150., 85.), (190., 125.))]),
    );
}

#[gpui::test]
fn zoom_and_view_buttons_preserve_text_composition_and_update_os_caret_bounds(cx: &mut TestAppContext) {
    use super::text::ime;
    let mut cx = editor(cx);
    cx.simulate_keystrokes("t");
    click(&mut cx, 130., 100.);
    cx.simulate_input("A");
    space(&mut cx, true);
    cx.simulate_input(" ");
    space(&mut cx, false);
    ime(&mut cx, |e, w, cx| {
        e.replace_and_mark_text_in_range(None, "k", Some(1..1), w, cx)
    });
    let before = ime(&mut cx, |e, w, cx| {
        e.bounds_for_range(3..3, Default::default(), w, cx)
    })
    .unwrap();
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    pinch(&mut cx, center, 1.);
    let after = ime(&mut cx, |e, w, cx| {
        e.bounds_for_range(3..3, Default::default(), w, cx)
    })
    .unwrap();
    assert_position(after.origin, center + (before.origin - center) * 2.);
    assert!((after.size.height - before.size.height * 2.).abs() < px(0.01));
    assert_eq!(ime(&mut cx, |e, w, cx| e.marked_text_range(w, cx)), Some(2..3));
    toolbar(&mut cx, "Fit");
    toolbar(&mut cx, "100%");
    assert_eq!(ime(&mut cx, |e, w, cx| e.marked_text_range(w, cx)), Some(2..3));
    ime(&mut cx, |e, w, cx| e.replace_text_in_range(None, "漢", w, cx));
    assert_image(
        saved(&mut cx),
        &expected(vec![Annotation::Text(Text {
            origin: Point::new(130., 100.),
            content: "A 漢".into(),
            size: 20.,
            color: INK,
        })]),
    );
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &expected(vec![]));
}

#[gpui::test]
fn navigation_is_ignored_over_toolbar_and_during_annotation_drags(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    let original = painted_image(&mut cx);
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    cx.simulate_mouse_move(center, None, Modifiers::none());
    cx.simulate_event(ScrollWheelEvent {
        position: center,
        delta: ScrollDelta::Lines(point(0., 1.)),
        ..Default::default()
    });
    assert_eq!(painted_image(&mut cx), original);
    let control = cx.debug_bounds("Select").unwrap().center();
    pinch(&mut cx, control, 1.);
    assert_eq!(painted_image(&mut cx), original);
    cx.simulate_keystrokes("r");
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        center + point(px(20.), px(20.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    // Preserve the pressed button while injecting navigation into an active drag.
    cx.simulate_event(PinchEvent {
        position: center,
        delta: 1.,
        ..Default::default()
    });
    cx.simulate_event(ScrollWheelEvent {
        position: center,
        delta: ScrollDelta::Pixels(point(px(0.), px(40.))),
        modifiers: zoom_modifiers(),
        ..Default::default()
    });
    command(&mut cx, "0");
    command(&mut cx, "1");
    space(&mut cx, true);
    cx.simulate_mouse_move(
        center + point(px(40.), px(30.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        center + point(px(40.), px(30.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    space(&mut cx, false);
    assert_eq!(painted_image(&mut cx), original);
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((160., 100.), (200., 130.))]),
    );
    cx.simulate_mouse_move(center, None, Modifiers::none());
    cx.simulate_event(ScrollWheelEvent {
        position: center,
        delta: ScrollDelta::Pixels(point(px(0.), px(40.))),
        modifiers: zoom_modifiers(),
        ..Default::default()
    });
    assert!((painted_image(&mut cx).size.width - px(352.)).abs() < px(0.01));
}

#[gpui::test]
fn pan_bounds_keep_both_image_edges_reachable_and_clip_overlays_to_canvas(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (140., 80.), (180., 120.), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 160., 80.);
    let canvas = cx.debug_bounds("editor-canvas").unwrap();
    pinch(&mut cx, canvas.center(), 1.);
    let before = painted_selection(&mut cx);
    space(&mut cx, true);
    window_drag(
        &mut cx,
        canvas.center(),
        canvas.center() + point(px(60.), px(40.)),
    );
    assert_eq!(
        painted_selection(&mut cx),
        before,
        "both axes fit and remain centered"
    );
    space(&mut cx, false);
    pinch(&mut cx, canvas.center(), 1.);
    space(&mut cx, true);
    // End a real pan outside the canvas; the constrained image edge stays visible.
    window_drag(
        &mut cx,
        canvas.center(),
        canvas.center() + point(px(1000.), px(1000.)),
    );
    let shown = painted_image(&mut cx);
    assert_position(shown.origin, canvas.origin + point(px(16.), px(16.)));
    window_drag(
        &mut cx,
        canvas.center(),
        canvas.center() - point(px(1000.), px(1000.)),
    );
    let shown = painted_image(&mut cx);
    assert_position(
        shown.bottom_right(),
        canvas.bottom_right() - point(px(16.), px(16.)),
    );
    // Bring the selection partly over the toolbar to check its actual paint mask.
    window_drag(&mut cx, canvas.center(), canvas.center() + point(px(0.), px(50.)));
    space(&mut cx, false);
    cx.update(|w, _| {
        let quads = w.painted_quads();
        let selection = quads
            .iter()
            .filter(|q| q.border_color == hsla(0.58, 0.9, 0.65, 1.))
            .collect::<Vec<_>>();
        assert!(!selection.is_empty());
        for quad in selection {
            assert!(quad.content_mask.bounds.top().as_f32() >= f32::from(canvas.top()));
            assert!(quad.content_mask.bounds.bottom().as_f32() <= f32::from(canvas.bottom()));
        }
    });
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((140., 80.), (180., 120.))]),
    );
}

#[gpui::test]
fn zoomed_and_panned_handles_and_crop_save_the_document_region(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (120., 80.), (180., 120.), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 150., 80.);
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    pinch(&mut cx, center, 3.);
    space(&mut cx, true);
    window_drag(&mut cx, center, center + point(px(40.), px(20.)));
    space(&mut cx, false);
    let corner = center + point(px(120.), px(100.));
    window_drag(&mut cx, corner, corner + point(px(40.), px(20.)));
    assert_image(
        saved(&mut cx),
        &expected(vec![rectangle((120., 80.), (190., 125.))]),
    );
    cx.simulate_keystrokes("c");
    window_drag(
        &mut cx,
        center - point(px(200.), px(140.)),
        center + point(px(280.), px(260.)),
    );
    let mut doc = Document::new(RgbaImage::from_pixel(320, 200, WHITE));
    doc.annotations = vec![rectangle((120., 80.), (190., 125.))];
    doc.crop = Some(snip_core::Rect {
        x: 100,
        y: 60,
        width: 120,
        height: 100,
    });
    let expected = Renderer::new().export(&doc);
    assert_image(saved(&mut cx), &expected);
    command(&mut cx, "0");
    assert_image(saved(&mut cx), &expected);
}

#[gpui::test]
fn a_manual_view_keeps_its_image_center_through_resize_and_dpi_changes(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (140., 80.), (180., 120.), Modifiers::none());
    cx.simulate_keystrokes("v");
    click(&mut cx, 160., 80.);
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    pinch(&mut cx, center, 3.);
    space(&mut cx, true);
    window_drag(&mut cx, center, center + point(px(40.), px(20.)));
    space(&mut cx, false);
    cx.simulate_resize(size(px(600.), px(400.)));
    cx.run_until_parked();
    let resized_center = cx.debug_bounds("editor-canvas").unwrap().center();
    let shown = painted_selection(&mut cx);
    assert_eq!(shown.size.width, px(180.));
    assert_position(shown.center(), resized_center + point(px(40.), px(20.)));
    cx.simulate_scale_factor_change(2.);
    cx.run_until_parked();
    let shown = painted_selection(&mut cx);
    assert_eq!(shown.size.width, px(90.));
    assert_position(shown.center(), resized_center + point(px(20.), px(10.)));
}

#[gpui::test]
fn very_large_captures_can_zoom_back_to_fit_below_ten_percent(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        super::super::open(RgbaImage::from_pixel(8192, 200, WHITE), 1., cx);
        cx.windows()[0]
    });
    let mut cx = VisualTestContext::from_window(window, cx);
    cx.simulate_scale_factor_change(1.);
    cx.simulate_resize(size(px(480.), px(320.)));
    cx.run_until_parked();
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    let fitted = painted_image(&mut cx);
    assert!((fitted.size.width - px(448.)).abs() <= px(1.));
    pinch(&mut cx, center, 1.);
    pinch(&mut cx, center, -0.99);
    assert_eq!(painted_image(&mut cx), fitted);
    command(&mut cx, "0");
    assert_eq!(painted_image(&mut cx), fitted);
}

#[gpui::test]
fn text_placement_caret_clicks_and_reediting_follow_a_zoomed_panned_view(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    let center = cx.debug_bounds("editor-canvas").unwrap().center();
    pinch(&mut cx, center, 3.);
    space(&mut cx, true);
    window_drag(&mut cx, center, center + point(px(40.), px(20.)));
    space(&mut cx, false);
    cx.simulate_keystrokes("t");
    let origin = center + point(px(-80.), px(20.));
    cx.simulate_mouse_move(origin, None, Modifiers::none());
    cx.simulate_click(origin, Modifiers::none());
    cx.simulate_input("Hello");
    pinch(&mut cx, center, 0.5);
    let label = Text {
        origin: Point::new(130., 100.),
        content: "Hello".into(),
        size: 20.,
        color: INK,
    };
    let mut renderer = Renderer::new();
    let (caret, height) = renderer.text_caret(&label, 1);
    // At 600%, the image point at canvas center remains (150, 95).
    let position = center + point(px((caret.x - 150.) * 6.), px((caret.y + height / 2. - 95.) * 6.));
    cx.simulate_mouse_move(position, None, Modifiers::none());
    cx.simulate_click(position, Modifiers::none());
    cx.simulate_input("!");
    cx.simulate_keystrokes("escape v");
    let start = center + point(px(-118.), px(50.));
    cx.simulate_mouse_move(start, None, Modifiers::none());
    cx.simulate_event(MouseDownEvent {
        position: start,
        button: MouseButton::Left,
        click_count: 2,
        ..Default::default()
    });
    cx.simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
    cx.simulate_input("B");
    cx.simulate_keystrokes("escape");
    assert_image(
        saved(&mut cx),
        &expected(vec![Annotation::Text(Text {
            content: "BH!ello".into(),
            ..label
        })]),
    );
    command(&mut cx, "z");
    assert_image(
        saved(&mut cx),
        &expected(vec![Annotation::Text(Text {
            origin: Point::new(130., 100.),
            content: "H!ello".into(),
            size: 20.,
            color: INK,
        })]),
    );
}
