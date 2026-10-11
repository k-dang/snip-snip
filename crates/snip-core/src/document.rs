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

    /// Snaps an arrow endpoint to 45° steps about `anchor`, preserving length.
    pub fn snap_45(self, anchor: Self) -> Self {
        let dx = self.x - anchor.x;
        let dy = self.y - anchor.y;
        // Keep constrained endpoints exact so unchanged edits remain history no-ops.
        if dx == 0.0 || dy == 0.0 || dx.abs() == dy.abs() {
            return self;
        }
        let octant = (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round() as i32;
        let length = dx.hypot(dy);
        let diagonal = length * std::f32::consts::FRAC_1_SQRT_2;
        let (x, y) = match octant.rem_euclid(8) {
            0 => (length, 0.0),
            1 => (diagonal, diagonal),
            2 => (0.0, length),
            3 => (-diagonal, diagonal),
            4 => (-length, 0.0),
            5 => (-diagonal, -diagonal),
            6 => (0.0, -length),
            _ => (diagonal, -diagonal),
        };
        Self::new(anchor.x + x, anchor.y + y)
    }

    /// A square's dragged corner, using the larger displacement from `anchor`.
    pub fn square_corner(self, anchor: Self) -> Self {
        let dx = self.x - anchor.x;
        let dy = self.y - anchor.y;
        let side = dx.abs().max(dy.abs());
        Self::new(anchor.x + side.copysign(dx), anchor.y + side.copysign(dy))
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
    Arrow(Arrow),
    Rectangle(Rectangle),
}

/// An arrow whose filled head ends exactly at `head`.
#[derive(Clone, Debug, PartialEq)]
pub struct Arrow {
    pub tail: Point,
    pub head: Point,
    pub color: Color,
    pub width: f32,
}

/// Shared by rasterization, hit testing, bounds, and live UI painting.
#[derive(Clone, Copy, Debug)]
pub struct ArrowGeometry {
    pub shaft: [Point; 2],
    pub head: [Point; 3],
}

impl Arrow {
    /// Short arrows shrink the head to fit; the shaft has butt caps so it
    /// never projects beyond the tip. Empty arrows have no geometry.
    pub fn geometry(&self) -> Option<ArrowGeometry> {
        let dx = self.head.x - self.tail.x;
        let dy = self.head.y - self.tail.y;
        let length = dx.hypot(dy);
        if length <= 0.0 || self.width <= 0.0 {
            return None;
        }
        let (ux, uy) = (dx / length, dy / length);
        let head_length = (self.width * 4.0).min(length);
        let half_width = head_length / 2.0;
        let base = Point::new(self.head.x - ux * head_length, self.head.y - uy * head_length);
        Some(ArrowGeometry {
            shaft: [self.tail, base],
            head: [
                self.head,
                Point::new(base.x - uy * half_width, base.y + ux * half_width),
                Point::new(base.x + uy * half_width, base.y - ux * half_width),
            ],
        })
    }
}

/// An unfilled box, with opposite corners in any order, in image pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Rectangle {
    pub start: Point,
    pub end: Point,
    pub color: Color,
    pub width: f32,
}

/// Fractional image-pixel bounds. Renderer bounds also cover rasterization's
/// antialiasing fringe; they are independent of the image clip and crop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
    pub fn from_corners(a: Point, b: Point) -> Self {
        Self {
            min: Point::new(a.x.min(b.x), a.y.min(b.y)),
            max: Point::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn grow(self, amount: f32) -> Self {
        Self {
            min: Point::new(self.min.x - amount, self.min.y - amount),
            max: Point::new(self.max.x + amount, self.max.y + amount),
        }
    }

    pub fn contains(self, point: Point) -> bool {
        (self.min.x..=self.max.x).contains(&point.x) && (self.min.y..=self.max.y).contains(&point.y)
    }

    pub(crate) fn include(&mut self, point: Point) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
    }
}

impl Rectangle {
    pub fn corners(&self) -> [Point; 4] {
        let b = Bounds::from_corners(self.start, self.end);
        [
            b.min,
            Point::new(b.max.x, b.min.y),
            b.max,
            Point::new(b.min.x, b.max.y),
        ]
    }

    pub fn is_empty(&self) -> bool {
        self.start.x == self.end.x || self.start.y == self.end.y || self.width <= 0.0
    }
}

impl Annotation {
    /// Translates geometry without changing style or clamping to the image.
    pub fn translate(&mut self, offset: Point) {
        let move_point = |p: &mut Point| {
            p.x += offset.x;
            p.y += offset.y;
        };
        match self {
            Self::Stroke(stroke) => stroke.points.iter_mut().for_each(move_point),
            Self::Text(text) => move_point(&mut text.origin),
            Self::Arrow(arrow) => {
                move_point(&mut arrow.tail);
                move_point(&mut arrow.head);
            }
            Self::Rectangle(rectangle) => {
                move_point(&mut rectangle.start);
                move_point(&mut rectangle.end);
            }
        }
    }
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
