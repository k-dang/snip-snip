//! Embeds the snip-snip mark as the Windows executable icon. GPUI loads icon
//! resource 1 for every window's title bar, and Explorer and the taskbar use
//! it too.

#[path = "src/icon.rs"]
mod icon;

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=src/icon.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let ico = out.join("snip-snip.ico");
    image::RgbaImage::from_raw(icon::SIZE, icon::SIZE, icon::pixels(false))
        .unwrap()
        .save(&ico)
        .unwrap();
    let rc = out.join("snip-snip.rc");
    // The resource compiler reads backslashes in string literals as escapes.
    let ico = ico.display().to_string().replace('\\', "/");
    fs::write(&rc, format!("1 ICON \"{ico}\"\n")).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
