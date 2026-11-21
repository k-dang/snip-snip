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
    menu: Menu,
    /// Copy latest cloud snip and Cloud library, inserted while signed in.
    cloud_items: [MenuItem; 2],
    cloud_shown: bool,
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
        let cloud_items = [
            MenuItem::with_id("snip-cloud-latest", "Copy latest cloud snip", true, None),
            MenuItem::with_id("snip-cloud-library", "Cloud library", true, None),
        ];
        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
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
                "snip-cloud-latest" => ShellEvent::CopyLatestCloudSnip,
                "snip-cloud-library" => ShellEvent::CloudLibrary,
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
            menu,
            cloud_items,
            cloud_shown: false,
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
    fn show_cloud_menu(&mut self, visible: bool) -> Result<()> {
        if visible == self.cloud_shown {
            return Ok(());
        }
        let [latest, library] = &self.cloud_items;
        if visible {
            // After New snip.
            self.menu.insert_items(&[latest, library], 1)?;
        } else {
            self.menu.remove(latest)?;
            self.menu.remove(library)?;
        }
        self.cloud_shown = visible;
        Ok(())
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
    let pixels = crate::icon::pixels(cfg!(target_os = "macos"));
    Ok(Icon::from_rgba(pixels, crate::icon::SIZE, crate::icon::SIZE)?)
}
