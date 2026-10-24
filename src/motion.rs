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
        .hover(|s| s.border_color(rgb(0x45454b)))
        .active(|s| s.scale(move || if reduce_motion() { 1_f32 } else { 0.982_f32 }))
}
pub fn layout() {
    if !reduce_motion() {
        animate_layout(LayoutAnimation::EASE_IN_EASE_OUT);
    }
}

/// A retained odometer. Each digit is a clipped reel moved by the native
/// animation driver; sampling never starts a Rust frame timer. The numeric
/// structure is memoized, so decimal points, units and unchanged reels stay put.
pub fn metric(read: impl Fn() -> String + 'static) -> impl IntoElement {
    metric_sized(read, MetricStyle::LARGE)
}

/// Compact readings for persistent title bars and dense inline status rows.
pub fn small_metric(read: impl Fn() -> String + 'static) -> impl IntoElement {
    metric_sized(read, MetricStyle::SMALL)
}

#[derive(Clone, Copy)]
struct MetricStyle {
    font: f32,
    height: f32,
    width: f32,
    unit: f32,
    compact: bool,
}
impl MetricStyle {
    const LARGE: Self = Self {
        font: 28.,
        height: 34.,
        width: 18.,
        unit: 12.,
        compact: false,
    };
    const SMALL: Self = Self {
        font: 12.,
        height: 16.,
        width: 8.,
        unit: 10.,
        compact: true,
    };
}

fn metric_sized(read: impl Fn() -> String + 'static, style: MetricStyle) -> impl IntoElement {
    let target = computed(read);
    let number = computed(move || split_unit(&target.get()).0);
    let shape = computed(move || number_shape(&number.get()));
    view()
        .flex_row()
        .items_end()
        .gap(px(if style.compact { 2. } else { 4. }))
        .py(px(if style.compact { 0. } else { 4. }))
        .accessible(true)
        .label(move || target.get())
        .child(dynamic(move || {
            let shape = shape.get();
            let Some(shape) = shape else {
                return text(move || number.get())
                    .text_size(px(if style.compact { style.font } else { 16. }))
                    .line_height(px(style.height))
                    .font_weight(FontWeight::MEDIUM)
                    .accessible(false)
                    .into_element();
            };
            view()
                .flex_row()
                .h(px(style.height))
                .items_start()
                .accessible(false)
                .children(shape.chars().enumerate().map(move |(index, ch)| {
                    if ch == '#' {
                        digit_reel(number, index, style).into_element()
                    } else {
                        text(ch.to_string())
                            .text_size(px(style.font))
                            .line_height(px(style.height))
                            .font_weight(FontWeight::MEDIUM)
                            .tabular_nums()
                            .accessible(false)
                            .into_element()
                    }
                }))
                .into_element()
        }))
        .child(
            text(move || split_unit(&target.get()).1)
                .text_size(px(style.unit))
                .pb(px(if style.compact { 0. } else { 3. }))
                .text_color(rgb(crate::MUTED))
                .accessible(false),
        )
}

// Every update moves one line, regardless of how far the numeric value jumps.
// Showing only old/new digits avoids a slot-machine sweep through intermediate
// values, and a shared duration keeps the whole reading legible as it changes.
fn digit_reel(number: Memo<String>, index: usize, style: MetricStyle) -> impl IntoElement {
    let initial = number.get_untracked();
    let previous = stored(initial.clone());
    let initial_digit = digit_at(&initial, index);
    let pair = signal(format!("{initial_digit}\n{initial_digit}"));
    let position = animated_value(0.);
    effect(move || {
        let next = number.get();
        let old = previous.get();
        let from = digit_at(&old, index);
        let to = digit_at(&next, index);
        if reduce_motion() {
            pair.set(format!("{to}\n{to}"));
            position.set(0.);
        } else if from != to {
            let increasing = next.parse::<f64>().unwrap_or(0.) >= old.parse::<f64>().unwrap_or(0.);
            let (digits, start, end) = digit_transition(from, to, increasing);
            // Text and transform are rebased in the same native update: the
            // starting line is exactly the previously displayed digit.
            pair.set(digits);
            position.set(-start * style.height);
            let _ = position.timing(-end * style.height, 260., Easing::EaseOut);
        }
        previous.replace(next);
    });
    view()
        .w(px(style.width))
        .h(px(style.height))
        .flex_shrink_0()
        .overflow_hidden()
        .accessible(false)
        .child(
            text(move || pair.get())
                .w(px(style.width))
                .h(px(style.height * 2.))
                .flex_shrink_0()
                .text_size(px(style.font))
                .line_height(px(style.height))
                .font_weight(FontWeight::MEDIUM)
                .tabular_nums()
                .text_center()
                .accessible(false)
                .bind_translate_y(&position),
        )
}

fn digit_at(number: &str, index: usize) -> u8 {
    number
        .chars()
        .nth(index)
        .and_then(|c| c.to_digit(10))
        .unwrap_or(0) as u8
}

fn number_shape(number: &str) -> Option<String> {
    if number.is_empty()
        || !number.chars().any(|ch| ch.is_ascii_digit())
        || !number
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+'))
    {
        return None;
    }
    Some(
        number
            .chars()
            .map(|ch| if ch.is_ascii_digit() { '#' } else { ch })
            .collect(),
    )
}

/// The old and new glyphs, and their row indices in the two-line strip.
fn digit_transition(from: u8, to: u8, increasing: bool) -> (String, f32, f32) {
    if increasing {
        (format!("{from}\n{to}"), 0., 1.)
    } else {
        (format!("{to}\n{from}"), 1., 0.)
    }
}

fn split_unit(text: &str) -> (String, String) {
    match text.rsplit_once(' ') {
        Some((n, unit))
            if matches!(
                unit,
                "%" | "GB" | "MB" | "B/s" | "kB/s" | "MB/s" | "GB/s" | "W"
            ) =>
        {
            (n.into(), unit.into())
        }
        _ => (text.into(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::{digit_transition, number_shape, split_unit};

    #[test]
    fn changed_digits_move_only_one_line_and_never_show_intermediate_values() {
        for from in 0..10 {
            for to in 0..10 {
                for increasing in [false, true] {
                    let (text, start, end) = digit_transition(from, to, increasing);
                    let lines: Vec<_> = text.lines().collect();
                    assert_eq!(lines.len(), 2);
                    assert_eq!(lines[start as usize], from.to_string());
                    assert_eq!(lines[end as usize], to.to_string());
                    assert_eq!((end - start).abs(), 1.);
                    assert_eq!(end > start, increasing);
                }
            }
        }
    }

    #[test]
    fn structure_tracks_digit_count_and_punctuation_not_readings() {
        assert_eq!(number_shape("18.40"), number_shape("19.35"));
        assert_ne!(number_shape("9.99"), number_shape("10.00"));
        assert_eq!(number_shape("-1.25"), Some("-#.##".into()));
        assert_eq!(number_shape("Unavailable"), None);
        assert_eq!(number_shape(""), None);
        assert_eq!(number_shape("NaN"), None);
        assert_eq!(split_unit("18.40 GB"), ("18.40".into(), "GB".into()));
        assert_eq!(split_unit("42 B/s"), ("42".into(), "B/s".into()));
        assert_eq!(split_unit("2.4 GB/s"), ("2.4".into(), "GB/s".into()));
    }
}
