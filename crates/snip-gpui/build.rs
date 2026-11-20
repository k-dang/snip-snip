//! Embeds the snip-snip mark as the Windows executable icon. GPUI loads icon
//! resource 1 for every window's title bar, and Explorer and the taskbar use
//! it too.

#[path = "src/shell/icon.rs"]
mod icon;

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=src/shell/icon.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let ico = out.join("snip-snip.ico");
    fs::write(&ico, encode_ico(&icon::pixels(false), icon::SIZE)).unwrap();
    let rc = out.join("snip-snip.rc");
    fs::write(
        &rc,
        format!("1 ICON \"{}\"\n", ico.display().to_string().replace('\\', "/")),
    )
    .unwrap();
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}

/// A single-image ICO holding a 32-bit BMP: BGRA rows bottom-up, followed by
/// an all-zero AND mask because the alpha channel carries transparency.
fn encode_ico(rgba: &[u8], size: usize) -> Vec<u8> {
    let mask_row = size.div_ceil(32) * 4;
    let image_len = 40 + rgba.len() + mask_row * size;
    let side = size as u8; // ICO stores 256 as 0; snip-snip's mark is 32.
    let mut ico = Vec::with_capacity(6 + 16 + image_len);
    ico.extend([0, 0, 1, 0, 1, 0]);
    ico.extend([side, side, 0, 0, 1, 0, 32, 0]);
    ico.extend((image_len as u32).to_le_bytes());
    ico.extend(22_u32.to_le_bytes());
    ico.extend(40_u32.to_le_bytes());
    ico.extend((size as i32).to_le_bytes());
    ico.extend((size as i32 * 2).to_le_bytes()); // XOR image plus AND mask.
    ico.extend([1, 0, 32, 0]);
    ico.extend([0; 24]);
    for row in rgba.chunks_exact(size * 4).rev() {
        for &[r, g, b, a] in row.as_chunks::<4>().0 {
            ico.extend([b, g, r, a]);
        }
    }
    ico.resize(ico.len() + mask_row * size, 0);
    ico
}
