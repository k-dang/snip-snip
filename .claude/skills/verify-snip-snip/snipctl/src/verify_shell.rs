//! A native shell scenario with one owned run and unconditional cleanup.

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, anyhow, ensure};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

use crate::{Input, Run, platform, run, screen, wait_for};

const SETTINGS: &str = "snip-snip Settings";
const BASELINE: &str = "alt+shift+s";

pub fn verify(chord: &str) -> Result<()> {
    let started = Instant::now();
    Input::validate_chord(chord)?;
    let candidate: HotKey = chord.parse().context("invalid test shortcut")?;
    ensure!(
        candidate != BASELINE.parse::<HotKey>()?,
        "choose a replacement other than Alt+Shift+S"
    );
    ensure!(
        candidate.key != Code::Escape
            && candidate
                .mods
                .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER),
        "use Ctrl, Alt, or Command with a supported key"
    );
    ensure!(
        run::snip_snip_pids().is_empty(),
        "another snip-snip is running; close it before verification"
    );
    ensure!(
        platform::desktop_unlocked() == Some(true),
        "the Windows desktop must be unlocked"
    );
    let exe = run::app_exe();
    ensure!(
        fs::metadata(&exe)
            .context("build the release app first")?
            .modified()?
            >= crate::newest_mtime(&run::repo().join("crates"))?,
        "release app is stale; run cargo build --release -p snip-gpui"
    );
    let primary = xcap::Monitor::all()?
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .context("no primary monitor")?;
    ensure!(
        primary.x()? <= 100
            && primary.y()? <= 100
            && primary.x()? + primary.width()? as i32 >= 900
            && primary.y()? + primary.height()? as i32 >= 600,
        "the verification fixture must fit on the primary monitor at 100,100 (800x500)"
    );
    let path = dirs::config_dir()
        .context("locating shortcut settings")?
        .join("snip-snip/shortcut");
    let run = run::start()?;
    let preferences = Preferences::preserve(path)?;
    let mut session = Session {
        run,
        preferences,
        finished: false,
    };
    session.run.step(&match &session.preferences.original {
        Some(original) => format!("Original preferences backup: {}", original.display()),
        None => "Original preferences absent; cleanup will remove test settings".into(),
    })?;
    let outcome = scenario(&session, chord, candidate);
    if let Err(error) = &outcome {
        if let Some(pid) = session.run.owned("app") {
            screen::shot(pid, &session.run.path("failure.png")).ok();
        }
        session.run.step(&format!("FAIL  {error:#}")).ok();
    }
    let cleanup = session.finish();
    let result = match (outcome, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(anyhow!("{error:#}; cleanup failed: {cleanup:#}")),
    };
    if result.is_ok() {
        session.run.step(&format!(
            "PASS  Automated shell checks ({:.1}s)",
            started.elapsed().as_secs_f32()
        ))?;
        session.run.step(
            "REVIEW  menu-overlay.png: no menu, selected menu row, or tray flyout in the frozen image",
        )?;
        session
            .run
            .step("REVIEW  settings-applied.png: correct applied chord and no clipped labels/errors")?;
    } else if let Err(error) = &result {
        session.run.step(&format!("FAIL  Verification: {error:#}")).ok();
    }
    println!("Evidence: {}", session.run.dir.display());
    result
}

fn scenario(session: &Session, chord: &str, candidate: HotKey) -> Result<()> {
    let run = &session.run;
    fs::write(&session.preferences.path, BASELINE).context("preparing isolated shortcut baseline")?;
    crate::launch_in(run)?;
    crate::doctor()?;
    assert_no_windows(run)?;
    run.step("PASS  Windowless launch")?;

    crate::shell_menu("settings")?;
    let pid = crate::app(run)?;
    let first = screen::titled(pid, SETTINGS)?.context("Settings did not open")?;
    crate::shell_menu("settings")?;
    let windows = screen::windows_of(pid)?;
    let settings = windows
        .iter()
        .filter(|w| w.title().is_ok_and(|title| title == SETTINGS))
        .collect::<Vec<_>>();
    ensure!(
        settings.len() == 1 && settings[0].id()? == first.id()? && platform::is_focused(settings[0])?,
        "reopening Settings must focus the same single window"
    );
    run.step("PASS  Single Settings window, focused on reopen")?;
    crate::shell_key(&["enter".into(), chord.into(), "tab".into(), "enter".into()])?;
    wait_for(
        "the applied shortcut file (the chord may be reserved or in use; try verify-shell CHORD)",
        Duration::from_secs(5),
        || {
            Ok(fs::read_to_string(&session.preferences.path)
                .ok()
                .and_then(|saved| saved.trim().parse::<HotKey>().ok())
                .filter(|saved| *saved == candidate))
        },
    )?;
    screen::titled(pid, SETTINGS)?
        .context("Settings closed unexpectedly")?
        .capture_image()?
        .save(run.path("settings-applied.png"))?;
    run.step("PASS  Replacement shortcut persisted through Settings")?;
    crate::shell_key(&["esc".into()])?;
    wait_for("Settings to close", Duration::from_secs(5), || {
        Ok(screen::titled(pid, SETTINGS)?.is_none().then_some(()))
    })?;

    crate::target()?;
    crate::snip(vec![200., 200., 600., 300.], chord)?;
    crate::shot("fixture-editor")?;
    run.step("Expect fixture.png: 600x300, blue=90000 green=90000 ink=0")?;
    crate::save("fixture")?;
    check_fixture(&run.path("fixture.png"))?;
    run.step(&format!(
        "PASS  Fixture export: {}",
        screen::inspect(&run.path("fixture.png"))?
    ))?;
    crate::close()?;
    assert_no_windows(run)?;
    run.step("PASS  Editor closure leaves the shell running")?;
    crate::shell_menu("quit")?;
    fs::copy(run.path("app.log"), run.path("app-first-launch.log"))?;
    crate::launch_in(run)?;
    assert_no_windows(run)?;
    let mut input = Input::new()?;
    crate::open_overlay(run, &mut input, chord).context("saved shortcut did not capture after restart")?;
    cancel_overlay(run, &mut input)?;
    run.step("PASS  Saved shortcut captures after restart")?;

    crate::shell_menu("new-snip")?;
    crate::shot("menu-overlay")?;
    cancel_overlay(run, &mut input)?;
    run.step("PASS  Menu capture opens and cancels")?;
    crate::shell_menu("quit")?;
    ensure!(run.owned("app").is_none(), "Quit left the app running");
    run.step("PASS  Native Quit exits")
}

fn assert_no_windows(run: &Run) -> Result<()> {
    let pid = crate::app(run)?;
    ensure!(
        screen::windows_of(pid)?.is_empty(),
        "expected a running shell with no app windows"
    );
    Ok(())
}

fn cancel_overlay(run: &Run, input: &mut Input) -> Result<()> {
    let pid = crate::app(run)?;
    let overlay = screen::titled(pid, screen::OVERLAY_TITLE)?.context("capture overlay is missing")?;
    platform::focus(&overlay, pid, input)?;
    input.chord("esc")?;
    wait_for("all capture overlays to close", Duration::from_secs(5), || {
        Ok(screen::windows_of(pid)?.is_empty().then_some(()))
    })
}

fn check_fixture(path: &Path) -> Result<()> {
    let image = image::open(path)?.to_rgb8();
    ensure!(
        image.dimensions() == (600, 300),
        "fixture export dimensions changed: {:?}",
        image.dimensions()
    );
    ensure!(
        image
            .enumerate_pixels()
            .all(|(x, _, pixel)| pixel.0 == screen::COLORS[usize::from(x >= 300)].1),
        "fixture export must contain exactly the expected blue and green halves"
    );
    Ok(())
}

struct Session {
    run: Run,
    preferences: Preferences,
    finished: bool,
}

impl Session {
    fn finish(&mut self) -> Result<()> {
        self.finished = true;
        let mut errors = Vec::new();
        for name in ["app", "fixture"] {
            if let Err(error) = self.run.stop(name) {
                errors.push(format!("stopping owned {name}: {error:#}"));
            }
        }
        // An app that could not be stopped might still finish an Apply. Keep
        // the original safely backed up rather than expose it to that write.
        let restore = if self.run.owned("app").is_none() {
            self.preferences.restore()
        } else {
            Err(anyhow!(
                "owned app is still running; original backup retained at {:?}",
                self.preferences.original
            ))
        };
        match restore {
            Ok(()) => {
                self.run.step("PASS  Original preferences restored")?;
            }
            Err(error) => errors.push(format!("restoring preferences: {error:#}")),
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.finished
            && let Err(error) = self.finish()
        {
            eprintln!("verification cleanup failed: {error:#}");
        }
    }
}

struct Preferences {
    path: PathBuf,
    original: Option<PathBuf>,
}

impl Preferences {
    fn preserve(path: PathBuf) -> Result<Self> {
        let parent = path.parent().context("shortcut settings have no parent")?;
        fs::create_dir_all(parent)?;
        let original = match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                ensure!(
                    metadata.is_file() && !metadata.is_symlink(),
                    "shortcut settings must be a regular file"
                );
                // Keep the original beside the test file on the same volume.
                // Rename preserves its bytes, permissions and other metadata.
                let backup = tempfile::NamedTempFile::new_in(parent)?.into_temp_path().keep()?;
                fs::remove_file(&backup)?;
                fs::rename(&path, &backup).context("preserving original preferences")?;
                Some(backup)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(error).context("checking original preferences"),
        };
        Ok(Self { path, original })
    }

    fn restore(&mut self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "removing test settings; original backup: {}",
                        self.original
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "none (original settings were absent)".into())
                    )
                });
            }
        }
        if let Some(original) = &self.original {
            fs::rename(original, &self.path)
                .with_context(|| format!("restoring original from {}", original.display()))?;
            self.original = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_a_failed_session_restores_original_preference_bytes() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("shortcut");
        let original = b"malformed\0preferences\xff";
        fs::write(&path, original).unwrap();
        {
            let session = Session {
                run: Run {
                    dir: folder.path().join("evidence"),
                },
                preferences: Preferences::preserve(path.clone()).unwrap(),
                finished: false,
            };
            fs::create_dir_all(&session.run.dir).unwrap();
            fs::write(&path, "control+alt+KeyK").unwrap();
            // A failed scenario returns before explicit finish, exercising Drop.
        }
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 2);
    }

    #[test]
    fn finishing_a_session_removes_preferences_that_were_originally_absent() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("shortcut");
        let mut session = Session {
            run: Run {
                dir: folder.path().join("evidence"),
            },
            preferences: Preferences::preserve(path.clone()).unwrap(),
            finished: false,
        };
        fs::create_dir_all(&session.run.dir).unwrap();
        fs::write(&path, BASELINE).unwrap();
        session.finish().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn failed_restore_retains_the_original_backup_and_reports_its_location() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("shortcut");
        fs::write(&path, "original").unwrap();
        let mut preferences = Preferences::preserve(path.clone()).unwrap();
        fs::create_dir(&path).unwrap();
        let error = preferences.restore().unwrap_err();
        let backup = preferences.original.as_ref().unwrap();
        assert_eq!(fs::read_to_string(backup).unwrap(), "original");
        assert!(format!("{error:#}").contains(&backup.display().to_string()));
    }

    #[test]
    fn fixture_check_rejects_wrong_pixels_even_when_color_totals_match() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("fixture.png");
        let mut image = image::RgbImage::from_fn(600, 300, |x, _| {
            image::Rgb(screen::COLORS[usize::from(x >= 300)].1)
        });
        image.save(&path).unwrap();
        check_fixture(&path).unwrap();
        image.put_pixel(0, 0, image::Rgb(screen::COLORS[1].1));
        image.put_pixel(300, 0, image::Rgb(screen::COLORS[0].1));
        image.save(&path).unwrap();
        assert!(check_fixture(&path).is_err());
    }
}
