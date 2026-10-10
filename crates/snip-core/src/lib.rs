//! Framework-agnostic screenshot document and export renderer.
//!
//! A [`Document`] is the original screenshot, an ordered list of annotations,
//! and an optional crop. All coordinates are in image pixels, so the UI's zoom
//! level never affects the output. [`Renderer`] produces the final image; the
//! editor preview and save/copy all use it, so what you see is what you export.

mod document;
mod history;
mod render;

pub use document::{
    Annotation, Arrow, ArrowGeometry, Bounds, Color, Document, Point, Rect, Rectangle, Stroke, Text,
};
pub use history::History;
pub use image::RgbaImage;
pub use render::{Renderer, encode_png};
