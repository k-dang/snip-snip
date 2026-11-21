//! Drive the Library and the tray's copy through rendered input, with the
//! bucket held in memory.

use std::{cell::RefCell, rc::Rc, sync::Arc};

use anyhow::Result;
use gpui::{Modifiers, TestAppContext, VisualTestContext};
use image::Rgba;
use snip_core::RgbaImage;

use crate::{
    clipboard::{Clipboard, ClipboardService},
    cloud::{
        CloudStore,
        testing::{MemoryStore, signed_in},
    },
};

#[derive(Default)]
struct TestClipboard(RefCell<Option<RgbaImage>>);

impl Clipboard for TestClipboard {
    fn write(&self, image: &RgbaImage) -> Result<()> {
        self.0.replace(Some(image.clone()));
        Ok(())
    }
}

fn solid(shade: u8) -> RgbaImage {
    RgbaImage::from_pixel(64, 48, Rgba([shade, shade, shade, 255]))
}

fn setup(cx: &mut TestAppContext, images: &[RgbaImage]) -> (Arc<MemoryStore>, Rc<TestClipboard>) {
    let store = Arc::new(MemoryStore::default());
    for image in images {
        store.upload(image).unwrap();
    }
    let clipboard = Rc::new(TestClipboard::default());
    cx.update(|cx| {
        cx.set_global(signed_in(store.clone()));
        cx.set_global(ClipboardService(clipboard.clone()));
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
    click(&mut library, "Delete");
    assert_eq!(store.snips.lock().unwrap().len(), 1);
    click(&mut library, "Confirm delete");
    assert!(store.snips.lock().unwrap().is_empty());
    assert!(
        library
            .debug_bounds("No cloud snips yet. Use Copy to cloud in the editor's Export menu.")
            .is_some()
    );
    assert!(clipboard.0.borrow().is_none(), "Delete must not also copy");
}

#[gpui::test]
fn signing_in_to_another_account_replaces_the_library(cx: &mut TestAppContext) {
    let (_, clipboard) = setup(cx, &[solid(40), solid(80)]);
    let mut library = open_library(cx);
    assert!(library.debug_bounds("snip-1").is_some());
    let other = Arc::new(MemoryStore::default());
    let other_snip = solid(160);
    other.upload(&other_snip).unwrap();
    cx.update(|cx| cx.set_global(signed_in(other)));
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut library = VisualTestContext::from_window(window, cx);
    assert!(library.debug_bounds("snip-1").is_none());
    click(&mut library, "snip-0");
    assert_eq!(clipboard.0.borrow().as_ref(), Some(&other_snip));
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
