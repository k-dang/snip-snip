use global_hotkey::hotkey::HotKey;
use gpui::{
    App, Bounds, Context, FocusHandle, KeyDownEvent, ModifiersChangedEvent, Render, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};

use super::{Shell, shortcut};

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
    let bounds = Bounds::centered(None, size(px(480.), px(320.)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("snip-snip Settings".into()),
                ..Default::default()
            }),
            window_min_size: Some(size(px(440.), px(300.))),
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
        if let Err(error) = cx.global::<Shell>().platform.check_keyboard_layout(cx) {
            self.message = Some(format!("{error:#}"));
            cx.notify();
            return;
        }
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
        if !self.recording {
            return;
        }
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

    fn apply(&mut self, cx: &mut Context<Self>) {
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
                    self.candidate = None;
                }
                self.message = None;
            }
            Err(error) => {
                self.message = Some(format!("Could not apply shortcut: {error:#}"));
            }
        }
        cx.notify();
    }

    pub(super) fn applied(&mut self) {
        self.candidate = None;
    }

    #[cfg(any(target_os = "macos", test))]
    pub(super) fn layout_changed(&mut self, cx: &mut Context<Self>) {
        self.cancel(cx);
        self.candidate = None;
        cx.notify();
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
                match shortcut::record(&event.keystroke) {
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
            let next = if self.record_focus.is_focused(window) {
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
                self.apply(cx);
            }
            cx.stop_propagation();
        } else if key == "escape" {
            window.remove_window();
        }
    }
}

fn line(text: String) -> impl IntoElement {
    div()
        .debug_selector({
            let text = text.clone();
            move || text
        })
        .child(text)
}

impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let applied = shortcut::label(cx.global::<Shell>().shortcut);
        let registered = cx.global::<Shell>().registered;
        let error = cx.global::<Shell>().error.clone();
        let status = if cx.global::<Shell>().applying {
            Some("Applying shortcut…".to_owned())
        } else {
            cx.global::<Shell>().status.clone()
        };
        let can_apply = self.candidate.is_some() && !self.recording && !cx.global::<Shell>().applying;
        let proposed = self.candidate.map(shortcut::label);
        let record_focus = self.record_focus.clone();
        let apply_focus = self.apply_focus.clone();
        div()
            .id("settings")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .bg(rgb(0x20242a))
            .text_color(rgb(0xf1f3f5))
            .text_size(px(15.))
            .on_key_down(cx.listener(|this, event, window, cx| this.on_key(event, window, cx)))
            .on_modifiers_changed(cx.listener(|this, event, _, cx| this.on_modifiers(event, cx)))
            .child(div().text_size(px(20.)).child("Capture shortcut"))
            .child(line(format!(
                "{}: {applied}",
                if registered { "Applied" } else { "Unavailable" }
            )))
            .child(line(if self.recording {
                "Press a shortcut; Escape cancels".to_owned()
            } else {
                proposed
                    .map(|chord| format!("Proposed: {chord}"))
                    .unwrap_or_else(|| "Record a shortcut, then Apply.".to_owned())
            }))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .id("record-shortcut")
                            .debug_selector(|| "Record shortcut".into())
                            .track_focus(&record_focus)
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .border_1()
                            .border_color(if record_focus.is_focused(window) {
                                rgb(0x71b7ff)
                            } else {
                                rgb(0x58616e)
                            })
                            .cursor_pointer()
                            .child("Record shortcut")
                            .on_click(cx.listener(|this, _, window, cx| this.record(window, cx))),
                    )
                    .child(
                        div()
                            .id("apply-shortcut")
                            .debug_selector(|| "Apply".into())
                            .track_focus(&apply_focus)
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .border_1()
                            .opacity(if can_apply { 1. } else { 0.45 })
                            .border_color(if apply_focus.is_focused(window) {
                                rgb(0x71b7ff)
                            } else {
                                rgb(0x58616e)
                            })
                            .cursor_pointer()
                            .child("Apply")
                            .on_click(cx.listener(|this, _, window, cx| {
                                window.focus(&this.apply_focus, cx);
                                this.apply(cx);
                            })),
                    ),
            )
            .children(error.into_iter().chain(self.message.clone()).map(|message| {
                div()
                    .debug_selector({
                        let message = message.clone();
                        move || message
                    })
                    .text_color(rgb(0xffb8a8))
                    .child(SharedString::from(message))
            }))
            .children(status.map(|status| div().text_color(rgb(0xa8d9c0)).child(line(status))))
    }
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
            .bg(rgb(0x20242a))
            .text_color(rgb(0xf1f3f5))
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
