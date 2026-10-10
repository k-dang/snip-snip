use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, fontdb};
use image::{ImageEncoder, RgbaImage, codecs::png::PngEncoder};
use tiny_skia::{LineCap, LineJoin, Paint, PathBuilder, Pixmap, Transform};

use crate::{Annotation, Arrow, Bounds, Color, Document, Point, Rectangle, Stroke, Text};

/// The bundled annotation font. Bundling (rather than using system fonts)
/// makes text identical on every machine and fast to load.
const FONT: &[u8] = include_bytes!("../fonts/Inter-SemiBold.ttf");

/// Line height as a multiple of font size.
const LINE_HEIGHT: f32 = 1.2;

/// Rasterizes documents. Owns the font and glyph caches, so create one and
/// reuse it for every render.
pub struct Renderer {
    fonts: FontSystem,
    glyphs: SwashCache,
    family: String,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_font_data(FONT.to_vec());
        let family = db.faces().next().expect("bundled font loads").families[0]
            .0
            .clone();
        Self {
            fonts: FontSystem::new_with_locale_and_db("en-US".into(), db),
            glyphs: SwashCache::new(),
            family,
        }
    }

    /// The screenshot with every annotation painted on, ignoring the crop.
    /// The editor previews this so the user can still adjust the crop.
    pub fn composite(&mut self, doc: &Document) -> RgbaImage {
        let (width, height) = doc.image.dimensions();
        // Screenshots are opaque, so straight RGBA is also valid premultiplied RGBA.
        let size = tiny_skia::IntSize::from_wh(width, height).expect("non-empty image");
        let mut pixmap = Pixmap::from_vec(doc.image.as_raw().clone(), size).expect("RGBA buffer");
        for annotation in &doc.annotations {
            match annotation {
                Annotation::Stroke(stroke) => paint_stroke(&mut pixmap, stroke),
                Annotation::Text(text) => self.paint_text(&mut pixmap, text),
                Annotation::Arrow(arrow) => paint_arrow(&mut pixmap, arrow),
                Annotation::Rectangle(rectangle) => paint_rectangle(&mut pixmap, rectangle),
            }
        }
        RgbaImage::from_raw(width, height, pixmap.take_demultiplied()).expect("same dimensions")
    }

    /// The final image: the composite, cropped. Save and copy both use this.
    pub fn export(&mut self, doc: &Document) -> RgbaImage {
        let composite = self.composite(doc);
        match doc.crop {
            Some(c) => image::imageops::crop_imm(&composite, c.x, c.y, c.width, c.height).to_image(),
            None => composite,
        }
    }

    /// Width and height of `text`'s layout box in image pixels, for hit
    /// testing and caret placement.
    pub fn text_size(&mut self, text: &Text) -> (f32, f32) {
        let buffer = self.layout(text);
        let mut size = (0.0f32, 0.0f32);
        for run in buffer.layout_runs() {
            size.0 = size.0.max(run.line_w);
            size.1 = size.1.max(run.line_top + run.line_height);
        }
        // An empty label still occupies one line so the caret has a height.
        (size.0, size.1.max(text.size * LINE_HEIGHT))
    }

    /// Finds the last-painted annotation under `point`, ignoring the crop.
    /// `tolerance` is nonnegative image-pixel slop, supplied by the caller.
    pub fn annotation_at(&mut self, doc: &Document, point: Point, tolerance: f32) -> Option<usize> {
        let tolerance = tolerance.max(0.0);
        doc.annotations.iter().rposition(|annotation| match annotation {
            Annotation::Stroke(stroke) => {
                if stroke.width <= 0.0 {
                    return false;
                }
                let radius = stroke.width / 2.0 + tolerance;
                match stroke.points.as_slice() {
                    [] => false,
                    [dot] => distance_to_segment(point, *dot, *dot) <= radius,
                    points => points
                        .windows(2)
                        .any(|p| distance_to_segment(point, p[0], p[1]) <= radius),
                }
            }
            Annotation::Arrow(arrow) => arrow.geometry().is_some_and(|geometry| {
                let [a, b, c] = geometry.head;
                inside_triangle(point, a, b, c)
                    || [(a, b), (b, c), (c, a)]
                        .iter()
                        .any(|&(a, b)| distance_to_segment(point, a, b) <= tolerance)
                    || shaft_contains(point, geometry.shaft, arrow.width, tolerance)
            }),
            Annotation::Rectangle(rectangle) => {
                if rectangle.is_empty() {
                    return false;
                }
                // Mitered corners form a rectangular ring, including their
                // square outer corners, rather than rounded segment caps.
                let bounds = Bounds::from_corners(rectangle.start, rectangle.end);
                let radius = rectangle.width / 2.0 + tolerance;
                let inner = bounds.grow(-radius);
                bounds.grow(radius).contains(point)
                    && !(point.x > inner.min.x
                        && point.x < inner.max.x
                        && point.y > inner.min.y
                        && point.y < inner.max.y)
            }
            Annotation::Text(text) => {
                let (width, height) = self.text_size(text);
                Bounds::from_corners(
                    text.origin,
                    Point::new(text.origin.x + width, text.origin.y + height),
                )
                .grow(tolerance)
                .contains(point)
            }
        })
    }

    /// Unclipped bounds in image pixels, including line width, glyph
    /// overhangs, and the one-pixel antialiasing fringe of vector shapes.
    pub fn bounds(&mut self, annotation: &Annotation) -> Bounds {
        match annotation {
            Annotation::Stroke(stroke) => {
                let Some(&first) = stroke.points.first() else {
                    return Bounds::from_corners(Point::new(0.0, 0.0), Point::new(0.0, 0.0));
                };
                let mut bounds = Bounds::from_corners(first, first);
                for &point in &stroke.points {
                    bounds.include(point);
                }
                bounds.grow(stroke.width.max(0.0) / 2.0 + 1.0)
            }
            Annotation::Arrow(arrow) => {
                let Some(geometry) = arrow.geometry() else {
                    return Bounds::from_corners(arrow.tail, arrow.head);
                };
                let mut bounds =
                    Bounds::from_corners(geometry.shaft[0], geometry.shaft[1]).grow(arrow.width / 2.0);
                for point in geometry.head {
                    bounds.include(point);
                }
                bounds.grow(1.0)
            }
            Annotation::Rectangle(rectangle) => Bounds::from_corners(rectangle.start, rectangle.end)
                .grow(rectangle.width.max(0.0) / 2.0 + 1.0),
            Annotation::Text(text) => {
                let (width, height) = self.text_size(text);
                let mut bounds = Bounds::from_corners(
                    text.origin,
                    Point::new(text.origin.x + width, text.origin.y + height),
                );
                let mut buffer = self.layout(text);
                let (ox, oy) = (text.origin.x.round(), text.origin.y.round());
                // Layout advance alone does not cover every glyph's ink,
                // especially accents, overhangs, and fractional origins.
                buffer.draw(
                    &mut self.fonts,
                    &mut self.glyphs,
                    cosmic_text::Color::rgba(0, 0, 0, 255),
                    |x, y, w, h, color| {
                        if color.a() != 0 {
                            bounds.include(Point::new(ox + x as f32, oy + y as f32));
                            bounds.include(Point::new(ox + x as f32 + w as f32, oy + y as f32 + h as f32));
                        }
                    },
                );
                bounds
            }
        }
    }

    fn layout(&mut self, text: &Text) -> Buffer {
        let metrics = Metrics::new(text.size, text.size * LINE_HEIGHT);
        let mut buffer = Buffer::new(&mut self.fonts, metrics);
        buffer.set_size(None, None);
        let attrs = Attrs::new().family(Family::Name(&self.family));
        buffer.set_text(&text.content, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);
        buffer
    }

    fn paint_text(&mut self, pixmap: &mut Pixmap, text: &Text) {
        let mut buffer = self.layout(text);
        let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
        let (ox, oy) = (text.origin.x.round() as i32, text.origin.y.round() as i32);
        let pixels = pixmap.data_mut();
        let color = cosmic_text::Color::rgba(text.color.r, text.color.g, text.color.b, text.color.a);
        buffer.draw(&mut self.fonts, &mut self.glyphs, color, |x, y, w, h, c| {
            for py in (oy + y)..(oy + y + h as i32) {
                for px in (ox + x)..(ox + x + w as i32) {
                    if (0..width).contains(&px) && (0..height).contains(&py) {
                        let i = (py * width + px) as usize * 4;
                        blend_over(&mut pixels[i..i + 4], c.as_rgba());
                    }
                }
            }
        });
    }
}

/// Encodes an image as PNG.
pub fn encode_png(image: &RgbaImage) -> Vec<u8> {
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("encoding to memory cannot fail");
    png
}

fn paint_stroke(pixmap: &mut Pixmap, stroke: &Stroke) {
    let Some((first, rest)) = stroke.points.split_first() else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color(skia_color(stroke.color));
    paint.anti_alias = true;

    // A click without movement is a dot.
    if rest.is_empty() {
        if let Some(dot) = PathBuilder::from_circle(first.x, first.y, stroke.width / 2.0) {
            pixmap.fill_path(
                &dot,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        return;
    }

    let mut path = PathBuilder::new();
    path.move_to(first.x, first.y);
    for p in rest {
        path.line_to(p.x, p.y);
    }
    let Some(path) = path.finish() else { return };
    let style = tiny_skia::Stroke {
        width: stroke.width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    pixmap.stroke_path(&path, &paint, &style, Transform::identity(), None);
}

fn skia_color(c: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

fn paint_arrow(pixmap: &mut Pixmap, arrow: &Arrow) {
    let Some(geometry) = arrow.geometry() else { return };
    let mut paint = Paint::default();
    paint.set_color(skia_color(arrow.color));
    paint.anti_alias = true;
    let [start, end] = geometry.shaft;
    let mut shaft = PathBuilder::new();
    shaft.move_to(start.x, start.y);
    shaft.line_to(end.x, end.y);
    if let Some(path) = shaft.finish() {
        pixmap.stroke_path(
            &path,
            &paint,
            &tiny_skia::Stroke {
                width: arrow.width,
                line_cap: LineCap::Butt,
                ..Default::default()
            },
            Transform::identity(),
            None,
        );
    }
    let mut head = PathBuilder::new();
    head.move_to(geometry.head[0].x, geometry.head[0].y);
    for point in &geometry.head[1..] {
        head.line_to(point.x, point.y);
    }
    head.close();
    if let Some(path) = head.finish() {
        pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn paint_rectangle(pixmap: &mut Pixmap, rectangle: &Rectangle) {
    if rectangle.is_empty() {
        return;
    }
    let corners = rectangle.corners();
    let mut path = PathBuilder::new();
    path.move_to(corners[0].x, corners[0].y);
    for point in &corners[1..] {
        path.line_to(point.x, point.y);
    }
    path.close();
    let Some(path) = path.finish() else { return };
    let mut paint = Paint::default();
    paint.set_color(skia_color(rectangle.color));
    paint.anti_alias = true;
    pixmap.stroke_path(
        &path,
        &paint,
        &tiny_skia::Stroke {
            width: rectangle.width,
            line_join: LineJoin::Miter,
            ..Default::default()
        },
        Transform::identity(),
        None,
    );
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / length_squared).clamp(0.0, 1.0)
    };
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

fn inside_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let cross = |a: Point, b: Point| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let sides = [cross(a, b), cross(b, c), cross(c, a)];
    sides.iter().all(|&v| v >= 0.0) || sides.iter().all(|&v| v <= 0.0)
}

fn shaft_contains(p: Point, [a, b]: [Point; 2], width: f32, tolerance: f32) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx.hypot(dy);
    if length == 0.0 {
        return false;
    }
    let along = ((p.x - a.x) * dx + (p.y - a.y) * dy) / length;
    let across = ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / length;
    along >= -tolerance && along <= length + tolerance && across <= width / 2.0 + tolerance
}

/// Source-over blend of a straight-alpha `src` onto a premultiplied pixel.
fn blend_over(dst: &mut [u8], [r, g, b, a]: [u8; 4]) {
    let a = a as u32;
    for (d, s) in dst[..3].iter_mut().zip([r, g, b]) {
        *d = ((s as u32 * a + *d as u32 * (255 - a) + 127) / 255) as u8;
    }
    dst[3] = (a + (dst[3] as u32 * (255 - a) + 127) / 255) as u8;
}
