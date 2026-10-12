//! Background controls and the applied global capture shortcut.

use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use futures::StreamExt;
use futures::channel::mpsc::UnboundedReceiver;
use global_hotkey::hotkey::HotKey;
use gpui::{App, BorrowAppContext, Global, WindowHandle};

mod native;
mod settings;
mod shortcut;
#[cfg(all(test, debug_assertions))]
mod tests;

pub enum ShellEvent {
    Settings,
    NewSnip,
    Hotkey {
        id: u32,
        pressed: bool,
    },
    Quit,
    #[cfg(any(target_os = "macos", test))]
    KeyboardLayoutChanged,
}

/// Native effects stay on GPUI's main thread. Tests deliver the same events
/// and control hotkey failures without touching machine-wide registrations.
pub trait ShellPlatform {
    fn check_keyboard_layout(&self, cx: &App) -> Result<()>;
    fn register(&mut self, shortcut: HotKey) -> Result<()>;
    fn unregister(&mut self, shortcut: HotKey) -> Result<()>;
    fn capture(&mut self, cx: &mut App);
    fn dismiss_menu(&mut self) -> Result<()>;
}

pub struct Shell {
    platform: Box<dyn ShellPlatform>,
    path: PathBuf,
    shortcut: HotKey,
    registered: bool,
    needs_repair: bool,
    applying: bool,
    status: Option<String>,
    error: Option<String>,
    recording: bool,
    settings: Option<WindowHandle<settings::Settings>>,
}

impl Global for Shell {}

pub fn install(
    mut platform: Box<dyn ShellPlatform>,
    path: PathBuf,
    mut events: UnboundedReceiver<ShellEvent>,
    cx: &mut App,
) {
    let (shortcut, mut error) = match fs::read_to_string(&path) {
        Ok(saved) => match saved
            .trim()
            .parse()
            .map_err(anyhow::Error::from)
            .and_then(shortcut::validate)
        {
            Ok(shortcut) => (shortcut, None),
            Err(err) => (
                shortcut::default_shortcut(),
                Some(format!(
                    "Could not read shortcut settings: {err:#}. Using the default for this session."
                )),
            ),
        },
        Err(err) if err.kind() == ErrorKind::NotFound => (shortcut::default_shortcut(), None),
        Err(err) => (
            shortcut::default_shortcut(),
            Some(format!(
                "Could not read shortcut settings: {err}. Using the default for this session."
            )),
        ),
    };
    let needs_repair = error.is_some();
    let registered = match platform
        .check_keyboard_layout(cx)
        .and_then(|_| platform.register(shortcut))
    {
        Ok(()) => true,
        Err(err) => {
            let registration = format!(
                "Could not register {}: {err:#}. Choose another shortcut or use New snip.",
                shortcut::label(shortcut)
            );
            error = Some(
                error
                    .map(|warning| format!("{warning}\n{registration}"))
                    .unwrap_or(registration),
            );
            false
        }
    };
    let show_error = error.is_some();
    cx.set_global(Shell {
        platform,
        path,
        shortcut,
        registered,
        needs_repair,
        applying: false,
        status: None,
        error,
        recording: false,
        settings: None,
    });
    if show_error {
        settings::open(cx);
    }
    cx.on_app_quit(|cx| {
        if cx.has_global::<Shell>() {
            cx.remove_global::<Shell>();
        }
        async {}
    })
    .detach();
    cx.spawn(async move |cx| {
        while let Some(event) = events.next().await {
            // Native menus dismiss after the selection callback returns. Give
            // their closing animation a turn before freezing the screen.
            if matches!(event, ShellEvent::NewSnip) {
                if !cx.update(dismiss_menu) {
                    continue;
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                // Explorer can consume the first Escape in its menu loop.
                // Check and dismiss any remaining flyout after that loop ends.
                let dismissed = cx.update(dismiss_menu);
                if !dismissed {
                    continue;
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(200))
                    .await;
            }
            cx.update(|cx| {
                if !cx.has_global::<Shell>() {
                    return;
                }
                match event {
                    ShellEvent::Settings => settings::open(cx),
                    #[cfg(any(target_os = "macos", test))]
                    ShellEvent::KeyboardLayoutChanged => {
                        cx.update_global::<Shell, _>(|shell, cx| {
                            match shell.platform.check_keyboard_layout(cx) {
                                Err(error) => {
                                    let released = if shell.registered {
                                        shell.platform.unregister(shell.shortcut)
                                    } else {
                                        Ok(())
                                    };
                                    shell.registered = false;
                                    shell.error = Some(match released {
                                        Ok(()) => format!("{error:#}"),
                                        Err(release) => format!(
                                            "{error:#}; could not release the old shortcut: {release:#}"
                                        ),
                                    });
                                }
                                Ok(()) if !shell.registered && !shell.applying => {
                                    match shell.platform.register(shell.shortcut) {
                                        Ok(()) => {
                                            shell.registered = true;
                                            shell.error = None;
                                        }
                                        Err(error) => {
                                            shell.error = Some(format!(
                                                "Could not register the capture shortcut: {error:#}"
                                            ))
                                        }
                                    }
                                }
                                Ok(()) => {}
                            }
                        });
                        if let Some(window) = cx.global::<Shell>().settings {
                            window
                                .update(cx, |settings, _, cx| settings.layout_changed(cx))
                                .ok();
                        }
                        if cx.global::<Shell>().error.is_some() {
                            settings::open(cx);
                        }
                    }
                    ShellEvent::NewSnip => cx.update_global::<Shell, _>(|shell, cx| {
                        if !shell.recording {
                            shell.platform.capture(cx);
                        }
                    }),
                    ShellEvent::Hotkey { id, pressed } => {
                        let shell = cx.global::<Shell>();
                        if pressed && shell.registered && id == shell.shortcut.id() {
                            if shell.recording {
                                let shortcut = shell.shortcut;
                                if let Some(handle) = shell.settings {
                                    handle
                                        .update(cx, |settings, _, cx| {
                                            settings.record_registered(shortcut, cx)
                                        })
                                        .ok();
                                }
                            } else {
                                cx.update_global::<Shell, _>(|shell, cx| shell.platform.capture(cx));
                            }
                        }
                    }
                    ShellEvent::Quit => {
                        for window in cx.windows() {
                            window.update(cx, |_, window, _| window.remove_window()).ok();
                        }
                        cx.remove_global::<Shell>();
                        cx.quit();
                    }
                }
            });
        }
    })
    .detach();
}

fn dismiss_menu(cx: &mut App) -> bool {
    if !cx.has_global::<Shell>() {
        return false;
    }
    if let Err(error) = cx.update_global::<Shell, _>(|shell, _| shell.platform.dismiss_menu()) {
        cx.global_mut::<Shell>().error = Some(format!(
            "Could not dismiss tray controls: {error:#}. Use the capture shortcut instead."
        ));
        settings::open(cx);
        return false;
    }
    true
}

pub fn start(cx: &mut App) -> Result<()> {
    let (tx, rx) = futures::channel::mpsc::unbounded();
    #[cfg(target_os = "macos")]
    {
        let layout_events = tx.clone();
        cx.on_keyboard_layout_change(move |_| {
            layout_events
                .unbounded_send(ShellEvent::KeyboardLayoutChanged)
                .ok();
        })
        .detach();
    }
    let platform = native::NativeShell::new(tx)?;
    let path = dirs::config_dir()
        .context("locating the user settings folder")?
        .join("snip-snip")
        .join("shortcut");
    install(Box::new(platform), path, rx, cx);
    eprintln!(
        "snip-snip running; capture shortcut: {}",
        shortcut::label(cx.global::<Shell>().shortcut)
    );
    Ok(())
}

pub fn show_startup_error(message: String, cx: &mut App) {
    settings::show_startup_error(message, cx);
}

fn save(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().context("settings path has no parent")?;
    fs::create_dir_all(parent).context("creating settings folder")?;
    let mut file = tempfile::NamedTempFile::new_in(parent).context("preparing settings file")?;
    file.write_all(contents).context("writing shortcut settings")?;
    file.as_file().sync_all().context("flushing shortcut settings")?;
    replace(file.into_temp_path(), path).context("replacing shortcut settings")?;
    Ok(())
}

fn replace(mut file: tempfile::TempPath, path: &Path) -> Result<()> {
    for attempt in 0..=4 {
        match file.persist(path) {
            Ok(()) => return Ok(()),
            Err(error) => {
                // Windows scanners may briefly open a just-written file without
                // delete sharing. Retry that bounded native contention off-thread.
                if cfg!(target_os = "windows")
                    && matches!(error.error.raw_os_error(), Some(5 | 32 | 33))
                    && attempt < 4
                {
                    file = error.path;
                    std::thread::sleep(std::time::Duration::from_millis(10 << attempt));
                } else {
                    return Err(error.error.into());
                }
            }
        }
    }
    unreachable!()
}

fn backup(path: &Path) -> Result<Option<tempfile::TempPath>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => anyhow::ensure!(
            metadata.is_file() || metadata.is_symlink(),
            "shortcut settings destination is not a file"
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("checking existing shortcut settings"),
    }
    let parent = path.parent().context("settings path has no parent")?;
    let backup = tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
    fs::remove_file(&backup)?;
    // A hard link preserves unreadable contents and permissions for rollback
    // without requiring read access. Copy is a fallback for file systems that
    // don't support hard links; a readable original is sufficient there.
    if fs::hard_link(path, &backup).is_err() {
        fs::copy(path, &backup).context("backing up existing shortcut settings")?;
    }
    Ok(Some(backup))
}

pub(super) fn apply(candidate: HotKey, cx: &mut App) -> Result<()> {
    cx.global::<Shell>().platform.check_keyboard_layout(cx)?;
    let candidate = shortcut::validate(candidate)?;
    let shell = cx.global::<Shell>();
    if candidate == shell.shortcut
        && shell.registered
        && !shell.needs_repair
        && shell.error.is_none()
        && !shell.applying
    {
        cx.global_mut::<Shell>().status = Some("Shortcut applied".to_owned());
        return Ok(());
    }
    let (path, replacing) = cx.update_global::<Shell, _>(|shell, _| -> Result<_> {
        anyhow::ensure!(!shell.applying, "A shortcut change is already in progress");
        let replacing = candidate != shell.shortcut || !shell.registered;
        if replacing {
            shell
                .platform
                .register(candidate)
                .context("registering the proposed shortcut; choose an available chord")?;
        }
        shell.applying = true;
        shell.status = None;
        Ok((shell.path.clone(), replacing))
    })?;
    let io = cx.background_executor().spawn({
        let path = path.clone();
        async move {
            let previous = backup(&path)?;
            save(&path, candidate.to_string().as_bytes())?;
            Ok(previous)
        }
    });
    cx.spawn(async move |cx| {
        let saved = io.await;
        if !cx.update(|cx| cx.has_global::<Shell>()) { return; }
        let outcome = match saved {
            Err(error) => Err(error),
            Ok(previous) => {
                let release = cx.update(|cx| cx.update_global::<Shell, _>(|shell, cx| {
                    // Registration preceded disk work; the layout may have
                    // changed while that work was pending.
                    shell.platform.check_keyboard_layout(cx)?;
                    if shell.registered && candidate != shell.shortcut {
                        shell.platform.unregister(shell.shortcut).context("releasing the previous shortcut; change was canceled")
                    } else { Ok(()) }
                }));
                match release {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        let rollback = cx.background_executor().spawn(async move {
                            match previous {
                                Some(previous) => replace(previous, &path),
                                None => fs::remove_file(&path).map_err(anyhow::Error::from),
                            }
                        }).await;
                        match rollback {
                            Ok(()) => Err(error),
                            Err(rollback) => Err(anyhow::anyhow!("{error:#}; could not restore previous settings: {rollback:#}")),
                        }
                    }
                }
            }
        };
        let succeeded = outcome.is_ok();
        cx.update(|cx| {
            if !cx.has_global::<Shell>() { return; }
            let window = cx.update_global::<Shell, _>(|shell, cx| {
                match outcome {
                    Ok(()) => {
                        shell.shortcut = candidate;
                        shell.registered = true;
                        shell.needs_repair = false;
                        shell.error = None;
                        shell.status = Some("Shortcut applied".to_owned());
                    }
                    Err(error) => {
                        let cleanup = if replacing { shell.platform.unregister(candidate) } else { Ok(()) };
                        let message = match cleanup {
                            Ok(()) => format!("Could not apply shortcut: {error:#}"),
                            Err(cleanup) => format!("Could not apply shortcut: {error:#}; could not release the unapplied shortcut: {cleanup:#}"),
                        };
                        shell.error = Some(message);
                        // Layout recovery waits for transaction cleanup so a
                        // same-chord retry cannot unregister its own recovery.
                        if !shell.registered && shell.platform.check_keyboard_layout(cx).is_ok() {
                            match shell.platform.register(shell.shortcut) {
                                Ok(()) => shell.registered = true,
                                Err(recovery) => {
                                    shell.error.as_mut().unwrap().push_str(&format!(
                                        "; could not restore the previous shortcut: {recovery:#}"
                                    ));
                                }
                            }
                        }
                    }
                }
                shell.applying = false;
                shell.settings
            });
            if let Some(window) = window {
                window.update(cx, |settings, _, cx| { if succeeded { settings.applied(); } cx.notify(); }).ok();
            }
        });
    }).detach();
    Ok(())
}
