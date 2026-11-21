//! Drive the Library and the tray's copy through rendered input, with the
//! bucket held in memory.

use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, FixedOffset};
use gpui::{AsyncWindowContext, Modifiers, Task, TestAppContext, VisualTestContext};
use image::Rgba;
use snip_core::RgbaImage;

use crate::cloud::{
    CloudStore,
    testing::{MemoryStore, signed_in},
};
use crate::editor::{ExportEnvironment, ExportServices};

#[derive(Default)]
struct Clipboard(RefCell<Option<RgbaImage>>);

impl ExportEnvironment for Clipboard {
    fn directories(&self) -> (Option<PathBuf>, Option<PathBuf>) {
        unreachable!("the Library never saves files")
    }
    fn local_time(&self) -> Result<DateTime<FixedOffset>> {
        unreachable!("the Library never saves files")
    }
    fn write_clipboard(&self, image: RgbaImage) -> Result<()> {
        self.0.replace(Some(image));
        Ok(())
    }
    fn choose_save_path(
        self: Rc<Self>,
        _: PathBuf,
        _: String,
        _: &mut AsyncWindowContext,
    ) -> Task<Result<Option<PathBuf>>> {
        unreachable!("the Library never saves files")
    }
}

fn solid(shade: u8) -> RgbaImage {
    RgbaImage::from_pixel(64, 48, Rgba([shade, shade, shade, 255]))
}

fn setup(cx: &mut TestAppContext, images: &[RgbaImage]) -> (Arc<MemoryStore>, Rc<Clipboard>) {
    let store = Arc::new(MemoryStore::default());
    for image in images {
        store.upload(image).unwrap();
    }
    let clipboard = Rc::new(Clipboard::default());
    cx.update(|cx| {
        cx.set_global(signed_in(store.clone()));
        cx.set_global(ExportServices(clipboard.clone()));
    });
    (store, clipboard)
}

fn open_library(cx: &mut TestAppContext) -> VisualTestContext {
    cx.update(super::open);
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    VisualTestContext::from_window(window, cx)
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let point = cx
        .debug_bounds(selector)
        .expect("rendered Library control")
        .center();
    cx.simulate_click(point, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn clicking_a_snip_copies_it(cx: &mut TestAppContext) {
    let (older, newer) = (solid(40), solid(200));
    let (_, clipboard) = setup(cx, &[older.clone(), newer.clone()]);
    let mut library = open_library(cx);
    assert!(library.debug_bounds("snip-1").is_some());
    assert!(library.debug_bounds("snip-2").is_none());
    click(&mut library, "snip-1");
    assert_eq!(clipboard.0.borrow().as_ref(), Some(&older));
    click(&mut library, "snip-0");
    assert_eq!(clipboard.0.borrow().as_ref(), Some(&newer));
}

#[gpui::test]
fn deleting_takes_a_confirming_click(cx: &mut TestAppContext) {
    let (store, clipboard) = setup(cx, &[solid(90)]);
    let mut library = open_library(cx);
    click(&mut library, "delete-0");
    assert_eq!(store.snips.lock().unwrap().len(), 1);
    click(&mut library, "delete-0");
    assert!(store.snips.lock().unwrap().is_empty());
    assert!(
        library
            .debug_bounds("No cloud snips yet. Use Copy to cloud in the editor's Export menu.")
            .is_some()
    );
    assert!(clipboard.0.borrow().is_none(), "Delete must not also copy");
}

#[gpui::test]
fn the_tray_copies_the_newest_snip_or_explains(cx: &mut TestAppContext) {
    let (store, clipboard) = setup(cx, &[]);
    cx.update(super::copy_latest);
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut library = VisualTestContext::from_window(window, cx);
    assert!(library.debug_bounds("There are no cloud snips yet.").is_some());

    store.upload(&solid(10)).unwrap();
    let newest = solid(250);
    store.upload(&newest).unwrap();
    cx.update(super::copy_latest);
    cx.run_until_parked();
    assert_eq!(clipboard.0.borrow().as_ref(), Some(&newest));
}
