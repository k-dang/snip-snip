//! macOS: screen units are points. UNTESTED: written from the platform's
//! documented behavior, never run. Prove it on a Mac and delete this warning.
//! Input needs Accessibility permission for the terminal running snipctl,
//! and window titles need Screen Recording permission.

use std::{path::Path, process::Command, time::Duration};

use anyhow::{Context as _, Result, bail};

use crate::{input::Input, screen};

/// Screen units are points.
pub const PHYSICAL_SCREEN_UNITS: bool = false;

pub fn init() {}

/// Screen units per GPUI logical pixel, and snipped-image pixels per screen
/// unit, for a display with backing `scale` (2 on Retina).
pub fn units(scale: f32) -> (f32, f32) {
    (1.0, scale)
}

/// A standard titled NSWindow's title bar, in points. GPUI's editor uses the
/// native title bar, so the content area is the frame minus this strip.
const TITLE_BAR: f32 = 28.0;

pub fn content_rect(window: &xcap::Window) -> Result<screen::Rect> {
    Ok(screen::Rect {
        x: window.x()? as f32,
        y: window.y()? as f32 + TITLE_BAR,
        w: window.width()? as f32,
        h: window.height()? as f32 - TITLE_BAR,
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

/// Fills the save panel that `mod+s` opened: type the file name into the
/// focused name field, then use Go to Folder (Cmd+Shift+G) for the directory.
pub fn save_as(_pid: u32, path: &Path, input: &mut Input) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("bad file name")?;
    let dir = path.parent().and_then(Path::to_str).context("bad directory")?;
    std::thread::sleep(Duration::from_millis(800));
    input.chord("cmd+a")?;
    input.text(name)?;
    input.chord("cmd+shift+g")?;
    std::thread::sleep(Duration::from_millis(500));
    input.text(dir)?;
    input.chord("enter")?;
    std::thread::sleep(Duration::from_millis(500));
    input.chord("enter")
}
