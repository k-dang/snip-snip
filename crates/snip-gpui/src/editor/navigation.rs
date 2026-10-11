//! View controls and platform input for navigating the editor canvas.

use gpui::{
    Context, KeyUpEvent, MouseButton, PinchEvent, ScrollDelta, ScrollWheelEvent, Window, div, prelude::*, rgb,
};

use super::{Editor, Gesture, button, view::ViewPreset};

impl Editor {
    pub(super) fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.modifiers.secondary() || self.gesture.is_some() {
            return;
        }
        let steps = match event.delta {
            ScrollDelta::Lines(delta) => delta.y / wheel_lines_per_notch(),
            ScrollDelta::Pixels(delta) => f32::from(delta.y) / 40.,
        };
        self.zoom(event.position, 1.1_f32.powf(steps), window, cx);
    }

    pub(super) fn on_pinch(&mut self, event: &PinchEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.gesture.is_none() {
            self.zoom(event.position, 1. + event.delta, window, cx);
        }
    }

    fn zoom(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        factor: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.viewport.zoom(position, factor) {
            self.update_cursor(window);
            cx.stop_propagation();
            cx.notify();
        }
    }

    pub(super) fn set_view(&mut self, preset: ViewPreset, window: &mut Window, cx: &mut Context<Self>) {
        if self.gesture.is_none() {
            self.viewport.reset(preset);
            self.update_cursor(window);
            cx.notify();
        }
        cx.stop_propagation();
    }

    pub(super) fn on_key_up(&mut self, event: &KeyUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "space" {
            self.release_space(window, cx);
        }
    }

    /// Key release and focus loss both end temporary pan at its current position.
    pub(super) fn release_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.space_held = false;
        if matches!(self.gesture, Some(Gesture::Pan(_))) {
            self.gesture = None;
        }
        self.update_cursor(window);
        cx.notify();
    }

    pub(super) fn view_button(
        &self,
        label: &'static str,
        preset: ViewPreset,
        key: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        button(label)
            .child(div().text_color(rgb(0x7A7A80)).text_xs().child(format!(
                "{}+{key}",
                if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" }
            )))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.set_view(preset, window, cx)))
    }
}

/// Recover notch counts from GPUI's Windows line deltas, including custom OS settings.
#[allow(unsafe_code)]
fn wheel_lines_per_notch() -> f32 {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{SPI_GETWHEELSCROLLLINES, SystemParametersInfoW};
        let mut lines = 3u32;
        // SAFETY: SPI_GETWHEELSCROLLLINES only writes a u32 into this live, correctly sized buffer.
        // The call reads system settings; it changes no state. Keep this FFI exception local.
        let read = unsafe {
            SystemParametersInfoW(
                SPI_GETWHEELSCROLLLINES,
                0,
                Some((&mut lines as *mut u32).cast()),
                Default::default(),
            )
        };
        if read.is_err() {
            lines = 3;
        }
        lines.max(1) as f32
    }
    #[cfg(not(windows))]
    {
        1.
    }
}
