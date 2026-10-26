//! Phone-first navigation. Tabs own destinations; resource rows open a detail
//! within Activity instead of exposing desktop-only categories on phones.
use crate::PanelSurface;
use crate::{
    COLORS, MUTED, Sample, gb, label, live_views, motion, providers::Extras, reading, title,
    visuals,
};
use nsh_core::{
    mobile::{TabBarItem, tab_bar},
    prelude::*,
};

const SECTIONS: [&str; 4] = ["Overview", "Activity", "Device", "About"];

pub fn shell(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    paused: Signal<bool>,
    ready: Signal<bool>,
) -> impl IntoElement {
    let section = signal(0_u32);
    // 0 is the Activity index; 1..=4 are actual resource destinations.
    let detail = signal(0_usize);
    effect(move || {
        if detail.get() > 0 {
            section.set(1);
        }
    });
    on_back_press(move || {
        // The native stack owns detail back presses and interactive gestures.
        if detail.get_untracked() != 0 {
            false
        } else if section.get_untracked() != 0 {
            section.set(0);
            true
        } else {
            false
        }
    });
    let icons = if cfg!(target_os = "android") {
        [
            "android:ic_menu_view",
            "android:ic_menu_sort_by_size",
            "android:ic_menu_manage",
            "android:ic_menu_info_details",
        ]
    } else {
        [
            "square.grid.2x2",
            "waveform.path.ecg",
            "iphone",
            "info.circle",
        ]
    };
    view()
        // Native navigation owns the top inset; the TabBar owns the bottom inset.
        .px_safe_area()
        .flex_1()
        .w_full()
        .h_full()
        .bg(rgb(crate::BG))
        .text_color(rgb(0xeeeef1))
        .child(
            nsh_base::nav_stack()
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .child(
                    nsh_base::nav_page()
                        .key("mobile-root")
                        .title(move || SECTIONS[section.get() as usize])
                        .header_visible(true)
                        .header_large_title(false)
                        .header_back_title("")
                        .header_style_with(|s| {
                            s.text_color(rgb(0xeeeef1)).tint_color(rgb(0xeeeef1))
                        })
                        .status_bar(nsh_core::StatusBarStyle::Light)
                        .child(scroll().flex_1().min_h(px(0.)).child(
                            view().px_4().pt_2().pb_5().child(dynamic(move || {
                                if !ready.get() {
                                    return view()
                                        .py_6()
                                        .child(label("Reading your device…"))
                                        .into_element();
                                }
                                match section.get() {
                                    0 => live_views::mobile_overview(
                                        data, extras, detail, section, paused,
                                    )
                                    .into_element(),
                                    1 => activity(data, detail).into_element(),
                                    2 => device(data, extras).into_element(),
                                    _ => about(paused).into_element(),
                                }
                            })),
                        )),
                )
                .child(when(
                    move || detail.get() > 0,
                    move || {
                        let resource = detail.get_untracked();
                        nsh_base::nav_page()
                            .key(format!("mobile-resource-{resource}"))
                            .title(resource_title(resource))
                            .header_visible(true)
                            .header_large_title(false)
                            .header_style_with(|s| {
                                s.text_color(rgb(0xeeeef1)).tint_color(rgb(0xeeeef1))
                            })
                            .status_bar(nsh_core::StatusBarStyle::Light)
                            .on_dismiss(move || detail.set(0))
                            .child(
                                scroll().flex_1().min_h(px(0.)).child(
                                    view()
                                        .px_4()
                                        .pt_2()
                                        .pb_5()
                                        .gap_3()
                                        .child(live_views::detail(data, resource, paused)),
                                ),
                            )
                    },
                )),
        )
        .child(
            tab_bar()
                .items(
                    SECTIONS
                        .into_iter()
                        .zip(icons)
                        .map(|(name, icon)| TabBarItem::new(name).icon(icon))
                        .collect::<Vec<_>>(),
                )
                .selected_index(section)
                .on_change(move |index: &u32| {
                    if *index < 4 {
                        detail.set(0);
                        section.set(*index);
                    }
                }),
        )
}

fn resource_title(resource: usize) -> &'static str {
    if resource == 3 { "Storage" } else { crate::TABS[resource] }
}

pub fn scope_note() -> impl IntoElement {
    view().bg(rgb(0x171c25)).rounded(px(8.)).p_3().gap_2()
        .child(title("Data scope",0x8eb9f1))
        .child(label("CPU and resident memory describe sysmon. Storage and network describe what this app can access. The OS protects other apps; their activity is not shown."))
}
fn activity(data: Signal<Sample>, detail: Signal<usize>) -> impl IntoElement {
    view()
        .gap_4()
        .child(label("Tap a resource to inspect its readings and history."))
        .child(
            view()
                .panel()
                .rounded(px(16.))
                .overflow_hidden()
                .children((1..=4).map(move |index| {
                    view()
                        .min_h(px(72.))
                        .px_4()
                        .py_3()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .test_id(&format!("mobile-resource-{index}"))
                        .label(crate::TABS[index])
                        .role(AccessibilityRole::Button)
                        .on_press(move || detail.set(index))
                        .child(visuals::badge_icon(index, COLORS[index]))
                        .child(
                            view()
                                .flex_1()
                                .gap_1()
                                .child(title(
                                    if index == 3 {
                                        "Storage"
                                    } else {
                                        crate::TABS[index]
                                    },
                                    0xeeeef1,
                                ))
                                .child(
                                    text(move || {
                                        data.with(|s| match index {
                                            1 => reading(
                                                s.cpu_available,
                                                format!("{:.1}%", s.cpu),
                                            ),
                                            2 => reading(
                                                s.memory_available,
                                                format!("{} · resident", gb(s.memory)),
                                            ),
                                            3 => reading(
                                                s.disk_available,
                                                format!("{} available", gb(s.free)),
                                            ),
                                            _ => reading(
                                                s.network_available,
                                                format!(
                                                    "↓ {}  ↑ {}",
                                                    crate::rate(s.rx),
                                                    crate::rate(s.tx)
                                                ),
                                            ),
                                        })
                                    })
                                    .text_size(px(12.))
                                    .text_color(rgb(MUTED)),
                                ),
                        )
                        .child(text("›").text_size(px(24.)).text_color(rgb(MUTED)))
                })),
        )
}
fn device(data: Signal<Sample>, extras: Signal<Extras>) -> impl IntoElement {
    view().gap_4()
        .child(view().panel().rounded(px(8.)).p_4().gap_3()
            .child(title("This device",0xeeeef1))
            .child(text(move||extras.with(|e|e.device_model.clone().unwrap_or_else(||"This device".into()))).text_size(px(16.)))
            .child(text(move||extras.with(|e|e.os_version.clone().unwrap_or_else(crate::system_name))).text_size(px(12.)).text_color(rgb(MUTED)))
            .child(crate::row("Architecture",std::env::consts::ARCH))
            .child(text(move||data.with(|s|format!("{} logical cores · {} physical memory",s.cores,reading(s.total>0,gb(s.total))))).text_size(px(12.)).text_color(rgb(MUTED)))
            .child(text(move||data.with(|s|format!("Storage available  {}",reading(s.disk_available,gb(s.free))))).text_size(px(12.)).text_color(rgb(MUTED))))
         .child(view().panel().rounded(px(8.)).p_4().gap_3()
            .child(title("Battery",COLORS[7]))
            .child(motion::metric(move||extras.with(|e|e.battery.as_ref().map(|b|format!("{:.0} %",b.percent)).unwrap_or_else(||"Unavailable".into()))))
            .child(text(move||extras.with(|e|e.battery.as_ref().map(|b|b.status.clone()).unwrap_or_else(||"No accessible battery reading".into()))).text_size(px(13.)).text_color(rgb(MUTED))))
        .child(text(move||extras.with(|e|e.battery_status.clone())).text_size(px(11.)).text_color(rgb(MUTED)))
        .child(dynamic(move|| match extras.with(|e|e.sound.clone()) {
            Some(output)=>view().panel().rounded(px(8.)).p_4().gap_2()
                .child(title("App audio output",0xeeeef1)).child(text(output).text_size(px(14.))).into_element(),
            None=>view().into_element(),
        }))
}
fn about(paused: Signal<bool>) -> impl IntoElement {
    view().gap_4()
        .child(view().panel().rounded(px(8.)).p_4().gap_3()
            .child(title("sysmon",COLORS[0]))
            .child(crate::row("Version",env!("CARGO_PKG_VERSION")))
            .child(crate::row("Sampling interval","2 seconds"))
            .child(crate::row("History","Latest 60 samples · kept in memory"))
            .child(label("Readings are kept on your device and discarded when the app closes. No account, analytics or cloud service is used.")))
        .child(view().panel().rounded(px(8.)).p_4().gap_3()
            .child(title("Sampling",0xeeeef1))
            .child(label("Pause holds the current readings and history. Resume continues collecting; historical gaps keep their actual duration."))
            .child(view().min_h(px(48.)).rounded(px(10.)).bg(rgb(0x243448)).items_center().justify_center()
                .test_id("mobile-about-sampling").role(AccessibilityRole::Button).label(move||if paused.get(){"Resume readings"}else{"Pause readings"})
                .on_press(move||paused.update(|p|*p=!*p))
                .child(text(move||if paused.get(){"Resume readings"}else{"Pause readings"}).text_color(rgb(0x9fcaff)).text_size(px(15.)))))
        .child(scope_note())
}
