//! A single tool strip; secondary controls float over the canvas instead of wrapping.

use gpui::{
    Anchor, App, ClickEvent, Context, Div, FontWeight, IntoElement, MouseButton, PathBuilder, Render, Role,
    Stateful, Window, anchored, canvas, deferred, div, point, prelude::*, px, rgb,
};

use super::{Editor, Gesture, PALETTE, Tool, WIDTHS, to_hsla, view::ViewPreset};

pub(super) const HEIGHT: f32 = 52.;
pub(super) const STATUS_HEIGHT: f32 = 24.;
const SURFACE: u32 = 0x292E33;
const BORDER: u32 = 0x41484E;
const MUTED: u32 = 0xACB5BC;
const SELECTED: u32 = 0x46565F;
pub(super) const HOVER: u32 = 0x3B4249;

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

impl Editor {
    pub(super) fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (color, width) = self.displayed_style(window);
        let menu_bounds = self.menu_bounds.clone();
        div()
            .on_children_prepainted(move |children, _, _| {
                // Direct toolbar children: tools, divider, ink, spacer, zoom,
                // divider, export. Keep trigger bounds for outside dismissal.
                for (menu, child) in [2, 4, 6].into_iter().enumerate() {
                    menu_bounds[menu].set(children[child]);
                }
            })
            .id("editor-toolbar")
            .debug_selector(|| "editor-toolbar".into())
            .h(px(HEIGHT))
            .flex_shrink_0()
            .px_2()
            .flex()
            .items_center()
            .gap(px(2.))
            .bg(rgb(0x202529))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div().flex().gap(px(1.)).flex_shrink_0().children(
                    [
                        ("Select", "V", Tool::Select, Icon::Select),
                        ("Pencil", "P", Tool::Pencil, Icon::Pencil),
                        ("Arrow", "A", Tool::Arrow, Icon::Arrow),
                        ("Rectangle", "R", Tool::Rectangle, Icon::Rectangle),
                        ("Text", "T", Tool::Text, Icon::Text),
                        ("Crop", "C", Tool::Crop, Icon::Crop),
                    ]
                    .map(|(label, key, tool, glyph)| {
                        control(label, true, cx, move |this, window, cx, _| {
                            this.set_tool(tool, window, cx);
                        })
                        .w(px(32.))
                        .justify_center()
                        .hover(|s| s.bg(rgb(HOVER)))
                        .aria_keyshortcuts(key)
                        .aria_selected(self.tool == tool)
                        .tooltip(move |_, cx| tooltip(format!("{label} · {key}"), cx))
                        .when(self.tool == tool, |b| {
                            b.bg(rgb(SELECTED)).border_color(rgb(0x657B86))
                        })
                        .child(icon(glyph, rgb(0xE5EAED)))
                    }),
                ),
            )
            .child(separator())
            .child(
                self.popup_trigger("Ink and stroke", Popup::Ink, 64., cx)
                    .child(
                        div()
                            .size(px(14.))
                            .rounded_full()
                            .bg(to_hsla(color))
                            .border_1()
                            .border_color(rgb(0x7B858C)),
                    )
                    .child(
                        div()
                            .w(px(14.))
                            .h(px(width.unwrap_or(self.ink_width)))
                            .rounded_full()
                            .bg(rgb(0xE5EAED)),
                    )
                    .child(icon(Icon::Chevron, rgb(MUTED)))
                    .when(self.popup == Some(Popup::Ink), |b| {
                        b.child(self.popover(Popup::Ink, window, cx))
                    }),
            )
            .child(div().flex_1().min_w_0())
            .child(
                self.popup_trigger("Zoom", Popup::Zoom, 60., cx)
                    .child(
                        div()
                            .debug_selector(|| "zoom-level".into())
                            .text_xs()
                            .child(self.viewport.zoom_label()),
                    )
                    .child(icon(Icon::Chevron, rgb(MUTED)))
                    .when(self.popup == Some(Popup::Zoom), |b| {
                        b.child(self.popover(Popup::Zoom, window, cx))
                    }),
            )
            .child(separator())
            .child(
                self.popup_trigger("Export", Popup::Export, 86., cx)
                    .bg(rgb(0xB4D6DA))
                    .hover(|s| s.bg(rgb(0xC4E4E7)))
                    .text_color(rgb(0x172A30))
                    .child(icon(Icon::Copy, rgb(0x172A30)))
                    .child("Export")
                    .child(icon(Icon::Chevron, rgb(0x172A30)))
                    .when(self.popup == Some(Popup::Export), |b| {
                        b.child(self.popover(Popup::Export, window, cx))
                    }),
            )
    }

    pub(super) fn status_line(&self, window: &Window) -> impl IntoElement {
        let (color, width) = self.displayed_style(window);
        let name = PALETTE
            .iter()
            .find(|(_, c)| *c == color)
            .map_or("Ink", |(name, _)| *name);
        let width_name = WIDTHS
            .iter()
            .find(|(_, w)| width.is_some_and(|v| (v - *w).abs() < 0.001))
            .map(|(n, _)| *n);
        let style = match self.tool {
            Tool::Crop => "Crop".to_owned(),
            _ => format!(
                "{name}{}",
                width_name.map_or(String::new(), |n| format!(" · {n}"))
            ),
        };
        div()
            .h(px(STATUS_HEIGHT))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .gap_3()
            .text_color(rgb(MUTED))
            .text_size(px(11.))
            .child(style)
            .child(
                div()
                    .debug_selector({
                        let status = self.status.clone();
                        move || status.to_string()
                    })
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_right()
                    .child(self.status.clone()),
            )
    }

    fn popup_trigger(
        &self,
        label: &'static str,
        popup: Popup,
        width: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        control(label, true, cx, move |this, window, cx, keyboard| {
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
        .track_focus(&self.menu_focus[popup.index()])
        .relative()
        .w(px(width))
        .justify_center()
        .when(popup != Popup::Export, |b| b.hover(|s| s.bg(rgb(HOVER))))
        .aria_expanded(self.popup == Some(popup))
        .tooltip(move |_, cx| tooltip(label.to_owned(), cx))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, window, cx| {
                this.cancel_gesture(window);
                this.update_cursor(window);
                cx.notify();
            }),
        )
        .when(self.popup == Some(popup), |b| b.bg(rgb(SELECTED)))
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
        div()
            .absolute()
            .top(px(48.))
            .w_0()
            .h_0()
            .when(popup == Popup::Ink, |d| d.left_0())
            .when(popup != Popup::Ink, |d| d.right_0())
            .child(
                deferred(
                    anchored()
                        .anchor(if popup == Popup::Ink {
                            Anchor::TopLeft
                        } else {
                            Anchor::TopRight
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
                                .text_color(rgb(0xE5EAED))
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
                    this.prepare_style_change(window, cx);
                    this.set_color(color, window, cx);
                })
                .size(px(34.))
                .rounded_full()
                .justify_center()
                .bg(to_hsla(color))
                .hover(move |s| s.bg(to_hsla(color)).opacity(0.85))
                .aria_selected(active == color)
                .border_color(rgb(if active == color { 0xE5EAED } else { 0x69747C }))
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
                    this.prepare_style_change(window, cx);
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
            .when(self.tool == Tool::Crop, |d| d.child(self.clear_crop_button(cx)))
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
                    menu_item(label, shortcut(key), cx, move |this, window, cx, _| {
                        if matches!(this.gesture, Some(Gesture::Crop(_))) {
                            this.cancel_gesture(window);
                        }
                        this.popup = None;
                        window.focus(&this.focus, cx);
                        this.set_view(preset, window, cx);
                    })
                }),
            )
            .child(
                div()
                    .mt_2()
                    .text_color(rgb(MUTED))
                    .child("Export resolution stays the same."),
            )
    }

    fn export_popover(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(heading("Finish your snip"))
            .child(menu_item("Copy", shortcut("C"), cx, |this, window, cx, _| {
                window.focus(&this.focus, cx);
                this.copy(false, window, cx);
            }))
            .child(menu_item(
                "Copy & Close",
                shortcut("Shift+C"),
                cx,
                |this, window, cx, _| {
                    window.focus(&this.focus, cx);
                    this.copy(true, window, cx);
                },
            ))
            .child(div().my_1().h(px(1.)).bg(rgb(BORDER)))
            .child(menu_item("Save…", shortcut("S"), cx, |this, window, cx, _| {
                window.focus(&this.focus, cx);
                this.save(window, cx);
            }))
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
        .focus_visible(|s| s.border_color(rgb(0x8CD4DD)))
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
    key: String,
    cx: &mut Context<Editor>,
    action: impl Fn(&mut Editor, &mut Window, &mut Context<Editor>, bool) + 'static,
) -> Stateful<Div> {
    control(label, true, cx, action)
        .hover(|s| s.bg(rgb(HOVER)))
        .w_full()
        .px_2()
        .justify_between()
        .aria_keyshortcuts(key.clone())
        .child(label)
        .child(div().text_color(rgb(MUTED)).text_size(px(10.)).child(key))
}

fn heading(label: &'static str) -> impl IntoElement {
    div().mb_1().font_weight(FontWeight::SEMIBOLD).child(label)
}

fn separator() -> impl IntoElement {
    div().mx_1().w(px(1.)).h(px(22.)).flex_shrink_0().bg(rgb(BORDER))
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
            .text_color(rgb(0xE5EAED))
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
    Chevron,
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
        Icon::Chevron => &[&[(5., 9.), (11., 15.), (17., 9.)]],
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
    .size(px(if matches!(glyph, Icon::Chevron) { 10. } else { 18. }))
}
