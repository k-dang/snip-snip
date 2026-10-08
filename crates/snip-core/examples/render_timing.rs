//! Times the editor's per-edit work on a large screenshot:
//! `cargo run --release -p snip-core --example render_timing [width height]`.

use std::time::Instant;

use snip_core::{Annotation, Color, Document, Point, Renderer, RgbaImage, Stroke, Text};

fn main() {
    let mut args = std::env::args()
        .skip(1)
        .map(|a| a.parse::<u32>().expect("pixel size"));
    let (width, height) = (args.next().unwrap_or(5120), args.next().unwrap_or(2880));
    let mut doc = Document::new(RgbaImage::from_pixel(
        width,
        height,
        image::Rgba([40, 44, 52, 255]),
    ));
    let mut renderer = Renderer::new();

    for i in 0..20 {
        let y = (i + 1) as f32 * height as f32 / 22.0;
        let points =
            (0..200).map(|x| Point::new(x as f32 * width as f32 / 200.0, y + (x as f32 / 7.0).sin() * 20.0));
        doc.annotations.push(Annotation::Stroke(Stroke {
            points: points.collect(),
            color: Color::rgb(229, 72, 77),
            width: 6.0,
        }));
        doc.annotations.push(Annotation::Text(Text {
            origin: Point::new(40.0, y - 60.0),
            content: format!("Annotation {i}"),
            size: 40.0,
            color: Color::rgb(229, 72, 77),
        }));
    }

    renderer.composite(&doc); // warm the glyph cache
    let runs = 10;
    let started = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(renderer.composite(&doc));
    }
    println!(
        "{width}x{height}, {} annotations: composite {:?}",
        doc.annotations.len(),
        started.elapsed() / runs
    );

    let started = Instant::now();
    let png = snip_core::encode_png(&renderer.export(&doc));
    println!(
        "export + PNG encode {:?} ({} KB)",
        started.elapsed(),
        png.len() / 1024
    );
}
