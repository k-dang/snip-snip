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

/// Screen units are physical pixels.
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

pub fn is_focused(window: &xcap::Window) -> Result<bool> {
    Ok(unsafe { GetForegroundWindow() } == hwnd(window)?)
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

/// Locate the actual tray button and menu item with Windows accessibility,
/// then select them using OS mouse input. Never invoke app-private commands.
pub fn tray_menu(pid: u32, label: &str, input: &mut Input) -> Result<()> {
    use windows::Win32::{
        System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx},
        UI::Accessibility::*,
        UI::WindowsAndMessaging::{FindWindowW, IsWindowVisible},
    };
    use windows::core::w;
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let root = unsafe { automation.GetRootElement()? };
    let point = |property, value: &str| -> Result<Option<(i32, i32)>> {
        let condition = unsafe { automation.CreatePropertyCondition(property, &value.into())? };
        // Query live Explorer surfaces rather than every app's accessibility
        // tree. Reacquire the overflow root after opening it.
        for class in [
            w!("Shell_TrayWnd"),
            w!("TopLevelWindowForOverflowXamlIsland"),
            w!("NotifyIconOverflowWindow"),
        ] {
            let Ok(window) = (unsafe { FindWindowW(class, None) }) else {
                continue;
            };
            if !unsafe { IsWindowVisible(window).as_bool() } {
                continue;
            }
            let surface = unsafe { automation.ElementFromHandle(window)? };
            let controls = unsafe { surface.FindAll(TreeScope_Descendants, &condition)? };
            for index in 0..unsafe { controls.Length()? } {
                let control = unsafe { controls.GetElement(index)? };
                if unsafe { control.CurrentIsOffscreen()? }.as_bool() {
                    continue;
                }
                let rect = unsafe { control.CurrentBoundingRectangle()? };
                if rect.right > rect.left
                    && rect.bottom > rect.top
                    && rect.right - rect.left <= 64
                    && rect.bottom - rect.top <= 64
                {
                    return Ok(Some(((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2)));
                }
            }
        }
        Ok(None)
    };
    if point(UIA_NamePropertyId, "snip-snip")?.is_none() {
        let overflow = match point(UIA_AutomationIdPropertyId, "SystemTray.OverflowButton")? {
            Some(point) => point,
            None => point(UIA_NamePropertyId, "Show Hidden Icons")?
                .context("tray icon and overflow button not found")?,
        };
        input.click(overflow)?;
    }
    let icon = wait_for("the snip-snip tray button", Duration::from_secs(3), || {
        point(UIA_NamePropertyId, "snip-snip")
    })?;
    input.click(icon)?;
    let name = unsafe { automation.CreatePropertyCondition(UIA_NamePropertyId, &label.into())? };
    let process =
        unsafe { automation.CreatePropertyCondition(UIA_ProcessIdPropertyId, &(pid as i32).into())? };
    let condition = unsafe { automation.CreateAndCondition(&name, &process)? };
    let item = wait_for("the tray menu item", Duration::from_secs(3), || {
        let Ok(item) = (unsafe { root.FindFirst(TreeScope_Descendants, &condition) }) else {
            return Ok(None);
        };
        if unsafe { item.CurrentIsOffscreen()? }.as_bool() {
            return Ok(None);
        }
        let rect = unsafe { item.CurrentBoundingRectangle()? };
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return Ok(None);
        }
        let point = POINT {
            x: (rect.left + rect.right) / 2,
            y: (rect.top + rect.bottom) / 2,
        };
        // Accessibility geometry may arrive before the native menu finishes
        // opening. Click only when hit testing finds this actual menu item.
        let hit = unsafe { automation.ElementFromPoint(point)? };
        Ok(
            (unsafe { hit.CurrentName()? } == label && unsafe { hit.CurrentProcessId()? } == pid as i32)
                .then_some((point.x, point.y)),
        )
    })?;
    input.click(item)
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

/// Fills the save dialog that `mod+s` opened: the file-name box has focus
/// with the suggested name selected, and accepts a full path.
pub fn save_as(pid: u32, path: &Path, input: &mut Input) -> Result<()> {
    let dialog = wait_for("the save dialog", Duration::from_secs(10), || {
        Ok(screen::windows_of(pid)?
            .into_iter()
            // Any window that is not the editor or an overlay is the dialog.
            .find(|w| {
                w.title()
                    .is_ok_and(|t| t != screen::EDITOR_TITLE && t != screen::OVERLAY_TITLE)
            }))
    })?;
    focus(&dialog, pid, input)?;
    std::thread::sleep(Duration::from_millis(400));
    input.chord("ctrl+a")?;
    input.text(path.to_str().context("non-UTF-8 path")?)?;
    input.chord("enter")
}
