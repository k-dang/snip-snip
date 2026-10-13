//! Real OS input through enigo: chords, typing, and mouse drags paced like a
//! hand on the keyboard and mouse. Positions are in screen units (physical
//! pixels on Windows, points on macOS).

use std::{thread::sleep, time::Duration};

use anyhow::{Result, bail};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard as _, Mouse as _, Settings};

pub struct Input(Enigo);

fn pause(ms: u64) {
    sleep(Duration::from_millis(ms));
}

impl Input {
    #[cfg(windows)]
    pub fn validate_chord(combo: &str) -> Result<()> {
        for key in combo.split('+') {
            parse_key(key)?;
        }
        Ok(())
    }

    pub fn new() -> Result<Self> {
        Ok(Self(Enigo::new(&Settings::default())?))
    }

    /// Presses a chord such as `mod+shift+z`: keys down in order, then up in
    /// reverse. `mod` is Ctrl on Windows and Cmd on macOS.
    pub fn chord(&mut self, combo: &str) -> Result<()> {
        let keys = combo.split('+').map(parse_key).collect::<Result<Vec<_>>>()?;
        for &key in &keys {
            self.0.key(key, Direction::Press)?;
            pause(25);
        }
        for &key in keys.iter().rev() {
            self.0.key(key, Direction::Release)?;
            pause(25);
        }
        pause(40);
        Ok(())
    }

    /// Types letters, digits, and spaces as key presses, so tool shortcuts
    /// see real keys; any other character is injected as Unicode text, which
    /// reaches the editor through the OS text input path.
    pub fn type_keys(&mut self, text: &str) -> Result<()> {
        for c in text.chars() {
            match c {
                'a'..='z' | '0'..='9' => self.chord(&c.to_string())?,
                'A'..='Z' => self.chord(&format!("shift+{}", c.to_ascii_lowercase()))?,
                ' ' => self.chord("space")?,
                _ => self.text(&c.to_string())?,
            }
        }
        Ok(())
    }

    /// Types arbitrary text as Unicode, for native text fields (file dialogs).
    pub fn text(&mut self, text: &str) -> Result<()> {
        self.0.text(text)?;
        pause(100);
        Ok(())
    }

    pub fn move_to(&mut self, (x, y): (i32, i32)) -> Result<()> {
        self.0.move_mouse(x, y, Coordinate::Abs)?;
        Ok(())
    }

    /// A left-button drag with intermediate moves.
    pub fn drag(&mut self, from: (i32, i32), to: (i32, i32)) -> Result<()> {
        self.drag_with_preview(from, to, || Ok(()))
    }

    pub fn drag_with_preview(
        &mut self,
        from: (i32, i32),
        to: (i32, i32),
        preview: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        self.move_to(from)?;
        pause(80);
        self.0.button(Button::Left, Direction::Press)?;
        pause(80);
        let result = (|| {
            const STEPS: i32 = 24;
            for s in 1..=STEPS {
                self.move_to((
                    from.0 + (to.0 - from.0) * s / STEPS,
                    from.1 + (to.1 - from.1) * s / STEPS,
                ))?;
                pause(12);
            }
            pause(150);
            preview()
        })();
        let release = self.0.button(Button::Left, Direction::Release);
        result?;
        release?;
        pause(150);
        Ok(())
    }

    pub fn click(&mut self, at: (i32, i32)) -> Result<()> {
        self.move_to(at)?;
        pause(80);
        self.0.button(Button::Left, Direction::Click)?;
        pause(150);
        Ok(())
    }

    pub fn double_click(&mut self, at: (i32, i32)) -> Result<()> {
        self.move_to(at)?;
        self.0.button(Button::Left, Direction::Click)?;
        pause(70);
        self.0.button(Button::Left, Direction::Click)?;
        pause(150);
        Ok(())
    }

    /// Always release the held key, including when the input action fails.
    pub fn held(&mut self, key: &str, action: impl FnOnce(&mut Self) -> Result<()>) -> Result<()> {
        let key = parse_key(key)?;
        self.0.key(key, Direction::Press)?;
        pause(50);
        let result = action(self);
        let release = self.0.key(key, Direction::Release);
        result?;
        release?;
        pause(100);
        Ok(())
    }

    pub fn scroll(&mut self, at: (i32, i32), notches: i32) -> Result<()> {
        self.move_to(at)?;
        self.0.scroll(notches, Axis::Vertical)?;
        pause(250);
        Ok(())
    }

    /// Moves the cursor out of the way so it doesn't show in screenshots.
    pub fn park(&mut self) -> Result<()> {
        self.move_to((0, 0))
    }
}

fn parse_key(name: &str) -> Result<Key> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "ctrl" => Key::Control,
        "shift" => Key::Shift,
        "alt" => Key::Alt,
        "cmd" => Key::Meta,
        "mod" if cfg!(target_os = "macos") => Key::Meta,
        "mod" => Key::Control,
        "esc" | "escape" => Key::Escape,
        "enter" => Key::Return,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "home" => Key::Home,
        "end" => Key::End,
        "space" => Key::Space,
        #[cfg(target_os = "macos")]
        code if code.starts_with("code:") => Key::Other(code[5..].parse::<u16>()?.into()),
        s if s.len() == 1 && s.chars().all(|c| c.is_ascii_alphanumeric()) => {
            Key::Unicode(s.chars().next().unwrap())
        }
        other => bail!("unknown key {other:?}"),
    })
}
