use global_hotkey::hotkey::HotKey;
use gpui::{
    App, Bounds, Context, Div, ElementId, FocusHandle, FontWeight, KeyDownEvent, ModifiersChangedEvent,
    MouseButton, Pixels, Render, SharedString, Stateful, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size, transparent_black,
};

use super::{Shell, shortcut};
use crate::{
    cloud::{self, Cloud, State},
    ui::{self, button, line},
};

pub struct Settings {
    focus: FocusHandle,
    record_focus: FocusHandle,
    apply_focus: FocusHandle,
    candidate: Option<HotKey>,
    recording: bool,
    modifiers_pressed: bool,
    message: Option<String>,
}

pub fn open(cx: &mut App) {
    cx.activate(true);
    if let Some(handle) = cx.global::<Shell>().settings
        && handle
            .update(cx, |_, window, cx| {
                window.activate_window();
                cx.notify();
            })
            .is_ok()
    {
        return;
    }
    let bounds = Bounds::centered(None, size(px(520.), px(520.)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("snip-snip Settings".into()),
                ..Default::default()
            }),
            window_min_size: Some(size(px(440.), px(420.))),
            ..Default::default()
        },
        |window, cx| {
            cx.new(|cx| {
                window.set_window_title("snip-snip Settings");
                window.activate_window();
                let focus = cx.focus_handle();
                let record_focus = cx.focus_handle();
                window.focus(&record_focus, cx);
                window.on_window_should_close(cx, |window, _| {
                    window.remove_window();
                    false
                });
                cx.observe_window_activation(window, |this: &mut Settings, window, cx| {
                    if !window.is_window_active() {
                        this.cancel(cx);
                    }
                })
                .detach();
                cx.on_blur(&record_focus, window, |this: &mut Settings, _, cx| {
                    this.cancel(cx)
                })
                .detach();
                cx.observe_global::<Cloud>(|_, cx| cx.notify()).detach();
                cx.on_release(|_, cx| {
                    if cx.has_global::<Shell>() {
                        cx.global_mut::<Shell>().recording = false;
                    }
                })
                .detach();
                Settings {
                    focus,
                    record_focus,
                    apply_focus: cx.focus_handle(),
                    candidate: None,
                    recording: false,
                    modifiers_pressed: false,
                    message: None,
                }
            })
        },
    );
    match opened {
        Ok(handle) => cx.global_mut::<Shell>().settings = Some(handle),
        Err(error) => eprintln!("settings: could not open: {error:#}"),
    }
}

impl Settings {
    fn record(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if cx.global::<Shell>().applying {
            return;
        }
        self.recording = true;
        self.modifiers_pressed = false;
        self.candidate = None;
        self.message = None;
        let shell = cx.global_mut::<Shell>();
        shell.recording = true;
        shell.status = None;
        window.focus(&self.record_focus, cx);
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.recording {
            self.stop_recording(cx);
        }
    }

    /// Drops the recording or the proposed shortcut, keeping the applied one.
    fn discard(&mut self, cx: &mut Context<Self>) {
        self.candidate = None;
        self.stop_recording(cx);
    }

    fn stop_recording(&mut self, cx: &mut Context<Self>) {
        self.recording = false;
        self.message = None;
        cx.global_mut::<Shell>().recording = false;
        cx.notify();
    }

    pub(super) fn record_registered(&mut self, shortcut: HotKey, cx: &mut Context<Self>) {
        if self.recording {
            self.candidate = Some(shortcut);
            self.cancel(cx);
        }
    }

    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording {
            return;
        }
        let Some(candidate) = self.candidate else {
            return;
        };
        if cx.global::<Shell>().applying {
            return;
        }
        match super::apply(candidate, cx) {
            Ok(()) => {
                if !cx.global::<Shell>().applying {
                    self.applied(window, cx);
                }
                self.message = None;
            }
            Err(error) => {
                self.message = Some(format!("Could not apply shortcut: {error:#}"));
            }
        }
        cx.notify();
    }

    /// Clears the proposal. Apply leaves the screen with it, so focus moves to
    /// Change, where Escape and Enter still reach the window.
    pub(super) fn applied(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.candidate = None;
        if self.apply_focus.is_focused(window) {
            window.focus(&self.record_focus, cx);
        }
    }

    fn on_modifiers(&mut self, event: &ModifiersChangedEvent, cx: &mut Context<Self>) {
        if !self.recording {
            return;
        }
        let modifiers = event.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.shift {
            self.modifiers_pressed = true;
        } else if self.modifiers_pressed && self.message.is_none() {
            self.message = Some(
                "The shortcut key wasn't received. It may be reserved or in use; try another chord."
                    .to_owned(),
            );
            cx.notify();
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke.key;
        if self.recording {
            if key == "escape" {
                self.cancel(cx);
            } else {
                match cx.global::<Shell>().platform.record_shortcut(&event.keystroke) {
                    Ok(candidate) => {
                        self.candidate = Some(candidate);
                        self.message = None;
                        self.cancel(cx);
                    }
                    Err(error) => {
                        self.message = Some(error.to_string());
                        cx.notify();
                    }
                }
            }
            cx.stop_propagation();
        } else if key == "tab" {
            // Apply is only on screen while a shortcut is proposed.
            let next = if self.record_focus.is_focused(window) && self.candidate.is_some() {
                &self.apply_focus
            } else {
                &self.record_focus
            };
            window.focus(next, cx);
            cx.stop_propagation();
            cx.notify();
        } else if key == "enter" || key == "space" {
            if self.record_focus.is_focused(window) {
                self.record(window, cx);
            } else if self.apply_focus.is_focused(window) {
                self.apply(window, cx);
            }
            cx.stop_propagation();
        } else if key == "escape" {
            window.remove_window();
        }
    }
}

impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.global::<Shell>();
        let registered = shell.registered;
        let applying = shell.applying;

        let current = row()
            .child(
                text_block("Capture shortcut")
                    .child(description("Starts a new snip from anywhere."))
                    .when(cfg!(target_os = "macos"), |this| {
                        this.child(description(
                            "Shortcuts use physical keys. Key names use U.S. positions.",
                        ))
                    }),
            )
            .when(!registered, |this| this.child(dot(ui::ERROR)))
            .child(key_caps(
                shell.shortcut,
                if registered { "Applied" } else { "Unavailable" },
            ))
            .child(
                focusable(button("record-shortcut", "Change"), &self.record_focus, window)
                    .on_click(cx.listener(|this, _, window, cx| this.record(window, cx))),
            );
        let pending = (self.recording || self.candidate.is_some()).then(|| {
            sub_row()
                .flex_wrap()
                .child(div().flex_1().child(match self.candidate {
                    Some(candidate) => key_caps(candidate, "Proposed"),
                    None => line("Press a shortcut. Escape cancels.").text_color(rgb(ui::MUTED)),
                }))
                // An Apply in flight can't be called back, so Cancel waits for it.
                .when(!applying, |this| {
                    this.child(
                        ghost(button("cancel-shortcut", "Cancel"))
                            // Keep focus on Change: moving it to the window would
                            // end recording and remove this button before the click.
                            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                            .on_click(cx.listener(|this, _, window, cx| {
                                window.focus(&this.record_focus, cx);
                                this.discard(cx);
                            })),
                    )
                })
                .when(self.candidate.is_some(), |this| {
                    this.child(
                        focusable(
                            primary(button("apply-shortcut", "Apply")),
                            &self.apply_focus,
                            window,
                        )
                        .when(applying, |this| this.opacity(0.45))
                        .on_click(cx.listener(|this, _, window, cx| {
                            window.focus(&this.apply_focus, cx);
                            this.apply(window, cx);
                        })),
                    )
                })
        });
        let progress = if applying {
            Some(("Applying shortcut…".to_owned(), ui::MUTED))
        } else {
            shell.status.clone().map(|status| (status, ui::SUCCESS))
        };
        let feedback: Vec<_> = shell
            .error
            .iter()
            .chain(&self.message)
            .map(|error| (error.clone(), ui::ERROR))
            .chain(progress)
            .collect();
        let feedback = (!feedback.is_empty()).then(|| {
            sub_row()
                .flex_col()
                .items_start()
                .gap_1()
                .text_size(SMALL)
                .children(
                    feedback
                        .into_iter()
                        .map(|(text, color)| line(text).text_color(rgb(color))),
                )
        });

        div()
            .id("settings")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1p5()
            .px(px(22.))
            .py_5()
            .bg(rgb(ui::SURFACE))
            .text_color(rgb(ui::TEXT))
            .text_size(px(14.))
            .on_key_down(cx.listener(|this, event, window, cx| this.on_key(event, window, cx)))
            .on_modifiers_changed(cx.listener(|this, event, _, cx| this.on_modifiers(event, cx)))
            .child(heading("Capture"))
            .child(card().child(current).children(pending).children(feedback))
            .child(heading("Cloud snips").mt_3())
            .child(cloud_card(cx))
    }
}

fn cloud_card(cx: &App) -> impl IntoElement {
    let cloud = cx.global::<Cloud>();
    let sign_out = || button("cloud-sign-out", "Sign out").on_click(|_, _, cx| cloud::sign_out(cx));
    let card = match cloud.state() {
        State::SignedOut => card().child(
            status_row(
                ui::MUTED,
                "Not connected",
                "Copy snips between your machines through an R2 bucket in your Cloudflare account.",
            )
            .child(primary(
                button("cloud-sign-in", "Sign in with Cloudflare").on_click(|_, _, cx| cloud::sign_in(cx)),
            )),
        ),
        State::SigningIn(_) => card().child(
            status_row(
                PENDING,
                "Waiting for browser",
                "Finish signing in in your browser.",
            )
            .child(button("cloud-cancel", "Cancel").on_click(|_, _, cx| cloud::cancel_sign_in(cx))),
        ),
        State::ChoosingAccount(_, accounts) => card()
            .child(
                status_row(
                    PENDING,
                    "Choose an account",
                    "Choose the Cloudflare account that holds your cloud snips.",
                )
                .child(sign_out()),
            )
            .children(accounts.iter().map(|account| {
                let chosen = account.clone();
                list_item(format!("cloud-account-{}", account.id), account.name.clone())
                    .on_click(move |_, _, cx| cloud::choose_account(chosen.clone(), cx))
            })),
        State::Connecting(_) => card().child(status_row(
            PENDING,
            "Setting up",
            "Setting up the snip-snip bucket…",
        )),
        State::R2NotEnabled(_, account) => {
            let dashboard = snip_cloud::Config::r2_dashboard(&account.id);
            card()
                .child(status_row(
                    ui::ERROR,
                    "R2 not enabled",
                    format!(
                        "R2 isn't enabled on {}. Enable it in the Cloudflare dashboard, then retry.",
                        account.name
                    ),
                ))
                .child(
                    sub_row()
                        .justify_end()
                        .child(ghost(sign_out()))
                        .child(
                            button("cloud-open-r2", "Open R2 in Cloudflare").on_click(move |_, _, cx| {
                                if let Err(error) = open::that(&dashboard) {
                                    cloud::report(format!("Could not open the browser: {error}"), cx);
                                }
                            }),
                        )
                        .child(primary(button("cloud-retry", "Retry")).on_click(|_, _, cx| cloud::retry(cx))),
                )
        }
        State::Ready(connection) => card().child(
            status_row(
                ui::SUCCESS,
                "Connected",
                format!("Signed in to {}", connection.account.name),
            )
            .child(sign_out().text_color(rgb(ui::ERROR))),
        ),
    };
    card.children(cloud.error().map(|message| {
        sub_row()
            .text_size(SMALL)
            .child(line(message.to_owned()).text_color(rgb(ui::ERROR)))
    }))
}

const RAISED: u32 = 0x272c33;
const SUNKEN: u32 = 0x23272e;
const HAIRLINE: u32 = 0x3a414a;
const KEYCAP: u32 = 0x2e343c;
const PENDING: u32 = 0xe6c76e;
const ON_FOCUS: u32 = 0x0d2236;
const ROW_INSET: Pixels = px(14.);
const SMALL: Pixels = px(12.);

fn heading(text: &'static str) -> Div {
    div()
        .px_0p5()
        .pb_1()
        .text_size(SMALL)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(ui::MUTED))
        .child(text)
}

fn card() -> Div {
    div()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .rounded_lg()
        .border_1()
        .border_color(rgb(HAIRLINE))
        .bg(rgb(RAISED))
        .overflow_hidden()
}

/// A setting: its label and description on the left, its controls on the right.
fn row() -> Div {
    div()
        .flex()
        .items_center()
        .gap_3()
        .px(ROW_INSET)
        .py(px(13.))
        .min_h(px(62.))
}

/// Follow-up content under a row, such as a pending change or an error.
fn sub_row() -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .px(ROW_INSET)
        .py(px(10.))
        .border_t_1()
        .border_color(rgb(HAIRLINE))
        .bg(rgb(SUNKEN))
}

/// The cloud connection's row: a status dot and summary over an explanation.
fn status_row(color: u32, summary: &'static str, detail: impl Into<SharedString>) -> Div {
    row().child(
        text_block(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(dot(color))
                .child("Cloudflare R2")
                .child(description(summary)),
        )
        .child(description(detail)),
    )
}

fn list_item(id: String, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .px(ROW_INSET)
        .py(px(11.))
        .border_t_1()
        .border_color(rgb(HAIRLINE))
        .cursor_pointer()
        .hover(|this| this.bg(rgb(KEYCAP)))
        .child(line(label).flex_1())
        .child(div().text_color(rgb(ui::MUTED)).child("›"))
}

fn text_block(title: impl IntoElement) -> Div {
    div().flex_1().min_w_0().flex().flex_col().gap_0p5().child(title)
}

fn description(text: impl Into<SharedString>) -> Div {
    line(text).text_size(SMALL).text_color(rgb(ui::MUTED))
}

fn dot(color: u32) -> Div {
    div().flex_none().size(px(8.)).rounded_full().bg(rgb(color))
}

/// A chord drawn as key caps. Tests find it as "{state}: {label}"; that text
/// is a selector only, since on screen the state shows as the dot and the
/// error row.
fn key_caps(chord: HotKey, state: &str) -> Div {
    let selector = format!("{state}: {}", shortcut::label(chord));
    div()
        .debug_selector(move || selector)
        .flex()
        .flex_none()
        .gap_1()
        .children(shortcut::keys(chord).into_iter().map(|key| {
            div()
                .min_w(px(24.))
                .flex()
                .justify_center()
                .px(px(7.))
                .py(px(2.))
                .rounded(px(5.))
                .border_1()
                .border_b_2()
                .border_color(rgb(ui::BORDER))
                .bg(rgb(KEYCAP))
                .text_size(SMALL)
                .font_weight(FontWeight::SEMIBOLD)
                .child(key)
        }))
}

fn primary(button: Stateful<Div>) -> Stateful<Div> {
    button
        .bg(rgb(ui::FOCUS))
        .border_color(rgb(ui::FOCUS))
        .text_color(rgb(ON_FOCUS))
        .font_weight(FontWeight::SEMIBOLD)
}

fn ghost(button: Stateful<Div>) -> Stateful<Div> {
    button.border_color(transparent_black())
}

/// A button that Tab and Enter can reach; its border shows focus.
fn focusable(button: Stateful<Div>, focus: &FocusHandle, window: &Window) -> Stateful<Div> {
    button
        .track_focus(focus)
        .when(focus.is_focused(window), |this| this.border_color(rgb(ui::TEXT)))
}

pub fn show_startup_error(message: String, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(520.), px(300.)), cx);
    cx.activate(true);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("snip-snip could not start".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                window.activate_window();
                window.on_window_should_close(cx, |_, cx| {
                    cx.quit();
                    false
                });
                StartupError { message, focus }
            })
        },
    );
    if let Err(error) = opened {
        eprintln!("snip-snip: could not display startup error: {error:#}");
        cx.quit();
    }
}

struct StartupError {
    message: String,
    focus: FocusHandle,
}

impl Render for StartupError {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .size_full()
            .p_5()
            .flex()
            .flex_col()
            .gap_4()
            .bg(rgb(ui::SURFACE))
            .text_color(rgb(ui::TEXT))
            .on_key_down(|event, _, cx| {
                if event.keystroke.key == "escape" || event.keystroke.key == "enter" {
                    cx.quit();
                }
            })
            .child("snip-snip could not start")
            .child(self.message.clone())
            .child(
                div()
                    .id("quit")
                    .cursor_pointer()
                    .child("Quit")
                    .on_click(|_, _, cx| cx.quit()),
            )
    }
}
