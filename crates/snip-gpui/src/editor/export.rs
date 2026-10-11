//! Clipboard and PNG export.

use std::{
    borrow::Cow,
    fs,
    io::ErrorKind,
    path::PathBuf,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result};
use chrono::{DateTime, FixedOffset, Local, TimeZone};
use futures::channel::oneshot;
use gpui::{App, AsyncWindowContext, Context, Global, Task, Window};
use snip_core::RgbaImage;

use super::Editor;

pub(super) trait ExportEnvironment {
    fn directories(&self) -> (Option<PathBuf>, Option<PathBuf>);
    fn local_time(&self) -> Result<DateTime<FixedOffset>>;
    fn write_clipboard(&self, image: RgbaImage) -> Result<()>;
    fn choose_save_path(
        self: Rc<Self>,
        directory: PathBuf,
        name: String,
        cx: &mut AsyncWindowContext,
    ) -> Task<Result<Option<PathBuf>>>;
}

pub(super) struct ExportServices(pub Rc<dyn ExportEnvironment>);

impl Global for ExportServices {}

impl Default for ExportServices {
    fn default() -> Self {
        Self(Rc::new(NativeExport))
    }
}

struct NativeExport;

impl ExportEnvironment for NativeExport {
    fn directories(&self) -> (Option<PathBuf>, Option<PathBuf>) {
        (dirs::picture_dir(), dirs::home_dir())
    }

    fn local_time(&self) -> Result<DateTime<FixedOffset>> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("reading the current time")?;
        let seconds = i64::try_from(elapsed.as_secs()).context("current time is out of range")?;
        Local
            .timestamp_opt(seconds, elapsed.subsec_nanos())
            .single()
            .map(|time| time.fixed_offset())
            .context("converting the current time to local time")
    }

    fn write_clipboard(&self, image: RgbaImage) -> Result<()> {
        // GPUI only writes PNG; arboard also supplies Windows bitmap formats.
        let data = arboard::ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: Cow::Owned(image.into_raw()),
        };
        arboard::Clipboard::new()?.set_image(data)?;
        Ok(())
    }

    fn choose_save_path(
        self: Rc<Self>,
        directory: PathBuf,
        name: String,
        cx: &mut AsyncWindowContext,
    ) -> Task<Result<Option<PathBuf>>> {
        window_save_picker(
            cx,
            |window, _| window.activate_window(),
            move |cx| cx.prompt_for_new_path(&directory, Some(&name)),
        )
    }
}

pub(super) fn window_save_picker(
    cx: &mut AsyncWindowContext,
    activate: impl FnOnce(&mut Window, &App) + 'static,
    prompt: impl FnOnce(&App) -> oneshot::Receiver<Result<Option<PathBuf>>> + 'static,
) -> Task<Result<Option<PathBuf>>> {
    cx.spawn(async move |cx| {
        // Reject an editor closed during folder preparation.
        cx.update(|window, cx| activate(window, cx))?;
        // Windows queues activation. Its foreground executor runs in order;
        // wait for that turn before GPUI captures the active dialog owner.
        cx.foreground_executor().spawn(async {}).await;
        let chosen = cx.update(|window, cx| {
            anyhow::ensure!(
                cx.active_window() == Some(window.window_handle()),
                "cannot activate the editor for the save dialog"
            );
            Ok(prompt(cx))
        })??;
        chosen.await.context("waiting for the save dialog")?
    })
}

fn save_defaults(directory: PathBuf, time: DateTime<FixedOffset>) -> Result<(PathBuf, String)> {
    fs::create_dir_all(&directory).with_context(|| format!("preparing {}", directory.display()))?;
    // Surface enumeration errors before opening the picker.
    for entry in fs::read_dir(&directory).with_context(|| format!("reading {}", directory.display()))? {
        entry.context("reading the save folder")?;
    }
    let stem = time.format("snip-%Y-%m-%d_%H-%M-%S-%3f").to_string();
    let mut name = format!("{stem}.png");
    let mut suffix = 2u64;
    loop {
        match fs::symlink_metadata(directory.join(&name)) {
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok((directory, name)),
            Err(error) => return Err(error).context("checking the suggested filename"),
            Ok(_) => {
                name = format!("{stem}-{suffix}.png");
                suffix += 1;
            }
        }
    }
}

impl Editor {
    pub(super) fn copy(&mut self, close_after_copy: bool, window: &mut Window, cx: &mut Context<Self>) {
        let image = self.export(window);
        let environment = cx.default_global::<ExportServices>().0.clone();
        self.status = match environment.write_clipboard(image) {
            Ok(()) if close_after_copy => {
                window.remove_window();
                return;
            }
            Ok(()) => "Copied to clipboard".into(),
            Err(error) => format!("Copy failed: {error:#}").into(),
        };
        cx.notify();
    }

    pub(super) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let image = self.export(window);
        let environment = cx.default_global::<ExportServices>().0.clone();
        let (pictures, home) = environment.directories();
        let defaults = pictures
            .or(home)
            .context("cannot locate Pictures or the home directory")
            .and_then(|directory| {
                environment
                    .local_time()
                    .map(|time| (directory.join("snip-snip"), time))
            });
        let (directory, time) = match defaults {
            Ok(defaults) => defaults,
            Err(error) => {
                self.status = format!("Save failed: {error:#}").into();
                cx.notify();
                return;
            }
        };
        // Pictures may be redirected to slow storage.
        let defaults = cx
            .background_executor()
            .spawn(async move { save_defaults(directory, time) });
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let written: Result<Option<PathBuf>> = async {
                let (directory, name) = defaults.await?;
                let Some(path) = environment.choose_save_path(directory, name, cx).await? else {
                    return Ok(None);
                };
                cx.background_executor()
                    .spawn(async move {
                        fs::write(&path, snip_core::encode_png(&image))
                            .with_context(|| format!("writing {}", path.display()))?;
                        Ok(Some(path))
                    })
                    .await
            }
            .await;
            let status = match written {
                Ok(Some(path)) => format!("Saved {}", path.display()),
                Err(error) => format!("Save failed: {error:#}"),
                Ok(None) => return,
            };
            this.update(cx, |this, cx| {
                this.status = status.into();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
