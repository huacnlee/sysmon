//! Retained live screens: sampling patches text and native chart slots, not cards.
use crate::PanelSurface;
use crate::{
    COLORS, MUTED, Sample,
    charts::{self, PlotKind},
    gb, label, motion,
    providers::Extras,
    rate, reading, title, visuals,
};
use nsh_core::prelude::*;

struct CardProps<V, N> {
    index: usize,
    heading: &'static str,
    value: V,
    note: N,
    values: Signal<Vec<f32>>,
    options: charts::PlotOptions,
    target: Signal<usize>,
    delay: f32,
}

fn series(data: Signal<Sample>, index: usize) -> Signal<Vec<f32>> {
    let values = signal(Vec::new());
    effect(move || {
        values.set(data.with(|s| match index {
            1 => s.history.clone(),
            2 => s.memory_history.clone(),
            3 => s.disk_history.clone(),
            5 => s.gpu_history.clone(),
            _ => s.network_history.clone(),
        }))
    });
    values
}

fn options(data: Signal<Sample>, index: usize, paused: Signal<bool>) -> charts::PlotOptions {
    let timestamps = signal(Vec::new());
    effect(move || {
        timestamps.set(data.with(|s| match index {
            1 => s.cpu_times.clone(),
            2 => s.memory_times.clone(),
            3 => s.disk_times.clone(),
            5 => s.gpu_times.clone(),
            _ => s.network_times.clone(),
        }))
    });
    let secondary = if index == 4 {
        let upload = signal(Vec::new());
        effect(move || upload.set(data.with(|s| s.upload_history.clone())));
        Some((upload, 0x559ee5))
    } else {
        None
    };
    charts::PlotOptions {
        fit_height: is_desktop(),
        timestamps: Some(timestamps),
        paused: Some(paused),
        secondary,
        units: match index {
            4 => charts::PlotUnits::BytesPerSecond,
            3 => charts::PlotUnits::StoragePercent,
            _ => charts::PlotUnits::Percent,
        },
    }
}

fn plot_kind(index: usize) -> PlotKind {
    match index {
        2 => PlotKind::Memory,
        4 => PlotKind::Bars,
        _ => PlotKind::Line,
    }
}

fn display_above(min_width: f32) -> impl Fn() -> nsh_core::element::Style + 'static {
    move || {
        nsh_core::element::Style::default().with(
            nsh_core::StyleKey::Display,
            nsh_core::StyleKey::Display
                .parse_value(nsh_core::StyleValue::Str(if window().width.0 < min_width {
                    "none"
                } else {
                    "flex"
                }))
                .expect("display value"),
        )
    }
}

fn pause_label(paused: Signal<bool>) -> &'static str {
    if paused.get() {
        "PAUSED"
    } else {
        "LIVE HISTORY"
    }
}

fn live_row(name: &str, read: impl Fn() -> String + 'static) -> impl IntoElement {
    view()
        .flex_row()
        .justify_between()
        .items_center()
        .gap_2()
        .child(label(name.to_owned()))
        .child(text(read).text_size(px(12.)).tabular_nums())
}

fn card<V, N>(props: CardProps<V, N>) -> impl IntoElement
where
    V: Fn() -> String + 'static,
    N: Fn() -> String + 'static,
{
    let CardProps {
        index,
        heading,
        value,
        note,
        values,
        options,
        target,
        delay,
    } = props;
    motion::enter(
        motion::interactive(
            card_shell(index, target)
                .child(card_header(index, heading))
                .child(card_body(index, value, note, values, options)),
        ),
        delay,
    )
}

fn card_shell(index: usize, target: Signal<usize>) -> View {
    view()
        .when(is_desktop(), |v| v.flex_1().min_h(px(0.)))
        .panel()
        .rounded(px(8.))
        .p(px(8.))
        .gap(px(6.))
        .on_press(move || target.set(index))
}

fn card_header(index: usize, heading: &str) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(visuals::icon(index, COLORS[index], 15.))
                .child(title(heading.to_owned(), COLORS[index])),
        )
        .child(text("↗").text_size(px(12.)).text_color(rgb(MUTED)))
}

fn card_body<V, N>(
    index: usize,
    value: V,
    note: N,
    values: Signal<Vec<f32>>,
    options: charts::PlotOptions,
) -> impl IntoElement
where
    V: Fn() -> String + 'static,
    N: Fn() -> String + 'static,
{
    view()
        .when(is_desktop(), |v| v.flex_1().min_h(px(0.)))
        .gap(px(6.))
        .child(card_reading(value, note))
        .child(card_plot(index, values, options))
}

fn card_reading<V, N>(value: V, note: N) -> impl IntoElement
where
    V: Fn() -> String + 'static,
    N: Fn() -> String + 'static,
{
    view()
        .flex_row()
        .items_end()
        .justify_between()
        .flex_shrink_0()
        .gap_2()
        .child(motion::metric(value))
        .child(
            text(note)
                .flex_1()
                .min_w(px(0.))
                .text_right()
                .text_size(px(11.))
                .text_color(rgb(MUTED)),
        )
}

fn card_plot(
    index: usize,
    values: Signal<Vec<f32>>,
    options: charts::PlotOptions,
) -> impl IntoElement {
    view()
        .w_full()
        .min_w(px(0.))
        .when(is_desktop(), |v| v.flex_1().min_h(px(0.)))
        .child(charts::plot_with_options(
            values,
            COLORS[index],
            plot_kind(index),
            true,
            options,
        ))
}

pub fn overview(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    tab: Signal<usize>,
    paused: Signal<bool>,
) -> impl IntoElement {
    view()
        .h_full()
        .flex_1()
        .min_h(px(0.))
        .test_id("overview-body")
        .gap_2()
        .child(
            view()
                .flex_1()
                .min_h(px(0.))
                .flex_row()
                .gap_2()
                .child(overview_cards(data, tab, paused))
                .child(snapshot_column(data, extras)),
        )
        .child(overview_workloads(data, extras, tab))
}

fn overview_cards(
    data: Signal<Sample>,
    tab: Signal<usize>,
    paused: Signal<bool>,
) -> impl IntoElement {
    view()
        .flex_1()
        .flex_grow(1.35)
        .min_w(px(0.))
        .min_h(px(0.))
        .gap_2()
        .child(cpu_card(data, tab, paused))
        .child(memory_card(data, tab, paused))
        .child(network_card(data, tab, paused))
}

fn cpu_card(data: Signal<Sample>, tab: Signal<usize>, paused: Signal<bool>) -> impl IntoElement {
    card(CardProps {
        index: 1,
        heading: "CPU activity",
        value: move || data.with(cpu_value),
        note: move || data.with(cpu_note),
        values: series(data, 1),
        options: options(data, 1, paused),
        target: tab,
        delay: 35.,
    })
}

fn memory_card(data: Signal<Sample>, tab: Signal<usize>, paused: Signal<bool>) -> impl IntoElement {
    card(CardProps {
        index: 2,
        heading: "Memory usage",
        value: move || data.with(memory_value),
        note: move || data.with(memory_note),
        values: series(data, 2),
        options: options(data, 2, paused),
        target: tab,
        delay: 85.,
    })
}

fn network_card(
    data: Signal<Sample>,
    tab: Signal<usize>,
    paused: Signal<bool>,
) -> impl IntoElement {
    card(CardProps {
        index: 4,
        heading: "Network transfer",
        value: move || data.with(network_value),
        note: move || data.with(network_note),
        values: series(data, 4),
        options: options(data, 4, paused),
        target: tab,
        delay: 135.,
    })
}

fn cpu_value(sample: &Sample) -> String {
    reading(sample.cpu_available, format!("{:.0} %", sample.cpu))
}

fn cpu_note(sample: &Sample) -> String {
    reading(
        !sample.history.is_empty(),
        format!(
            "{:.1}% average · {} cores",
            sample.history.iter().sum::<f32>() / sample.history.len().max(1) as f32,
            sample.cores
        ),
    )
}

fn memory_value(sample: &Sample) -> String {
    reading(sample.memory_available, gb(sample.memory))
}

fn memory_note(sample: &Sample) -> String {
    if cfg!(target_os = "ios") {
        "Resident memory".into()
    } else {
        format!(
            "{} available",
            reading(
                sample.memory_available,
                gb(sample.total.saturating_sub(sample.memory))
            )
        )
    }
}

fn network_value(sample: &Sample) -> String {
    reading(sample.network_available, rate(sample.rx))
}

fn network_note(sample: &Sample) -> String {
    format!("↑ {}", reading(sample.network_available, rate(sample.tx)))
}

fn snapshot_column(data: Signal<Sample>, extras: Signal<Extras>) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(px(0.))
        .min_h(px(0.))
        .inline_style(display_above(700.))
        .gap_2()
        .child(snapshot_panel(data, extras))
        .child(crate::app_list::compact_from_data(data, true))
}

fn snapshot_panel(data: Signal<Sample>, extras: Signal<Extras>) -> impl IntoElement {
    motion::enter(
        view()
            .p_1()
            .flex_shrink_0()
            .gap_2()
            .child(snapshot_heading())
            .child(live_row(memory_row_label(), move || {
                data.with(memory_snapshot_value)
            }))
            .child(memory_meter(data))
            .child(memory_legend())
            .child(snapshot_details(data, extras)),
        90.,
    )
}

fn snapshot_heading() -> impl IntoElement {
    view()
        .flex_row()
        .justify_between()
        .items_center()
        .child(title("Device snapshot", 0xeeeef1))
}

fn memory_row_label() -> &'static str {
    if cfg!(target_os = "ios") {
        "App resident / physical"
    } else {
        "Memory in use"
    }
}

fn memory_snapshot_value(sample: &Sample) -> String {
    reading(
        sample.memory_available,
        format!("{} of {}", gb(sample.memory), gb(sample.total)),
    )
}

fn memory_meter(data: Signal<Sample>) -> impl IntoElement {
    let used = animated_value(0.);
    effect(move || {
        let target = data.with(|s| {
            if s.memory_available && s.total > 0 {
                s.memory as f32 / s.total as f32
            } else {
                0.
            }
        });
        if reduce_motion() {
            used.set(target);
        } else {
            let _ = used.spring(
                target,
                Spring {
                    stiffness: 150.,
                    damping: 24.,
                    ..Default::default()
                },
            );
        }
    });
    view()
        .h(px(6.))
        .w_full()
        .rounded_full()
        .overflow_hidden()
        .bg(rgb(0x2c2c30))
        .child(
            view()
                .h(px(6.))
                .w_full()
                .origin_left()
                .bg_linear_to_r()
                .gradient_from(rgb(COLORS[2]))
                .gradient_to(rgb(charts::gradient_end(COLORS[2])))
                .bind_transform(nsh_core::TransformKind::ScaleX, &used),
        )
}

fn memory_legend() -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(title(used_memory_label(), COLORS[2]))
        .child(label(available_memory_label()))
}

fn used_memory_label() -> &'static str {
    if cfg!(target_os = "ios") {
        "● App resident"
    } else {
        "● Used"
    }
}

fn available_memory_label() -> &'static str {
    if cfg!(target_os = "ios") {
        "● Other memory · usage unknown"
    } else {
        "● Available"
    }
}

fn snapshot_details(data: Signal<Sample>, extras: Signal<Extras>) -> impl IntoElement {
    view()
        .border_1()
        .border_color(rgb(0x404046))
        .rounded(px(8.))
        .p(px(10.))
        .gap_2()
        .child(live_row("Operating system", crate::system_name))
        .child(live_row("Host", crate::system_host))
        .child(live_row("Storage available", move || {
            data.with(|sample| reading(sample.disk_available, gb(sample.free)))
        }))
        .child(live_row("Battery", move || extras.with(battery_label)))
        .child(live_row("Uptime", move || uptime(data)))
}

fn battery_label(extras: &Extras) -> String {
    extras
        .battery
        .as_ref()
        .map(|battery| format!("{:.0}% · {}", battery.percent, battery.status))
        .unwrap_or_else(|| "Unavailable".into())
}

fn uptime(data: Signal<Sample>) -> String {
    let _ = data.with(|sample| sample.last_at);
    if cfg!(target_os = "ios") {
        "Unavailable".into()
    } else {
        crate::duration(crate::system_uptime())
    }
}

fn overview_workloads(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    tab: Signal<usize>,
) -> impl IntoElement {
    view()
        .flex_row()
        .flex_shrink_0()
        .h(move || px(if window().width.0 < 800. { 44. } else { 56. }))
        .gap_2()
        .children(
            [
                (10, "Development projects"),
                (11, "Coding sessions"),
                (12, "Docker containers"),
            ]
            .into_iter()
            .enumerate()
            .map(move |(order, (index, caption))| {
                workload_tile(data, extras, tab, index, caption, order)
            }),
        )
}

fn workload_tile(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    tab: Signal<usize>,
    index: usize,
    caption: &'static str,
    order: usize,
) -> impl IntoElement {
    motion::enter(
        motion::interactive(
            view()
                .flex_1()
                .min_w(px(0.))
                .h_full()
                .border_1()
                .border_color(rgb(0x404046))
                .rounded(px(8.))
                .px(px(10.))
                .py(px(8.))
                .flex_row()
                .items_center()
                .gap_2()
                .test_id(&format!("overview-workload-{index}"))
                .label(caption)
                .tooltip(crate::hint(caption))
                .on_press(move || tab.set(index))
                .child(visuals::badge_icon(index, COLORS[index]))
                .child(workload_tile_body(data, extras, index, caption))
                .child(label("↗")),
        ),
        180. + order as f32 * 45.,
    )
}

fn workload_tile_body(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    index: usize,
    caption: &'static str,
) -> impl IntoElement {
    view()
        .flex_1()
        .gap_1()
        .child(
            text(move || workload_count(data, extras, index))
                .text_size(px(15.))
                .tabular_nums(),
        )
        .child(
            view()
                .inline_style(display_above(800.))
                .child(label(caption)),
        )
}

fn workload_count(data: Signal<Sample>, extras: Signal<Extras>, index: usize) -> String {
    match index {
        10 => data.with(|sample| sample.workloads.projects.len().to_string()),
        11 => data.with(|sample| sample.workloads.agents.len().to_string()),
        _ => extras.with(|e| e.containers.len().to_string()),
    }
}

pub fn detail(data: Signal<Sample>, tab: usize, paused: Signal<bool>) -> impl IntoElement {
    let values = series(data, tab);
    let plot_options = options(data, tab, paused);
    view()
        .when(is_desktop(), |v| {
            if tab <= 2 {
                v.flex_shrink_0()
            } else {
                v.flex_1().min_h(px(0.))
            }
        })
        .gap_2()
        .child(detail_history(data, tab, paused, values, plot_options))
        .child(detail_stats_panel(data, tab))
        .when(tab > 2, |v| {
            v.child(
                view()
                    .when(is_desktop(), |v| v.flex_1().min_h(px(0.)))
                    .child(crate::volume_network_panel(data, tab)),
            )
        })
}

fn detail_history(
    data: Signal<Sample>,
    tab: usize,
    paused: Signal<bool>,
    values: Signal<Vec<f32>>,
    options: charts::PlotOptions,
) -> impl IntoElement {
    motion::enter(
        view()
            .when(is_desktop(), |v| {
                v.h(move || px((window().height.0 * 0.26).clamp(146., 220.)))
                    .flex_shrink_0()
                    .min_h(px(0.))
            })
            .panel()
            .rounded(px(8.))
            .p_4()
            .gap_2()
            .child(detail_header(tab, paused))
            .child(
                view()
                    .when(is_desktop(), |v| {
                        v.flex_1().min_h(px(0.)).flex_row().items_center()
                    })
                    .gap_4()
                    .child(detail_latest(data, tab))
                    .child(detail_plot(tab, values, options)),
            ),
        20.,
    )
}

fn detail_header(tab: usize, paused: Signal<bool>) -> impl IntoElement {
    view()
        .flex_row()
        .justify_between()
        .items_center()
        .child(title(detail_caption(tab), COLORS[tab]))
        .child(
            text(move || pause_label(paused))
                .text_size(px(10.))
                .text_color(rgb(COLORS[tab])),
        )
}

fn detail_caption(tab: usize) -> &'static str {
    match tab {
        1 => {
            if cfg!(any(target_os = "ios", target_os = "android")) {
                "App CPU"
            } else {
                "Processor utilization"
            }
        }
        2 => {
            if cfg!(any(target_os = "ios", target_os = "android")) {
                "Resident memory"
            } else {
                "Physical memory in use"
            }
        }
        3 => "Available storage",
        _ => "Download",
    }
}

fn detail_latest(data: Signal<Sample>, tab: usize) -> impl IntoElement {
    view()
        .w(px(160.))
        .gap_2()
        .child(motion::metric(move || {
            data.with(|s| latest_reading(s, tab))
        }))
        .child(label("Latest 60 samples"))
        .child(label("Sampled every 2 seconds"))
}

fn latest_reading(sample: &Sample, tab: usize) -> String {
    match tab {
        1 => reading(sample.cpu_available, format!("{:.0} %", sample.cpu)),
        2 => reading(sample.memory_available, gb(sample.memory)),
        3 => reading(sample.disk_available, gb(sample.free)),
        _ => reading(sample.network_available, rate(sample.rx)),
    }
}

fn detail_plot(
    tab: usize,
    values: Signal<Vec<f32>>,
    options: charts::PlotOptions,
) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(px(220.))
        .when(is_desktop(), |v| v.h_full().min_h(px(0.)))
        .child(charts::plot_with_options(
            values,
            COLORS[tab],
            plot_kind(tab),
            false,
            options,
        ))
}

fn detail_stats_panel(data: Signal<Sample>, tab: usize) -> impl IntoElement {
    view().flex_shrink_0().child(dynamic(move || {
        crate::detail_stats(&data.get(), tab).into_element()
    }))
}

/// Mobile overview is a single-column resource dashboard, without desktop
/// workspace counters or an app table the phone cannot populate.
pub fn mobile_overview(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    detail: Signal<usize>,
    paused: Signal<bool>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(card(CardProps {
            index: 1,
            heading: "App CPU",
            value: move || data.with(mobile_cpu_value),
            note: move || data.with(core_note),
            values: series(data, 1),
            options: options(data, 1, paused),
            target: detail,
            delay: 0.,
        }))
        .child(card(CardProps {
            index: 2,
            heading: "Resident memory",
            value: move || data.with(mobile_memory_value),
            note: || "App memory in use".into(),
            values: series(data, 2),
            options: options(data, 2, paused),
            target: detail,
            delay: 40.,
        }))
        .child(card(CardProps {
            index: 3,
            heading: "Storage",
            value: move || data.with(mobile_storage_value),
            note: || "Available on the data volume".into(),
            values: series(data, 3),
            options: options(data, 3, paused),
            target: detail,
            delay: 80.,
        }))
        .child(card(CardProps {
            index: 4,
            heading: "Network",
            value: move || data.with(network_value),
            note: move || data.with(network_note),
            values: series(data, 4),
            options: options(data, 4, paused),
            target: detail,
            delay: 120.,
        }))
        .child(mobile_battery_panel(extras))
}

fn mobile_cpu_value(sample: &Sample) -> String {
    reading(sample.cpu_available, format!("{:.1} %", sample.cpu))
}

fn mobile_memory_value(sample: &Sample) -> String {
    reading(sample.memory_available, gb(sample.memory))
}

fn mobile_storage_value(sample: &Sample) -> String {
    reading(sample.disk_available, gb(sample.free))
}

fn core_note(sample: &Sample) -> String {
    format!("{} logical cores", sample.cores)
}

fn mobile_battery_panel(extras: Signal<Extras>) -> impl IntoElement {
    view()
        .panel()
        .rounded(px(8.))
        .p(px(10.))
        .gap_2()
        .child(title("Battery", COLORS[7]))
        .child(text(move || extras.with(battery_label)).text_size(px(14.)))
}

/// Driver activity is independent of the slower hardware-identity provider.
pub fn gpu_detail(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    paused: Signal<bool>,
) -> impl IntoElement {
    let values = series(data, 5);
    let plot_options = options(data, 5, paused);
    view().flex_1().h_full().min_h(px(0.)).gap_2().child(
        view()
            .flex_1()
            .min_h(px(0.))
            .panel()
            .rounded(px(8.))
            .p_4()
            .gap_2()
            .child(gpu_header(paused))
            .child(text(move || extras.with(gpu_name)).text_size(px(16.)))
            .child(motion::metric(move || data.with(gpu_usage)))
            .child(label("Driver-reported activity · busiest visible GPU"))
            .child(charts::plot_with_options(
                values,
                COLORS[5],
                PlotKind::Line,
                false,
                plot_options,
            ))
            .child(gpu_note(data)),
    )
}

fn gpu_header(paused: Signal<bool>) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .child(title("GPU activity", COLORS[5]))
        .child(
            text(move || pause_label(paused))
                .text_size(px(10.))
                .text_color(rgb(COLORS[5])),
        )
}

fn gpu_name(extras: &Extras) -> String {
    extras
        .gpu
        .clone()
        .unwrap_or_else(|| "Graphics processor".into())
}

fn gpu_usage(sample: &Sample) -> String {
    sample
        .gpu
        .map(|value| format!("{value:.1} %"))
        .unwrap_or_else(|| "Unavailable".into())
}

fn gpu_note(data: Signal<Sample>) -> impl IntoElement {
    text(move || {
        if data.with(|sample| sample.gpu.is_none()) {
            "The graphics driver does not expose utilization to this app. Hardware identity may still be available."
                .to_owned()
        } else {
            "Read-only system telemetry. Per-app GPU attribution is not collected.".to_owned()
        }
    })
    .text_size(px(11.))
    .text_color(rgb(MUTED))
}
