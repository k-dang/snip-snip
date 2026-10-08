use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, fontdb};
use image::{ImageEncoder, RgbaImage, codecs::png::PngEncoder};
use tiny_skia::{LineCap, LineJoin, Paint, PathBuilder, Pixmap, Transform};

use crate::{Annotation, Color, Document, Stroke, Text};

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

/// Source-over blend of a straight-alpha `src` onto a premultiplied pixel.
fn blend_over(dst: &mut [u8], [r, g, b, a]: [u8; 4]) {
    let a = a as u32;
    for (d, s) in dst[..3].iter_mut().zip([r, g, b]) {
        *d = ((s as u32 * a + *d as u32 * (255 - a) + 127) / 255) as u8;
    }
    dst[3] = (a + (dst[3] as u32 * (255 - a) + 127) / 255) as u8;
}
