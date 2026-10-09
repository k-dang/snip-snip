//! Small platform shims for behavior GPUI doesn't expose.

use gpui::Window;

/// Gives `window` keyboard focus by making it the foreground window.
///
/// GPUI's `WindowKind::PopUp` is already topmost on both platforms, but on
/// Windows opening one from a background app doesn't activate it, so keys
/// (e.g. Esc) would go to whatever app was focused before the hotkey.
#[cfg(windows)]
pub fn take_foreground(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::SetForegroundWindow};

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    // Allowed because we are handling the user's hotkey press.
    let _ = unsafe { SetForegroundWindow(HWND(handle.hwnd.get() as _)) };
}

#[cfg(not(windows))]
pub fn take_foreground(_window: &Window) {}
