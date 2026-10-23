//! Retained real-time charts. Geometry uses native animation slots: samples do
//! not rebuild view trees and the Rust engine does no work on animation frames.
use nsh_core::{
    animation::Easing,
    canvas::{CanvasGradient, CanvasTextAlign, LineCap, LineJoin},
    ui::prelude::*,
};
use std::{cell::RefCell, rc::Rc};

const CAPACITY: usize = 60;
const MUTED: u32 = 0x77777f;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotKind {
    Line,
    Memory,
    Bars,
}

fn visible_samples(values: &[f32]) -> Vec<f32> {
    values[values.len().saturating_sub(CAPACITY)..]
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(0., 100.))
        .collect()
}

fn y_domain(values: &[f32], kind: PlotKind) -> (f32, f32) {
    if kind != PlotKind::Memory || values.is_empty() {
        return (0., 100.);
    }
    let min = values.iter().copied().fold(100_f32, f32::min);
    let max = values.iter().copied().fold(0_f32, f32::max);
    let span = ((max - min) * 1.3).clamp(1., 100.);
    let low = ((min + max) * 0.5 - span * 0.5).clamp(0., 100. - span);
    (low, low + span)
}
fn axis_value(value: f32, span: f32) -> String {
    if span <= 2. {
        format!("{value:.2}")
    } else {
        format!("{value:.0}")
    }
}

/// The chart grows with observed history, instead of implying an unseen
/// two-minute past. Seven intervals provide a stable minimum 14-second view.
fn horizon_samples(count: usize) -> usize {
    count.saturating_sub(1).clamp(7, CAPACITY - 1)
}
fn time_label(seconds: usize) -> String {
    match seconds {
        0 => "now".into(),
        n if n % 60 == 0 => format!("−{}m", n / 60),
        n if n >= 60 => format!("−{}m{}s", n / 60, n % 60),
        n => format!("−{n}s"),
    }
}
fn slot_x(slot: usize, left: f32, right: f32, horizon: usize) -> f32 {
    right - (CAPACITY - 1 - slot.min(CAPACITY - 1)) as f32 * (right - left) / horizon as f32
}
fn slot_at_fraction(fraction: f32, horizon: usize) -> usize {
    ((CAPACITY - 1) as f32 - (1. - fraction.clamp(0., 1.)) * horizon as f32)
        .round()
        .clamp(0., (CAPACITY - 1) as f32) as usize
}

fn fallback_width(compact: bool) -> f32 {
    let screen = window().width.0;
    let body =
        screen - if screen >= 1000. { 168. } else { 0. } - if screen < 600. { 24. } else { 40. };
    if compact {
        let card = if body < 592. {
            body
        } else {
            ((body - 12.) / 2.25).max(280.)
        };
        ((card - 24.) * 0.48).max(80.)
    } else {
        let interior = body - 32.;
        if interior >= 396. {
            interior - 176.
        } else {
            interior
        }
        .max(100.)
    }
}

/// `values` are percentages. For Bars, the caller supplies values relative to
/// its actual current peak; axis wording explicitly identifies that scale.
/// The horizontal domain starts with observed launch history, grows from a
/// minimum 14 seconds, and retains at most two minutes. No samples are invented.
/// Keep this mounted across sampling updates; update the signal in place.
pub fn plot(
    values: Signal<Vec<f32>>,
    color: u32,
    kind: PlotKind,
    compact: bool,
) -> impl IntoElement {
    let initial = visible_samples(&values.get_untracked());
    let initial_domain = y_domain(&initial, kind);
    let initial_targets: [f32; CAPACITY] = std::array::from_fn(|i| {
        let start = CAPACITY - initial.len();
        if i >= start { initial[i - start] } else { 0. }
    });
    let samples = signal(initial);
    let measured_width = signal(0_f32);
    let cursor = signal(None::<usize>);
    let slots: Rc<[AnimatedValue; CAPACITY]> =
        Rc::new(std::array::from_fn(|i| animated_value(initial_targets[i])));
    let last_targets = Rc::new(RefCell::new(initial_targets));
    let domain_low = animated_value(initial_domain.0);
    let domain_high = animated_value(initial_domain.1);
    let prior_domain = Rc::new(RefCell::new(initial_domain));
    let beat = animated_value(1.);
    let update_slots = slots.clone();
    effect(move || {
        let next = visible_samples(&values.get());
        let reduced = reduce_motion();
        let domain = y_domain(&next, kind);
        if reduced {
            domain_low.set(domain.0);
            domain_high.set(domain.1);
        } else if *prior_domain.borrow() != domain {
            let _ = domain_low.timing(domain.0, 520., Easing::EaseOut);
            let _ = domain_high.timing(domain.1, 520., Easing::EaseOut);
        }
        *prior_domain.borrow_mut() = domain;

        let mut previous = last_targets.borrow_mut();
        let start = CAPACITY - next.len();
        let changed = next != samples.get_untracked();
        for i in 0..CAPACITY {
            let target = if i >= start { next[i - start] } else { 0. };
            if reduced {
                // Also stop an in-flight transition when the OS preference
                // changes while its target stays identical.
                update_slots[i].set(target);
            } else if previous[i] != target {
                let _ = update_slots[i].timing(target, 520., Easing::EaseOut);
            }
            previous[i] = target;
        }
        if changed && !next.is_empty() && !reduced {
            beat.set(0.);
            let _ = beat.timing(1., 900., Easing::EaseOut);
        }
        samples.set(next);
    });
    let draw_slots = slots.clone();
    let drawing = canvas(move || {
        let observed = samples.get();
        let count = observed.len();
        let horizon = horizon_samples(count);
        let (domain_min, domain_max) = y_domain(&observed, kind);
        let domain_span = domain_max - domain_min;

        let w = if measured_width.get() > 0. {
            measured_width.get()
        } else {
            fallback_width(compact)
        };
        let h = if compact {
            94.
        } else if window().width.0 < 600. {
            154.
        } else {
            176.
        };
        let left = if compact {
            if kind == PlotKind::Bars { 8. } else { 2. }
        } else {
            31.
        };
        let right = (w - 8.).max(left + 2.);
        let top = if compact { 9. } else { 20. };
        let bottom = h - 19.;
        let plot_h = bottom - top;
        let plot_w = right - left;
        let accent = rgb(color).to_hsla();
        let mut p = Painter::new().font(if compact {
            "9px system-ui"
        } else {
            "10px system-ui"
        });
        // A restrained dotted grid leaves the readings and trace dominant.
        for fraction in [0_f32, 0.5, 1.] {
            let y = top + plot_h * fraction;
            p = p
                .stroke_color(rgb(if fraction == 1. { 0x36363b } else { 0x2b2b30 }))
                .line_width(0.7)
                .line_dash(&[px(1.), px(4.)])
                .begin_path()
                .move_to(left, y)
                .line_to(right, y)
                .stroke();
            if !compact {
                let label = if kind == PlotKind::Bars {
                    if fraction == 0. {
                        "peak".into()
                    } else if fraction == 1. {
                        "0".into()
                    } else {
                        String::new()
                    }
                } else {
                    axis_value(domain_max - domain_span * fraction, domain_span)
                };
                p = p.fill_color(rgb(MUTED)).fill_text(&label, 2., y + 3.);
            }
        }
        p = p.line_dash(&[]).fill_color(rgb(MUTED));
        p = p
            .fill_text(&time_label(horizon * 2), left, h - 3.)
            .text_align(CanvasTextAlign::Right)
            .fill_text("now", right, h - 3.)
            .text_align(CanvasTextAlign::Left);
        if !compact {
            p = p
                .text_align(CanvasTextAlign::Center)
                .fill_text(&time_label(horizon), left + plot_w * 0.5, h - 3.)
                .text_align(CanvasTextAlign::Left);
        }
        if compact && kind == PlotKind::Memory && count > 0 {
            p = p.font("8px system-ui").fill_color(rgb(MUTED)).fill_text(
                &format!(
                    "range {}–{}%",
                    axis_value(domain_min, domain_span),
                    axis_value(domain_max, domain_span)
                ),
                left + 2.,
                7.,
            );
        }
        if count == 0 {
            return p
                .fill_color(rgb(0x8b8b94))
                .font(if compact {
                    "9px system-ui"
                } else {
                    "11px system-ui"
                })
                .fill_text(
                    if compact {
                        "No history"
                    } else {
                        "No samples available"
                    },
                    left + 4.,
                    top + plot_h * 0.6,
                );
        }
        let first = CAPACITY - count;
        let step = plot_w / horizon as f32;
        let xy: Vec<_> = (first..CAPACITY)
            .map(|i| {
                let span = domain_high.sub(&domain_low);
                let fraction = draw_slots[i].sub(&domain_low).div(&span);
                let y = fraction.interpolate(&[0., 1.], &[bottom, top]);
                (slot_x(i, left, right, horizon), y)
            })
            .collect();
        let fill =
            CanvasGradient::linear(Point::new(px(0.), px(top)), Point::new(px(0.), px(bottom)))
                .add_color_stop(Percentage(0.), accent.opacity(0.27))
                .add_color_stop(Percentage(0.7), accent.opacity(0.07))
                .add_color_stop(Percentage(1.), accent.opacity(0.008));
        match kind {
            PlotKind::Line | PlotKind::Memory => {
                p = p.fill_style(fill).begin_path().move_to(xy[0].0, bottom);
                for (x, y) in &xy {
                    p = p.line_to(*x, y);
                }
                p = p.line_to(xy.last().unwrap().0, bottom).close_path().fill();
                // A broad low-alpha under-stroke adds optical softness without
                // expensive blur layers or a different, fabricated curve.
                for (width, alpha) in [(5_f32, 0.08_f32), (1.75, 0.98)] {
                    p = p
                        .stroke_color(accent.opacity(alpha))
                        .line_width(width)
                        .line_cap(LineCap::Round)
                        .line_join(LineJoin::Round)
                        .begin_path();
                    for (i, (x, y)) in xy.iter().enumerate() {
                        p = if i == 0 {
                            p.move_to(*x, y)
                        } else {
                            p.line_to(*x, y)
                        };
                    }
                    p = p.stroke();
                }
                let (x, y) = xy.last().unwrap();
                // A single native pulse per new sample settles to stillness;
                // it does not keep an idle monitor redrawing indefinitely.
                if !reduce_motion() {
                    let ring = beat.interpolate(&[0., 1.], &[3., 9.]);
                    let alpha = beat.interpolate(&[0., 1.], &[0.32, 0.]);
                    p = p
                        .save()
                        .global_alpha(&alpha)
                        .stroke_color(accent)
                        .line_width(1.)
                        .begin_path()
                        .arc(*x, y, &ring, 0., std::f32::consts::TAU)
                        .stroke()
                        .restore();
                }
                p = p
                    .fill_color(rgb(0x141416))
                    .begin_path()
                    .arc(*x, y, 4., 0., std::f32::consts::TAU)
                    .fill()
                    .fill_color(accent)
                    .begin_path()
                    .arc(*x, y, 2.5, 0., std::f32::consts::TAU)
                    .fill();
            }
            PlotKind::Bars => {
                let bar_w = (step * 0.57).clamp(1.3, if compact { 12. } else { 16. });
                for (i, (x, y)) in xy.iter().enumerate() {
                    let alpha = if i + 1 == xy.len() { 1. } else { 0.62 };
                    let opacity =
                        draw_slots[first + i].interpolate(&[0., 0.05, 100.], &[0., alpha, alpha]);
                    p = p
                        .save()
                        .global_alpha(&opacity)
                        .stroke_color(accent)
                        .line_width(bar_w)
                        .line_cap(LineCap::Round)
                        .begin_path()
                        .move_to(*x, bottom - bar_w * 0.5)
                        .line_to(*x, y)
                        .stroke()
                        .restore();
                }
            }
        }
        let selected = cursor.get().map(|i| i.clamp(first, CAPACITY - 1));
        if let Some(index) = selected {
            let (x, y) = &xy[index - first];
            p = p
                .stroke_color(accent.opacity(0.55))
                .line_width(0.8)
                .line_dash(&[px(2.), px(3.)])
                .begin_path()
                .move_to(*x, top)
                .line_to(*x, bottom)
                .stroke()
                .line_dash(&[])
                .fill_color(rgb(0x151519))
                .begin_path()
                .arc(*x, y, 4.5, 0., std::f32::consts::TAU)
                .fill()
                .stroke_color(accent)
                .line_width(1.5)
                .begin_path()
                .arc(*x, y, 3., 0., std::f32::consts::TAU)
                .stroke();
            let value = observed[index - first];
            let text = if kind == PlotKind::Bars {
                format!("{value:.0}% of peak")
            } else {
                format!("{value:.1}%")
            };
            let age = (CAPACITY - 1 - index) * 2;
            if compact {
                let readout = if kind == PlotKind::Bars {
                    format!("{value:.0}% peak")
                } else {
                    text
                };
                p = p
                    .fill_color(rgb(0x18181b).opacity(0.96))
                    .fill_rect(left, 0., plot_w, 16.)
                    .font("9px system-ui")
                    .fill_color(rgb(0xeeeef4))
                    .fill_text(&readout, left + 3., 11.);
            } else {
                p = p
                    .font("10px system-ui")
                    .fill_color(rgb(0xeeeef4))
                    .fill_text(&format!("{text}  ·  {age}s ago"), left + 3., top - 6.);
            }
        } else if !compact {
            p = p.font("10px system-ui").fill_color(accent).fill_text(
                if kind == PlotKind::Bars {
                    "RELATIVE TRANSFER"
                } else if kind == PlotKind::Memory {
                    "WINDOWED MEMORY %"
                } else {
                    "UTILIZATION %"
                },
                left + 3.,
                top - 6.,
            );
        }
        p
    })
    .test_id(&format!(
        "chart-{}-{}",
        match kind {
            PlotKind::Line => "line",
            PlotKind::Memory => "memory",
            PlotKind::Bars => "bars",
        },
        if compact { "compact" } else { "detail" }
    ))
    .label("Live resource history chart")
    .w_full()
    .h(move || {
        px(if compact {
            94.
        } else if window().width.0 < 600. {
            154.
        } else {
            176.
        })
    })
    .on_layout(move |bounds: &Bounds<Points>| {
        measured_width.try_set(bounds.size.width.0);
    })
    .on_mouse_move(move |point: &Point<Points>| {
        // Native pointer/exit events already queued before a page transition
        // can arrive after the canvas owner is gone. Ignore only those events.
        let (Some(measured), Some(observed)) = (measured_width.try_get(), samples.try_get()) else {
            return;
        };
        let w = if measured > 0. {
            measured
        } else {
            fallback_width(compact)
        };
        let left = if compact {
            if kind == PlotKind::Bars { 8. } else { 2. }
        } else {
            31.
        };
        let fraction = ((point.x.0 - left) / (w - 8. - left).max(1.)).clamp(0., 1.);
        cursor.try_set(Some(slot_at_fraction(
            fraction,
            horizon_samples(observed.len()),
        )));
    })
    .on_hover_out(move || {
        cursor.try_set(None);
    });
    view().w_full().child(drawing)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_actual_finite_samples_are_retained() {
        assert_eq!(
            visible_samples(&[f32::NAN, 23., 140., -3.]),
            vec![23., 100., 0.]
        );
        assert_eq!(
            visible_samples(&(0..100).map(|v| v as f32).collect::<Vec<_>>()).len(),
            CAPACITY
        );
    }
    #[test]
    fn memory_domain_is_padded_bounded_and_explicitly_zoomed() {
        assert_eq!(y_domain(&[0.12, 0.18], PlotKind::Memory), (0., 1.));
        let (low, high) = y_domain(&[71., 72.], PlotKind::Memory);
        assert!(low < 71. && high > 72.);
        assert!(high - low >= 1.);
        let (low, high) = y_domain(&[99.9, 100.], PlotKind::Memory);
        assert!(low >= 0. && high <= 100. && high - low >= 1.);
        assert_eq!(y_domain(&[71., 72.], PlotKind::Line), (0., 100.));
    }
    #[test]
    fn time_domain_grows_only_with_actual_samples() {
        assert_eq!(horizon_samples(0), 7);
        assert_eq!(horizon_samples(1), 7);
        assert_eq!(horizon_samples(8), 7);
        assert_eq!(horizon_samples(11), 10);
        assert_eq!(horizon_samples(60), 59);
        assert_eq!(horizon_samples(100), 59);
        assert_eq!(time_label(horizon_samples(8) * 2), "−14s");
        assert_eq!(time_label(horizon_samples(11) * 2), "−20s");
        assert_eq!(time_label(horizon_samples(60) * 2), "−1m58s");
        assert_eq!(time_label(60), "−1m");
    }
    #[test]
    fn adaptive_geometry_and_hover_use_the_same_real_time_domain() {
        let horizon = horizon_samples(8);
        let first = CAPACITY - 8;
        assert_eq!(slot_x(first, 10., 150., horizon), 10.);
        assert_eq!(slot_x(59, 10., 150., horizon), 150.);
        assert_eq!(slot_at_fraction(0., horizon), first);
        assert_eq!(slot_at_fraction(1., horizon), 59);
        assert_eq!(slot_x(58, 10., 150., horizon_samples(2)), 130.);
        let full = horizon_samples(60);
        assert_eq!(slot_x(0, 10., 150., full), 10.);
    }
}
