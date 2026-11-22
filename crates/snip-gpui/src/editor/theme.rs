//! The editor's colors. Settings and Library use `crate::ui`'s palette instead.

use gpui::{Background, linear_color_stop, linear_gradient, rgb};

/// Popovers, tooltips, and toasts.
pub(super) const SURFACE: u32 = 0x1F1F25;
pub(super) const BORDER: u32 = 0x35353F;
/// Floating dock and toast edges, over any canvas color.
pub(super) const EDGE: u32 = 0xFFFFFF17;
pub(super) const HOVER: u32 = 0x2E2E37;
pub(super) const SELECTED: u32 = 0x3A3A46;
pub(super) const TEXT: u32 = 0xE4E4EA;
pub(super) const MUTED: u32 = 0xA1A1AD;
pub(super) const ICON: u32 = 0xA6A6B2;
pub(super) const SUCCESS: u32 = 0x5EE0A0;
pub(super) const ERROR: u32 = 0xFF6B6B;

/// The selected tool and the Export button share one violet-to-blue fill.
pub(super) fn accent() -> Background {
    linear_gradient(
        135.,
        linear_color_stop(rgb(0x8B6CFF), 0.),
        linear_color_stop(rgb(0x4F8CFF), 1.),
    )
}
