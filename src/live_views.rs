//! Retained live screens: sampling patches text and native chart slots, not cards.
use crate::{
    COLORS, MUTED, PANEL, Sample,
    charts::{self, PlotKind},
    chip, gb, label, motion,
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
            _ => crate::normalized(&s.network_history),
        }))
    });
    values
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
    tab: Signal<usize>,
    delay: f32,
) -> impl IntoElement {
    motion::enter(
        motion::interactive(
            view()
                .bg(rgb(PANEL))
                .rounded(px(14.))
                .p_3()
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
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(
                            view()
                                .flex_1()
                                .min_w(px(100.))
                                .gap_1()
                                .child(motion::metric(value))
                                .child(text(note).text_size(px(11.)).text_color(rgb(MUTED))),
                        )
                        .child(view().w(relative(0.54)).min_w(px(120.)).child(charts::plot(
                            values,
                            COLORS[index],
                            match index {
                                2 => PlotKind::Memory,
                                4 => PlotKind::Bars,
                                _ => PlotKind::Line,
                            },
                            true,
                        ))),
                ),
        ),
        delay,
    )
}
pub fn overview(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    tab: Signal<usize>,
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
    view().gap_3()
      .child(view().flex_row().flex_wrap().items_start().gap_3()
        .child(view().flex_1().min_w(px(280.)).gap_2()
          .child(card(1,"CPU activity",move||data.with(|s|reading(s.cpu_available,format!("{:.0} %",s.cpu))),move||data.with(|s|reading(!s.history.is_empty(),format!("{:.1}% average · {} cores",s.history.iter().sum::<f32>()/s.history.len().max(1) as f32,s.cores))),cpu,tab,35.))
          .child(card(2,"Memory by type",move||data.with(|s|reading(s.memory_available,gb(s.memory))),move||data.with(|s|if cfg!(target_os="ios"){"sysmon resident memory".into()}else{format!("{} available",reading(s.memory_available,gb(s.total.saturating_sub(s.memory))))}),memory,tab,85.))
          .child(card(4,"Network transfer",move||data.with(|s|reading(s.network_available,rate(s.rx))),move||data.with(|s|format!("↑ {}",reading(s.network_available,rate(s.tx)))),network,tab,135.)))
        .child(view().flex_1().flex_grow(1.25).min_w(px(300.)).gap_3()
          .child(motion::enter(view().p_1().gap_3()
            .child(view().flex_row().justify_between().items_center().child(title("Device snapshot",0xeeeef1)).child(chip(if cfg!(target_os="ios"){"App + device context"}else{"On this device"},0x8cbf96)))
            .child(label("Resources are sampled locally. Select a category for its history, applications and device details."))
            .child(live_row(if cfg!(target_os="ios"){"App resident / physical"}else{"Memory in use"},move||data.with(|s|reading(s.memory_available,format!("{} of {}",gb(s.memory),gb(s.total))))))
            .child(view().h(px(6.)).w_full().rounded_full().overflow_hidden().bg(rgb(0x2c2c30))
                .child(view().h(px(6.)).w_full().origin_left().bg(rgb(COLORS[2])).bind_transform(nsh_core::TransformKind::ScaleX,&used)))
            .child(view().flex_row().flex_wrap().gap_3().child(title(if cfg!(target_os="ios"){"● App resident"}else{"● Used"},COLORS[2])).child(label(if cfg!(target_os="ios"){"● Other memory · usage unknown"}else{"● Available"})))
            .child(view().border_1().border_color(rgb(0x2b2b2d)).rounded(px(12.)).p_3().gap_3()
                .child(live_row("Operating system",crate::system_name)).child(live_row("Host",crate::system_host))
                .child(live_row("Storage available",move||data.with(|s|reading(s.disk_available,gb(s.free)))))
                .child(live_row("Battery",move||extras.with(|e|e.battery.as_ref().map(|b|format!("{:.0}% · {}",b.percent,b.status)).unwrap_or_else(||"Unavailable".into()))))
                .child(live_row("Uptime",move||{let _=data.with(|s|s.last_at);if cfg!(target_os="ios"){"Unavailable".into()}else{crate::duration(crate::system_uptime())}}))),90.))
          .child(crate::app_list::compact_from_data(data,true))))
      .child(view().flex_row().flex_wrap().gap_3().children([(10,"Development projects"),(11,"Coding sessions"),(12,"Docker containers")].into_iter().enumerate().map(move|(i,(index,caption))|
          motion::enter(motion::interactive(view().flex_1().min_w(px(220.)).border_1().border_color(rgb(0x2b2b2d)).rounded(px(12.)).p_3().flex_row().items_center().gap_2().on_press(move||tab.set(index))
            .child(visuals::badge_icon(index,COLORS[index]))
            .child(view().flex_1().gap_1().child(text(move||match index{10=>data.with(|s|s.workloads.projects.len().to_string()),11=>data.with(|s|s.workloads.agents.len().to_string()),_=>extras.with(|e|e.containers.len().to_string())}).text_size(px(15.)).tabular_nums()).child(label(caption))).child(label("↗"))),180.+i as f32*45.))))
}
pub fn detail(data: Signal<Sample>, tab: usize) -> impl IntoElement {
    let values = series(data, tab);
    let caption = match tab {
        1 => {
            if cfg!(target_os = "ios") {
                "sysmon CPU · App scope"
            } else {
                "Processor utilization"
            }
        }
        2 => {
            if cfg!(target_os = "ios") {
                "sysmon resident memory"
            } else {
                "Physical memory in use"
            }
        }
        3 => "Available storage",
        _ => "Download",
    };
    view()
        .gap_3()
        .child(motion::enter(
            view()
                .bg(rgb(PANEL))
                .rounded(px(14.))
                .p_4()
                .gap_3()
                .child(
                    view()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(title(caption, COLORS[tab]))
                        .child(chip("LIVE HISTORY", COLORS[tab])),
                )
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
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
                                .child(label("Up to 2 minutes"))
                                .child(label("Sampled every 2 seconds")),
                        )
                        .child(view().flex_1().min_w(px(220.)).child(charts::plot(
                            values,
                            COLORS[tab],
                            match tab {
                                4 => PlotKind::Bars,
                                2 => PlotKind::Memory,
                                _ => PlotKind::Line,
                            },
                            false,
                        ))),
                ),
            20.,
        ))
        .child(dynamic(move || {
            crate::detail_stats(&data.get(), tab).into_element()
        }))
        .when(tab > 2, |v| {
            v.child(dynamic(move || {
                crate::volume_network_panel(&data.get(), tab).into_element()
            }))
        })
}
