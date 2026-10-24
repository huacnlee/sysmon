//! Desktop navigation and persistent telemetry share the app-owned title bar.
use nsh_core::{
    desktop::TitleBarStyle,
    ui::{desktop, prelude::*},
};

pub fn install() {
    set_color_scheme(Appearance::Dark);
    effect(|| {
        if is_desktop() {
            desktop::set_title_bar(TitleBarStyle::Hidden, Some(px(40.)));
        }
    });
}

/// The primary destinations remain visible as icons with their live readings.
/// Less frequently used categories live in the framework's dismissible menu.
pub fn bar(
    tab: Signal<usize>,
    _paused: Signal<bool>,
    data: Signal<crate::Sample>,
) -> impl IntoElement {
    view()
        .test_id("sysmon-titlebar")
        .window_drag(true)
        .flex_shrink_0()
        .h(px(40.))
        .pl(move || px(desktop::title_bar().left.0.max(12.) + 8.))
        .pr(move || px(desktop::title_bar().right.0.max(12.) + 8.))
        .bg(rgb(0x101010))
        .border_b_1()
        .border_color(rgb(0x242426))
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(dynamic(move || {
            if window().width.0 < 760. {
                return view().into_element();
            }
            text("sysmon")
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .into_element()
        }))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_1()
                .flex_shrink_0()
                .window_drag(false)
                .child(
                    view()
                        .relative()
                        .w(px(28.))
                        .h(px(28.))
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .bg(rgb(0x101010))
                        .test_id("titlebar-overview")
                        .label("Overview")
                        .role(AccessibilityRole::Button)
                        .accessibility_state(move || AccessibilityState {
                            selected: tab.get() == 0,
                            ..Default::default()
                        })
                        .window_drag(false)
                        .pointer_events(PointerEvents::BoxOnly)
                        .tooltip("Overview")
                        .on_press(move || tab.set(0))
                        .hover(|s| s.bg(rgb(0x18181a)))
                        .child(dynamic(move || {
                            crate::visuals::icon(
                                0,
                                if tab.get() == 0 {
                                    crate::COLORS[0]
                                } else {
                                    crate::MUTED
                                },
                                16.,
                            )
                        }))
                        .child(selection(tab, Some(0))),
                )
                .child(metric(tab, 1, "CPU", crate::COLORS[1], "CPU", move || {
                    data.with(|s| {
                        if s.cpu_available {
                            format!("{:.0} %", s.cpu)
                        } else {
                            "—".into()
                        }
                    })
                }))
                .child(metric(tab, 5, "GPU", crate::COLORS[5], "GPU", move || {
                    data.with(|s| {
                        s.gpu
                            .map(|value| format!("{value:.0} %"))
                            .unwrap_or_else(|| "—".into())
                    })
                }))
                .child(metric(
                    tab,
                    2,
                    "Mem",
                    crate::COLORS[2],
                    "Memory",
                    move || {
                        data.with(|s| {
                            if s.memory_available {
                                crate::gb(s.memory)
                            } else {
                                "—".into()
                            }
                        })
                    },
                ))
                .child(
                    view()
                        .relative()
                        .h(px(28.))
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .px(px(6.))
                        .rounded(px(4.))
                        .flex_shrink_0()
                        .bg(rgb(0x101010))
                        .test_id("titlebar-disk-io")
                        .label("Disk read and write rates. Open storage details")
                        .role(AccessibilityRole::Button)
                        .window_drag(false)
                        .pointer_events(PointerEvents::BoxOnly)
                        .accessibility_state(move || AccessibilityState {
                            selected: tab.get() == 3,
                            ..Default::default()
                        })
                        .tooltip("Disk read / write")
                        .on_press(move || tab.set(3))
                        .hover(|s| s.bg(rgb(0x18181a)))
                        .child(crate::visuals::icon(3, crate::COLORS[3], 15.))
                        .child(
                            view()
                                .flex_row()
                                .items_center()
                                .gap(px(4.))
                                .child(io_reading(data, true))
                                .child(io_reading(data, false)),
                        )
                        .child(selection(tab, Some(3))),
                )
                .child(more_menu(tab)),
        )
}

fn metric(
    tab: Signal<usize>,
    index: usize,
    name: &'static str,
    color: u32,
    hint: &'static str,
    read: impl Fn() -> String + 'static,
) -> impl IntoElement {
    view()
        .relative()
        .h(px(28.))
        .flex_row()
        .items_center()
        .gap(px(5.))
        .px(px(6.))
        .rounded(px(4.))
        .flex_shrink_0()
        .bg(rgb(0x101010))
        .test_id(&format!("titlebar-{}", name.to_ascii_lowercase()))
        .label(format!("{name} details"))
        .role(AccessibilityRole::Button)
        .accessibility_state(move || AccessibilityState {
            selected: tab.get() == index,
            ..Default::default()
        })
        .window_drag(false)
        .pointer_events(PointerEvents::BoxOnly)
        .tooltip(hint)
        .on_press(move || tab.set(index))
        .hover(|s| s.bg(rgb(0x18181a)))
        .child(crate::visuals::icon(index, color, 15.))
        .child(
            view()
                .w(px(if index == 2 { 62. } else { 35. }))
                .child(crate::motion::small_metric(read)),
        )
        .child(selection(tab, Some(index)))
}

/// A quiet, category-colored baseline replaces the old selected pills.
fn selection(tab: Signal<usize>, index: Option<usize>) -> impl IntoElement {
    view()
        .absolute()
        .left(px(6.))
        .right(px(6.))
        .bottom_0()
        .h(px(2.))
        .bg(move || rgb(crate::COLORS[tab.get()]))
        .opacity(move || {
            if index.map_or_else(|| matches!(tab.get(), 4 | 6..=12), |i| tab.get() == i) {
                1_f32
            } else {
                0_f32
            }
        })
        .accessible(false)
}

fn io_reading(data: Signal<crate::Sample>, read: bool) -> impl IntoElement {
    view()
        .w(px(72.))
        .flex_row()
        .items_center()
        .gap(px(3.))
        .child(
            text(if read { "↓" } else { "↑" })
                .text_size(px(10.))
                .text_color(rgb(crate::MUTED)),
        )
        .child(crate::motion::small_metric(move || {
            data.with(|s| {
                if s.disk_io_available {
                    crate::rate(if read { s.disk_read } else { s.disk_write })
                } else {
                    "—".into()
                }
            })
        }))
}

fn more_menu(tab: Signal<usize>) -> impl IntoElement {
    nsh_base::menu()
        .placement(Placement::Bottom)
        .align(Align::End)
        .offset(px(6.))
        .margin(px(8.))
        .trigger(
            view()
                .relative()
                .w(px(28.))
                .h(px(28.))
                .rounded(px(4.))
                .items_center()
                .justify_center()
                .bg(rgb(0x101010))
                .test_id("titlebar-more")
                .label("More categories")
                .role(AccessibilityRole::Button)
                .window_drag(false)
                .pointer_events(PointerEvents::BoxOnly)
                .tooltip("More")
                .hover(|s| s.bg(rgb(0x18181a)))
                .child(
                    canvas(
                        Painter::new()
                            .fill_color(rgb(0xb6b6bf))
                            .begin_path()
                            .arc(3., 8., 1.4, 0., std::f32::consts::TAU)
                            .fill()
                            .begin_path()
                            .arc(8., 8., 1.4, 0., std::f32::consts::TAU)
                            .fill()
                            .begin_path()
                            .arc(13., 8., 1.4, 0., std::f32::consts::TAU)
                            .fill(),
                    )
                    .w(px(16.))
                    .h(px(16.))
                    .accessible(false),
                )
                .child(selection(tab, None)),
        )
        .child(
            view()
                .test_id("titlebar-more-menu")
                .w(px(224.))
                .p_1()
                .gap_1()
                .rounded(px(10.))
                .bg(rgb(0x202023))
                .border_1()
                .border_color(rgb(0x36363c))
                .window_drag(false)
                .child(menu_heading("Device"))
                .children(
                    [4, 6, 7, 8, 9]
                        .into_iter()
                        .map(move |index| menu_destination(tab, index)),
                )
                .child(
                    nsh_base::menu_separator()
                        .h(px(1.))
                        .mx_2()
                        .my_1()
                        .bg(rgb(0x38383e)),
                )
                .child(menu_heading("Workspace"))
                .children(
                    [10, 11, 12]
                        .into_iter()
                        .map(move |index| menu_destination(tab, index)),
                ),
        )
}
fn menu_heading(name: &'static str) -> impl IntoElement {
    view()
        .px_3()
        .pt_2()
        .pb_1()
        .child(text(name).text_size(px(10.)).text_color(rgb(crate::MUTED)))
}
fn menu_destination(tab: Signal<usize>, index: usize) -> impl IntoElement {
    nsh_base::menu_item()
        .accessibility_label(crate::TABS[index])
        .on_select(move || tab.set(index))
        .rounded(px(6.))
        .bg(move || {
            rgb(if tab.get() == index {
                0x303039
            } else {
                0x202023
            })
        })
        .hover(|s| s.bg(rgb(0x35353d)))
        .child(
            view()
                .test_id(&format!("menu-{}", crate::TABS[index]))
                .flex_row()
                .items_center()
                .gap_3()
                .px_3()
                .py_2()
                .window_drag(false)
                .pointer_events(PointerEvents::BoxOnly)
                .child(crate::visuals::icon(index, crate::COLORS[index], 16.))
                .child(
                    text(crate::TABS[index])
                        .text_size(px(12.))
                        .text_color(rgb(0xeeeef1)),
                ),
        )
}
