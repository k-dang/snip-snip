use anyhow::{Result, bail};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
#[cfg(any(not(target_os = "macos"), test))]
use gpui::Keystroke;

pub fn default_shortcut() -> HotKey {
    HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyS)
}

pub fn validate(shortcut: HotKey) -> Result<HotKey> {
    if !shortcut
        .mods
        .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
        || shortcut.key == Code::Escape
    {
        bail!("Use Ctrl, Alt/Option, or Win/Command with a supported key. Escape cancels recording.");
    }
    Ok(shortcut)
}

#[cfg(any(not(target_os = "macos"), test))]
pub fn record(key: &Keystroke) -> Result<HotKey> {
    let mut chord = String::new();
    for (present, name) in [
        (key.modifiers.control, "control+"),
        (key.modifiers.alt, "alt+"),
        (key.modifiers.platform, "super+"),
        (key.modifiers.shift, "shift+"),
    ] {
        if present {
            chord.push_str(name);
        }
    }
    chord.push_str(&key.key);
    validate(chord.parse()?)
}

pub fn label(shortcut: HotKey) -> String {
    let mut parts = Vec::new();
    for (modifier, name) in [
        (Modifiers::CONTROL, "Ctrl"),
        (
            Modifiers::ALT,
            if cfg!(target_os = "macos") {
                "Option"
            } else {
                "Alt"
            },
        ),
        (
            Modifiers::SUPER,
            if cfg!(target_os = "macos") {
                "Command"
            } else {
                "Win"
            },
        ),
        (Modifiers::SHIFT, "Shift"),
    ] {
        if shortcut.mods.contains(modifier) {
            parts.push(name.to_owned());
        }
    }
    let key = shortcut.key.to_string();
    parts.push(
        key.strip_prefix("Key")
            .or_else(|| key.strip_prefix("Digit"))
            .unwrap_or(&key)
            .to_owned(),
    );
    parts.join("+")
}
