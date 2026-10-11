//! Export is driven through rendered input; only the OS effects are substituted.

use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};

use anyhow::{Result, anyhow};
use chrono::{DateTime, FixedOffset};
use futures::channel::oneshot;
use gpui::{AsyncWindowContext, Task};

use super::super::export::{ExportEnvironment, ExportServices, window_save_picker};
use super::*;

struct TestExports {
    root: tempfile::TempDir,
    pictures: RefCell<Option<PathBuf>>,
    home: RefCell<Option<PathBuf>>,
    time: RefCell<Result<DateTime<FixedOffset>, String>>,
    suggestions: RefCell<Vec<(PathBuf, String)>>,
    picker_windows: RefCell<Vec<Option<gpui::AnyWindowHandle>>>,
    clipboard: RefCell<Option<RgbaImage>>,
    clipboard_error: Cell<bool>,
    picker_error: Cell<bool>,
}

impl TestExports {
    fn new() -> Rc<Self> {
        let root = tempfile::tempdir().unwrap();
        Rc::new(Self {
            pictures: RefCell::new(Some(root.path().join("relocated-pictures"))),
            home: RefCell::new(Some(root.path().join("home"))),
            root,
            time: RefCell::new(Ok(
                DateTime::parse_from_rfc3339("2026-10-11T23:59:59.007-04:00").unwrap()
            )),
            suggestions: RefCell::default(),
            picker_windows: RefCell::default(),
            clipboard: RefCell::default(),
            clipboard_error: Cell::new(false),
            picker_error: Cell::new(false),
        })
    }

    fn configure(cx: &mut TestAppContext) -> Rc<Self> {
        let effects = Self::new();
        cx.update(|cx| cx.set_global(ExportServices(effects.clone())));
        effects
    }
}

impl ExportEnvironment for TestExports {
    fn directories(&self) -> (Option<PathBuf>, Option<PathBuf>) {
        (self.pictures.borrow().clone(), self.home.borrow().clone())
    }

    fn local_time(&self) -> Result<DateTime<FixedOffset>> {
        self.time.borrow().clone().map_err(|error| anyhow!(error))
    }

    fn write_clipboard(&self, image: RgbaImage) -> Result<()> {
        if self.clipboard_error.get() {
            return Err(anyhow!("clipboard unavailable"));
        }
        self.clipboard.replace(Some(image));
        Ok(())
    }

    fn choose_save_path(
        self: Rc<Self>,
        directory: PathBuf,
        name: String,
        cx: &mut AsyncWindowContext,
    ) -> Task<Result<Option<PathBuf>>> {
        window_save_picker(
            cx,
            |window, cx| {
                // Native Windows activation is queued, unlike GPUI's test platform.
                let window = window.window_handle();
                cx.spawn(async move |cx| {
                    window.update(cx, |_, window, _| window.activate_window()).ok();
                })
                .detach();
            },
            move |cx| {
                self.picker_windows.borrow_mut().push(cx.active_window());
                self.suggestions
                    .borrow_mut()
                    .push((directory.clone(), name.clone()));
                if self.picker_error.get() {
                    let (tx, rx) = oneshot::channel();
                    tx.send(Err(anyhow!("picker unavailable"))).unwrap();
                    return rx;
                }
                cx.prompt_for_new_path(&directory, Some(&name))
            },
        )
    }
}

pub(super) fn install(cx: &mut TestAppContext) {
    if cx.read(|cx| cx.try_global::<ExportServices>().is_none()) {
        TestExports::configure(cx);
    }
}

fn assert_status(cx: &mut VisualTestContext, status: String) {
    // GPUI's rendered-selector lookup requires a static string.
    assert!(cx.debug_bounds(Box::leak(status.into_boxed_str())).is_some());
}

#[gpui::test]
fn save_defers_folder_access_and_keeps_input_available_before_the_picker(cx: &mut TestAppContext) {
    let effects = TestExports::configure(cx);
    let directory = effects.pictures.borrow().as_ref().unwrap().join("snip-snip");
    let mut cx = editor(cx);
    let window = cx.update(|window, _| window.window_handle());
    let other = cx.cx.update(|app| {
        super::super::open(RgbaImage::from_pixel(320, 200, WHITE), 1., app);
        *app.windows().last().unwrap()
    });
    let mut other = VisualTestContext::from_window(other, &cx.cx);
    other.run_until_parked();
    cx.update(|window, _| window.activate_window());
    let modifier = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    // Dispatch input without draining async work, so folder preparation is pending.
    cx.cx
        .dispatch_keystroke(window, gpui::Keystroke::parse(&format!("{modifier}-s")).unwrap());
    assert!(
        !directory.exists(),
        "Save must return before accessing the folder"
    );
    assert!(!cx.cx.did_prompt_for_new_path());
    cx.cx
        .dispatch_keystroke(window, gpui::Keystroke::parse(&format!("{modifier}-c")).unwrap());
    assert_image(effects.clipboard.borrow().clone().unwrap(), &expected(vec![]));
    other.update(|window, _| window.activate_window());
    assert_ne!(cx.cx.read(|app| app.active_window()), Some(window));
    cx.run_until_parked();
    assert!(directory.is_dir());
    assert!(cx.cx.did_prompt_for_new_path());
    assert_eq!(
        effects.picker_windows.borrow().as_slice(),
        &[Some(window)],
        "the picker belongs to the editor that initiated Save"
    );
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();

    cx.cx
        .dispatch_keystroke(window, gpui::Keystroke::parse(&format!("{modifier}-s")).unwrap());
    cx.cx.dispatch_keystroke(
        window,
        gpui::Keystroke::parse(&format!("{modifier}-shift-c")).unwrap(),
    );
    cx.run_until_parked();
    assert_eq!(cx.cx.read(|app| app.windows().len()), 1);
    assert!(
        !cx.cx.did_prompt_for_new_path(),
        "a closed editor must not open a delayed picker"
    );
    assert_eq!(effects.suggestions.borrow().len(), 1);
}

#[gpui::test]
fn save_suggests_a_readable_unique_name_in_the_pictures_folder(cx: &mut TestAppContext) {
    let effects = TestExports::configure(cx);
    let directory = effects.pictures.borrow().as_ref().unwrap().join("snip-snip");
    let name = "snip-2026-10-11_23-59-59-007.png";
    let mut cx = editor(cx);
    assert!(!directory.exists(), "opening an editor creates no save folder");
    command(&mut cx, "s");
    cx.run_until_parked();
    assert_eq!(
        effects.suggestions.borrow().as_slice(),
        &[(directory.clone(), name.into())]
    );
    assert!(directory.is_dir());
    cx.simulate_new_path_selection(|default| {
        assert_eq!(default, directory);
        Some(directory.join(name))
    });
    cx.run_until_parked();
    assert_image(
        image::open(directory.join(name)).unwrap().into_rgba8(),
        &expected(vec![]),
    );
    std::fs::write(directory.join("snip-2026-10-11_23-59-59-007-2.png"), b"keep me").unwrap();
    toolbar(&mut cx, "Save…");
    cx.run_until_parked();
    assert_eq!(
        effects.suggestions.borrow().last().unwrap().1,
        "snip-2026-10-11_23-59-59-007-3.png"
    );
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    assert_eq!(
        std::fs::read(directory.join("snip-2026-10-11_23-59-59-007-2.png")).unwrap(),
        b"keep me"
    );
    assert!(!directory.join("snip-2026-10-11_23-59-59-007-3.png").exists());
}

#[gpui::test]
fn save_falls_back_to_home_and_reports_unavailable_locations_and_time(cx: &mut TestAppContext) {
    let effects = TestExports::configure(cx);
    effects.pictures.replace(None);
    let directory = effects.home.borrow().as_ref().unwrap().join("snip-snip");
    let mut cx = editor(cx);
    command(&mut cx, "s");
    cx.simulate_new_path_selection(|default| {
        assert_eq!(default, directory);
        None
    });
    cx.run_until_parked();
    assert!(directory.is_dir());
    assert!(
        cx.debug_bounds("320 × 200 px").is_some(),
        "cancel is not an error"
    );

    effects.home.replace(None);
    command(&mut cx, "s");
    cx.run_until_parked();
    assert!(!cx.cx.did_prompt_for_new_path());
    assert!(
        cx.debug_bounds("Save failed: cannot locate Pictures or the home directory")
            .is_some()
    );
    effects.home.replace(Some(effects.root.path().join("home")));
    *effects.time.borrow_mut() = Err("clock unavailable".into());
    command(&mut cx, "s");
    cx.run_until_parked();
    assert!(!cx.cx.did_prompt_for_new_path());
    assert!(cx.debug_bounds("Save failed: clock unavailable").is_some());
    *effects.time.borrow_mut() = Ok(DateTime::parse_from_rfc3339("2026-10-12T00:00:00.001-04:00").unwrap());
    command(&mut cx, "s");
    cx.run_until_parked();
    assert_eq!(
        effects.suggestions.borrow().last().unwrap().1,
        "snip-2026-10-12_00-00-00-001.png"
    );
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    assert_eq!(cx.cx.read(|app| app.windows().len()), 1);
}

fn annotated_crop(cx: &mut VisualTestContext) -> RgbaImage {
    cx.simulate_keystrokes("r");
    drag(cx, (40., 40.), (180., 100.), Modifiers::none());
    cx.simulate_keystrokes("c");
    drag(cx, (20., 20.), (220., 180.), Modifiers::none());
    cx.simulate_keystrokes("t");
    click(cx, 30., 120.);
    cx.simulate_input("Hello");
    let full = expected(vec![
        rectangle((40., 40.), (180., 100.)),
        Annotation::Text(Text {
            origin: Point::new(30., 120.),
            content: "Hello".into(),
            size: 20.,
            color: INK,
        }),
    ]);
    image::imageops::crop_imm(&full, 20, 20, 200, 160).to_image()
}

#[gpui::test]
fn save_reports_folder_picker_and_write_failures_then_retries_the_document(cx: &mut TestAppContext) {
    let effects = TestExports::configure(cx);
    let blocked = effects.root.path().join("not-a-folder");
    std::fs::write(&blocked, b"keep me").unwrap();
    effects.pictures.replace(Some(blocked.clone()));
    let mut cx = editor(cx);
    let expected = annotated_crop(&mut cx);
    command(&mut cx, "s");
    cx.run_until_parked();
    assert!(!cx.cx.did_prompt_for_new_path());
    let folder = blocked.join("snip-snip");
    let error = std::fs::create_dir_all(&folder).unwrap_err();
    assert_status(
        &mut cx,
        format!("Save failed: preparing {}: {error}", folder.display()),
    );
    assert_eq!(std::fs::read(&blocked).unwrap(), b"keep me");

    effects
        .pictures
        .replace(Some(effects.root.path().join("pictures")));
    effects.picker_error.set(true);
    toolbar(&mut cx, "Save…");
    cx.run_until_parked();
    assert!(cx.debug_bounds("Save failed: picker unavailable").is_some());
    effects.picker_error.set(false);
    let bad_path = effects.root.path().to_path_buf();
    command(&mut cx, "s");
    cx.simulate_new_path_selection(|_| Some(bad_path.clone()));
    cx.run_until_parked();
    let error = std::fs::write(&bad_path, b"invalid").unwrap_err();
    assert_status(
        &mut cx,
        format!("Save failed: writing {}: {error}", bad_path.display()),
    );
    assert_eq!(cx.cx.read(|app| app.windows().len()), 1);
    let path = effects.root.path().join("chosen-name.png");
    command(&mut cx, "s");
    cx.simulate_new_path_selection(|_| Some(path.clone()));
    cx.run_until_parked();
    assert_image(image::open(&path).unwrap().into_rgba8(), &expected);
    assert_status(&mut cx, format!("Saved {}", path.display()));
    // Export finalized the text once, even though the first OS effect failed.
    command(&mut cx, "z");
    assert_image(
        saved(&mut cx),
        &image::imageops::crop_imm(
            &super::expected(vec![rectangle((40., 40.), (180., 100.))]),
            20,
            20,
            200,
            160,
        )
        .to_image(),
    );
}

#[gpui::test]
fn copy_and_close_keeps_the_editor_on_failure_and_closes_only_after_success(cx: &mut TestAppContext) {
    let effects = TestExports::configure(cx);
    let mut cx = editor(cx);
    let expected = annotated_crop(&mut cx);
    effects.clipboard_error.set(true);
    command(&mut cx, "shift-c");
    cx.run_until_parked();
    assert_eq!(cx.cx.read(|app| app.windows().len()), 1);
    assert!(cx.debug_bounds("Copy failed: clipboard unavailable").is_some());
    assert!(effects.clipboard.borrow().is_none());
    toolbar(&mut cx, "Copy");
    cx.run_until_parked();
    assert_eq!(cx.cx.read(|app| app.windows().len()), 1);
    assert!(cx.debug_bounds("Copy failed: clipboard unavailable").is_some());
    effects.clipboard_error.set(false);
    command(&mut cx, "c");
    cx.run_until_parked();
    assert_eq!(cx.cx.read(|app| app.windows().len()), 1);
    assert!(cx.debug_bounds("Copied to clipboard").is_some());
    assert_image(effects.clipboard.borrow().clone().unwrap(), &expected);
    effects.clipboard.replace(None);
    command(&mut cx, "shift-c");
    cx.run_until_parked();
    assert_eq!(cx.cx.read(|app| app.windows().len()), 0);
    assert_image(effects.clipboard.borrow().clone().unwrap(), &expected);
}

#[gpui::test]
fn toolbar_copy_and_close_cancels_an_unfinished_drag_and_leaves_other_editors_open(app: &mut TestAppContext) {
    let effects = TestExports::configure(app);
    let mut first = editor(app);
    let expected_first = annotated_crop(&mut first);
    let second = app.update(|app| {
        super::super::open(RgbaImage::from_pixel(320, 200, WHITE), 1., app);
        *app.windows().last().unwrap()
    });
    let mut second = VisualTestContext::from_window(second, app);
    second.simulate_resize(size(px(900.), px(600.)));
    second.run_until_parked();
    second.simulate_keystrokes("a");
    let from = image_point(&mut second, 40., 40.);
    let to = image_point(&mut second, 140., 100.);
    second.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    second.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    command(&mut second, "c");
    second.run_until_parked();
    assert_image(effects.clipboard.borrow().clone().unwrap(), &expected(vec![]));
    assert_eq!(app.read(|app| app.windows().len()), 2);
    toolbar(&mut second, "Copy & Close");
    second.run_until_parked();
    assert_eq!(app.read(|app| app.windows().len()), 1);
    assert_image(effects.clipboard.borrow().clone().unwrap(), &expected(vec![]));
    assert_image(saved(&mut first), &expected_first);
}
