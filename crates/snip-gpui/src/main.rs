//! snip-snip GPUI feasibility spike.
//!
//! Runs in the background; press Alt+Shift+S to freeze every monitor, drag a
//! region, and annotate it in the editor. Timings are logged to stderr.

mod capture;
mod editor;
mod overlay;
mod platform;

use anyhow::{Context as _, Result};
use futures::StreamExt;
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use gpui::{App, Application, WindowBounds, WindowOptions, prelude::*, px, size};

fn main() {
    Application::new().run(|cx: &mut App| {
        if let Err(err) = run(cx) {
            eprintln!("snip-snip: {err:#}");
            cx.quit();
        }
    });
}

fn run(cx: &mut App) -> Result<()> {
    open_keepalive_window(cx)?;

    let manager = GlobalHotKeyManager::new().context("creating hotkey manager")?;
    let hotkey = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyS);
    manager.register(hotkey).context("registering Alt+Shift+S")?;

    // The handler runs on the platform event loop; forward presses to a GPUI
    // task so snipping runs with full app access and no polling.
    let (tx, mut presses) = futures::channel::mpsc::unbounded();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state == HotKeyState::Pressed {
            tx.unbounded_send(std::time::Instant::now()).ok();
        }
    }));
    cx.spawn(async move |cx| {
        let _manager = manager; // dropping it unregisters the hotkey
        while let Some(pressed) = presses.next().await {
            eprintln!("snip: hotkey delivered after {:?}", pressed.elapsed());
            cx.update(overlay::start).ok();
        }
    })
    .detach();

    eprintln!("snip-snip running; press Alt+Shift+S to snip");
    Ok(())
}

/// GPUI on Windows quits when its last window closes, but this app must keep
/// running after the editor is closed. A hidden window keeps it alive.
/// macOS doesn't quit on last-window-close, so it needs no workaround.
fn open_keepalive_window(cx: &mut App) -> Result<()> {
    if cfg!(windows) {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(gpui::Bounds::centered(
                None,
                size(px(1.), px(1.)),
                cx,
            ))),
            titlebar: None,
            show: false,
            focus: false,
            kind: gpui::WindowKind::PopUp,
            ..Default::default()
        };
        cx.open_window(options, |_, cx| cx.new(|_| gpui::Empty))
            .context("opening keepalive window")?;
    }
    Ok(())
}
