use image::RgbaImage;

/// A position in image pixels. Fractional so strokes keep sub-pixel precision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// An integer pixel rectangle, used for crops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// The rectangle spanned by two corners, rounded outward to whole pixels
    /// and clamped to a `width` × `height` image. `None` if it is empty.
    pub fn from_corners(a: Point, b: Point, width: u32, height: u32) -> Option<Self> {
        let clamp = |v: f32, max: u32| v.clamp(0.0, max as f32);
        let x0 = clamp(a.x.min(b.x).floor(), width) as u32;
        let y0 = clamp(a.y.min(b.y).floor(), height) as u32;
        let x1 = clamp(a.x.max(b.x).ceil(), width) as u32;
        let y1 = clamp(a.y.max(b.y).ceil(), height) as u32;
        (x1 > x0 && y1 > y0).then(|| Self {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        })
    }
}

/// Straight (non-premultiplied) RGBA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

/// A freehand pencil stroke.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub points: Vec<Point>,
    pub color: Color,
    pub width: f32,
}

/// A single-line text label. `origin` is the top-left of its layout box.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub origin: Point,
    pub content: String,
    pub size: f32,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    Stroke(Stroke),
    Text(Text),
}

/// The editable state of one screenshot. `image` is never modified; edits
/// live in `annotations` (painted in order) and `crop` (applied on export).
#[derive(Clone, Debug)]
pub struct Document {
    pub image: RgbaImage,
    pub annotations: Vec<Annotation>,
    pub crop: Option<Rect>,
}

impl Document {
    /// `image` is a screenshot and is assumed to be fully opaque.
    pub fn new(image: RgbaImage) -> Self {
        Self {
            image,
            annotations: Vec::new(),
            crop: None,
        }
    }
}
