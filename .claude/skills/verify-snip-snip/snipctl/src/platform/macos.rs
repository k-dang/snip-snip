//! macOS: screen units are points. Capture and clipboard have native proof;
//! save-panel automation and the full feature pass remain unverified.
//! Input needs Accessibility permission for the terminal running snipctl,
//! and window titles need Screen Recording permission.

use std::{path::Path, process::Command, time::Duration};

use anyhow::{Context as _, Result, bail};

use crate::{input::Input, run::wait_for, screen};

pub const PHYSICAL_SCREEN_UNITS: bool = false;

pub fn init() {}

/// macOS OS input and GPUI layout both use points, including on Retina.
pub fn screen_units_per_logical(_scale: f32) -> f32 {
    1.0
}

/// The native frame includes the title bar; the painted viewport supplies
/// its actual height rather than assuming a fixed macOS title-bar size.
pub fn content_rect(window: &xcap::Window, painted_content: (f32, f32)) -> Result<screen::Rect> {
    Ok(screen::Rect {
        x: window.x()? as f32,
        y: window.y()? as f32 + window.height()? as f32 - painted_content.1,
        w: window.width()? as f32,
        h: painted_content.1,
    })
}

/// Activates the app that owns the window; its key window takes focus.
pub fn focus(window: &xcap::Window, pid: u32, _input: &mut Input) -> Result<()> {
    if window.is_focused()? {
        return Ok(());
    }
    let script = format!(
        "tell application \"System Events\" to set frontmost of (first process whose unix id is {pid}) to true"
    );
    let status = Command::new("osascript").args(["-e", &script]).status()?;
    if !status.success() {
        bail!("osascript could not activate pid {pid}");
    }
    std::thread::sleep(Duration::from_millis(300));
    Ok(())
}

/// No cheap public check; injected input fails visibly when the screen is locked.
pub fn desktop_unlocked() -> Option<bool> {
    None
}

pub fn clipboard_formats() -> Result<Vec<String>> {
    Ok(Vec::new())
}

#[derive(PartialEq, Eq)]
enum SavePanelState {
    Absent,
    Unfocused,
    Focused,
}

fn save_panel_state(pid: u32) -> Result<SavePanelState> {
    // xcap reports app focus on macOS; use Accessibility to check the dialog.
    let script = format!(
        r#"tell application "System Events"
    tell first application process whose unix id is {pid}
        if (count of (windows whose subrole is "AXDialog")) is 0 then return "absent"
        if not frontmost then return "unfocused"
        set focusedWindow to value of attribute "AXFocusedWindow"
        if subrole of focusedWindow is "AXDialog" then return "focused"
        return "unfocused"
    end tell
end tell"#
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .context("inspecting the native save panel")?;
    if !output.status.success() {
        bail!(
            "inspecting the native save panel: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    match String::from_utf8(output.stdout)?.trim() {
        "absent" => Ok(SavePanelState::Absent),
        "unfocused" => Ok(SavePanelState::Unfocused),
        "focused" => Ok(SavePanelState::Focused),
        state => bail!("unexpected native save panel state: {state}"),
    }
}

fn wait_for_save_panel(pid: u32) -> Result<()> {
    wait_for("the save panel to take focus", Duration::from_secs(10), || {
        Ok((save_panel_state(pid)? == SavePanelState::Focused).then_some(()))
    })
}

/// Fills the save panel that `mod+s` opened: type the file name into the
/// focused name field, then use Go to Folder (Cmd+Shift+G) for the directory.
pub fn save_as(pid: u32, path: &Path, input: &mut Input) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("bad file name")?;
    let dir = path.parent().and_then(Path::to_str).context("bad directory")?;
    wait_for_save_panel(pid)?;
    input.chord("cmd+a")?;
    input.text(name)?;
    input.chord("cmd+shift+g")?;
    std::thread::sleep(Duration::from_millis(500));
    input.text(dir)?;
    input.chord("enter")?;
    std::thread::sleep(Duration::from_millis(500));
    input.chord("enter")
}

/// Send Escape only after the native save panel exists and has focus.
pub fn cancel_save(pid: u32, input: &mut Input) -> Result<()> {
    wait_for_save_panel(pid)?;
    input.chord("esc")?;
    wait_for("the save panel to close", Duration::from_secs(5), || {
        Ok((save_panel_state(pid)? == SavePanelState::Absent).then_some(()))
    })
}
