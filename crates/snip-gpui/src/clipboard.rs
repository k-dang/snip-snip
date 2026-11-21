//! The system clipboard, substituted in tests.

use std::{borrow::Cow, rc::Rc};

use anyhow::Result;
use gpui::{App, Global};
use snip_core::RgbaImage;

pub trait Clipboard {
    fn write(&self, image: &RgbaImage) -> Result<()>;
}

pub struct ClipboardService(pub Rc<dyn Clipboard>);

impl Global for ClipboardService {}

impl Default for ClipboardService {
    fn default() -> Self {
        Self(Rc::new(SystemClipboard))
    }
}

struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn write(&self, image: &RgbaImage) -> Result<()> {
        // GPUI only writes PNG; arboard also supplies Windows bitmap formats.
        let data = arboard::ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: Cow::Borrowed(image.as_raw()),
        };
        arboard::Clipboard::new()?.set_image(data)?;
        Ok(())
    }
}

pub fn write(image: &RgbaImage, cx: &mut App) -> Result<()> {
    cx.default_global::<ClipboardService>().0.clone().write(image)
}
