//! The snip-snip mark: four open corners on a transparent background.
//!
//! Shared by the tray/menu-bar icon and `build.rs`, which embeds it as the
//! Windows executable icon that GPUI uses for window title bars.

pub const SIZE: usize = 32;

/// RGBA pixels, `SIZE` x `SIZE`, top row first. A `template` icon is a plain
/// alpha shape for AppKit to tint; otherwise white strokes get a one-pixel
/// dark outline so they read on light and dark backgrounds.
pub fn pixels(template: bool) -> Vec<u8> {
    let mut pixels = vec![0_u8; SIZE * SIZE * 4];
    let layers: &[(usize, u8)] = if template {
        &[(0, 30)]
    } else {
        &[(1, 30), (0, 245)]
    };
    for &(padding, ink) in layers {
        let side = |n| (4 - padding..8 + padding).contains(&n) || (24 - padding..28 + padding).contains(&n);
        let arm = |n| (4 - padding..14 + padding).contains(&n) || (18 - padding..28 + padding).contains(&n);
        for y in 0..SIZE {
            for x in 0..SIZE {
                if side(x) && arm(y) || side(y) && arm(x) {
                    let offset = (y * SIZE + x) * 4;
                    pixels[offset..offset + 4].copy_from_slice(&[ink, ink, ink, 255]);
                }
            }
        }
    }
    pixels
}
