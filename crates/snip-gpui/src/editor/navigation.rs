//! View controls and platform input for navigating the editor canvas.

use gpui::{Context, KeyUpEvent, PinchEvent, ScrollDelta, ScrollWheelEvent, Window};

use super::{Editor, view::ViewPreset};

impl Editor {
    pub(super) fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.modifiers.secondary() || self.has_gesture() {
            return;
        }
        let steps = match event.delta {
            ScrollDelta::Lines(delta) => delta.y / wheel_lines_per_notch(),
            ScrollDelta::Pixels(delta) => f32::from(delta.y) / 40.,
        };
        self.zoom(event.position, 1.1_f32.powf(steps), window, cx);
    }

    pub(super) fn on_pinch(&mut self, event: &PinchEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_gesture() {
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
        if !self.has_gesture() {
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
        self.pan = None;
        self.update_cursor(window);
        cx.notify();
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
