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
const FOREGROUND: u32 = 0xeeeef1;
const LINK: u32 = 0x9fcaff;

#[derive(Clone, Copy)]
struct MobileSignals {
    section: Signal<u32>,
    // 0 is no resource detail; 1..=4 are actual resource destinations.
    detail: Signal<usize>,
    data: Signal<Sample>,
    extras: Signal<Extras>,
    paused: Signal<bool>,
    ready: Signal<bool>,
}

pub fn shell(
    data: Signal<Sample>,
    extras: Signal<Extras>,
    paused: Signal<bool>,
    ready: Signal<bool>,
) -> impl IntoElement {
    let signals = MobileSignals {
        section: signal(0_u32),
        detail: signal(0_usize),
        data,
        extras,
        paused,
        ready,
    };
    install_navigation(signals);

    view()
        // Native navigation owns the top inset; the TabBar owns the bottom inset.
        .px_safe_area()
        .flex_1()
        .w_full()
        .h_full()
        .bg(rgb(crate::BG))
        .text_color(rgb(FOREGROUND))
        .child(navigation(signals))
        .child(tab_bar_for(signals.section, signals.detail))
}

fn install_navigation(signals: MobileSignals) {
    effect(move || {
        if signals.detail.get() > 0 {
            signals.section.set(1);
        }
    });
    on_back_press(move || {
        // The native stack owns detail back presses and interactive gestures.
        if signals.detail.get_untracked() != 0 {
            false
        } else if signals.section.get_untracked() != 0 {
            signals.section.set(0);
            true
        } else {
            false
        }
    });
}

fn navigation(signals: MobileSignals) -> impl IntoElement {
    nsh_base::nav_stack()
        .flex_1()
        .min_h(px(0.))
        .w_full()
        .child(root_page(signals))
        .child(resource_page(signals.data, signals.detail, signals.paused))
}

fn root_page(signals: MobileSignals) -> Screen {
    page("mobile-root", move || {
        SECTIONS[signals.section.get() as usize]
    })
    .header_back_title("")
    .child(scroll_page(root_content(signals)))
}

fn resource_page(
    data: Signal<Sample>,
    detail: Signal<usize>,
    paused: Signal<bool>,
) -> impl IntoElement {
    when(
        move || detail.get() > 0,
        move || {
            let resource = detail.get_untracked();
            page(
                format!("mobile-resource-{resource}"),
                resource_title(resource),
            )
            .on_dismiss(move || detail.set(0))
            .child(scroll_page(
                view()
                    .gap_3()
                    .child(live_views::detail(data, resource, paused)),
            ))
        },
    )
}

fn page(key: impl Into<SharedString>, title: impl IntoReactive<SharedString>) -> Screen {
    nsh_base::nav_page()
        .key(key)
        .title(title)
        .header_visible(true)
        .header_large_title(false)
        .header_style_with(|s| s.text_color(rgb(FOREGROUND)).tint_color(rgb(FOREGROUND)))
        .status_bar(nsh_core::StatusBarStyle::Light)
}

fn scroll_page(content: impl IntoElement) -> Scroll {
    scroll()
        .flex_1()
        .min_h(px(0.))
        .child(view().px_4().pt_2().pb_5().child(content))
}

fn root_content(signals: MobileSignals) -> impl IntoElement {
    dynamic(move || {
        if signals.ready.get() {
            section_view(signals, signals.section.get())
        } else {
            loading().into_element()
        }
    })
}

fn section_view(signals: MobileSignals, section: u32) -> AnyElement {
    match section {
        0 => live_views::mobile_overview(
            signals.data,
            signals.extras,
            signals.detail,
            signals.paused,
        )
        .into_element(),
        1 => activity(signals.data, signals.detail).into_element(),
        2 => device(signals.data, signals.extras).into_element(),
        _ => about(signals.paused).into_element(),
    }
}

fn loading() -> impl IntoElement {
    view().py_6().child(label("Reading your device…"))
}

fn tab_bar_for(section: Signal<u32>, detail: Signal<usize>) -> impl IntoElement {
    tab_bar()
        .items(tab_items())
        .selected_index(section)
        .on_change(move |index: &u32| {
            if *index < SECTIONS.len() as u32 {
                detail.set(0);
                section.set(*index);
            }
        })
}

fn tab_items() -> Vec<TabBarItem> {
    SECTIONS
        .into_iter()
        .zip(tab_icons())
        .map(|(name, icon)| TabBarItem::new(name).icon(icon))
        .collect()
}

fn tab_icons() -> [&'static str; 4] {
    if cfg!(target_os = "android") {
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
    }
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
                .children((1..=4).map(move |index| resource_row(data, detail, index))),
        )
}

fn resource_row(data: Signal<Sample>, detail: Signal<usize>, index: usize) -> impl IntoElement {
    view()
        .min_h(px(72.))
        .px_4()
        .py_3()
        .flex_row()
        .items_center()
        .gap_3()
        .test_id(&format!("mobile-resource-{index}"))
        .label(resource_title(index))
        .role(AccessibilityRole::Button)
        .on_press(move || detail.set(index))
        .child(visuals::badge_icon(index, COLORS[index]))
        .child(resource_details(data, index))
        .child(chevron())
}

fn resource_details(data: Signal<Sample>, index: usize) -> impl IntoElement {
    view()
        .flex_1()
        .gap_1()
        .child(title(resource_title(index), FOREGROUND))
        .child(
            text(move || data.with(|s| resource_reading(s, index)))
                .text_size(px(12.))
                .text_color(rgb(MUTED)),
        )
}

fn resource_title(resource: usize) -> &'static str {
    if resource == 3 {
        "Storage"
    } else {
        crate::TABS[resource]
    }
}

fn resource_reading(sample: &Sample, index: usize) -> String {
    match index {
        1 => reading(sample.cpu_available, format!("{:.1}%", sample.cpu)),
        2 => reading(
            sample.memory_available,
            format!("{} · resident", gb(sample.memory)),
        ),
        3 => reading(
            sample.disk_available,
            format!("{} available", gb(sample.free)),
        ),
        _ => reading(
            sample.network_available,
            format!("↓ {}  ↑ {}", crate::rate(sample.rx), crate::rate(sample.tx)),
        ),
    }
}

fn chevron() -> impl IntoElement {
    text("›").text_size(px(24.)).text_color(rgb(MUTED))
}

fn device(data: Signal<Sample>, extras: Signal<Extras>) -> impl IntoElement {
    view()
        .gap_4()
        .child(device_summary(data, extras))
        .child(battery_panel(extras))
        .child(battery_note(extras))
        .child(sound_panel(extras))
}

fn device_summary(data: Signal<Sample>, extras: Signal<Extras>) -> impl IntoElement {
    card()
        .child(title("This device", FOREGROUND))
        .child(text(move || extras.with(device_model)).text_size(px(16.)))
        .child(dim_text(move || extras.with(os_version)))
        .child(crate::row("Architecture", std::env::consts::ARCH))
        .child(dim_text(move || data.with(core_summary)))
        .child(dim_text(move || data.with(storage_summary)))
}

fn battery_panel(extras: Signal<Extras>) -> impl IntoElement {
    card()
        .child(title("Battery", COLORS[7]))
        .child(motion::metric(move || extras.with(battery_percent)))
        .child(dim_text_size(move || extras.with(battery_status), 13.))
}

fn battery_note(extras: Signal<Extras>) -> impl IntoElement {
    text(move || extras.with(|e| e.battery_status.clone()))
        .text_size(px(11.))
        .text_color(rgb(MUTED))
}

fn sound_panel(extras: Signal<Extras>) -> impl IntoElement {
    dynamic(move || match extras.with(|e| e.sound.clone()) {
        Some(output) => card()
            .child(title("App audio output", FOREGROUND))
            .child(text(output).text_size(px(14.)))
            .into_element(),
        None => view().into_element(),
    })
}

fn device_model(extras: &Extras) -> String {
    extras
        .device_model
        .clone()
        .unwrap_or_else(|| "This device".into())
}

fn os_version(extras: &Extras) -> String {
    extras.os_version.clone().unwrap_or_else(crate::system_name)
}

fn core_summary(sample: &Sample) -> String {
    format!(
        "{} logical cores · {} physical memory",
        sample.cores,
        reading(sample.total > 0, gb(sample.total))
    )
}

fn storage_summary(sample: &Sample) -> String {
    format!(
        "Storage available  {}",
        reading(sample.disk_available, gb(sample.free))
    )
}

fn battery_percent(extras: &Extras) -> String {
    extras
        .battery
        .as_ref()
        .map(|b| format!("{:.0} %", b.percent))
        .unwrap_or_else(|| "Unavailable".into())
}

fn battery_status(extras: &Extras) -> String {
    extras
        .battery
        .as_ref()
        .map(|b| b.status.clone())
        .unwrap_or_else(|| "No accessible battery reading".into())
}

fn about(paused: Signal<bool>) -> impl IntoElement {
    view()
        .gap_4()
        .child(about_panel())
        .child(sampling_panel(paused))
        .child(scope_note())
}

fn about_panel() -> impl IntoElement {
    card()
        .child(title("sysmon", COLORS[0]))
        .child(crate::row("Version", env!("CARGO_PKG_VERSION")))
        .child(crate::row("Sampling interval", "2 seconds"))
        .child(crate::row("History", "Latest 60 samples · kept in memory"))
        .child(label(
            "Readings are kept on your device and discarded when the app closes. No account, analytics or cloud service is used.",
        ))
}

fn sampling_panel(paused: Signal<bool>) -> impl IntoElement {
    card()
        .child(title("Sampling", FOREGROUND))
        .child(label(
            "Pause holds the current readings and history. Resume continues collecting; historical gaps keep their actual duration.",
        ))
        .child(sampling_button(paused))
}

fn sampling_button(paused: Signal<bool>) -> impl IntoElement {
    view()
        .min_h(px(48.))
        .rounded(px(10.))
        .bg(rgb(0x243448))
        .items_center()
        .justify_center()
        .test_id("mobile-about-sampling")
        .role(AccessibilityRole::Button)
        .label(move || sampling_action(paused))
        .on_press(move || paused.update(|p| *p = !*p))
        .child(
            text(move || sampling_action(paused))
                .text_color(rgb(LINK))
                .text_size(px(15.)),
        )
}

fn sampling_action(paused: Signal<bool>) -> &'static str {
    if paused.get() {
        "Resume readings"
    } else {
        "Pause readings"
    }
}

pub fn scope_note() -> impl IntoElement {
    view()
        .bg(rgb(0x171c25))
        .rounded(px(8.))
        .p_3()
        .gap_2()
        .child(title("Data scope", 0x8eb9f1))
        .child(label(
            "CPU and resident memory describe sysmon. Storage and network describe what this app can access. The OS protects other apps; their activity is not shown.",
        ))
}

fn card() -> View {
    view().panel().rounded(px(8.)).p_4().gap_3()
}

fn dim_text(content: impl Fn() -> String + 'static) -> impl IntoElement {
    dim_text_size(content, 12.)
}

fn dim_text_size(content: impl Fn() -> String + 'static, size: f32) -> impl IntoElement {
    text(content).text_size(px(size)).text_color(rgb(MUTED))
}
