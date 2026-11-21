//! The Library window: the newest cloud snips as thumbnails. Clicking one
//! copies it; Delete removes it from the bucket after a confirming click.

use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

use futures::{StreamExt, channel::mpsc};
use gpui::{
    App, Bounds, Context, ElementId, Global, ObjectFit, RenderImage, SharedString, Task, TitlebarOptions,
    Window, WindowBounds, WindowHandle, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use snip_cloud::CloudSnip;
use snip_core::RgbaImage;

use crate::{
    cloud::{self, CloudStore},
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

/// The outcome of the last action, shown under the header.
type Status = Result<SharedString, SharedString>;

pub struct Library {
    /// `None` until the first listing arrives.
    tiles: Option<Vec<Tile>>,
    status: Option<Status>,
    /// The snip whose Delete was clicked once and awaits confirmation.
    confirming: Option<CloudSnip>,
    /// Replaced thumbnails to release from the GPU atlas on the next frame.
    released: Vec<Arc<RenderImage>>,
    /// Listing and thumbnails; replacing it cancels the previous load.
    loading: Task<()>,
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
    let Some(copied) = copy(cx, |store| match store.latest(1)?.first() {
        Some(snip) => store.image(snip).map(Some),
        None => Ok(None),
    }) else {
        return;
    };
    cx.spawn(async move |cx| {
        let failure = match copied.await {
            Ok(true) => return,
            Ok(false) => "There are no cloud snips yet.".to_owned(),
            Err(message) => format!("Could not copy the latest cloud snip: {message}"),
        };
        cx.update(|cx| show(Some(Err(failure.into())), cx));
    })
    .detach();
}

/// Fetches an image from the bucket and puts it on the clipboard. Resolves
/// to `false` when `fetch` finds nothing to copy.
fn copy(
    cx: &mut App,
    fetch: impl FnOnce(&dyn CloudStore) -> snip_cloud::Result<Option<RgbaImage>> + Send + 'static,
) -> Option<Task<Result<bool, String>>> {
    let fetched = cloud::run(cx, fetch)?;
    Some(cx.spawn(async move |cx| {
        let Some(image) = fetched.await? else {
            return Ok(false);
        };
        cx.update(|cx| crate::clipboard::write(&image, cx))
            .map(|()| true)
            .map_err(|error| format!("{error:#}"))
    }))
}

fn show(status: Option<Status>, cx: &mut App) {
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
                cx.observe_global::<cloud::Cloud>(|_, cx| cx.notify()).detach();
                let mut library = Library {
                    tiles: None,
                    status,
                    confirming: None,
                    released: Vec::new(),
                    loading: Task::ready(()),
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
        self.loading = cx.spawn(async move |this, cx| {
            let listed = listing.await;
            let Ok(needed) = this.update(cx, |this, cx| this.listed(listed, cx)) else {
                return;
            };
            if needed.is_empty() {
                return;
            }
            let (arrived, mut arrivals) = mpsc::unbounded();
            let queue = Arc::new(Mutex::new(needed.into_iter()));
            let workers = cx.update(|cx| {
                (0..WORKERS)
                    .filter_map(|_| {
                        let (queue, arrived) = (queue.clone(), arrived.clone());
                        cloud::run(cx, move |store| fetch_thumbnails(store, &queue, &arrived))
                    })
                    .collect::<Vec<_>>()
            });
            drop(arrived);
            while let Some((snip, bytes, image)) = arrivals.next().await {
                if this
                    .update(cx, |this, cx| this.thumbnail_arrived(snip, bytes, image, cx))
                    .is_err()
                {
                    return;
                }
            }
            for worker in workers {
                if let Err(message) = worker.await {
                    this.update(cx, |this, cx| {
                        this.status = Some(Err(format!("Could not load a thumbnail: {message}").into()));
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            }
        });
    }

    /// Shows a listing, keeping thumbnails already on screen, and returns the
    /// snips that still need one with any bytes this connection cached.
    fn listed(&mut self, listed: Result<Vec<CloudSnip>, String>, cx: &mut Context<Self>) -> Vec<Needed> {
        let snips = match listed {
            Ok(snips) => snips,
            Err(message) => {
                self.status = Some(Err(format!("Could not list cloud snips: {message}").into()));
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
        let cache = cloud::connection(cx).map(|connection| connection.thumbnails.borrow().clone());
        let needed = tiles
            .iter()
            .filter(|tile| tile.thumbnail.is_none())
            .map(|tile| {
                let cached = cache.as_ref().and_then(|cache| cache.get(&tile.snip).cloned());
                (tile.snip.clone(), cached)
            })
            .collect();
        self.tiles = Some(tiles);
        cx.notify();
        needed
    }

    fn thumbnail_arrived(
        &mut self,
        snip: CloudSnip,
        bytes: Arc<Vec<u8>>,
        image: Arc<RenderImage>,
        cx: &mut Context<Self>,
    ) {
        if let Some(connection) = cloud::connection(cx) {
            connection.thumbnails.borrow_mut().insert(snip.clone(), bytes);
        }
        match self.tiles.iter_mut().flatten().find(|tile| tile.snip == snip) {
            Some(tile) => self.released.extend(tile.thumbnail.replace(image)),
            None => self.released.push(image),
        }
        cx.notify();
    }

    fn copy(&mut self, snip: CloudSnip, label: SharedString, cx: &mut Context<Self>) {
        let Some(copied) = copy(cx, move |store| store.image(&snip).map(Some)) else {
            return;
        };
        self.confirming = None;
        self.status = Some(Ok("Copying…".into()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let status = match copied.await {
                Ok(_) => Ok(format!("Copied the snip from {label}").into()),
                Err(message) => Err(format!("Copy failed: {message}").into()),
            };
            this.update(cx, |this, cx| {
                this.status = Some(status);
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
        self.status = Some(Ok("Deleting…".into()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let deleted = deleted.await;
            this.update(cx, |this, cx| {
                this.status = Some(match deleted {
                    Ok(()) => {
                        if let Some(connection) = cloud::connection(cx) {
                            connection.thumbnails.borrow_mut().remove(&snip);
                        }
                        if let Some(tiles) = &mut this.tiles
                            && let Some(index) = tiles.iter().position(|tile| tile.snip == snip)
                        {
                            this.released.extend(tiles.remove(index).thumbnail);
                        }
                        Ok("Deleted the snip".into())
                    }
                    Err(message) => Err(format!("Delete failed: {message}").into()),
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
        let account = cloud::connection(cx).map(|connection| connection.account.name.clone());
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
            .children(self.status.clone().map(|status| match status {
                Ok(text) => line(text).text_color(rgb(ui::SUCCESS)),
                Err(text) => line(text).text_color(rgb(ui::ERROR)),
            }))
            .child(body)
    }
}

/// A snip without a thumbnail on screen, with its cached bytes if any.
type Needed = (CloudSnip, Option<Arc<Vec<u8>>>);
/// A thumbnail ready to show, with its bytes for the cache.
type Arrival = (CloudSnip, Arc<Vec<u8>>, Arc<RenderImage>);

/// One of the Library's thumbnail workers: downloads and decodes snips from
/// the shared queue, sending each as it is ready, until the queue is empty, a
/// fetch fails, or the Library stops listening.
fn fetch_thumbnails(
    store: &dyn CloudStore,
    queue: &Mutex<std::vec::IntoIter<Needed>>,
    arrived: &mpsc::UnboundedSender<Arrival>,
) -> snip_cloud::Result<()> {
    while !arrived.is_closed() {
        let Some((snip, cached)) = queue.lock().unwrap().next() else {
            break;
        };
        let bytes = match cached {
            Some(bytes) => bytes,
            None => Arc::new(store.thumbnail(&snip)?),
        };
        let image = crate::capture::to_render_image(&snip_cloud::decode(&bytes)?);
        arrived.unbounded_send((snip, bytes, image)).ok();
    }
    Ok(())
}

fn uploaded_label(uploaded: SystemTime) -> String {
    let uploaded: chrono::DateTime<chrono::Local> = uploaded.into();
    if uploaded.date_naive() == chrono::Local::now().date_naive() {
        uploaded.format("Today %H:%M").to_string()
    } else {
        uploaded.format("%b %-d, %H:%M").to_string()
    }
}
