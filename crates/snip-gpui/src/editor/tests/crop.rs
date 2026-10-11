//! Crop interactions use rendered editor input and saved pixels.

use super::*;
use gpui::{Bounds, hsla};

fn painted_handles(cx: &mut VisualTestContext) -> Vec<Bounds<Pixels>> {
    cx.run_until_parked();
    cx.update(|window, _| {
        let scale = window.scale_factor();
        let mut handles: Vec<_> = window
            .painted_quads()
            .into_iter()
            .filter(|quad| quad.border_color == hsla(0.58, 0.9, 0.45, 1.))
            .map(|quad| {
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
            .collect();
        // GPUI splits each outline into border strips with identical outer bounds.
        handles.dedup();
        handles
    })
}

fn cropped(annotations: Vec<Annotation>, x: u32, y: u32, width: u32, height: u32) -> RgbaImage {
    image::imageops::crop_imm(&expected(annotations), x, y, width, height).to_image()
}

#[gpui::test]
fn each_crop_corner_keeps_its_opposite_fixed_and_undoes_in_one_step(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (50., 50.), (160., 130.), Modifiers::none());
    let marks = || vec![rectangle((50., 50.), (160., 130.))];
    cx.simulate_keystrokes("c");
    drag(&mut cx, (40., 40.), (180., 140.), Modifiers::none());
    let original = cropped(marks(), 40, 40, 140, 100);
    for (from, to, region) in [
        ((40., 40.), (30.5, 25.5), (30, 25, 150, 115)),
        ((180., 40.), (200.5, 25.5), (40, 25, 161, 115)),
        ((180., 140.), (200.5, 155.5), (40, 40, 161, 116)),
        ((40., 140.), (30.5, 155.5), (30, 40, 150, 116)),
    ] {
        drag(&mut cx, from, to, Modifiers::none());
        let (x, y, width, height) = region;
        let resized = cropped(marks(), x, y, width, height);
        assert_image(saved(&mut cx), &resized);
        command(&mut cx, "z");
        assert_image(saved(&mut cx), &original);
        command(&mut cx, "shift-z");
        assert_image(saved(&mut cx), &resized);
        command(&mut cx, "z");
    }
}

#[gpui::test]
fn crop_handles_follow_the_corners_and_only_show_with_crop_active(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("c");
    drag(&mut cx, (40., 40.), (180., 140.), Modifiers::none());
    let handles = painted_handles(&mut cx);
    assert_eq!(handles.len(), 4);
    for (handle, (x, y)) in handles
        .into_iter()
        .zip([(40., 40.), (180., 40.), (180., 140.), (40., 140.)])
    {
        let expected = image_point(&mut cx, x, y);
        assert!((handle.center().x - expected.x).abs() <= px(1.));
        assert!((handle.center().y - expected.y).abs() <= px(1.));
        assert_eq!(handle.size, size(px(8.), px(8.)));
    }
    cx.simulate_keystrokes("p");
    assert!(painted_handles(&mut cx).is_empty());
    assert_image(saved(&mut cx), &cropped(vec![], 40, 40, 140, 100));
}

#[gpui::test]
fn clearing_a_crop_restores_annotations_and_is_one_reversible_edit(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("r");
    drag(&mut cx, (20., 20.), (80., 80.), Modifiers::none());
    let full = expected(vec![rectangle((20., 20.), (80., 80.))]);
    cx.simulate_keystrokes("c");
    toolbar(&mut cx, "Clear crop"); // Disabled: no crop and no history change.
    drag(&mut cx, (120., 40.), (250., 160.), Modifiers::none());
    let region = image::imageops::crop_imm(&full, 120, 40, 130, 120).to_image();
    toolbar(&mut cx, "Clear crop");
    assert!(painted_handles(&mut cx).is_empty());
    assert_image(saved(&mut cx), &full);
    command(&mut cx, "z");
    assert_image(saved(&mut cx), &region);
    command(&mut cx, "shift-z");
    assert_image(saved(&mut cx), &full);
    command(&mut cx, "z");
    command(&mut cx, "z");
    toolbar(&mut cx, "Clear crop");
    command(&mut cx, "shift-z");
    assert_image(saved(&mut cx), &region); // Disabled clear preserves redo.
    cx.simulate_keystrokes("p");
    cx.run_until_parked();
    assert!(cx.debug_bounds("Clear crop").is_none());
}

#[gpui::test]
fn live_crop_dimensions_match_the_output_and_stay_inside_the_canvas(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("c");
    let from = image_point(&mut cx, 40., 40.);
    let to = image_point(&mut cx, 180., 140.);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("140 × 100 px").is_none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let label = cx.debug_bounds("140 × 100 px").expect("live dimensions");
    let canvas = cx.debug_bounds("editor-canvas").unwrap();
    assert_eq!(label.intersect(&canvas), label);
    assert!(!label.intersects(&Bounds::new(to - point(px(4.), px(4.)), size(px(8.), px(8.)))));
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("140 × 100 px").is_none());
    assert_eq!(saved(&mut cx).dimensions(), (140, 100));

    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    let outside = canvas.origin - point(px(40.), px(40.));
    cx.simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let label = cx.debug_bounds("180 × 140 px").expect("clamped crop dimensions");
    assert_eq!(label.intersect(&canvas), label);
    cx.simulate_keystrokes("escape");
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("180 × 140 px").is_none());
    assert_eq!(saved(&mut cx).dimensions(), (140, 100));
}

#[gpui::test]
fn crop_handles_clamp_at_image_and_opposite_edges_and_choose_the_nearest_corner(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("c");
    drag(&mut cx, (180., 140.), (0., 0.), Modifiers::none());
    // The grab lies outside the image but within the visible corner handle.
    drag(&mut cx, (-3., -3.), (17., 17.), Modifiers::none());
    assert_image(saved(&mut cx), &cropped(vec![], 20, 20, 160, 120));
    command(&mut cx, "z");
    let from = image_point(&mut cx, 0., 0.);
    let outside = cx.debug_bounds("editor-canvas").unwrap().bottom_right() + point(px(40.), px(40.));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    assert_image(saved(&mut cx), &cropped(vec![], 179, 139, 1, 1));

    toolbar(&mut cx, "Clear crop");
    drag(&mut cx, (25., 25.), (26., 26.), Modifiers::none());
    drag(&mut cx, (26., 25.), (35., 15.), Modifiers::none());
    assert_image(saved(&mut cx), &cropped(vec![], 25, 15, 10, 11));
    command(&mut cx, "z");
    // All four hit regions overlap; an equal-distance grab has a stable winner.
    drag(&mut cx, (25.5, 25.5), (15.5, 15.5), Modifiers::none());
    assert_image(saved(&mut cx), &cropped(vec![], 15, 15, 11, 11));
}

#[gpui::test]
fn canceled_and_unchanged_crop_gestures_preserve_the_crop_and_redo(cx: &mut TestAppContext) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("c");
    drag(&mut cx, (40., 40.), (180., 140.), Modifiers::none());
    drag(&mut cx, (180., 140.), (200., 160.), Modifiers::none());
    command(&mut cx, "z");
    click(&mut cx, 100., 100.); // Empty replacement preserves the previous crop.
    drag(&mut cx, (90., 90.), (120., 90.), Modifiers::none());
    click(&mut cx, 40., 40.); // A handle click is not an edit either.
    let corner = image_point(&mut cx, 40., 40.);
    let changed = image_point(&mut cx, 20., 20.);
    for cancel in ["delete", "backspace", "escape", "p", "toolbar", "return"] {
        cx.simulate_mouse_down(corner, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(changed, MouseButton::Left, Modifiers::none());
        match cancel {
            "toolbar" => toolbar(&mut cx, "Fit"),
            "return" => cx.simulate_mouse_move(corner, MouseButton::Left, Modifiers::none()),
            key => cx.simulate_keystrokes(key),
        }
        cx.run_until_parked();
        assert!(cx.debug_bounds("160 × 120 px").is_none());
        cx.simulate_mouse_up(corner, MouseButton::Left, Modifiers::none());
        assert_image(saved(&mut cx), &cropped(vec![], 40, 40, 140, 100));
        cx.simulate_keystrokes("c");
    }
    command(&mut cx, "shift-z");
    assert_image(saved(&mut cx), &cropped(vec![], 40, 40, 160, 120));
}

#[gpui::test]
fn zoomed_and_panned_crop_adjustment_keeps_image_pixel_dimensions_at_both_dpi_scales(
    cx: &mut TestAppContext,
) {
    let mut cx = editor(cx);
    cx.simulate_keystrokes("c");
    for scale in [1., 2.] {
        command(&mut cx, "0");
        toolbar(&mut cx, "Clear crop");
        cx.simulate_resize(size(px(600.), px(400.)));
        cx.simulate_scale_factor_change(scale);
        cx.run_until_parked();
        cx.simulate_keystrokes("c");
        drag(&mut cx, (140., 80.), (180., 120.), Modifiers::none());
        let center = cx.debug_bounds("editor-canvas").unwrap().center();
        super::zoom::pinch(&mut cx, center, 3.);
        super::zoom::space(&mut cx, true);
        super::zoom::window_drag(&mut cx, center, center + point(px(12.), px(8.)));
        super::zoom::space(&mut cx, false);
        let logical_scale = 4. / scale;
        let from = center + point(px(20. * logical_scale + 12.), px(20. * logical_scale + 8.));
        let to = from + point(px(20. * logical_scale), px(10. * logical_scale));
        cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let label = cx
            .debug_bounds("60 × 50 px")
            .expect("image-pixel dimensions through navigation");
        let canvas = cx.debug_bounds("editor-canvas").unwrap();
        assert_eq!(label.intersect(&canvas), label);
        for handle in painted_handles(&mut cx) {
            assert_eq!(handle.size, size(px(8.), px(8.)));
        }
        // View input during adjustment must not change the crop under the pointer.
        super::zoom::pinch(&mut cx, to, 1.);
        cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        assert_image(saved(&mut cx), &cropped(vec![], 140, 80, 60, 50));

        cx.simulate_scale_factor_change(if scale == 1. { 2. } else { 1. });
        cx.run_until_parked();
        let corners = painted_handles(&mut cx);
        let from = corners[1].center();
        // Read the visible corner to click it, but use a specified 20-image-pixel delta.
        let new_logical_scale = if scale == 1. { 2. } else { 4. };
        super::zoom::window_drag(&mut cx, from, from + point(px(20. * new_logical_scale), px(0.)));
        assert_image(saved(&mut cx), &cropped(vec![], 140, 80, 80, 50));
        command(&mut cx, "0");
        assert_image(saved(&mut cx), &cropped(vec![], 140, 80, 80, 50));
    }
}
