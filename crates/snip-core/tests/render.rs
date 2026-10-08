use image::Rgba;
use snip_core::{Annotation, Color, Document, Point, Rect, Renderer, Stroke, Text};

const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
const RED: Color = Color::rgb(255, 0, 0);

fn blank(width: u32, height: u32) -> Document {
    Document::new(image::RgbaImage::from_pixel(width, height, WHITE))
}

#[test]
fn crop_selects_the_right_region_at_full_resolution() {
    let mut doc = blank(100, 80);
    doc.image.put_pixel(30, 20, Rgba([0, 0, 255, 255]));
    doc.crop = Some(Rect {
        x: 30,
        y: 20,
        width: 40,
        height: 10,
    });

    let out = Renderer::new().export(&doc);

    assert_eq!(out.dimensions(), (40, 10));
    assert_eq!(out.get_pixel(0, 0), &Rgba([0, 0, 255, 255]));
}

#[test]
fn strokes_land_on_image_pixels_and_survive_the_crop() {
    let mut doc = blank(100, 100);
    doc.annotations.push(Annotation::Stroke(Stroke {
        points: vec![Point::new(10.0, 50.0), Point::new(90.0, 50.0)],
        color: RED,
        width: 6.0,
    }));
    doc.crop = Some(Rect {
        x: 40,
        y: 40,
        width: 20,
        height: 20,
    });

    let out = Renderer::new().export(&doc);

    // Image (50, 50) is crop (10, 10).
    assert_eq!(out.get_pixel(10, 10), &Rgba([255, 0, 0, 255]));
    assert_eq!(out.get_pixel(10, 0), &WHITE);
}

#[test]
fn a_click_without_movement_draws_a_dot() {
    let mut doc = blank(20, 20);
    doc.annotations.push(Annotation::Stroke(Stroke {
        points: vec![Point::new(10.0, 10.0)],
        color: RED,
        width: 6.0,
    }));

    let out = Renderer::new().composite(&doc);

    assert_eq!(out.get_pixel(10, 10), &Rgba([255, 0, 0, 255]));
}

#[test]
fn text_paints_only_inside_its_reported_bounds() {
    let mut renderer = Renderer::new();
    let text = Text {
        origin: Point::new(20.0, 30.0),
        content: "Hello".into(),
        size: 24.0,
        color: RED,
    };
    let (w, h) = renderer.text_size(&text);
    assert!(w > 40.0 && h >= 24.0, "unexpected text size {w}x{h}");

    let mut doc = blank(200, 100);
    doc.annotations.push(Annotation::Text(text));
    let out = renderer.composite(&doc);

    let mut inked = 0;
    for (x, y, p) in out.enumerate_pixels() {
        if p != &WHITE {
            let inside = (20.0..=20.0 + w + 1.0).contains(&(x as f32))
                && (30.0..=30.0 + h + 1.0).contains(&(y as f32));
            assert!(inside, "ink outside text bounds at ({x}, {y})");
            inked += 1;
        }
    }
    assert!(inked > 100, "text drew almost nothing ({inked} pixels)");
}

#[test]
fn rect_from_corners_normalizes_and_clamps() {
    let r = Rect::from_corners(Point::new(50.5, 40.0), Point::new(-10.0, 10.2), 40, 100);
    assert_eq!(
        r,
        Some(Rect {
            x: 0,
            y: 10,
            width: 40,
            height: 30
        })
    );
    assert_eq!(
        Rect::from_corners(Point::new(5.0, 5.0), Point::new(5.0, 9.0), 40, 40),
        None
    );
}
