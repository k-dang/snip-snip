//! Mouse drags that keep tracking after the cursor leaves the element.

use gpui::{Context, DispatchPhase, Entity, MouseButton, MouseMoveEvent, MouseUpEvent, Window};

/// Sends every mouse move and left-button release in `window` to `view`,
/// wherever the cursor is. Call from a paint callback each frame.
///
/// While a button is held the OS keeps delivering the mouse to the window it
/// was pressed in, even over another monitor, but element listeners such as
/// `on_mouse_up` only fire while the cursor is over the element. Without this,
/// a drag released outside never ends. The handlers must ignore events when
/// no drag is in progress.
pub fn track<V: 'static>(
    window: &mut Window,
    view: Entity<V>,
    on_move: fn(&mut V, &MouseMoveEvent, &mut Window, &mut Context<V>),
    on_up: fn(&mut V, &mut Window, &mut Context<V>),
) {
    window.on_mouse_event({
        let view = view.clone();
        move |event: &MouseMoveEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble {
                view.update(cx, |view, cx| on_move(view, event, window, cx));
            }
        }
    });
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
            view.update(cx, |view, cx| on_up(view, window, cx));
        }
    });
}
