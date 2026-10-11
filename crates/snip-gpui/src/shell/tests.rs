//! Drive Settings and shell events through the agreed GPUI / native-effect seam.

use std::{cell::RefCell, collections::HashSet, rc::Rc};

use anyhow::{Result, bail};
use futures::channel::mpsc;
use global_hotkey::hotkey::HotKey;
use gpui::{App, Modifiers, TestAppContext, VisualTestContext};

use super::{ShellEvent, ShellPlatform};

#[derive(Default)]
struct NativeState {
    registered: HashSet<HotKey>,
    captures: usize,
    conflicts: HashSet<HotKey>,
    released: bool,
    unsupported_layout: bool,
    layout_rejections: usize,
    fail_release: Option<HotKey>,
}

struct TestPlatform(Rc<RefCell<NativeState>>);

impl ShellPlatform for TestPlatform {
    fn check_keyboard_layout(&self, _: &App) -> Result<()> {
        let mut state = self.0.borrow_mut();
        if state.unsupported_layout {
            state.layout_rejections += 1;
            bail!("Unsupported keyboard layout; use New snip or switch layouts");
        }
        Ok(())
    }
    fn dismiss_menu(&mut self) -> Result<()> {
        Ok(())
    }
    fn register(&mut self, shortcut: HotKey) -> Result<()> {
        let mut state = self.0.borrow_mut();
        if state.conflicts.contains(&shortcut) {
            bail!("shortcut is in use");
        }
        if !state.registered.insert(shortcut) {
            bail!("shortcut is already registered");
        }
        Ok(())
    }
    fn unregister(&mut self, shortcut: HotKey) -> Result<()> {
        if self.0.borrow().fail_release == Some(shortcut) {
            bail!("could not release shortcut");
        }
        self.0.borrow_mut().registered.remove(&shortcut);
        Ok(())
    }
    fn capture(&mut self, _: &mut App) {
        self.0.borrow_mut().captures += 1;
    }
}

#[gpui::test]
fn layout_changes_cancel_recording_and_keep_menu_capture_available(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let native = Rc::new(RefCell::new(NativeState::default()));
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| {
        super::install(
            Box::new(TestPlatform(native.clone())),
            folder.path().join("shortcut"),
            rx,
            cx,
        )
    });
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    click(&mut view, "Record shortcut");
    native.borrow_mut().unsupported_layout = true;
    tx.unbounded_send(ShellEvent::KeyboardLayoutChanged).unwrap();
    view.run_until_parked();
    assert!(view.debug_bounds(UNAVAILABLE_DEFAULT).is_some());
    assert!(view.debug_bounds("Press a shortcut; Escape cancels").is_none());
    tx.unbounded_send(ShellEvent::NewSnip).unwrap();
    view.run_until_parked();
    view.executor().advance_clock(std::time::Duration::from_secs(1));
    view.run_until_parked();
    assert_eq!(native.borrow().captures, 1);
    native.borrow_mut().unsupported_layout = false;
    tx.unbounded_send(ShellEvent::KeyboardLayoutChanged).unwrap();
    view.run_until_parked();
    assert!(view.debug_bounds(APPLIED_DEFAULT).is_some());
}

#[gpui::test(iterations = 32)]
fn returning_to_a_supported_layout_during_same_chord_rollback_recovers(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("shortcut");
    std::fs::write(&path, "alt+shift+s").unwrap();
    let chord: HotKey = "alt+shift+s".parse().unwrap();
    let native = Rc::new(RefCell::new(NativeState::default()));
    native.borrow_mut().conflicts.insert(chord);
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| super::install(Box::new(TestPlatform(native.clone())), path.clone(), rx, cx));
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    native.borrow_mut().conflicts.clear();
    click(&mut view, "Record shortcut");
    view.simulate_keystrokes("alt-shift-s tab");
    view.cx
        .dispatch_keystroke(window, gpui::Keystroke::parse("enter").unwrap());
    native.borrow_mut().unsupported_layout = true;
    tx.unbounded_send(ShellEvent::KeyboardLayoutChanged).unwrap();
    // Pause after the native layout event and pending Apply have observed the
    // unsupported layout, while preference rollback is still scheduled.
    while native.borrow().layout_rejections < 2 {
        assert!(view.executor().tick(), "pending layout checks must finish");
    }
    native.borrow_mut().unsupported_layout = false;
    tx.unbounded_send(ShellEvent::KeyboardLayoutChanged).unwrap();
    view.run_until_parked();
    assert!(view.debug_bounds(APPLIED_DEFAULT).is_some());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "alt+shift+s");
    assert_eq!(native.borrow().registered, HashSet::from([chord]));
    tx.unbounded_send(ShellEvent::Hotkey {
        id: chord.id(),
        pressed: true,
    })
    .unwrap();
    view.run_until_parked();
    assert_eq!(native.borrow().captures, 1);
}

#[gpui::test]
fn a_layout_change_during_apply_restores_settings_and_disables_both_chords(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("shortcut");
    std::fs::write(&path, "alt+shift+s").unwrap();
    let native = Rc::new(RefCell::new(NativeState::default()));
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| super::install(Box::new(TestPlatform(native.clone())), path.clone(), rx, cx));
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    click(&mut view, "Record shortcut");
    view.simulate_keystrokes("ctrl-alt-k tab");
    view.cx
        .dispatch_keystroke(window, gpui::Keystroke::parse("enter").unwrap());
    native.borrow_mut().unsupported_layout = true;
    tx.unbounded_send(ShellEvent::KeyboardLayoutChanged).unwrap();
    view.run_until_parked();
    assert!(view.debug_bounds(UNAVAILABLE_DEFAULT).is_some());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "alt+shift+s");
    assert!(native.borrow().registered.is_empty());
    tx.unbounded_send(ShellEvent::Hotkey {
        id: "ctrl+alt+k".parse::<HotKey>().unwrap().id(),
        pressed: true,
    })
    .unwrap();
    view.run_until_parked();
    assert_eq!(native.borrow().captures, 0);
    native.borrow_mut().unsupported_layout = false;
    tx.unbounded_send(ShellEvent::KeyboardLayoutChanged).unwrap();
    view.run_until_parked();
    assert!(view.debug_bounds(APPLIED_DEFAULT).is_some());
    assert_eq!(
        native.borrow().registered,
        HashSet::from(["alt+shift+s".parse().unwrap()])
    );
}

#[gpui::test]
fn a_release_failure_restores_the_original_settings_and_working_chord(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("shortcut");
    std::fs::write(&path, "alt+shift+s").unwrap();
    let native = Rc::new(RefCell::new(NativeState::default()));
    native.borrow_mut().fail_release = Some("alt+shift+s".parse().unwrap());
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| super::install(Box::new(TestPlatform(native.clone())), path.clone(), rx, cx));
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    click(&mut view, "Record shortcut");
    view.simulate_keystrokes("ctrl-alt-k");
    click(&mut view, "Apply");
    view.run_until_parked();
    assert!(view.debug_bounds(APPLIED_DEFAULT).is_some());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "alt+shift+s");
    assert_eq!(
        native.borrow().registered,
        HashSet::from(["alt+shift+s".parse().unwrap()])
    );
    assert_eq!(
        std::fs::read_dir(folder.path()).unwrap().count(),
        1,
        "temporary backups must be cleaned up"
    );
}

#[cfg(unix)]
#[gpui::test]
fn unreadable_but_replaceable_settings_can_be_repaired_through_apply(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("shortcut");
    std::fs::write(&path, "alt+shift+s").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0)).unwrap();
    let native = Rc::new(RefCell::new(NativeState::default()));
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| super::install(Box::new(TestPlatform(native)), path.clone(), rx, cx));
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    click(&mut view, "Record shortcut");
    view.simulate_keystrokes("ctrl-alt-k");
    click(&mut view, "Apply");
    view.run_until_parked();
    assert!(view.debug_bounds(APPLIED_NEW).is_some());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "control+alt+KeyK");
}

impl Drop for TestPlatform {
    fn drop(&mut self) {
        let mut state = self.0.borrow_mut();
        state.registered.clear();
        state.released = true;
    }
}

const APPLIED_DEFAULT: &str = if cfg!(target_os = "macos") {
    "Applied: Option+Shift+S"
} else {
    "Applied: Alt+Shift+S"
};
const PROPOSED_DEFAULT: &str = if cfg!(target_os = "macos") {
    "Proposed: Option+Shift+S"
} else {
    "Proposed: Alt+Shift+S"
};
const APPLIED_NEW: &str = if cfg!(target_os = "macos") {
    "Applied: Ctrl+Option+K"
} else {
    "Applied: Ctrl+Alt+K"
};
const PROPOSED_NEW: &str = if cfg!(target_os = "macos") {
    "Proposed: Ctrl+Option+K"
} else {
    "Proposed: Ctrl+Alt+K"
};
const UNAVAILABLE_DEFAULT: &str = if cfg!(target_os = "macos") {
    "Unavailable: Option+Shift+S"
} else {
    "Unavailable: Alt+Shift+S"
};

fn click(cx: &mut VisualTestContext, label: &'static str) {
    let point = cx
        .debug_bounds(label)
        .expect("rendered settings control")
        .center();
    cx.simulate_click(point, Modifiers::none());
}

#[gpui::test]
fn recording_a_shortcut_displays_it_before_apply(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let native = Rc::new(RefCell::new(NativeState::default()));
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| {
        super::install(
            Box::new(TestPlatform(native.clone())),
            folder.path().join("shortcut"),
            rx,
            cx,
        )
    });
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    click(&mut view, "Record shortcut");
    view.simulate_keystrokes("ctrl-alt-k");
    assert!(view.debug_bounds(PROPOSED_NEW).is_some());
    assert!(view.debug_bounds(APPLIED_DEFAULT).is_some());
}

#[gpui::test]
fn applying_replaces_the_shortcut_and_remembers_it_on_restart(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("shortcut");
    let native = Rc::new(RefCell::new(NativeState::default()));
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| super::install(Box::new(TestPlatform(native.clone())), path.clone(), rx, cx));
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    click(&mut view, "Record shortcut");
    view.simulate_keystrokes("ctrl-alt-k");
    click(&mut view, "Apply");
    assert!(view.debug_bounds(APPLIED_NEW).is_some());
    let old: HotKey = "alt+shift+s".parse().unwrap();
    let new: HotKey = "ctrl+alt+k".parse().unwrap();
    assert_eq!(native.borrow().registered, HashSet::from([new]));
    tx.unbounded_send(ShellEvent::Hotkey {
        id: old.id(),
        pressed: true,
    })
    .unwrap();
    tx.unbounded_send(ShellEvent::Hotkey {
        id: new.id(),
        pressed: false,
    })
    .unwrap();
    tx.unbounded_send(ShellEvent::Hotkey {
        id: new.id(),
        pressed: true,
    })
    .unwrap();
    view.run_until_parked();
    assert_eq!(native.borrow().captures, 1);

    let restarted = view.new_app();
    let fresh = Rc::new(RefCell::new(NativeState::default()));
    let (tx, rx) = mpsc::unbounded();
    restarted.update(|cx| super::install(Box::new(TestPlatform(fresh.clone())), path, rx, cx));
    assert_eq!(fresh.borrow().registered, HashSet::from([new]));
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    restarted.run_until_parked();
    let window = restarted.update(|cx| cx.windows()[0]);
    let mut restarted = VisualTestContext::from_window(window, &restarted);
    assert!(restarted.debug_bounds(APPLIED_NEW).is_some());
}

#[gpui::test]
fn startup_conflict_keeps_menu_capture_and_settings_available(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let native = Rc::new(RefCell::new(NativeState::default()));
    native
        .borrow_mut()
        .conflicts
        .insert("alt+shift+s".parse().unwrap());
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| {
        super::install(
            Box::new(TestPlatform(native.clone())),
            folder.path().join("shortcut"),
            rx,
            cx,
        )
    });
    cx.run_until_parked();
    let window = cx.update(|cx| {
        assert_eq!(cx.windows().len(), 1);
        cx.windows()[0]
    });
    let mut view = VisualTestContext::from_window(window, cx);
    assert!(view.debug_bounds(UNAVAILABLE_DEFAULT).is_some());
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    view.run_until_parked();
    view.update(|_, cx| assert_eq!(cx.windows().len(), 1));
    tx.unbounded_send(ShellEvent::NewSnip).unwrap();
    view.run_until_parked();
    view.executor().advance_clock(std::time::Duration::from_secs(1));
    view.run_until_parked();
    assert_eq!(native.borrow().captures, 1);
    click(&mut view, "Record shortcut");
    view.simulate_keystrokes("ctrl-alt-k");
    click(&mut view, "Apply");
    assert!(view.debug_bounds(APPLIED_NEW).is_some());
    view.simulate_keystrokes("escape");
    view.run_until_parked();
    view.cx.update(|cx| assert!(cx.windows().is_empty()));
    tx.unbounded_send(ShellEvent::Hotkey {
        id: "ctrl+alt+k".parse::<HotKey>().unwrap().id(),
        pressed: true,
    })
    .unwrap();
    view.run_until_parked();
    assert_eq!(native.borrow().captures, 2);
    tx.unbounded_send(ShellEvent::Quit).unwrap();
    view.run_until_parked();
    assert!(native.borrow().released);
    assert!(native.borrow().registered.is_empty());
}

#[gpui::test]
fn recording_validates_cancels_and_accepts_the_registered_chord_without_capture(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let native = Rc::new(RefCell::new(NativeState::default()));
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| {
        super::install(
            Box::new(TestPlatform(native.clone())),
            folder.path().join("shortcut"),
            rx,
            cx,
        )
    });
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    click(&mut view, "Record shortcut");
    view.simulate_modifiers_change(Modifiers {
        control: true,
        shift: true,
        ..Modifiers::none()
    });
    view.simulate_modifiers_change(Modifiers::none());
    assert!(
        view.debug_bounds(
            "The shortcut key wasn't received. It may be reserved or in use; try another chord."
        )
        .is_some()
    );
    view.simulate_keystrokes("s");
    assert!(
        view.debug_bounds(
            "Use Ctrl, Alt/Option, or Win/Command with a supported key. Escape cancels recording."
        )
        .is_some()
    );
    view.simulate_keystrokes("shift-s");
    assert!(view.debug_bounds(APPLIED_DEFAULT).is_some());
    view.simulate_keystrokes("escape");
    assert!(view.debug_bounds("Press a shortcut; Escape cancels").is_none());
    view.simulate_keystrokes("enter");
    tx.unbounded_send(ShellEvent::Hotkey {
        id: "alt+shift+s".parse::<HotKey>().unwrap().id(),
        pressed: true,
    })
    .unwrap();
    view.run_until_parked();
    assert!(view.debug_bounds(PROPOSED_DEFAULT).is_some());
    assert_eq!(native.borrow().captures, 0);
    view.simulate_keystrokes("tab enter");
    assert!(view.debug_bounds("Shortcut applied").is_some());
}

#[gpui::test]
fn a_conflicting_replacement_preserves_the_applied_shortcut_and_saved_value(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("shortcut");
    std::fs::write(&path, "shift+alt+KeyS").unwrap();
    let native = Rc::new(RefCell::new(NativeState::default()));
    native
        .borrow_mut()
        .conflicts
        .insert("ctrl+alt+k".parse().unwrap());
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| super::install(Box::new(TestPlatform(native.clone())), path.clone(), rx, cx));
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    click(&mut view, "Record shortcut");
    view.simulate_keystrokes("ctrl-alt-k");
    click(&mut view, "Apply");
    assert!(view.debug_bounds("Could not apply shortcut: registering the proposed shortcut; choose an available chord: shortcut is in use").is_some());
    assert!(view.debug_bounds(APPLIED_DEFAULT).is_some());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "shift+alt+KeyS");
    assert_eq!(
        native.borrow().registered,
        HashSet::from(["alt+shift+s".parse().unwrap()])
    );
    native.borrow_mut().conflicts.clear();
    click(&mut view, "Apply");
    view.run_until_parked();
    assert!(view.debug_bounds(APPLIED_NEW).is_some());
}

#[gpui::test]
fn malformed_settings_are_preserved_until_the_user_applies_a_shortcut(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("shortcut");
    std::fs::write(&path, "shift+s").unwrap();
    let native = Rc::new(RefCell::new(NativeState::default()));
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| super::install(Box::new(TestPlatform(native)), path.clone(), rx, cx));
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    assert!(view.debug_bounds(APPLIED_DEFAULT).is_some());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "shift+s");
    click(&mut view, "Record shortcut");
    tx.unbounded_send(ShellEvent::Hotkey {
        id: "alt+shift+s".parse::<HotKey>().unwrap().id(),
        pressed: true,
    })
    .unwrap();
    view.run_until_parked();
    click(&mut view, "Apply");
    assert!(view.debug_bounds("Shortcut applied").is_some());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "shift+alt+KeyS");
}

#[gpui::test]
fn an_unwritable_settings_destination_does_not_replace_the_working_shortcut(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("shortcut");
    let native = Rc::new(RefCell::new(NativeState::default()));
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| super::install(Box::new(TestPlatform(native.clone())), path.clone(), rx, cx));
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    std::fs::create_dir(&path).unwrap();
    click(&mut view, "Record shortcut");
    view.simulate_keystrokes("ctrl-alt-k");
    click(&mut view, "Apply");
    assert!(view.debug_bounds(APPLIED_DEFAULT).is_some());
    assert!(view.debug_bounds("Shortcut applied").is_none());
    assert_eq!(
        native.borrow().registered,
        HashSet::from(["alt+shift+s".parse().unwrap()])
    );
    tx.unbounded_send(ShellEvent::Hotkey {
        id: "alt+shift+s".parse::<HotKey>().unwrap().id(),
        pressed: true,
    })
    .unwrap();
    view.run_until_parked();
    assert_eq!(native.borrow().captures, 1);
}

#[gpui::test]
fn applying_defers_disk_work_and_finishes_after_settings_closes(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("shortcut");
    let native = Rc::new(RefCell::new(NativeState::default()));
    let (tx, rx) = mpsc::unbounded();
    cx.update(|cx| super::install(Box::new(TestPlatform(native.clone())), path.clone(), rx, cx));
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    cx.run_until_parked();
    let window = cx.update(|cx| cx.windows()[0]);
    let mut view = VisualTestContext::from_window(window, cx);
    click(&mut view, "Record shortcut");
    view.simulate_keystrokes("ctrl-alt-k");
    view.simulate_keystrokes("tab");
    view.cx
        .dispatch_keystroke(window, gpui::Keystroke::parse("enter").unwrap());
    assert!(!path.exists(), "Apply must return before filesystem work runs");
    view.cx
        .dispatch_keystroke(window, gpui::Keystroke::parse("escape").unwrap());
    assert!(view.cx.read(|cx| cx.windows().is_empty()));
    view.run_until_parked();
    assert_eq!(
        native.borrow().registered,
        HashSet::from(["ctrl+alt+k".parse().unwrap()])
    );
    tx.unbounded_send(ShellEvent::Settings).unwrap();
    view.run_until_parked();
    let window = view.cx.update(|cx| cx.windows()[0]);
    let mut reopened = VisualTestContext::from_window(window, &view.cx);
    assert!(reopened.debug_bounds(APPLIED_NEW).is_some());
}
