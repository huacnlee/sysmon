//! App-owned desktop title bar, with native platform window controls.
use nsh_core::{
    desktop::TitleBarStyle,
    ui::{desktop, prelude::*},
};

pub fn install() {
    set_color_scheme(Appearance::Dark);
    effect(|| {
        if is_desktop() {
            desktop::set_title_bar(TitleBarStyle::Hidden, Some(px(36.)));
        }
    });
}

/// Reserve the platform's real control bounds, including full-screen changes.
/// Interactive children are explicitly excluded from the draggable background.
pub fn bar(tab: Signal<usize>, paused: Signal<bool>) -> impl IntoElement {
    view()
        .test_id("sysmon-titlebar")
        .window_drag(true)
        .h(move || px(desktop::title_bar().top.0.max(36.)))
        .pl(move || px(desktop::title_bar().left.0.max(12.) + 8.))
        .pr(move || px(desktop::title_bar().right.0.max(12.) + 8.))
        .bg(rgb(0x101010))
        .border_b_1()
        .border_color(rgb(0x242426))
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(crate::visuals::icon(1, 0xbfc3cb, 15.))
                .child(
                    text("sysmon")
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .child(crate::label("/"))
                .child(
                    text(move || crate::TABS[tab.get()])
                        .text_size(px(12.))
                        .text_color(rgb(crate::MUTED)),
                ),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .window_drag(false)
                .child(crate::motion::live_status(paused, false))
                .child(
                    view()
                        .test_id("sampling-toggle")
                        .label("Toggle sampling")
                        .role(AccessibilityRole::Button)
                        .window_drag(false)
                        .px_3()
                        .py_1()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(rgb(0x333336))
                        .transition()
                        .duration(move || if reduce_motion() { 0_f32 } else { 160_f32 })
                        .ease_out()
                        .hover(|s| s.bg(rgb(0x242426)))
                        .active(|s| s.scale(move || if reduce_motion() { 1_f32 } else { 0.96_f32 }))
                        .on_press(move || paused.update(|v| *v = !*v))
                        .child(
                            text(move || if paused.get() { "Resume" } else { "Pause" })
                                .text_size(px(11.)),
                        ),
                ),
        )
}
