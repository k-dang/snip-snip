//! Windows: screen units are physical pixels.

use std::{path::Path, time::Duration};

use anyhow::{Context as _, Result};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::ClientToScreen,
    System::StationsAndDesktops::{CloseDesktop, DESKTOP_SWITCHDESKTOP, OpenInputDesktop},
    UI::{
        HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
        WindowsAndMessaging::{GetClientRect, GetForegroundWindow, SetForegroundWindow},
    },
};

use crate::{input::Input, run::wait_for, screen};

pub const PHYSICAL_SCREEN_UNITS: bool = true;

/// Makes every coordinate this process sees physical pixels, matching xcap.
pub fn init() {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }.ok();
}

/// Screen units per GPUI logical pixel, and snipped-image pixels per screen
/// unit, for a monitor with `scale` (its DPI / 96).
pub fn units(scale: f32) -> (f32, f32) {
    (scale, 1.0)
}

fn hwnd(window: &xcap::Window) -> Result<HWND> {
    Ok(HWND(window.id()? as usize as *mut _))
}

/// The window's client area (below the native title bar) in screen pixels.
pub fn content_rect(window: &xcap::Window) -> Result<screen::Rect> {
    let hwnd = hwnd(window)?;
    let mut rect = RECT::default();
    let mut origin = POINT::default();
    unsafe {
        GetClientRect(hwnd, &mut rect)?;
        ClientToScreen(hwnd, &mut origin).ok()?;
    }
    Ok(screen::Rect {
        x: origin.x as f32,
        y: origin.y as f32,
        w: rect.right as f32,
        h: rect.bottom as f32,
    })
}

/// Brings the window to the foreground. Windows refuses foreground changes
/// from background processes unless input just arrived, so tap Alt first.
pub fn focus(window: &xcap::Window, _pid: u32, input: &mut Input) -> Result<()> {
    let hwnd = hwnd(window)?;
    if unsafe { GetForegroundWindow() } == hwnd {
        return Ok(());
    }
    input.chord("alt")?;
    unsafe { SetForegroundWindow(hwnd) }.ok()?;
    wait_for("the window to take focus", Duration::from_secs(2), || {
        Ok((unsafe { GetForegroundWindow() } == hwnd).then_some(()))
    })
}

/// False when the workstation is locked, where injected input goes nowhere.
pub fn desktop_unlocked() -> Option<bool> {
    let desktop = unsafe { OpenInputDesktop(Default::default(), false, DESKTOP_SWITCHDESKTOP) };
    Some(desktop.map(|d| unsafe { CloseDesktop(d) }).is_ok())
}

/// The clipboard's format names, e.g. `PNG`, `CF_DIBV5`.
pub fn clipboard_formats() -> Result<Vec<String>> {
    use clipboard_win::{Clipboard, raw};
    let _open = Clipboard::new_attempts(10).map_err(|e| anyhow::anyhow!("opening clipboard: {e}"))?;
    Ok(raw::EnumFormats::new()
        .map(|format| match format {
            2 => "CF_BITMAP".into(),
            8 => "CF_DIB".into(),
            17 => "CF_DIBV5".into(),
            _ => raw::format_name_big(format).unwrap_or_else(|| format!("#{format}")),
        })
        .collect())
}

/// Finds the native save dialog belonging to this verification run.
fn save_dialog(pid: u32) -> Result<xcap::Window> {
    wait_for("the save dialog", Duration::from_secs(10), || {
        Ok(screen::windows_of(pid)?.into_iter().find(|w| {
            w.title()
                .is_ok_and(|t| t != screen::EDITOR_TITLE && t != screen::OVERLAY_TITLE)
        }))
    })
}

/// Fills the focused file-name box, which accepts a full path.
pub fn save_as(pid: u32, path: &Path, input: &mut Input) -> Result<()> {
    let dialog = save_dialog(pid)?;
    focus(&dialog, pid, input)?;
    std::thread::sleep(Duration::from_millis(400));
    input.chord("ctrl+a")?;
    input.text(path.to_str().context("non-UTF-8 path")?)?;
    input.chord("enter")
}

/// Focus the modal dialog itself; focusing its editor can send Escape there.
pub fn cancel_save(pid: u32, input: &mut Input) -> Result<()> {
    let dialog = save_dialog(pid)?;
    let dialog_id = dialog.id()?;
    focus(&dialog, pid, input)?;
    input.chord("esc")?;
    wait_for("the save dialog to close", Duration::from_secs(5), || {
        Ok((!screen::windows_of(pid)?
            .iter()
            .any(|window| window.id().ok() == Some(dialog_id)))
        .then_some(()))
    })
}
