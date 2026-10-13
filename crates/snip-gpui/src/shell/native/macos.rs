use anyhow::{Context as _, Result, ensure};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSEventType};

pub(super) fn record_shortcut() -> Result<HotKey> {
    let main = MainThreadMarker::new().context("Shortcut recording requires the main thread")?;
    // GPUI dispatches key handlers synchronously from AppKit's current event.
    let event = NSApplication::sharedApplication(main)
        .currentEvent()
        .context("The shortcut key wasn't received; try recording it again")?;
    ensure!(
        event.r#type() == NSEventType::KeyDown,
        "The shortcut key wasn't received; try recording it again"
    );
    from_native_key(event.keyCode(), event.modifierFlags())
}

fn from_native_key(keycode: u16, flags: NSEventModifierFlags) -> Result<HotKey> {
    let key = physical_key(keycode).context("That physical key is not supported for capture shortcuts")?;
    let mut modifiers = Modifiers::empty();
    for (flag, modifier) in [
        (NSEventModifierFlags::Control, Modifiers::CONTROL),
        (NSEventModifierFlags::Option, Modifiers::ALT),
        (NSEventModifierFlags::Command, Modifiers::SUPER),
        (NSEventModifierFlags::Shift, Modifiers::SHIFT),
    ] {
        if flags.contains(flag) {
            modifiers |= modifier;
        }
    }
    super::super::shortcut::validate(HotKey::new(Some(modifiers), key))
}

// Inverse of the virtual-key table accepted by global-hotkey's macOS backend.
// Code names describe physical U.S. positions, independent of the input source.
fn physical_key(keycode: u16) -> Option<Code> {
    Some(match keycode {
        0x00 => Code::KeyA,
        0x01 => Code::KeyS,
        0x02 => Code::KeyD,
        0x03 => Code::KeyF,
        0x04 => Code::KeyH,
        0x05 => Code::KeyG,
        0x06 => Code::KeyZ,
        0x07 => Code::KeyX,
        0x08 => Code::KeyC,
        0x09 => Code::KeyV,
        0x0b => Code::KeyB,
        0x0c => Code::KeyQ,
        0x0d => Code::KeyW,
        0x0e => Code::KeyE,
        0x0f => Code::KeyR,
        0x10 => Code::KeyY,
        0x11 => Code::KeyT,
        0x12 => Code::Digit1,
        0x13 => Code::Digit2,
        0x14 => Code::Digit3,
        0x15 => Code::Digit4,
        0x16 => Code::Digit6,
        0x17 => Code::Digit5,
        0x18 => Code::Equal,
        0x19 => Code::Digit9,
        0x1a => Code::Digit7,
        0x1b => Code::Minus,
        0x1c => Code::Digit8,
        0x1d => Code::Digit0,
        0x1e => Code::BracketRight,
        0x1f => Code::KeyO,
        0x20 => Code::KeyU,
        0x21 => Code::BracketLeft,
        0x22 => Code::KeyI,
        0x23 => Code::KeyP,
        0x24 => Code::Enter,
        0x25 => Code::KeyL,
        0x26 => Code::KeyJ,
        0x27 => Code::Quote,
        0x28 => Code::KeyK,
        0x29 => Code::Semicolon,
        0x2a => Code::Backslash,
        0x2b => Code::Comma,
        0x2c => Code::Slash,
        0x2d => Code::KeyN,
        0x2e => Code::KeyM,
        0x2f => Code::Period,
        0x30 => Code::Tab,
        0x31 => Code::Space,
        0x32 => Code::Backquote,
        0x33 => Code::Backspace,
        0x35 => Code::Escape,
        0x39 => Code::CapsLock,
        0x40 => Code::F17,
        0x41 => Code::NumpadDecimal,
        0x43 => Code::NumpadMultiply,
        0x45 => Code::NumpadAdd,
        0x46 => Code::PrintScreen,
        0x47 => Code::NumLock,
        0x48 => Code::AudioVolumeUp,
        0x49 => Code::AudioVolumeDown,
        0x4a => Code::AudioVolumeMute,
        0x4b => Code::NumpadDivide,
        0x4c => Code::NumpadEnter,
        0x4e => Code::NumpadSubtract,
        0x4f => Code::F18,
        0x50 => Code::F19,
        0x51 => Code::NumpadEqual,
        0x52 => Code::Numpad0,
        0x53 => Code::Numpad1,
        0x54 => Code::Numpad2,
        0x55 => Code::Numpad3,
        0x56 => Code::Numpad4,
        0x57 => Code::Numpad5,
        0x58 => Code::Numpad6,
        0x59 => Code::Numpad7,
        0x5a => Code::F20,
        0x5b => Code::Numpad8,
        0x5c => Code::Numpad9,
        0x60 => Code::F5,
        0x61 => Code::F6,
        0x62 => Code::F7,
        0x63 => Code::F3,
        0x64 => Code::F8,
        0x65 => Code::F9,
        0x67 => Code::F11,
        0x69 => Code::F13,
        0x6a => Code::F16,
        0x6b => Code::F14,
        0x6d => Code::F10,
        0x6f => Code::F12,
        0x71 => Code::F15,
        0x72 => Code::Insert,
        0x73 => Code::Home,
        0x74 => Code::PageUp,
        0x75 => Code::Delete,
        0x76 => Code::F4,
        0x77 => Code::End,
        0x78 => Code::F2,
        0x79 => Code::PageDown,
        0x7a => Code::F1,
        0x7b => Code::ArrowLeft,
        0x7c => Code::ArrowRight,
        0x7d => Code::ArrowDown,
        0x7e => Code::ArrowUp,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_key_positions_determine_the_shortcut() {
        let flags = NSEventModifierFlags::Option | NSEventModifierFlags::Shift;
        assert_eq!(
            from_native_key(0x01, flags).unwrap(),
            "alt+shift+s".parse().unwrap()
        );
        // AZERTY's A character is at the physical U.S. Q position.
        assert_eq!(
            from_native_key(0x0c, flags).unwrap(),
            "alt+shift+q".parse().unwrap()
        );
    }

    #[test]
    fn native_modifiers_preserve_shift_on_digits_and_punctuation() {
        let flags = NSEventModifierFlags::Command | NSEventModifierFlags::Shift;
        assert_eq!(
            from_native_key(0x12, flags).unwrap(),
            "super+shift+Digit1".parse().unwrap()
        );
        assert_eq!(
            from_native_key(0x2b, flags).unwrap(),
            "super+shift+Comma".parse().unwrap()
        );
    }

    #[test]
    fn recording_rejects_unsupported_keys_and_invalid_chords() {
        assert!(from_native_key(0xffff, NSEventModifierFlags::Control).is_err());
        assert!(from_native_key(0x01, NSEventModifierFlags::Shift).is_err());
        assert!(from_native_key(0x35, NSEventModifierFlags::Command).is_err());
    }
}
