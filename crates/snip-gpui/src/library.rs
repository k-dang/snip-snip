//! The Library window: the newest cloud snips as thumbnails. Clicking one
//! copies it; Delete removes it from the bucket after a confirming click.

use std::{
    collections::HashMap,
    sync::{Arc, Weak},
    time::SystemTime,
};

use gpui::{
    App, Bounds, Context, ElementId, Global, ObjectFit, RenderImage, SharedString, TitlebarOptions, Window,
    WindowBounds, WindowHandle, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use snip_cloud::CloudSnip;

use crate::{
    cloud::{self, Cloud, CloudStore, State},
    ui::{self, button, line},
};

// GPUI's rendered-element selectors are only available in debug builds.
#[cfg(all(test, debug_assertions))]
mod tests;

/// How many snips the Library shows, newest first.
const LIMIT: usize = 100;
/// Thumbnails download this many at a time.
const WORKERS: usize = 4;

#[derive(Default)]
struct LibraryWindow(Option<WindowHandle<Library>>);

impl Global for LibraryWindow {}

/// Encoded thumbnails downloaded this run, valid for one signed-in bucket.
#[derive(Default)]
struct Thumbnails {
    store: Option<Weak<dyn CloudStore>>,
    bytes: HashMap<CloudSnip, Arc<Vec<u8>>>,
}

impl Global for Thumbnails {}

/// The cache for the current bucket, emptied after a sign-out or account change.
fn thumbnails(cx: &mut App) -> &mut HashMap<CloudSnip, Arc<Vec<u8>>> {
    let current = cloud::store(cx).map(|store| Arc::downgrade(&store));
    let cache = cx.default_global::<Thumbnails>();
    let same = match (&cache.store, &current) {
        (Some(cached), Some(current)) => cached.ptr_eq(current),
        _ => false,
    };
    if !same {
        *cache = Thumbnails {
            store: current,
            bytes: HashMap::new(),
        };
    }
    &mut cache.bytes
}

pub struct Library {
    /// `None` until the first listing arrives.
    tiles: Option<Vec<Tile>>,
    status: Option<SharedString>,
    /// The snip whose Delete was clicked once and awaits confirmation.
    confirming: Option<CloudSnip>,
    /// Replaced thumbnails to release from the GPU atlas on the next frame.
    released: Vec<Arc<RenderImage>>,
}

struct Tile {
    snip: CloudSnip,
    label: SharedString,
    thumbnail: Option<Arc<RenderImage>>,
}

pub fn open(cx: &mut App) {
    show(None, cx);
}

/// Copies the newest cloud snip, from the tray. Failures open the Library to
/// explain, since the tray has nowhere to show them.
pub fn copy_latest(cx: &mut App) {
    let Some(latest) = cloud::run(cx, |store| match store.latest(1)?.first() {
        Some(snip) => store.image(snip).map(Some),
        None => Ok(None),
    }) else {
        return;
    };
    cx.spawn(async move |cx| {
        let latest = latest.await;
        cx.update(|cx| {
            let failure = match latest {
                Ok(Some(image)) => match crate::clipboard::write(&image, cx) {
                    Ok(()) => return,
                    Err(error) => format!("Could not copy the latest cloud snip: {error:#}"),
                },
                Ok(None) => "There are no cloud snips yet.".to_owned(),
                Err(message) => format!("Could not copy the latest cloud snip: {message}"),
            };
            show(Some(failure.into()), cx);
        });
    })
    .detach();
}

fn show(status: Option<SharedString>, cx: &mut App) {
    cx.activate(true);
    if let Some(handle) = cx.default_global::<LibraryWindow>().0 {
        let reopened = handle.update(cx, |library, window, cx| {
            window.activate_window();
            if status.is_some() {
                library.status = status.clone();
            }
            library.reload(cx);
        });
        if reopened.is_ok() {
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(760.), px(560.)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("snip-snip Library".into()),
                ..Default::default()
            }),
            window_min_size: Some(size(px(520.), px(360.))),
            ..Default::default()
        },
        |window, cx| {
            window.activate_window();
            // See the editor: OS-initiated closes leak GPUI windows on Windows.
            window.on_window_should_close(cx, |window, _| {
                window.remove_window();
                false
            });
            cx.new(|cx| {
                cx.observe_global::<Cloud>(|_, cx| cx.notify()).detach();
                let mut library = Library {
                    tiles: None,
                    status,
                    confirming: None,
                    released: Vec::new(),
                };
                library.reload(cx);
                library
            })
        },
    );
    match opened {
        Ok(handle) => cx.set_global(LibraryWindow(Some(handle))),
        Err(error) => eprintln!("library: could not open: {error:#}"),
    }
}

impl Library {
    fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(listing) = cloud::run(cx, |store| store.latest(LIMIT)) else {
            return;
        };
        self.confirming = None;
        cx.spawn(async move |this, cx| {
            let listed = listing.await;
            let Ok(missing) = this.update(cx, |this, cx| this.listed(listed, cx)) else {
                return;
            };
            // Interleave so the newest thumbnails arrive first.
            for worker in 0..WORKERS {
                let share: Vec<_> = missing.iter().skip(worker).step_by(WORKERS).cloned().collect();
                let this = this.clone();
                cx.spawn(async move |cx| {
                    for snip in share {
                        let fetch = cx.update(|cx| {
                            let cached = thumbnails(cx).get(&snip).cloned();
                            let fetching = snip.clone();
                            cloud::run(cx, move |store| thumbnail(store, &fetching, cached))
                        });
                        let Some(fetch) = fetch else {
                            return;
                        };
                        let fetched = fetch.await;
                        if this
                            .update(cx, |this, cx| this.thumbnail_arrived(snip, fetched, cx))
                            .is_err()
                        {
                            return;
                        }
                    }
                })
                .detach();
            }
        })
        .detach();
    }

    /// Shows a listing, keeping thumbnails already on screen, and returns the
    /// snips that still need one.
    fn listed(&mut self, listed: Result<Vec<CloudSnip>, String>, cx: &mut Context<Self>) -> Vec<CloudSnip> {
        let snips = match listed {
            Ok(snips) => snips,
            Err(message) => {
                self.status = Some(format!("Could not list cloud snips: {message}").into());
                cx.notify();
                return Vec::new();
            }
        };
        let mut previous = self.tiles.take().unwrap_or_default();
        let tiles: Vec<Tile> = snips
            .into_iter()
            .map(|snip| {
                let thumbnail = previous
                    .iter_mut()
                    .find(|tile| tile.snip == snip)
                    .and_then(|tile| tile.thumbnail.take());
                Tile {
                    label: uploaded_label(snip.uploaded()).into(),
                    snip,
                    thumbnail,
                }
            })
            .collect();
        self.released
            .extend(previous.into_iter().filter_map(|tile| tile.thumbnail));
        let missing = tiles
            .iter()
            .filter(|tile| tile.thumbnail.is_none())
            .map(|tile| tile.snip.clone())
            .collect();
        self.tiles = Some(tiles);
        cx.notify();
        missing
    }

    fn thumbnail_arrived(
        &mut self,
        snip: CloudSnip,
        fetched: Result<(Arc<Vec<u8>>, Arc<RenderImage>), String>,
        cx: &mut Context<Self>,
    ) {
        match fetched {
            Ok((bytes, image)) => {
                thumbnails(cx).insert(snip.clone(), bytes);
                match self.tiles.iter_mut().flatten().find(|tile| tile.snip == snip) {
                    Some(tile) => self.released.extend(tile.thumbnail.replace(image)),
                    None => self.released.push(image),
                }
            }
            Err(message) => self.status = Some(format!("Could not load a thumbnail: {message}").into()),
        }
        cx.notify();
    }

    fn copy(&mut self, snip: CloudSnip, label: SharedString, cx: &mut Context<Self>) {
        let Some(image) = cloud::run(cx, move |store| store.image(&snip)) else {
            return;
        };
        self.confirming = None;
        self.status = Some("Copying…".into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let image = image.await;
            this.update(cx, |this, cx| {
                let status = match image {
                    Ok(image) => match crate::clipboard::write(&image, cx) {
                        Ok(()) => format!("Copied the snip from {label}"),
                        Err(error) => format!("Copy failed: {error:#}"),
                    },
                    Err(message) => format!("Copy failed: {message}"),
                };
                this.status = Some(status.into());
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn delete(&mut self, snip: CloudSnip, cx: &mut Context<Self>) {
        if self.confirming.as_ref() != Some(&snip) {
            self.confirming = Some(snip);
            return cx.notify();
        }
        let deleting = snip.clone();
        let Some(deleted) = cloud::run(cx, move |store| store.delete(&deleting)) else {
            return;
        };
        self.confirming = None;
        self.status = Some("Deleting…".into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let deleted = deleted.await;
            this.update(cx, |this, cx| {
                this.status = Some(match deleted {
                    Ok(()) => {
                        thumbnails(cx).remove(&snip);
                        if let Some(tiles) = &mut this.tiles
                            && let Some(index) = tiles.iter().position(|tile| tile.snip == snip)
                        {
                            this.released.extend(tiles.remove(index).thumbnail);
                        }
                        "Deleted the snip".into()
                    }
                    Err(message) => format!("Delete failed: {message}").into(),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn tile(&self, index: usize, tile: &Tile, cx: &mut Context<Self>) -> impl IntoElement {
        let confirming = self.confirming.as_ref() == Some(&tile.snip);
        let (copy, label) = (tile.snip.clone(), tile.label.clone());
        let delete = tile.snip.clone();
        div()
            .id(ElementId::NamedInteger("snip".into(), index as u64))
            .debug_selector(move || format!("snip-{index}"))
            .w(px(220.))
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x3a414b))
            .hover(|style| style.border_color(rgb(ui::FOCUS)))
            .cursor_pointer()
            .child(
                div()
                    .h(px(140.))
                    .w_full()
                    .rounded_sm()
                    .bg(rgb(0x15181c))
                    .children(
                        tile.thumbnail
                            .clone()
                            .map(|image| img(image).size_full().object_fit(ObjectFit::Contain)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(13.))
                    .child(div().flex_1().child(tile.label.clone()))
                    .child(
                        button(
                            format!("delete-{index}"),
                            if confirming { "Confirm delete" } else { "Delete" },
                        )
                        .when(confirming, |button| {
                            button.border_color(rgb(ui::ERROR)).text_color(rgb(ui::ERROR))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.delete(delete.clone(), cx);
                        })),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.copy(copy.clone(), label.clone(), cx)))
    }
}

impl Render for Library {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.released.drain(..) {
            window.drop_image(image).ok();
        }
        let account = match &cx.global::<Cloud>().state {
            State::Ready(_, account) => Some(account.name.clone()),
            _ => None,
        };
        let header = div()
            .flex()
            .items_center()
            .gap_3()
            .child(div().text_size(px(20.)).child("Cloud library"))
            .child(div().flex_1())
            .children(
                account
                    .clone()
                    .map(|name| div().text_color(rgb(ui::MUTED)).child(name)),
            )
            .when(account.is_some(), |header| {
                header.child(
                    button("refresh", "Refresh").on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
                )
            });
        let message = |text: &'static str| line(text).text_color(rgb(ui::MUTED)).into_any_element();
        let body = if account.is_none() {
            message("Sign in with Cloudflare in Settings to see your cloud snips.")
        } else {
            match &self.tiles {
                None => message("Loading…"),
                Some(tiles) if tiles.is_empty() => {
                    message("No cloud snips yet. Use Copy to cloud in the editor's Export menu.")
                }
                Some(tiles) => {
                    let mut grid = div().flex().flex_wrap().gap_3();
                    for (index, tile) in tiles.iter().enumerate() {
                        grid = grid.child(self.tile(index, tile, cx));
                    }
                    grid.into_any_element()
                }
            }
        };
        div()
            .id("library")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .bg(rgb(ui::SURFACE))
            .text_color(rgb(ui::TEXT))
            .text_size(px(15.))
            .child(header)
            .children(
                self.status
                    .clone()
                    .map(|status| line(status).text_color(rgb(ui::SUCCESS))),
            )
            .child(body)
    }
}

fn thumbnail(
    store: &dyn CloudStore,
    snip: &CloudSnip,
    cached: Option<Arc<Vec<u8>>>,
) -> snip_cloud::Result<(Arc<Vec<u8>>, Arc<RenderImage>)> {
    let bytes = match cached {
        Some(bytes) => bytes,
        None => Arc::new(store.thumbnail(snip)?),
    };
    let image = crate::capture::to_render_image(&snip_cloud::decode(&bytes)?);
    Ok((bytes, image))
}

fn uploaded_label(uploaded: SystemTime) -> String {
    let uploaded: chrono::DateTime<chrono::Local> = uploaded.into();
    if uploaded.date_naive() == chrono::Local::now().date_naive() {
        uploaded.format("Today %H:%M").to_string()
    } else {
        uploaded.format("%b %-d, %H:%M").to_string()
    }
}
