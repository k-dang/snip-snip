//! The dock: one row of tools floating over the bottom of the canvas. Secondary
//! controls open in popovers above it instead of wrapping.

use gpui::{
    Anchor, App, BoxShadow, ClickEvent, Context, Div, FontWeight, IntoElement, MouseButton, PathBuilder,
    Render, Role, Stateful, Window, anchored, canvas, deferred, div, hsla, linear_color_stop,
    linear_gradient, point, prelude::*, px, rgb, rgba,
};

use super::{
    Editor, PALETTE, Tool, WIDTHS,
    theme::{BORDER, EDGE, HOVER, ICON, MUTED, SELECTED, SURFACE, TEXT, accent},
    to_hsla,
    view::ViewPreset,
};

const BUTTON: f32 = 32.;
const DOCK_PADDING: f32 = 4.;
/// Buttons, padding, and the 1px border.
const DOCK_HEIGHT: f32 = BUTTON + 2. * DOCK_PADDING + 2.;
pub(super) const DOCK_MARGIN: f32 = 18.;
/// Space between the dock's edge and what floats above it: toasts and popovers.
pub(super) const FLOAT_GAP: f32 = 10.;
/// Canvas inset that keeps the fitted image clear of the dock.
pub(super) const BOTTOM_INSET: f32 = DOCK_MARGIN + DOCK_HEIGHT + 16.;
const RADIUS: f32 = 10.;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Popup {
    Ink,
    Zoom,
    Export,
}

impl Popup {
    fn index(self) -> usize {
        self as usize
    }
}

/// How a popover trigger looks: quiet like the tools, or filled like a primary action.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Trigger {
    Neutral,
    Primary,
}

impl Editor {
    pub(super) fn dock(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (color, width) = self.displayed_style(window);
        div()
            .id("editor-dock")
            .debug_selector(|| "editor-dock".into())
            // Clicks between controls must not reach the canvas behind the dock.
            .occlude()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(DOCK_PADDING))
            .rounded(px(14.))
            .bg(linear_gradient(
                180.,
                linear_color_stop(rgba(0x26262EF2), 0.),
                linear_color_stop(rgba(0x18181DF2), 1.),
            ))
            .border_1()
            .border_color(rgba(EDGE))
            .shadow(vec![
                BoxShadow::new(px(0.), px(10.), hsla(0., 0., 0., 0.35)).blur_radius(px(24.)),
                BoxShadow::new(px(0.), px(1.), hsla(0., 0., 1., 0.06)).inset(),
            ])
            .children(
                [
                    ("Select", "V", Tool::Select, Icon::Select),
                    ("Pencil", "P", Tool::Pencil, Icon::Pencil),
                    ("Arrow", "A", Tool::Arrow, Icon::Arrow),
                    ("Rectangle", "R", Tool::Rectangle, Icon::Rectangle),
                    ("Text", "T", Tool::Text, Icon::Text),
                    ("Crop", "C", Tool::Crop, Icon::Crop),
                ]
                .map(|(label, key, tool, glyph)| {
                    let selected = self.model.tool() == tool;
                    control(label, true, cx, move |this, window, cx, _| {
                        this.set_tool(tool, window, cx);
                    })
                    .size(px(BUTTON))
                    .justify_center()
                    .rounded(px(RADIUS))
                    .aria_keyshortcuts(key)
                    .aria_selected(selected)
                    .tooltip(move |_, cx| tooltip(format!("{label} · {key}"), cx))
                    .when(!selected, |b| b.hover(|s| s.bg(rgb(HOVER))))
                    .when(selected, |b| b.bg(accent()))
                    .child(icon(glyph, rgb(if selected { 0xFFFFFF } else { ICON })))
                }),
            )
            .child(separator())
            .child(
                self.popup_trigger(
                    "Ink and stroke",
                    Popup::Ink,
                    50.,
                    Trigger::Neutral,
                    true,
                    window,
                    cx,
                )
                .gap(px(6.))
                .child(
                    div()
                        .size(px(14.))
                        .rounded_full()
                        .bg(to_hsla(color))
                        .border_2()
                        .border_color(rgba(0xFFFFFF30)),
                )
                .child(
                    div()
                        .w(px(14.))
                        .h(px(width.unwrap_or(self.model.ink_width())))
                        .rounded_full()
                        .bg(rgb(TEXT)),
                ),
            )
            .child(separator())
            .child(
                self.popup_trigger("Zoom", Popup::Zoom, 52., Trigger::Neutral, true, window, cx)
                    .text_color(rgb(TEXT))
                    .child(
                        div()
                            .debug_selector(|| "zoom-level".into())
                            .text_xs()
                            .child(self.viewport.zoom_label()),
                    ),
            )
            .child(
                self.popup_trigger(
                    "Export",
                    Popup::Export,
                    88.,
                    Trigger::Primary,
                    !self.uploading,
                    window,
                    cx,
                )
                .ml(px(2.))
                .gap(px(6.))
                .when(!self.uploading, |b| {
                    b.child(icon(Icon::Copy, rgb(0xFFFFFF))).child("Export")
                })
                .when(self.uploading, |b| b.child("Uploading…")),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn popup_trigger(
        &self,
        label: &'static str,
        popup: Popup,
        width: f32,
        style: Trigger,
        enabled: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let open = self.popup == Some(popup);
        control(label, enabled, cx, move |this, window, cx, keyboard| {
            if this.popup == Some(popup) {
                this.dismiss_popup(window, cx);
                return;
            }
            this.popup = Some(popup);
            this.popup_return_focus = if keyboard {
                this.menu_focus[popup.index()].clone()
            } else {
                this.focus.clone()
            };
            if keyboard {
                window.focus(&this.popup_focus, cx);
            } else {
                window.focus(&this.focus, cx);
            }
            cx.notify();
        })
        .when(enabled, |b| {
            b.track_focus(&self.menu_focus[popup.index()]).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.cancel_gesture(window);
                    cx.notify();
                }),
            )
        })
        .relative()
        .w(px(width))
        .h(px(BUTTON))
        .rounded(px(RADIUS))
        .justify_center()
        .child({
            // Records the trigger's bounds, border included, so a click on it
            // doesn't count as outside its popover.
            let bounds = self.menu_bounds[popup.index()].clone();
            canvas(move |b, _, _| bounds.set(b), |_, (), _, _| {})
                .absolute()
                .top(px(-1.))
                .bottom(px(-1.))
                .left(px(-1.))
                .right(px(-1.))
        })
        .aria_expanded(open)
        .tooltip(move |_, cx| tooltip(label.to_owned(), cx))
        .map(|b| match style {
            Trigger::Neutral => b
                .when(enabled, |b| b.hover(|s| s.bg(rgb(HOVER))))
                .when(open, |b| b.bg(rgb(SELECTED))),
            Trigger::Primary => b
                .bg(accent())
                .when(enabled, |b| b.hover(|s| s.opacity(0.92)))
                .text_color(rgb(0xFFFFFF))
                .font_weight(FontWeight::SEMIBOLD),
        })
        .when(open, |b| b.child(self.popover(popup, window, cx)))
    }

    pub(super) fn dismiss_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.popup = None;
        window.focus(&self.popup_return_focus, cx);
        cx.notify();
    }

    fn popover(&self, popup: Popup, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match popup {
            Popup::Ink => self.ink_popover(window, cx).into_any_element(),
            Popup::Zoom => self.zoom_popover(cx).into_any_element(),
            Popup::Export => self.export_popover(cx).into_any_element(),
        };
        // Popovers open upward from the dock, aligned to their trigger's outer
        // edge. The trigger sits inside the dock's padding and border.
        div()
            .absolute()
            .top(px(-(FLOAT_GAP + DOCK_PADDING + 1.)))
            .w_0()
            .h_0()
            .when(popup == Popup::Ink, |d| d.left_0())
            .when(popup != Popup::Ink, |d| d.right_0())
            .child(
                deferred(
                    anchored()
                        .anchor(if popup == Popup::Ink {
                            Anchor::BottomLeft
                        } else {
                            Anchor::BottomRight
                        })
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            div()
                                .id("toolbar-popover")
                                .debug_selector(|| "toolbar-popover".into())
                                .track_focus(&self.popup_focus)
                                .w(px(240.))
                                .p_3()
                                .rounded_lg()
                                .bg(rgb(SURFACE))
                                .text_color(rgb(TEXT))
                                .text_xs()
                                .border_1()
                                .border_color(rgb(BORDER))
                                .shadow_lg()
                                .occlude()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_mouse_down_out(cx.listener(
                                    move |this, event: &gpui::MouseDownEvent, _, cx| {
                                        if this.popup == Some(popup)
                                            && !this.menu_bounds[popup.index()]
                                                .get()
                                                .contains(&event.position)
                                        {
                                            this.popup = None;
                                            cx.notify();
                                        }
                                    },
                                ))
                                .child(body),
                        ),
                )
                .priority(1),
            )
    }

    fn ink_popover(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (active, width) = self.displayed_style(window);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(heading("Ink"))
            .child(div().flex().gap_2().children(PALETTE.map(|(label, color)| {
                control(label, true, cx, move |this, window, cx, _| {
                    this.set_color(color, window, cx);
                })
                .size(px(34.))
                .rounded_full()
                .justify_center()
                .bg(to_hsla(color))
                .hover(move |s| s.bg(to_hsla(color)).opacity(0.85))
                .aria_selected(active == color)
                .border_color(rgb(if active == color { TEXT } else { 0x69747C }))
                .tooltip(move |_, cx| tooltip(label.to_owned(), cx))
                .when(active == color, |b| {
                    b.child(icon(
                        Icon::Check,
                        rgb(if label == "Yellow" { 0x172A30 } else { 0xFFFFFF }),
                    ))
                })
            })))
            .child(heading("Stroke"))
            .child(div().flex().gap_1().children(WIDTHS.map(|(label, value)| {
                control(label, width.is_some(), cx, move |this, window, cx, _| {
                    this.set_width(value, window, cx);
                })
                .flex_1()
                .h(px(44.))
                .flex_col()
                .justify_center()
                .border_color(rgb(BORDER))
                .when(width.is_some(), |b| b.hover(|s| s.bg(rgb(HOVER))))
                .aria_selected(width.is_some_and(|w| (w - value).abs() < 0.001))
                .when(width.is_some_and(|w| (w - value).abs() < 0.001), |b| {
                    b.bg(rgb(SELECTED)).border_color(rgb(0x8599A3))
                })
                .child(div().w(px(20.)).h(px(value)).rounded_full().bg(rgb(0xE5EAED)))
                .child(label)
            })))
            .child(div().text_color(rgb(MUTED)).child(if width.is_none() {
                "Text uses ink color; stroke is unavailable."
            } else {
                "Shared across drawing tools."
            }))
            .when(self.model.tool() == Tool::Crop, |d| {
                d.child(self.clear_crop_button(cx))
            })
    }

    fn zoom_popover(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(heading("View"))
            .children(
                [
                    ("Fit", ViewPreset::Fit, "0"),
                    ("100%", ViewPreset::ActualSize, "1"),
                ]
                .map(|(label, preset, key)| {
                    menu_item(label, Some(shortcut(key)), cx, move |this, window, cx, _| {
                        if this.model.is_cropping() {
                            this.cancel_gesture(window);
                        }
                        this.popup = None;
                        window.focus(&this.focus, cx);
                        this.set_view(preset, window, cx);
                    })
                }),
            )
            .child(div().mt_2().text_color(rgb(MUTED)).child({
                let (w, h) = self.model.document().export_size();
                format!("Exports at {w} × {h} px.")
            }))
    }

    fn export_popover(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(heading("Finish your snip"))
            .child(menu_item("Copy", None, cx, |this, window, cx, _| {
                window.focus(&this.focus, cx);
                this.copy(false, window, cx);
            }))
            .child(menu_item(
                "Copy & Close",
                Some(shortcut("C")),
                cx,
                |this, window, cx, _| {
                    window.focus(&this.focus, cx);
                    this.copy(true, window, cx);
                },
            ))
            .when(crate::cloud::store(cx).is_some(), |menu| {
                menu.child(menu_item(
                    "Copy to cloud",
                    Some(shortcut("Shift+D")),
                    cx,
                    |this, window, cx, _| {
                        window.focus(&this.focus, cx);
                        this.copy_to_cloud(window, cx);
                    },
                ))
            })
            .child(div().my_1().h(px(1.)).bg(rgb(BORDER)))
            .child(menu_item(
                "Save…",
                Some(shortcut("S")),
                cx,
                |this, window, cx, _| {
                    window.focus(&this.focus, cx);
                    this.save(window, cx);
                },
            ))
    }
}

/// Disabled controls consume input without interrupting text editing.
pub(super) fn control(
    label: &'static str,
    enabled: bool,
    cx: &mut Context<Editor>,
    action: impl Fn(&mut Editor, &mut Window, &mut Context<Editor>, bool) + 'static,
) -> Stateful<Div> {
    div()
        .id(label)
        .debug_selector(|| label.into())
        .focusable()
        .tab_stop(enabled)
        .role(Role::Button)
        .aria_label(label)
        .h(px(36.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_1()
        .text_xs()
        .rounded_md()
        .border_1()
        .border_color(gpui::transparent_black())
        .focus_visible(|s| s.border_color(rgb(0xB3A4FF)))
        .when(!enabled, |b| b.opacity(0.4))
        .when(enabled, |b| b.cursor_pointer())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            cx.stop_propagation();
            if enabled {
                action(this, window, cx, matches!(event, ClickEvent::Keyboard(_)));
            }
        }))
        .on_key_down(|event, _, cx| {
            if (event.keystroke.key == "enter" || event.keystroke.key == "space")
                && event.keystroke.modifiers == gpui::Modifiers::none()
            {
                // GPUI activates on key-up; keep these keys out of text and pan input.
                cx.stop_propagation();
            }
        })
}

fn menu_item(
    label: &'static str,
    key: Option<String>,
    cx: &mut Context<Editor>,
    action: impl Fn(&mut Editor, &mut Window, &mut Context<Editor>, bool) + 'static,
) -> Stateful<Div> {
    let item = control(label, true, cx, action)
        .hover(|s| s.bg(rgb(HOVER)))
        .w_full()
        .px_2()
        .justify_between()
        .child(label);
    match key {
        Some(key) => item
            .aria_keyshortcuts(key.clone())
            .child(div().text_color(rgb(MUTED)).text_size(px(10.)).child(key)),
        None => item,
    }
}

fn heading(label: &'static str) -> impl IntoElement {
    div().mb_1().font_weight(FontWeight::SEMIBOLD).child(label)
}

fn separator() -> impl IntoElement {
    div().mx_1().w(px(1.)).h(px(18.)).flex_shrink_0().bg(rgb(BORDER))
}

fn shortcut(key: &str) -> String {
    format!("{}+{key}", if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" })
}

struct Tooltip(String);

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(SURFACE))
            .text_color(rgb(TEXT))
            .text_xs()
            .border_1()
            .border_color(rgb(BORDER))
            .shadow_md()
            .child(self.0.clone())
    }
}

fn tooltip(label: String, cx: &mut App) -> gpui::AnyView {
    cx.new(|_| Tooltip(label)).into()
}

#[derive(Clone, Copy)]
enum Icon {
    Select,
    Pencil,
    Arrow,
    Rectangle,
    Text,
    Crop,
    Copy,
    Check,
}

fn icon(glyph: Icon, color: impl Into<gpui::Hsla>) -> impl IntoElement {
    let color = color.into();
    let paths: &[&[(f32, f32)]] = match glyph {
        Icon::Select => &[&[(5., 3.), (17., 12.), (11., 13.), (8., 19.), (5., 3.)]],
        Icon::Pencil => &[
            &[
                (4., 16.),
                (5., 11.),
                (15., 2.),
                (19., 6.),
                (9., 16.),
                (4., 17.),
                (4., 16.),
            ],
            &[(12., 5.), (16., 9.)],
        ],
        Icon::Arrow => &[&[(4., 17.), (17., 4.)], &[(7., 4.), (17., 4.), (17., 14.)]],
        Icon::Rectangle => &[&[(3., 4.), (18., 4.), (18., 17.), (3., 17.), (3., 4.)]],
        Icon::Text => &[
            &[(4., 4.), (18., 4.)],
            &[(11., 4.), (11., 18.)],
            &[(7., 18.), (15., 18.)],
        ],
        Icon::Crop => &[
            &[(6., 2.), (6., 16.), (20., 16.)],
            &[(2., 6.), (16., 6.), (16., 20.)],
        ],
        Icon::Copy => &[
            &[(7., 7.), (18., 7.), (18., 19.), (7., 19.), (7., 7.)],
            &[(14., 7.), (14., 3.), (3., 3.), (3., 15.), (7., 15.)],
        ],
        Icon::Check => &[&[(4., 11.), (9., 16.), (18., 5.)]],
    };
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let scale = f32::from(bounds.size.width) / 22.;
            for line in paths {
                let mut path = PathBuilder::stroke(px(1.65 * scale));
                for (index, &(x, y)) in line.iter().enumerate() {
                    let p = bounds.origin + point(px(x * scale), px(y * scale));
                    if index == 0 {
                        path.move_to(p);
                    } else {
                        path.line_to(p);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .size(px(18.))
}
