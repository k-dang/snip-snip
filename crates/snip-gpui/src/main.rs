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
use gpui::{App, QuitMode};

fn main() {
    // Keep running with no windows open; the app lives in the background
    // waiting for the hotkey.
    gpui_platform::application()
        .with_quit_mode(QuitMode::Explicit)
        .run(|cx: &mut App| {
            if let Err(err) = run(cx) {
                eprintln!("snip-snip: {err:#}");
                cx.quit();
            }
        });
}

fn run(cx: &mut App) -> Result<()> {
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
            cx.update(overlay::start);
        }
    })
    .detach();

    eprintln!("snip-snip running; press Alt+Shift+S to snip");
    Ok(())
}
