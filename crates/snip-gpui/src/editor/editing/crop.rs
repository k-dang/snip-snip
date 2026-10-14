//! Crop edits retain their starting geometry so live adjustment never drifts.
use super::{DRAG_THRESHOLD, Editing, Gesture, HANDLE_HIT_RADIUS, Handle, PointerHint};
use snip_core::{Point, Rect};

pub(super) enum CropGesture {
    Create {
        start: Point,
        previous: Option<Rect>,
    },
    Resize {
        original: Rect,
        position: Point,
        opposite: Point,
        grab: Point,
        started: bool,
    },
}

impl CropGesture {
    pub fn previous(&self) -> Option<Rect> {
        match self {
            Self::Create { previous, .. } => *previous,
            Self::Resize { original, .. } => Some(*original),
        }
    }

    pub fn update(&mut self, p: Point, (width, height): (u32, u32), scale: f32) -> Option<Rect> {
        match self {
            Self::Create { start, .. } => {
                if start.x == p.x || start.y == p.y {
                    None
                } else {
                    Rect::from_corners(*start, p, width, height)
                }
            }
            Self::Resize {
                original,
                position,
                opposite,
                grab,
                started,
            } => {
                let offset = Point::new(p.x - grab.x, p.y - grab.y);
                *started |= offset.x.hypot(offset.y) * scale >= DRAG_THRESHOLD;
                if !*started {
                    return Some(*original);
                }
                // Round outward, and stop one pixel before the fixed corner.
                let boundary = |start: f32, fixed: f32, delta: f32, limit: u32| {
                    if start < fixed {
                        (start + delta).floor().clamp(0., fixed - 1.)
                    } else {
                        (start + delta).ceil().clamp(fixed + 1., limit as f32)
                    }
                };
                let corner = Point::new(
                    boundary(position.x, opposite.x, offset.x, width),
                    boundary(position.y, opposite.y, offset.y, height),
                );
                Rect::from_corners(corner, *opposite, width, height)
            }
        }
    }
}

pub(crate) fn crop_handles(crop: Rect) -> [(Point, Handle); 4] {
    let (left, top) = (crop.x as f32, crop.y as f32);
    let (right, bottom) = ((crop.x + crop.width) as f32, (crop.y + crop.height) as f32);
    let corners = [
        Point::new(left, top),
        Point::new(right, top),
        Point::new(right, bottom),
        Point::new(left, bottom),
    ];
    std::array::from_fn(|i| {
        (
            corners[i],
            Handle::RectangleCorner {
                opposite: corners[(i + 2) % 4],
            },
        )
    })
}

impl Editing {
    fn crop_handle_at(&self, p: Point, scale: f32) -> Option<(Point, Handle)> {
        crop_handles(self.doc.crop?)
            .into_iter()
            .map(|(position, handle)| {
                let distance = (position.x - p.x).hypot(position.y - p.y);
                (position, handle, distance)
            })
            .filter(|(_, _, distance)| *distance <= HANDLE_HIT_RADIUS / scale)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(position, handle, _)| (position, handle))
    }

    pub(super) fn begin_crop(&mut self, p: Point, scale: f32, inside: bool) {
        if let Some((position, Handle::RectangleCorner { opposite })) = self.crop_handle_at(p, scale) {
            self.gesture = Some(Gesture::Crop(CropGesture::Resize {
                original: self.doc.crop.unwrap(),
                position,
                opposite,
                grab: p,
                started: false,
            }));
        } else if inside {
            let previous = self.doc.crop.take();
            self.gesture = Some(Gesture::Crop(CropGesture::Create { start: p, previous }));
        }
    }

    pub(super) fn crop_hint(&self, p: Point, scale: f32) -> PointerHint {
        let target = match &self.gesture {
            Some(Gesture::Crop(CropGesture::Create { .. })) => None,
            Some(Gesture::Crop(CropGesture::Resize {
                position, opposite, ..
            })) => Some((*position, Handle::RectangleCorner { opposite: *opposite })),
            _ => self.crop_handle_at(p, scale),
        };
        match target {
            Some((position, handle)) => PointerHint::Resize { handle, position },
            None => PointerHint::Crosshair,
        }
    }
}
