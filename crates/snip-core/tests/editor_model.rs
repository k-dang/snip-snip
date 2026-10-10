use image::Rgba;
use snip_core::{
    Annotation, Arrow, Color, Document, History, Point, Rect, Rectangle, Renderer, RgbaImage, Stroke, Text,
};

const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
const RED: Color = Color::rgb(255, 0, 0);

fn blank() -> Document {
    Document::new(RgbaImage::from_pixel(160, 160, WHITE))
}

fn arrow(tail: Point, head: Point, width: f32) -> Annotation {
    Annotation::Arrow(Arrow {
        tail,
        head,
        color: RED,
        width,
    })
}

fn rectangle(start: Point, end: Point, width: f32) -> Annotation {
    Annotation::Rectangle(Rectangle {
        start,
        end,
        color: RED,
        width,
    })
}

fn samples() -> Vec<Annotation> {
    vec![
        Annotation::Stroke(Stroke {
            points: vec![
                Point::new(30.3, 40.7),
                Point::new(60.2, 70.8),
                Point::new(90.1, 35.6),
            ],
            color: RED,
            width: 5.5,
        }),
        Annotation::Stroke(Stroke {
            points: vec![Point::new(60.3, 60.7)],
            color: RED,
            width: 7.0,
        }),
        Annotation::Text(Text {
            origin: Point::new(20.7, 30.3),
            content: "Ágj مرحبا".into(),
            size: 20.0,
            color: RED,
        }),
        arrow(Point::new(30.3, 40.7), Point::new(110.8, 100.2), 5.5),
        arrow(Point::new(90.8, 80.2), Point::new(35.3, 45.7), 8.0),
        arrow(Point::new(70.3, 60.7), Point::new(73.8, 62.2), 8.0),
        rectangle(Point::new(95.3, 100.7), Point::new(30.2, 40.8), 5.5),
    ]
}

#[test]
fn shapes_paint_at_image_resolution_and_survive_export_crop() {
    let mut doc = blank();
    doc.annotations
        .push(arrow(Point::new(20.0, 40.0), Point::new(100.0, 40.0), 6.0));
    doc.annotations
        .push(rectangle(Point::new(100.0, 100.0), Point::new(20.0, 60.0), 6.0));
    let mut renderer = Renderer::new();
    let preview = renderer.composite(&doc);
    assert_eq!(*preview.get_pixel(50, 40), Rgba([255, 0, 0, 255]));
    assert_ne!(*preview.get_pixel(85, 45), WHITE, "filled arrowhead");
    assert_eq!(
        *preview.get_pixel(101, 40),
        WHITE,
        "shaft cannot stick past the tip"
    );
    assert_eq!(*preview.get_pixel(20, 80), Rgba([255, 0, 0, 255]));
    assert_eq!(
        *preview.get_pixel(60, 80),
        WHITE,
        "rectangle interior stays clear"
    );
    doc.crop = Some(Rect {
        x: 10,
        y: 30,
        width: 110,
        height: 80,
    });
    let exported = renderer.export(&doc);
    assert_eq!(
        exported,
        image::imageops::crop_imm(&preview, 10, 30, 110, 80).to_image()
    );
}

#[test]
fn rectangles_render_identically_in_every_drag_direction() {
    let mut renderer = Renderer::new();
    let mut expected = None;
    for (a, b) in [
        (Point::new(30.5, 40.2), Point::new(90.7, 100.8)),
        (Point::new(90.7, 100.8), Point::new(30.5, 40.2)),
        (Point::new(30.5, 100.8), Point::new(90.7, 40.2)),
        (Point::new(90.7, 40.2), Point::new(30.5, 100.8)),
    ] {
        let mut doc = blank();
        doc.annotations.push(rectangle(a, b, 4.0));
        let rendered = renderer.composite(&doc);
        if let Some(expected) = &expected {
            assert_eq!(&rendered, expected);
        } else {
            expected = Some(rendered);
        }
    }
}

#[test]
fn degenerate_shapes_paint_nothing_and_cannot_be_hit() {
    let mut doc = blank();
    doc.annotations = vec![
        arrow(Point::new(60.0, 60.0), Point::new(60.0, 60.0), 6.0),
        rectangle(Point::new(40.0, 40.0), Point::new(40.0, 90.0), 6.0),
        rectangle(Point::new(40.0, 40.0), Point::new(90.0, 40.0), 6.0),
    ];
    let mut renderer = Renderer::new();
    assert_eq!(renderer.composite(&doc), doc.image);
    assert_eq!(renderer.annotation_at(&doc, Point::new(60.0, 60.0), 3.0), None);
}

#[test]
fn every_painted_pixel_is_inside_annotation_bounds() {
    let mut renderer = Renderer::new();
    for annotation in samples() {
        let bounds = renderer.bounds(&annotation);
        let mut doc = blank();
        doc.annotations.push(annotation.clone());
        let out = renderer.composite(&doc);
        let mut inked = 0;
        for (x, y, pixel) in out.enumerate_pixels() {
            if *pixel != WHITE {
                assert!(
                    bounds.contains(Point::new(x as f32, y as f32)),
                    "{annotation:?} paints outside {bounds:?} at ({x}, {y})"
                );
                inked += 1;
            }
        }
        assert!(inked > 0, "annotation must paint something");
    }
}

#[test]
fn translating_every_annotation_moves_its_render_and_bounds() {
    let offset = Point::new(9.0, 13.0);
    let mut renderer = Renderer::new();
    for mut annotation in samples() {
        let mut before = blank();
        before.annotations.push(annotation.clone());
        let bounds = renderer.bounds(&annotation);
        let original = renderer.composite(&before);
        annotation.translate(offset);
        let moved_bounds = renderer.bounds(&annotation);
        assert!((moved_bounds.min.x - bounds.min.x - offset.x).abs() < 0.001);
        assert!((moved_bounds.max.y - bounds.max.y - offset.y).abs() < 0.001);
        let mut after = blank();
        after.annotations.push(annotation);
        let moved = renderer.composite(&after);
        for (x, y, pixel) in moved.enumerate_pixels() {
            let expected = if x >= 9 && y >= 13 {
                original.get_pixel(x - 9, y - 13)
            } else {
                &WHITE
            };
            assert_eq!(pixel, expected, "translation at ({x}, {y})");
        }
    }
}

#[test]
fn hits_follow_geometry_and_topmost_paint_order() {
    let mut doc = blank();
    doc.annotations.push(Annotation::Stroke(Stroke {
        points: vec![Point::new(10.0, 10.0), Point::new(110.0, 110.0)],
        color: RED,
        width: 2.0,
    }));
    doc.annotations
        .push(rectangle(Point::new(30.0, 30.0), Point::new(90.0, 90.0), 4.0));
    let mut renderer = Renderer::new();
    assert_eq!(renderer.annotation_at(&doc, Point::new(30.0, 30.0), 0.0), Some(1));
    assert_eq!(
        renderer.annotation_at(&doc, Point::new(60.0, 60.0), 0.0),
        Some(0),
        "empty box allows hits underneath"
    );
    assert_eq!(
        renderer.annotation_at(&doc, Point::new(60.0, 40.0), 0.0),
        None,
        "diagonal's bounding box is not its hit area"
    );
    assert_eq!(renderer.annotation_at(&doc, Point::new(27.0, 60.0), 0.0), None);
    assert_eq!(renderer.annotation_at(&doc, Point::new(27.0, 60.0), 2.0), Some(1));
    assert_eq!(
        renderer.annotation_at(&doc, Point::new(28.5, 28.5), 0.0),
        Some(1),
        "mitered outer corner"
    );
    doc.annotations.reverse();
    assert_eq!(renderer.annotation_at(&doc, Point::new(30.0, 30.0), 0.0), Some(1));
}

#[test]
fn arrow_hits_shaft_and_filled_head_with_image_pixel_tolerance() {
    let mut doc = blank();
    doc.annotations
        .push(arrow(Point::new(20.0, 60.0), Point::new(100.0, 60.0), 4.0));
    let mut renderer = Renderer::new();
    assert_eq!(renderer.annotation_at(&doc, Point::new(40.0, 60.0), 0.0), Some(0));
    assert_eq!(renderer.annotation_at(&doc, Point::new(87.0, 65.0), 0.0), Some(0));
    assert_eq!(renderer.annotation_at(&doc, Point::new(90.0, 68.0), 0.0), None);
    assert_eq!(renderer.annotation_at(&doc, Point::new(40.0, 64.0), 0.0), None);
    assert_eq!(renderer.annotation_at(&doc, Point::new(40.0, 64.0), 2.0), Some(0));
    assert_eq!(renderer.annotation_at(&doc, Point::new(101.0, 60.0), 0.0), None);
}

#[test]
fn dots_and_text_have_useful_hit_regions() {
    let mut renderer = Renderer::new();
    let mut doc = blank();
    let text = Text {
        origin: Point::new(30.0, 40.0),
        content: "Hello".into(),
        size: 20.0,
        color: RED,
    };
    let (width, height) = renderer.text_size(&text);
    doc.annotations.push(Annotation::Text(text));
    assert_eq!(
        renderer.annotation_at(&doc, Point::new(30.0 + width / 2.0, 40.0 + height / 2.0), 0.0),
        Some(0)
    );
    assert_eq!(renderer.annotation_at(&doc, Point::new(28.0, 42.0), 0.0), None);
    assert_eq!(renderer.annotation_at(&doc, Point::new(28.0, 42.0), 3.0), Some(0));
    doc.annotations.push(Annotation::Stroke(Stroke {
        points: vec![Point::new(100.0, 100.0)],
        color: RED,
        width: 6.0,
    }));
    assert_eq!(
        renderer.annotation_at(&doc, Point::new(102.0, 100.0), 0.0),
        Some(1)
    );
    assert_eq!(renderer.annotation_at(&doc, Point::new(104.0, 100.0), 0.0), None);
    assert_eq!(
        renderer.annotation_at(&doc, Point::new(104.0, 100.0), 2.0),
        Some(1)
    );
}

#[test]
fn history_restores_every_annotation_and_crop_without_touching_pixels() {
    let mut doc = blank();
    let mut history = History::new(&doc);
    let mut states = vec![doc.clone()];
    for annotation in samples() {
        doc.annotations.push(annotation);
        history.commit(&doc);
        states.push(doc.clone());
    }
    for crop in [
        Rect {
            x: 10,
            y: 20,
            width: 80,
            height: 90,
        },
        Rect {
            x: 30,
            y: 40,
            width: 50,
            height: 60,
        },
    ] {
        doc.crop = Some(crop);
        history.commit(&doc);
        states.push(doc.clone());
    }
    // Image data is not part of history, even if the caller replaces pixels.
    doc.image.put_pixel(0, 0, Rgba([0, 0, 255, 255]));
    let mut renderer = Renderer::new();
    for state in states[..states.len() - 1].iter().rev() {
        assert!(history.undo(&mut doc));
        assert_eq!(doc.annotations, state.annotations);
        assert_eq!(doc.crop, state.crop);
        assert_eq!(*doc.image.get_pixel(0, 0), Rgba([0, 0, 255, 255]));
    }
    let unchanged = renderer.export(&doc);
    assert!(!history.undo(&mut doc));
    assert_eq!(renderer.export(&doc), unchanged);
    for state in &states[1..] {
        assert!(history.redo(&mut doc));
        assert_eq!(doc.annotations, state.annotations);
        assert_eq!(doc.crop, state.crop);
    }
    let unchanged = renderer.export(&doc);
    assert!(!history.redo(&mut doc));
    assert_eq!(renderer.export(&doc), unchanged);
}

#[test]
fn no_op_commits_preserve_redo_and_new_edits_discard_it() {
    let mut doc = blank();
    let mut history = History::new(&doc);
    history.commit(&doc);
    assert!(!history.undo(&mut doc));
    doc.annotations
        .push(arrow(Point::new(20.0, 40.0), Point::new(100.0, 40.0), 4.0));
    history.commit(&doc);
    assert!(history.undo(&mut doc));
    history.commit(&doc);
    assert!(history.redo(&mut doc));
    assert!(history.undo(&mut doc));
    doc.annotations
        .push(rectangle(Point::new(30.0, 40.0), Point::new(100.0, 100.0), 4.0));
    history.commit(&doc);
    assert!(!history.redo(&mut doc));
    assert!(history.undo(&mut doc));
    assert!(doc.annotations.is_empty());
    assert!(!history.undo(&mut doc), "no hidden no-op step");
    assert_eq!(
        Renderer::new().export(&doc),
        doc.image,
        "undone ink cannot reappear on export"
    );
}

#[test]
fn a_text_session_and_reedit_each_undo_in_one_step() {
    let mut doc = blank();
    let mut history = History::new(&doc);
    doc.annotations.push(Annotation::Text(Text {
        origin: Point::new(20.0, 30.0),
        content: String::new(),
        size: 20.0,
        color: RED,
    }));
    for c in "Hello".chars() {
        let Annotation::Text(text) = &mut doc.annotations[0] else {
            unreachable!()
        };
        text.content.push(c);
    }
    history.commit(&doc);
    let original = doc.annotations.clone();
    let Annotation::Text(text) = &mut doc.annotations[0] else {
        unreachable!()
    };
    text.content.push_str(" world");
    history.commit(&doc);
    assert!(history.undo(&mut doc));
    assert_eq!(doc.annotations, original);
    assert!(history.undo(&mut doc));
    assert!(doc.annotations.is_empty());
    assert!(history.redo(&mut doc));
    assert_eq!(doc.annotations, original);
}
