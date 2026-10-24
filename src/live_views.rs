//! Retained live screens: sampling patches text and native chart slots, not cards.
use crate::PanelSurface;
use crate::{
    COLORS, MUTED, Sample,
    charts::{self, PlotKind},
    gb, label, motion,
    providers::Extras,
    rate, reading, title, visuals,
};
use nsh_core::ui::prelude::*;

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
        units: if index == 4 {
            charts::PlotUnits::BytesPerSecond
        } else if index == 3 {
            charts::PlotUnits::StoragePercent
        } else {
            charts::PlotUnits::Percent
        },
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
fn card(
    index: usize,
    heading: &str,
    value: impl Fn() -> String + 'static,
    note: impl Fn() -> String + 'static,
    values: Signal<Vec<f32>>,
    options: charts::PlotOptions,
    tab: Signal<usize>,
    delay: f32,
) -> impl IntoElement {
    motion::enter(
        motion::interactive(
            view()
                .when(is_desktop(), |v| v.flex_1().min_h(px(0.)))
                .panel()
                .rounded(px(8.))
                .p(px(10.))
                .gap_2()
                .on_press(move || tab.set(index))
                .child(
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
                        .child(text("↗").text_size(px(12.)).text_color(rgb(MUTED))),
                )
                .child(
                    view()
                        .when(is_desktop(), |v| v.flex_1().min_h(px(0.)))
                        .when(is_desktop() || window().width.0 >= 360., |v| {
                            v.flex_row().items_center()
                        })
                        .gap_2()
                        .child(
                            view()
                                .when(is_desktop() || window().width.0 >= 360., |v| v.flex_1())
                                .min_w(px(140.))
                                .gap_1()
                                .child(motion::metric(value))
                                .child(text(note).text_size(px(11.)).text_color(rgb(MUTED))),
                        )
                        .child(
                            view()
                                .w(move || {
                                    relative(if !is_desktop() && window().width.0 < 360. {
                                        1.
                                    } else {
                                        0.54
                                    })
                                })
                                .min_w(px(120.))
                                .when(is_desktop(), |v| v.h_full().min_h(px(0.)))
                                .child(charts::plot_with_options(
                                    values,
                                    COLORS[index],
                                    match index {
                                        2 => PlotKind::Memory,
                                        4 => PlotKind::Bars,
                                        _ => PlotKind::Line,
                                    },
                                    true,
                                    options,
                                )),
                        ),
                ),
        ),
        delay,
    )
}
pub fn overview(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    tab: Signal<usize>,
    paused: Signal<bool>,
) -> impl IntoElement {
    let cpu = series(data, 1);
    let memory = series(data, 2);
    let network = series(data, 4);
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
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.))
                        .min_h(px(0.))
                        .gap_2()
                        .child(card(
                            1,
                            "CPU activity",
                            move || {
                                data.with(|s| reading(s.cpu_available, format!("{:.0} %", s.cpu)))
                            },
                            move || {
                                data.with(|s| {
                                    reading(
                                        !s.history.is_empty(),
                                        format!(
                                            "{:.1}% average · {} cores",
                                            s.history.iter().sum::<f32>()
                                                / s.history.len().max(1) as f32,
                                            s.cores
                                        ),
                                    )
                                })
                            },
                            cpu,
                            options(data, 1, paused),
                            tab,
                            35.,
                        ))
                        .child(card(
                            2,
                            "Memory usage",
                            move || data.with(|s| reading(s.memory_available, gb(s.memory))),
                            move || {
                                data.with(|s| {
                                    if cfg!(target_os = "ios") {
                                        "Resident memory".into()
                                    } else {
                                        format!(
                                            "{} available",
                                            reading(
                                                s.memory_available,
                                                gb(s.total.saturating_sub(s.memory))
                                            )
                                        )
                                    }
                                })
                            },
                            memory,
                            options(data, 2, paused),
                            tab,
                            85.,
                        ))
                        .child(card(
                            4,
                            "Network transfer",
                            move || data.with(|s| reading(s.network_available, rate(s.rx))),
                            move || {
                                data.with(|s| {
                                    format!("↑ {}", reading(s.network_available, rate(s.tx)))
                                })
                            },
                            network,
                            options(data, 4, paused),
                            tab,
                            135.,
                        )),
                )
                .child(
                    view()
                        .flex_1()
                        .flex_grow(1.12)
                        .min_w(px(0.))
                        .min_h(px(0.))
                        .inline_style(move || {
                            nsh_core::ui::Style::default().with(
                                nsh_core::StyleKey::Display,
                                nsh_core::PatchValue::Str(if window().width.0 < 700. {
                                    "none".into()
                                } else {
                                    "flex".into()
                                }),
                            )
                        })
                        .gap_2()
                        .child(motion::enter(
                            view()
                                .p_1()
                                .flex_shrink_0()
                                .gap_2()
                                .child(
                                    view()
                                        .flex_row()
                                        .justify_between()
                                        .items_center()
                                        .child(title("Device snapshot", 0xeeeef1)),
                                )
                                .child(live_row(
                                    if cfg!(target_os = "ios") {
                                        "App resident / physical"
                                    } else {
                                        "Memory in use"
                                    },
                                    move || {
                                        data.with(|s| {
                                            reading(
                                                s.memory_available,
                                                format!("{} of {}", gb(s.memory), gb(s.total)),
                                            )
                                        })
                                    },
                                ))
                                .child(
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
                                                .bg(rgb(COLORS[2]))
                                                .bind_transform(
                                                    nsh_core::TransformKind::ScaleX,
                                                    &used,
                                                ),
                                        ),
                                )
                                .child(
                                    view()
                                        .flex_row()
                                        .flex_wrap()
                                        .gap_2()
                                        .child(title(
                                            if cfg!(target_os = "ios") {
                                                "● App resident"
                                            } else {
                                                "● Used"
                                            },
                                            COLORS[2],
                                        ))
                                        .child(label(if cfg!(target_os = "ios") {
                                            "● Other memory · usage unknown"
                                        } else {
                                            "● Available"
                                        })),
                                )
                                .child(
                                    view()
                                        .border_1()
                                        .border_color(rgb(0x404046))
                                        .rounded(px(8.))
                                        .p(px(10.))
                                        .gap_2()
                                        .child(live_row("Operating system", crate::system_name))
                                        .child(live_row("Host", crate::system_host))
                                        .child(live_row("Storage available", move || {
                                            data.with(|s| reading(s.disk_available, gb(s.free)))
                                        }))
                                        .child(live_row("Battery", move || {
                                            extras.with(|e| {
                                                e.battery
                                                    .as_ref()
                                                    .map(|b| {
                                                        format!("{:.0}% · {}", b.percent, b.status)
                                                    })
                                                    .unwrap_or_else(|| "Unavailable".into())
                                            })
                                        }))
                                        .child(live_row("Uptime", move || {
                                            let _ = data.with(|s| s.last_at);
                                            if cfg!(target_os = "ios") {
                                                "Unavailable".into()
                                            } else {
                                                crate::duration(crate::system_uptime())
                                            }
                                        })),
                                ),
                            90.,
                        ))
                        .child(crate::app_list::compact_from_data(data, true)),
                ),
        )
        .child(
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
                    .map(move |(i, (index, caption))| {
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
                                    .tooltip(caption)
                                    .on_press(move || tab.set(index))
                                    .child(visuals::badge_icon(index, COLORS[index]))
                                    .child(
                                        view()
                                            .flex_1()
                                            .gap_1()
                                            .child(
                                                text(move || match index {
                                                    10 => data.with(|s| {
                                                        s.workloads.projects.len().to_string()
                                                    }),
                                                    11 => data.with(|s| {
                                                        s.workloads.agents.len().to_string()
                                                    }),
                                                    _ => extras
                                                        .with(|e| e.containers.len().to_string()),
                                                })
                                                .text_size(px(15.))
                                                .tabular_nums(),
                                            )
                                            .child(
                                                view()
                                                    .inline_style(move || {
                                                        nsh_core::ui::Style::default().with(
                                                            nsh_core::StyleKey::Display,
                                                            nsh_core::PatchValue::Str(
                                                                if window().width.0 < 800. {
                                                                    "none".into()
                                                                } else {
                                                                    "flex".into()
                                                                },
                                                            ),
                                                        )
                                                    })
                                                    .child(label(caption)),
                                            ),
                                    )
                                    .child(label("↗")),
                            ),
                            180. + i as f32 * 45.,
                        )
                    }),
                ),
        )
}
pub fn detail(data: Signal<Sample>, tab: usize, paused: Signal<bool>) -> impl IntoElement {
    let values = series(data, tab);
    let plot_options = options(data, tab, paused);
    let caption = match tab {
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
    };
    view()
        .when(is_desktop(), |v| {
            if tab <= 2 {
                v.flex_shrink_0()
            } else {
                v.flex_1().min_h(px(0.))
            }
        })
        .gap_2()
        .child(motion::enter(
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
                .child(
                    view()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(title(caption, COLORS[tab]))
                        .child(
                            text(move || {
                                if paused.get() {
                                    "PAUSED"
                                } else {
                                    "LIVE HISTORY"
                                }
                            })
                            .text_size(px(10.))
                            .text_color(rgb(COLORS[tab])),
                        ),
                )
                .child(
                    view()
                        .when(is_desktop(), |v| {
                            v.flex_1().min_h(px(0.)).flex_row().items_center()
                        })
                        .gap_4()
                        .child(
                            view()
                                .w(px(160.))
                                .gap_2()
                                .child(motion::metric(move || {
                                    data.with(|s| match tab {
                                        1 => reading(s.cpu_available, format!("{:.0} %", s.cpu)),
                                        2 => reading(s.memory_available, gb(s.memory)),
                                        3 => reading(s.disk_available, gb(s.free)),
                                        _ => reading(s.network_available, rate(s.rx)),
                                    })
                                }))
                                .child(label("Latest 60 samples"))
                                .child(label("Sampled every 2 seconds")),
                        )
                        .child(
                            view()
                                .flex_1()
                                .min_w(px(220.))
                                .when(is_desktop(), |v| v.h_full().min_h(px(0.)))
                                .child(charts::plot_with_options(
                                    values,
                                    COLORS[tab],
                                    match tab {
                                        4 => PlotKind::Bars,
                                        2 => PlotKind::Memory,
                                        _ => PlotKind::Line,
                                    },
                                    false,
                                    plot_options,
                                )),
                        ),
                ),
            20.,
        ))
        .child(view().flex_shrink_0().child(dynamic(move || {
            crate::detail_stats(&data.get(), tab).into_element()
        })))
        .when(tab > 2, |v| {
            v.child(
                view()
                    .when(is_desktop(), |v| v.flex_1().min_h(px(0.)))
                    .child(crate::volume_network_panel(data, tab)),
            )
        })
}

/// Mobile overview is a single-column resource dashboard, without desktop
/// workspace counters or an app table the phone cannot populate.
pub fn mobile_overview(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    detail: Signal<usize>,
    _section: Signal<u32>,
    paused: Signal<bool>,
) -> impl IntoElement {
    let cpu = series(data, 1);
    let memory = series(data, 2);
    let disk = series(data, 3);
    let network = series(data, 4);
    view()
        .gap_2()
        .child(card(
            1,
            "App CPU",
            move || data.with(|s| reading(s.cpu_available, format!("{:.1} %", s.cpu))),
            move || data.with(|s| format!("{} logical cores", s.cores)),
            cpu,
            options(data, 1, paused),
            detail,
            0.,
        ))
        .child(card(
            2,
            "Resident memory",
            move || data.with(|s| reading(s.memory_available, gb(s.memory))),
            || "App memory in use".into(),
            memory,
            options(data, 2, paused),
            detail,
            40.,
        ))
        .child(card(
            3,
            "Storage",
            move || data.with(|s| reading(s.disk_available, gb(s.free))),
            || "Available on the data volume".into(),
            disk,
            options(data, 3, paused),
            detail,
            80.,
        ))
        .child(card(
            4,
            "Network",
            move || data.with(|s| reading(s.network_available, crate::rate(s.rx))),
            move || data.with(|s| format!("↑ {}", reading(s.network_available, crate::rate(s.tx)))),
            network,
            options(data, 4, paused),
            detail,
            120.,
        ))
        .child(
            view()
                .panel()
                .rounded(px(8.))
                .p(px(10.))
                .gap_2()
                .child(title("Battery", COLORS[7]))
                .child(
                    text(move || {
                        extras.with(|e| {
                            e.battery
                                .as_ref()
                                .map(|b| format!("{:.0}% · {}", b.percent, b.status))
                                .unwrap_or_else(|| "Not available on this device".into())
                        })
                    })
                    .text_size(px(14.)),
                ),
        )
        .child(crate::mobile::scope_note())
}

/// Driver activity is independent of the slower hardware-identity provider.
pub fn gpu_detail(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    paused: Signal<bool>,
) -> impl IntoElement {
    let values = series(data, 5);
    let plot_options = options(data, 5, paused);
    view().flex_1().h_full().min_h(px(0.)).gap_2()
        .child(view().flex_1().min_h(px(0.)).panel().rounded(px(8.)).p_4().gap_2()
            .child(view().flex_row().items_center().justify_between()
                .child(title("GPU activity",COLORS[5]))
                .child(text(move||if paused.get(){"PAUSED"}else{"LIVE HISTORY"}).text_size(px(10.)).text_color(rgb(COLORS[5]))))
            .child(text(move||extras.with(|e|e.gpu.clone().unwrap_or_else(||"Graphics processor".into()))).text_size(px(16.)))
            .child(motion::metric(move||data.with(|s|s.gpu.map(|v|format!("{v:.1} %")).unwrap_or_else(||"Unavailable".into()))))
            .child(label("Driver-reported activity · busiest visible GPU"))
            .child(charts::plot_with_options(values,COLORS[5],PlotKind::Line,false,plot_options))
            .child(text(move||data.with(|s|if s.gpu.is_none(){"The graphics driver does not expose utilization to this app. Hardware identity may still be available.".to_owned()}else{"Read-only system telemetry. Per-app GPU attribution is not collected.".to_owned()})).text_size(px(11.)).text_color(rgb(MUTED))))
}
