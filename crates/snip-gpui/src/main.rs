//! Screenshot capture and annotation, available through the native tray/menu bar.

mod capture;
mod drag;
mod editor;
mod overlay;
mod shell;

use gpui::{App, QuitMode};

fn main() {
    gpui_platform::application()
        .with_quit_mode(QuitMode::Explicit)
        .run(|cx: &mut App| {
            // The editor and Settings activate the app on macOS; closing its
            // last window gives focus back while leaving the shell running.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.hide();
                }
            })
            .detach();
            if let Err(error) = shell::start(cx) {
                eprintln!("snip-snip: {error:#}");
                shell::show_startup_error(format!("{error:#}"), cx);
            }
        });
}
