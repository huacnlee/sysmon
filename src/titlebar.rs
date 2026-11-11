//! Desktop navigation and persistent telemetry share the app-owned title bar.
use crate::{COLORS, MUTED, Sample, gb, hint, motion, rate, visuals};
use nsh_core::{desktop::TitleBarStyle, element::desktop, prelude::*};

const BACKGROUND: u32 = 0x101010;
const HOVER_BACKGROUND: u32 = 0x18181a;
const FOREGROUND: u32 = 0xeeeef1;

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
pub fn bar(tab: Signal<usize>, _paused: Signal<bool>, data: Signal<Sample>) -> impl IntoElement {
    view()
        .test_id("sysmon-titlebar")
        .window_drag(true)
        .flex_shrink_0()
        .h(px(40.))
        .pl(move || px(desktop::title_bar().left.0.max(12.) + 8.))
        .pr(move || px(desktop::title_bar().right.0.max(12.) + 8.))
        .bg(rgb(BACKGROUND))
        .border_b_1()
        .border_color(rgb(0x242426))
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(brand())
        .child(destinations(tab, data))
}

fn brand() -> impl IntoElement {
    dynamic(move || {
        if window().width.0 < 760. {
            view().into_element()
        } else {
            text("sysmon")
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .into_element()
        }
    })
}

fn destinations(tab: Signal<usize>, data: Signal<Sample>) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .flex_shrink_0()
        .window_drag(false)
        .child(overview_button(tab))
        .child(metric(tab, 1, "CPU", COLORS[1], "CPU", move || {
            data.with(cpu_reading)
        }))
        .child(metric(tab, 5, "GPU", COLORS[5], "GPU", move || {
            data.with(gpu_reading)
        }))
        .child(metric(tab, 2, "Mem", COLORS[2], "Memory", move || {
            data.with(memory_reading)
        }))
        .child(disk_io_button(tab, data))
        .child(more_menu(tab))
}

fn overview_button(tab: Signal<usize>) -> impl IntoElement {
    control()
        .w(px(28.))
        .h(px(28.))
        .test_id("titlebar-overview")
        .label("Overview")
        .role(AccessibilityRole::Button)
        .accessibility_state(move || AccessibilityState {
            selected: tab.get() == 0,
            ..Default::default()
        })
        .tooltip(hint("Overview"))
        .on_press(move || tab.set(0))
        .child(dynamic(move || {
            visuals::icon(0, if tab.get() == 0 { COLORS[0] } else { MUTED }, 16.)
        }))
        .child(selection(tab, Some(0)))
}

fn metric(
    tab: Signal<usize>,
    index: usize,
    name: &'static str,
    color: u32,
    hint: &'static str,
    read: impl Fn() -> String + 'static,
) -> impl IntoElement {
    control()
        .h(px(28.))
        .gap(px(5.))
        .px(px(6.))
        .flex_shrink_0()
        .test_id(&format!("titlebar-{}", name.to_ascii_lowercase()))
        .label(format!("{name} details"))
        .role(AccessibilityRole::Button)
        .accessibility_state(move || AccessibilityState {
            selected: tab.get() == index,
            ..Default::default()
        })
        .tooltip(crate::hint(hint))
        .on_press(move || tab.set(index))
        .child(visuals::icon(index, color, 15.))
        .child(
            view()
                .w(px(if index == 2 { 62. } else { 35. }))
                .child(motion::small_metric(read)),
        )
        .child(selection(tab, Some(index)))
}

fn disk_io_button(tab: Signal<usize>, data: Signal<Sample>) -> impl IntoElement {
    control()
        .h(px(28.))
        .gap(px(6.))
        .px(px(6.))
        .flex_shrink_0()
        .test_id("titlebar-disk-io")
        .label("Disk read and write rates. Open storage details")
        .role(AccessibilityRole::Button)
        .accessibility_state(move || AccessibilityState {
            selected: tab.get() == 3,
            ..Default::default()
        })
        .tooltip(hint("Disk read / write"))
        .on_press(move || tab.set(3))
        .child(visuals::icon(3, COLORS[3], 15.))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .child(io_reading(data, true))
                .child(io_reading(data, false)),
        )
        .child(selection(tab, Some(3)))
}

fn control() -> View {
    view()
        .relative()
        .flex_row()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .bg(rgb(BACKGROUND))
        .window_drag(false)
        .pointer_events(PointerEvents::BoxOnly)
        .hover(|s| s.bg(rgb(HOVER_BACKGROUND)))
}

fn cpu_reading(sample: &Sample) -> String {
    if sample.cpu_available {
        format!("{:.0} %", sample.cpu)
    } else {
        "—".into()
    }
}

fn gpu_reading(sample: &Sample) -> String {
    sample
        .gpu
        .map(|value| format!("{value:.0} %"))
        .unwrap_or_else(|| "—".into())
}

fn memory_reading(sample: &Sample) -> String {
    if sample.memory_available {
        gb(sample.memory)
    } else {
        "—".into()
    }
}

/// A quiet, category-colored baseline replaces the old selected pills.
fn selection(tab: Signal<usize>, index: Option<usize>) -> impl IntoElement {
    view()
        .absolute()
        .left(px(6.))
        .right(px(6.))
        .bottom_0()
        .h(px(2.))
        .bg(move || rgb(COLORS[tab.get()]))
        .opacity(move || {
            if index.map_or_else(|| matches!(tab.get(), 4 | 6..=12), |i| tab.get() == i) {
                1_f32
            } else {
                0_f32
            }
        })
        .accessible(false)
}

fn io_reading(data: Signal<Sample>, read: bool) -> impl IntoElement {
    view()
        .w(px(72.))
        .flex_row()
        .items_center()
        .gap(px(3.))
        .child(
            text(if read { "↓" } else { "↑" })
                .text_size(px(10.))
                .text_color(rgb(MUTED)),
        )
        .child(motion::small_metric(move || {
            data.with(|sample| disk_rate(sample, read))
        }))
}

fn disk_rate(sample: &Sample, read: bool) -> String {
    if sample.disk_io_available {
        rate(if read {
            sample.disk_read
        } else {
            sample.disk_write
        })
    } else {
        "—".into()
    }
}

fn more_menu(tab: Signal<usize>) -> impl IntoElement {
    nsh_base::menu()
        .placement(Placement::Bottom)
        .align(Align::End)
        .offset(px(6.))
        .margin(px(8.))
        .trigger(more_trigger(tab))
        .child(more_menu_content(tab))
}

fn more_trigger(tab: Signal<usize>) -> impl IntoElement {
    control()
        .w(px(28.))
        .h(px(28.))
        .test_id("titlebar-more")
        .label("More categories")
        .role(AccessibilityRole::Button)
        .tooltip(hint("More"))
        .child(more_dots())
        .child(selection(tab, None))
}

fn more_dots() -> impl IntoElement {
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
    .accessible(false)
}

fn more_menu_content(tab: Signal<usize>) -> impl IntoElement {
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
        .child(menu_separator())
        .child(menu_heading("Workspace"))
        .children(
            [10, 11, 12]
                .into_iter()
                .map(move |index| menu_destination(tab, index)),
        )
}

fn menu_separator() -> impl IntoElement {
    nsh_base::menu_separator()
        .h(px(1.))
        .mx_2()
        .my_1()
        .bg(rgb(0x38383e))
}

fn menu_heading(name: &'static str) -> impl IntoElement {
    view()
        .px_3()
        .pt_2()
        .pb_1()
        .child(text(name).text_size(px(10.)).text_color(rgb(MUTED)))
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
        .child(menu_destination_body(index))
}

fn menu_destination_body(index: usize) -> impl IntoElement {
    view()
        .test_id(&format!("menu-{}", crate::TABS[index]))
        .flex_row()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .window_drag(false)
        .pointer_events(PointerEvents::BoxOnly)
        .child(visuals::icon(index, COLORS[index], 16.))
        .child(
            text(crate::TABS[index])
                .text_size(px(12.))
                .text_color(rgb(FOREGROUND)),
        )
}
