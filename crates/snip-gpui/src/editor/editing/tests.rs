//! Editing rules are exercised through the same interface as the window.
use super::{Editing, Tool};
use snip_core::{Point, RgbaImage};

fn drag(model: &mut Editing, from: Point, to: Point) {
    model.pointer_down(from, 1., 1., false);
    model.update_gesture(to, false, 1.);
    model.finish_gesture();
}

fn select(model: &mut Editing, position: Point) {
    model.set_tool(Tool::Select);
    drag(model, position, position);
}

#[test]
fn a_corner_drag_returning_to_its_start_preserves_redo() {
    let mut model = Editing::new(RgbaImage::new(320, 200));
    model.set_tool(Tool::Rectangle);
    drag(&mut model, Point::new(40., 40.), Point::new(100., 90.));
    let original = model.document().annotations.clone();
    model.set_tool(Tool::Select);
    drag(&mut model, Point::new(70., 40.), Point::new(90., 60.));
    let moved = model.document().annotations.clone();
    model.restore_history(false);
    select(&mut model, Point::new(70., 40.));
    let corner = Point::new(40., 40.);
    model.pointer_down(corner, 1., 1., false);
    model.update_gesture(Point::new(20., 20.), false, 1.);
    model.update_gesture(corner, false, 1.);
    model.finish_gesture();
    assert_eq!(model.document().annotations, original);
    model.restore_history(true);
    assert_eq!(model.document().annotations, moved);
}

#[test]
fn an_unchanged_shift_arrow_resize_preserves_redo() {
    let mut model = Editing::new(RgbaImage::new(320, 200));
    model.set_tool(Tool::Arrow);
    drag(&mut model, Point::new(40., 40.), Point::new(40., 100.));
    model.set_tool(Tool::Rectangle);
    drag(&mut model, Point::new(200., 120.), Point::new(260., 160.));
    let complete = model.document().annotations.clone();
    model.restore_history(false);
    select(&mut model, Point::new(40., 70.));
    let head = Point::new(40., 100.);
    model.pointer_down(head, 1., 1., false);
    model.update_gesture(Point::new(60., 120.), true, 1.);
    model.update_gesture(head, true, 1.);
    model.finish_gesture();
    model.restore_history(true);
    assert_eq!(model.document().annotations, complete);
    model.restore_history(false);
    model.restore_history(false);
    assert!(model.document().annotations.is_empty());
}
