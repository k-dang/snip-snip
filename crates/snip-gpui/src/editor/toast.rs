//! Export outcomes appear as a small toast in the bottom-right corner, above the dock.
//! Successes fade on their own; errors stay until dismissed or replaced.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, Context, IntoElement, SharedString, Task, div, prelude::*, px, rgb, rgba,
};

use super::{
    Editor,
    theme::{EDGE, ERROR, HOVER, MUTED, SUCCESS, SURFACE, TEXT},
    toolbar::control,
};
use crate::ui;

const SUCCESS_LIFETIME: Duration = Duration::from_secs(2);

pub(super) struct Toast {
    message: SharedString,
    error: bool,
    /// Distinguishes toasts so each one replays the fade-in.
    serial: usize,
    /// Clears a success toast; dropping it (a newer toast) cancels the timer.
    _dismiss: Option<Task<()>>,
}

impl Editor {
    pub(super) fn toast_success(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.show_toast(message.into(), false, cx);
    }

    pub(super) fn toast_error(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.show_toast(message.into(), true, cx);
    }

    fn show_toast(&mut self, message: SharedString, error: bool, cx: &mut Context<Self>) {
        let dismiss = (!error).then(|| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SUCCESS_LIFETIME).await;
                this.update(cx, |this, cx| {
                    this.toast = None;
                    cx.notify();
                })
                .ok();
            })
        });
        let serial = self.toast.as_ref().map_or(0, |toast| toast.serial + 1);
        self.toast = Some(Toast {
            message,
            error,
            serial,
            _dismiss: dismiss,
        });
        cx.notify();
    }

    pub(super) fn toast_pill(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let toast = self.toast.as_ref()?;
        Some(
            div()
                .id("editor-toast")
                .debug_selector(|| "editor-toast".into())
                // Clicks on the toast must not reach the canvas behind it.
                .occlude()
                .max_w(px(420.))
                .min_w_0()
                .h(px(30.))
                .pl(px(10.))
                .pr(px(if toast.error { 4. } else { 12. }))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded_full()
                .bg(rgb(SURFACE))
                .border_1()
                .border_color(if toast.error {
                    rgba((ERROR << 8) | 0x55)
                } else {
                    rgba(EDGE)
                })
                .shadow_md()
                .text_size(px(12.))
                .text_color(rgb(TEXT))
                .child(
                    div()
                        .size(px(6.))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(rgb(if toast.error { ERROR } else { SUCCESS })),
                )
                .child(ui::line(toast.message.clone()).min_w_0().truncate())
                .when(toast.error, |pill| {
                    pill.child(
                        control("Dismiss", true, cx, |this, window, cx, _| {
                            this.toast = None;
                            // The focused button disappears; keep shortcuts working.
                            window.focus(&this.focus, cx);
                            cx.notify();
                        })
                        .size(px(22.))
                        .justify_center()
                        .rounded_full()
                        .text_color(rgb(MUTED))
                        .hover(|s| s.bg(rgb(HOVER)).text_color(rgb(TEXT)))
                        .child("×"),
                    )
                })
                .with_animation(
                    ("editor-toast", toast.serial),
                    Animation::new(Duration::from_millis(160)),
                    |pill, delta| pill.opacity(delta).mt(px(6. * (1. - delta))),
                ),
        )
    }
}
