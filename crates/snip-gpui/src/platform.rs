//! Small platform shims for behavior GPUI doesn't expose.

use gpui::Window;

/// Puts `window` above all other windows and gives it keyboard focus.
///
/// GPUI's `WindowKind::PopUp` is already above everything on macOS
/// (`NSPopUpWindowLevel`), but on Windows it only sets `WS_EX_TOOLWINDOW`.
#[cfg(windows)]
pub fn raise_and_focus(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos,
        },
    };

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = HWND(handle.hwnd.get() as _);
    unsafe {
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW;
        if let Err(err) = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags) {
            eprintln!("failed to raise overlay: {err}");
        }
        // Allowed because we are handling the user's hotkey press.
        let _ = SetForegroundWindow(hwnd);
    }
}

#[cfg(not(windows))]
pub fn raise_and_focus(_window: &Window) {}
