//! Native resources share GPUI's main-thread Windows/AppKit event loop.

use anyhow::{Context as _, Result};
use futures::channel::mpsc::UnboundedSender;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::App;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

use super::{ShellEvent, ShellPlatform};

#[cfg(target_os = "macos")]
mod macos;

pub struct NativeShell {
    // Dropping the icon also releases its native menu.
    _tray: TrayIcon,
    manager: Option<GlobalHotKeyManager>,
}

impl NativeShell {
    pub fn new(tx: UnboundedSender<ShellEvent>) -> Result<Self> {
        let menu = Menu::new();
        menu.append_items(&[
            &MenuItem::with_id("snip-new", "New snip", true, None),
            &MenuItem::with_id("snip-settings", "Settings", true, None),
            &MenuItem::with_id("snip-quit", "Quit", true, None),
        ])
        .context("creating the tray menu")?;
        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("snip-snip")
            .with_menu_on_left_click(true);
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon()?);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon()?);
        let tray = builder
            .build()
            .context("creating the snip-snip tray/menu-bar icon")?;
        let menu_tx = tx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            eprintln!("shell: menu {}", event.id.0);
            let event = match event.id.0.as_str() {
                "snip-new" => ShellEvent::NewSnip,
                "snip-settings" => ShellEvent::Settings,
                "snip-quit" => ShellEvent::Quit,
                _ => return,
            };
            menu_tx.unbounded_send(event).ok();
        }));
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            tx.unbounded_send(ShellEvent::Hotkey {
                id: event.id,
                pressed: event.state == HotKeyState::Pressed,
            })
            .ok();
        }));
        Ok(Self {
            _tray: tray,
            manager: None,
        })
    }
}

impl ShellPlatform for NativeShell {
    fn record_shortcut(&self, key: &gpui::Keystroke) -> Result<HotKey> {
        #[cfg(target_os = "macos")]
        {
            let _ = key;
            macos::record_shortcut()
        }
        #[cfg(not(target_os = "macos"))]
        super::shortcut::record(key)
    }
    fn dismiss_menu(&mut self) -> Result<()> {
        #[cfg(windows)]
        dismiss_windows_tray()?;
        Ok(())
    }
    fn register(&mut self, shortcut: HotKey) -> Result<()> {
        if self.manager.is_none() {
            self.manager = Some(GlobalHotKeyManager::new().context("creating hotkey manager")?);
        }
        self.manager
            .as_ref()
            .unwrap()
            .register(shortcut)
            .context("the shortcut may be reserved or in use")?;
        Ok(())
    }
    fn unregister(&mut self, shortcut: HotKey) -> Result<()> {
        self.manager
            .as_ref()
            .context("hotkey manager unavailable")?
            .unregister(shortcut)?;
        Ok(())
    }
    fn capture(&mut self, cx: &mut App) {
        #[cfg(windows)]
        finish_composition();
        crate::overlay::start(cx);
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn finish_composition() {
    // Flush pending native menu/flyout redraws before reading the desktop.
    unsafe {
        windows::Win32::Graphics::Dwm::DwmFlush().ok();
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn dismiss_windows_tray() -> Result<()> {
    use windows::{
        Win32::UI::{
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_ESCAPE,
            },
            WindowsAndMessaging::{
                EndMenu, FindWindowW, GetForegroundWindow, IsWindowVisible, SetForegroundWindow,
            },
        },
        core::w,
    };
    let _ = unsafe { EndMenu() };
    // Explorer keeps its overflow flyout open after selecting a tray command.
    // Escape must pass through its input dispatcher, not a synthetic WM_KEYDOWN
    // on the outer XAML host. Send it only while that flyout owns foreground.
    for class in [
        w!("TopLevelWindowForOverflowXamlIsland"),
        w!("NotifyIconOverflowWindow"),
    ] {
        if let Ok(window) = unsafe { FindWindowW(class, None) }
            && unsafe { IsWindowVisible(window).as_bool() }
        {
            anyhow::ensure!(
                unsafe {
                    GetForegroundWindow() == window
                        || (SetForegroundWindow(window).as_bool() && GetForegroundWindow() == window)
                },
                "Windows could not focus the tray flyout"
            );
            let inputs = [
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_ESCAPE,
                            ..Default::default()
                        },
                    },
                },
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_ESCAPE,
                            dwFlags: KEYEVENTF_KEYUP,
                            ..Default::default()
                        },
                    },
                },
            ];
            anyhow::ensure!(
                unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } == 2,
                "Windows could not dismiss the tray flyout"
            );
        }
    }
    Ok(())
}

impl Drop for NativeShell {
    fn drop(&mut self) {
        GlobalHotKeyEvent::set_event_handler(None::<fn(GlobalHotKeyEvent)>);
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    }
}

fn icon() -> Result<Icon> {
    // Four open corners on a transparent background. AppKit uses the alpha as
    // a template; Windows gets white strokes over a one-pixel dark outline.
    let mut pixels = vec![0_u8; 32 * 32 * 4];
    let layers: &[(usize, u8)] = if cfg!(target_os = "macos") {
        &[(0, 30)]
    } else {
        &[(1, 30), (0, 245)]
    };
    for &(padding, ink) in layers {
        let side = |n| (4 - padding..8 + padding).contains(&n) || (24 - padding..28 + padding).contains(&n);
        let arm = |n| (4 - padding..14 + padding).contains(&n) || (18 - padding..28 + padding).contains(&n);
        for y in 0..32_usize {
            for x in 0..32_usize {
                if side(x) && arm(y) || side(y) && arm(x) {
                    let offset = (y * 32 + x) * 4;
                    pixels[offset..offset + 4].copy_from_slice(&[ink, ink, ink, 255]);
                }
            }
        }
    }
    Ok(Icon::from_rgba(pixels, 32, 32)?)
}
