//! Copy to cloud through rendered input, with the bucket held in memory.

use std::sync::{Arc, atomic::Ordering};

use super::*;
use crate::cloud::testing::{MemoryStore, signed_in};

fn signed_in_editor(cx: &mut TestAppContext) -> (VisualTestContext, Arc<MemoryStore>) {
    let store = Arc::new(MemoryStore::default());
    cx.update(|cx| cx.set_global(signed_in(store.clone())));
    (editor(cx), store)
}

fn copy_to_cloud(cx: &mut VisualTestContext) {
    let modifier = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    cx.simulate_keystrokes(&format!("{modifier}-u"));
    cx.run_until_parked();
}

#[gpui::test]
fn copy_to_cloud_copies_uploads_and_closes(cx: &mut TestAppContext) {
    let effects = save_flow::TestExports::configure(cx);
    let (mut editor, store) = signed_in_editor(cx);
    toolbar(&mut editor, "Export");
    assert!(editor.debug_bounds("Copy to cloud").is_some());
    copy_to_cloud(&mut editor);
    assert_eq!(editor.cx.read(|app| app.windows().len()), 0);
    let uploaded = store.snips.lock().unwrap()[0].1.clone();
    assert_image(uploaded, &expected(vec![]));
    assert_image(effects.clipboard.borrow().clone().unwrap(), &expected(vec![]));
}

#[gpui::test]
fn a_failed_upload_keeps_the_editor_and_the_copy(cx: &mut TestAppContext) {
    let effects = save_flow::TestExports::configure(cx);
    let (mut editor, store) = signed_in_editor(cx);
    store.offline.store(true, Ordering::Relaxed);
    copy_to_cloud(&mut editor);
    assert_eq!(editor.cx.read(|app| app.windows().len()), 1);
    assert!(
        editor
            .debug_bounds("Copy to cloud failed: the network is unreachable")
            .is_some()
    );
    assert_image(effects.clipboard.borrow().clone().unwrap(), &expected(vec![]));
    store.offline.store(false, Ordering::Relaxed);
    copy_to_cloud(&mut editor);
    assert_eq!(editor.cx.read(|app| app.windows().len()), 0);
    assert_eq!(store.snips.lock().unwrap().len(), 1);
}

#[gpui::test]
fn signed_out_editors_offer_no_cloud_copy(cx: &mut TestAppContext) {
    save_flow::TestExports::configure(cx);
    let mut editor = editor(cx);
    toolbar(&mut editor, "Export");
    assert!(editor.debug_bounds("Copy to cloud").is_none());
    copy_to_cloud(&mut editor);
    assert_eq!(editor.cx.read(|app| app.windows().len()), 1);
}
