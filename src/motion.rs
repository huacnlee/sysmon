//! Small, native-driven motions. No Rust frame timers or per-frame tree rebuilds.
use nsh_core::ui::{LayoutAnimation, prelude::*};

pub fn enter<S: Styled + Element>(element: S, delay: f32) -> S {
    let progress = animated_value(if reduce_motion() { 1_f32 } else { 0_f32 });
    if !reduce_motion() {
        let _ = progress.timing_with(
            1.,
            Timing {
                duration_ms: 360.,
                delay_ms: delay,
                easing: Easing::EaseOut,
            },
        );
    }
    element
        .bind_opacity(&progress)
        .bind_translate_y(&progress.interpolate(&[0., 1.], &[12., 0.]))
        .bind_scale(&progress.interpolate(&[0., 1.], &[0.985, 1.]))
}
pub fn interactive<S: Styled + Interactive>(element: S) -> S {
    element
        .transition()
        .duration(move || if reduce_motion() { 0_f32 } else { 160_f32 })
        .ease_out()
        .hover(|s| s.scale(move || if reduce_motion() { 1_f32 } else { 1.008_f32 }))
        .active(|s| s.scale(move || if reduce_motion() { 1_f32 } else { 0.982_f32 }))
}
pub fn layout() {
    if !reduce_motion() {
        animate_layout(LayoutAnimation::EASE_IN_EASE_OUT);
    }
}

/// Keep text nodes mounted; only changed readings receive a short native lift.
pub fn metric(read: impl Fn() -> String + 'static) -> impl IntoElement {
    let target = computed(read);
    let previous = stored(String::new());
    let opacity = animated_value(1.);
    let lift = animated_value(0.);
    effect(move || {
        let next = target.get();
        let old = previous.get();
        let reduced = reduce_motion();
        if reduced {
            opacity.set(1.);
            lift.set(0.);
        } else if !old.is_empty() && old != next {
            opacity.set(0.35);
            lift.set(4.);
            let _ = Animation::parallel([
                Animation::timing(&opacity, 1., Timing::new(220., Easing::EaseOut)),
                Animation::spring(
                    &lift,
                    0.,
                    Spring {
                        stiffness: 260.,
                        damping: 28.,
                        ..Default::default()
                    },
                ),
            ])
            .start();
        }
        previous.replace(next);
    });
    let number = move || split_unit(&target.get()).0;
    let unit = move || split_unit(&target.get()).1;
    view()
        .flex_row()
        .items_baseline()
        .gap_1()
        .py_1()
        .bind_opacity(&opacity)
        .bind_translate_y(&lift)
        .child(
            text(number)
                .text_size(move || {
                    px(if target.get() == "Unavailable" {
                        16.
                    } else {
                        28.
                    })
                })
                .font_weight(FontWeight::MEDIUM)
                .tabular_nums(),
        )
        .child(text(unit).text_size(px(12.)).text_color(rgb(crate::MUTED)))
}
fn split_unit(text: &str) -> (String, String) {
    match text.rsplit_once(' ') {
        Some((n, unit)) if matches!(unit, "%" | "GB" | "MB" | "kB/s" | "MB/s" | "W") => {
            (n.into(), unit.into())
        }
        _ => (text.into(), String::new()),
    }
}
pub fn live_status(paused: Signal<bool>, compact: bool) -> impl IntoElement {
    let value = animated_value(1.);
    let cycle = Animation::sequence([
        Animation::timing(&value, 0.74, Timing::new(380., Easing::EaseInOut)),
        Animation::timing(&value, 1., Timing::new(380., Easing::EaseInOut)),
    ]);
    effect(move || {
        cycle.stop();
        if !paused.get() && !reduce_motion() {
            let _ = cycle.start();
        } else {
            value.set(1.);
        }
    });
    text(move || {
        if paused.get() {
            "Ⅱ Paused"
        } else if compact && window().width.0 < 600. {
            "● Live"
        } else {
            "● Live · 2s"
        }
    })
    .text_size(px(11.))
    .text_color(move || rgb(if paused.get() { crate::MUTED } else { 0x49b47b }))
    .bind_opacity(&value)
}
