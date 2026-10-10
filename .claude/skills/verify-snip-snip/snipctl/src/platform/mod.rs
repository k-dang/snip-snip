//! The only OS-specific code: focusing a window, the editor's content area,
//! coordinate units, the save dialog, and a few doctor checks. Everything
//! else is shared.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(windows)]
pub use windows::*;
