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

/// Units belong to the observation, never to its position in a normalized chart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlotUnits {
    #[default]
    Percent,
    BytesPerSecond,
    StoragePercent,
}
#[derive(Clone, Copy, Default)]
pub struct PlotOptions {
    /// Monotonic seconds, aligned with the supplied values.
    pub timestamps: Option<Signal<Vec<f64>>>,
    pub paused: Option<Signal<bool>>,
    /// Optional second transfer series sharing the same time and value domains.
    pub secondary: Option<(Signal<Vec<f32>>, u32)>,
    pub units: PlotUnits,
    /// Fill the native height allocated by a desktop resource card.
    pub fit_height: bool,
}

fn observations(values: &[f32], units: PlotUnits) -> Vec<f32> {
    values[values.len().saturating_sub(CAPACITY)..]
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .map(|v| match units {
            PlotUnits::Percent | PlotUnits::StoragePercent => v.clamp(0., 100.),
            PlotUnits::BytesPerSecond => v.max(0.),
        })
        .collect()
}
fn domain_for(values: &[f32], secondary: &[f32], kind: PlotKind, units: PlotUnits) -> (f32, f32) {
    if units == PlotUnits::BytesPerSecond {
        let peak = values
            .iter()
            .chain(secondary)
            .copied()
            .fold(0_f32, f32::max);
        (0., (peak * 1.12).max(1.))
    } else {
        y_domain(values, kind)
    }
}
fn rate_label(value: f32) -> String {
    if value >= 1_000_000_000. {
        format!("{:.1} GB/s", value / 1_000_000_000.)
    } else if value >= 1_000_000. {
        format!("{:.1} MB/s", value / 1_000_000.)
    } else if value >= 1_000. {
        format!("{:.1} kB/s", value / 1_000.)
    } else {
        format!("{value:.0} B/s")
    }
}
/// Ages are relative to the last captured sample, so a paused chart describes
/// its frozen capture rather than claiming old observations happened now.
fn sample_ages(timestamps: &[f64], count: usize) -> Vec<f64> {
    let times = &timestamps[timestamps.len().saturating_sub(count)..];
    if times.len() == count
        && !times.is_empty()
        && times.iter().all(|v| v.is_finite())
        && times.windows(2).all(|pair| pair[1] >= pair[0])
    {
        let end = times[count - 1];
        times.iter().map(|at| end - at).collect()
    } else {
        (0..count).map(|i| (count - 1 - i) as f64 * 2.).collect()
    }
}
fn time_x(age: f64, horizon: f64, left: f32, right: f32) -> f32 {
    right - (age / horizon.max(0.001)) as f32 * (right - left)
}
fn nearest_sample(ages: &[f64], fraction: f32, horizon: f64) -> Option<usize> {
    let age = (1. - fraction.clamp(0., 1.)) as f64 * horizon;
    ages.iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (**a - age).abs().total_cmp(&(**b - age).abs()))
        .map(|(i, _)| i)
}

#[cfg(test)]
fn visible_samples(values: &[f32]) -> Vec<f32> {
    observations(values, PlotUnits::Percent)
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
#[cfg(test)]
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
#[cfg(test)]
fn slot_x(slot: usize, left: f32, right: f32, horizon: usize) -> f32 {
    right - (CAPACITY - 1 - slot.min(CAPACITY - 1)) as f32 * (right - left) / horizon as f32
}
#[cfg(test)]
fn slot_at_fraction(fraction: f32, horizon: usize) -> usize {
    ((CAPACITY - 1) as f32 - (1. - fraction.clamp(0., 1.)) * horizon as f32)
        .round()
        .clamp(0., (CAPACITY - 1) as f32) as usize
}

fn segments(ages: &[f64]) -> Vec<std::ops::Range<usize>> {
    if ages.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut start = 0;
    for index in 1..ages.len() {
        if ages[index - 1] - ages[index] > 6. {
            result.push(start..index);
            start = index;
        }
    }
    result.push(start..ages.len());
    result
}

fn tooltip_bounds(
    anchor: f32,
    measured_width: f32,
    width: f32,
    height: f32,
) -> (f32, f32, f32, f32) {
    let w = measured_width.min((width - 4.).max(0.));
    let x = (anchor - w * 0.5).clamp(2., (width - w - 2.).max(2.));
    (x, 2., w, height)
}

/// Keep the time axis and at least a 16pt trace region inside a short host.
fn vertical_bounds(height: f32, preferred_top: f32) -> (f32, f32) {
    let axis_room = if height < 84. { 19. } else { 25. };
    let bottom = (height - axis_room).max(0.);
    let top = preferred_top.min((bottom - 16.).max(0.));
    (top, bottom)
}
fn default_height(compact: bool, width: f32) -> f32 {
    if compact {
        94.
    } else if width < 600. {
        154.
    } else {
        176.
    }
}

fn fallback_width(compact: bool) -> f32 {
    let screen = window().width.0;
    let body = screen - if screen < 600. { 24. } else { 40. };
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
/// minimum 14 seconds, and retains 60 samples with actual pause gaps. No samples are invented.
/// Keep this mounted across sampling updates; update the signal in place.
#[allow(dead_code)] // Compatibility entry for percentage-only callers.
pub fn plot(
    values: Signal<Vec<f32>>,
    color: u32,
    kind: PlotKind,
    compact: bool,
) -> impl IntoElement {
    plot_with_options(values, color, kind, compact, PlotOptions::default())
}

pub fn plot_with_options(
    values: Signal<Vec<f32>>,
    color: u32,
    kind: PlotKind,
    compact: bool,
    options: PlotOptions,
) -> impl IntoElement {
    let initial = observations(&values.get_untracked(), options.units);
    let initial_secondary = options
        .secondary
        .map(|(values, _)| observations(&values.get_untracked(), options.units))
        .unwrap_or_default();
    let initial_domain = domain_for(&initial, &initial_secondary, kind, options.units);
    let initial_targets: [f32; CAPACITY] = std::array::from_fn(|i| {
        let start = CAPACITY - initial.len();
        if i >= start { initial[i - start] } else { 0. }
    });
    let samples = signal(initial);
    let secondary_samples = signal(initial_secondary);
    let secondary_slots: Rc<[AnimatedValue; CAPACITY]> =
        Rc::new(std::array::from_fn(|_| animated_value(0.)));
    let update_secondary = secondary_slots.clone();
    let measured_width = signal(0_f32);
    let measured_height = signal(0_f32);
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
        let next = observations(&values.get(), options.units);
        let secondary = options
            .secondary
            .map(|(values, _)| observations(&values.get(), options.units))
            .unwrap_or_default();
        let reduced = reduce_motion();
        let domain = domain_for(&next, &secondary, kind, options.units);
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
        let secondary_changed = secondary != secondary_samples.get_untracked();
        if secondary_changed || reduced {
            let start = CAPACITY - secondary.len();
            for i in 0..CAPACITY {
                let target = if i >= start { secondary[i - start] } else { 0. };
                if reduced {
                    update_secondary[i].set(target);
                } else {
                    let _ = update_secondary[i].timing(target, 520., Easing::EaseOut);
                }
            }
        }
        secondary_samples.set(secondary);
        samples.set(next);
    });
    // Native horizontal claiming lets a finger inspect history while vertical
    // drags remain available to the surrounding page's scroll view.
    let select_at: Rc<dyn Fn(f32)> = Rc::new(move |x| {
        let (Some(measured), Some(observed)) = (measured_width.try_get(), samples.try_get()) else {
            return;
        };
        let w = if measured > 0. {
            measured
        } else {
            fallback_width(compact)
        };
        let left = if compact {
            10.
        } else if options.units == PlotUnits::BytesPerSecond {
            64.
        } else {
            31.
        };
        let fraction = ((x - left) / (w - 10. - left).max(1.)).clamp(0., 1.);
        let Some(times) = options
            .timestamps
            .map(|s| s.try_get())
            .unwrap_or(Some(Vec::new()))
        else {
            return;
        };
        let ages = sample_ages(&times, observed.len());
        let horizon = ages.first().copied().unwrap_or(0.).max(14.);
        cursor.try_set(
            nearest_sample(&ages, fraction, horizon).map(|index| CAPACITY - observed.len() + index),
        );
    });
    let mouse_select = select_at.clone();
    let touch_select = select_at.clone();
    let grant_select = select_at.clone();
    let draw_slots = slots.clone();
    let drawing = canvas(move || {
        let observed = samples.get();
        let count = observed.len();
        let secondary = secondary_samples.get();
        let ages = sample_ages(
            &options.timestamps.map(|s| s.get()).unwrap_or_default(),
            count,
        );
        let horizon = ages.first().copied().unwrap_or(0.).max(14.);
        let (domain_min, domain_max) = domain_for(&observed, &secondary, kind, options.units);
        let paused = options.paused.is_some_and(|value| value.get());
        let domain_span = domain_max - domain_min;

        let w = if measured_width.get() > 0. {
            measured_width.get()
        } else {
            fallback_width(compact)
        };
        let h = if options.fit_height && measured_height.get() > 0. {
            measured_height.get()
        } else {
            default_height(compact, window().width.0)
        };
        let left = if compact {
            10.
        } else if options.units == PlotUnits::BytesPerSecond {
            64.
        } else {
            31.
        };
        let right = (w - 10.).max(left + 2.);
        let tooltip_size = if compact { 9. } else { 10. };
        let tooltip_rows = if options.units == PlotUnits::BytesPerSecond
            && options.secondary.is_some()
            && h >= 84.
        {
            2.
        } else {
            1.
        };
        let tooltip_line = text_metrics(&format!("{tooltip_size}px system-ui"), "0123456789")
            .height
            .0
            .max(tooltip_size + 2.);
        // Reserve the readout's measured height and the full 9pt endpoint pulse.
        // Pointer movement never changes the plot's bounds or hides a trace.
        let (top, bottom) = vertical_bounds(h, (tooltip_line * tooltip_rows + 16.).max(28.));
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
                let label = if options.units == PlotUnits::BytesPerSecond {
                    rate_label(domain_max - domain_span * fraction)
                } else if kind == PlotKind::Bars {
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
            .fill_text(&time_label(horizon.round() as usize), left, h - 3.)
            .text_align(CanvasTextAlign::Right)
            .fill_text(if paused { "paused" } else { "latest" }, right, h - 3.)
            .text_align(CanvasTextAlign::Left);
        if !compact {
            p = p
                .text_align(CanvasTextAlign::Center)
                .fill_text(
                    &time_label((horizon * 0.5).round() as usize),
                    left + plot_w * 0.5,
                    h - 3.,
                )
                .text_align(CanvasTextAlign::Left);
        }
        if compact && kind == PlotKind::Memory && count > 0 && cursor.get().is_none() {
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
        let step = plot_w / (horizon as f32 / 2.).max(1.);
        let xy: Vec<_> = (first..CAPACITY)
            .map(|i| {
                let span = domain_high.sub(&domain_low);
                let fraction = draw_slots[i].sub(&domain_low).div(&span);
                let y = fraction.interpolate(&[0., 1.], &[bottom, top]);
                (time_x(ages[i - first], horizon, left, right), y)
            })
            .collect();
        let fill =
            CanvasGradient::linear(Point::new(px(0.), px(top)), Point::new(px(0.), px(bottom)))
                .add_color_stop(Percentage(0.), accent.opacity(0.27))
                .add_color_stop(Percentage(0.7), accent.opacity(0.07))
                .add_color_stop(Percentage(1.), accent.opacity(0.008));
        match kind {
            PlotKind::Line | PlotKind::Memory => {
                for range in segments(&ages) {
                    let points = &xy[range];
                    p = p
                        .fill_style(fill.clone())
                        .begin_path()
                        .move_to(points[0].0, bottom);
                    for (x, y) in points {
                        p = p.line_to(*x, y);
                    }
                    p = p
                        .line_to(points.last().unwrap().0, bottom)
                        .close_path()
                        .fill();
                    for (width, alpha) in [(5_f32, 0.08_f32), (1.75, 0.98)] {
                        p = p
                            .stroke_color(accent.opacity(alpha))
                            .line_width(width)
                            .line_cap(LineCap::Round)
                            .line_join(LineJoin::Round)
                            .begin_path();
                        for (i, (x, y)) in points.iter().enumerate() {
                            p = if i == 0 {
                                p.move_to(*x, y)
                            } else {
                                p.line_to(*x, y)
                            };
                        }
                        p = p.stroke();
                    }
                }
                let (x, y) = xy.last().unwrap();
                // A single native pulse per new sample settles to stillness;
                // it does not keep an idle monitor redrawing indefinitely.
                if !reduce_motion() {
                    let ring = beat.interpolate(&[0., 1.], &[3., if h < 84. { 5. } else { 9. }]);
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
        if let Some((_, second_color)) = options.secondary {
            let second_count = secondary.len().min(count);
            if second_count > 0 {
                p = p
                    .stroke_color(rgb(second_color))
                    .line_width(1.6)
                    .line_cap(LineCap::Round)
                    .line_join(LineJoin::Round)
                    .begin_path();
                let second_first = CAPACITY - secondary.len();
                for offset in 0..second_count {
                    let index = secondary.len() - second_count + offset;
                    let at = count - second_count + offset;
                    let fraction = secondary_slots[second_first + index]
                        .sub(&domain_low)
                        .div(&domain_high.sub(&domain_low));
                    let y = fraction.interpolate(&[0., 1.], &[bottom, top]);
                    let x = time_x(ages[at], horizon, left, right);
                    p = if offset == 0 || ages[at - 1] - ages[at] > 6. {
                        p.move_to(x, &y)
                    } else {
                        p.line_to(x, &y)
                    };
                }
                p = p.stroke();
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
            let text = if options.units == PlotUnits::BytesPerSecond {
                let rx = rate_label(value);
                if options.secondary.is_some() {
                    let second = secondary
                        .len()
                        .checked_sub(count - (index - first))
                        .and_then(|i| secondary.get(i));
                    second.map_or_else(
                        || format!("↓ {rx}"),
                        |tx| format!("↓ {rx}  ↑ {}", rate_label(*tx)),
                    )
                } else {
                    rx
                }
            } else if options.units == PlotUnits::StoragePercent {
                format!("{value:.1}% used")
            } else if kind == PlotKind::Memory {
                format!("{value:.1}% memory")
            } else if kind == PlotKind::Bars {
                format!("{value:.0}% of peak")
            } else {
                format!("{value:.1}%")
            };
            let age = ages[index - first].round() as usize;
            let mut lines: Vec<String> = if tooltip_rows > 1. {
                text.split("  ").map(str::to_owned).collect()
            } else {
                vec![text]
            };
            lines[0].push_str(&format!(" · −{age}s"));
            let base_size = if compact { 9. } else { 10. };
            let font = format!("{base_size}px system-ui");
            let max_width = lines
                .iter()
                .map(|line| measure_text(&font, line).0)
                .fold(0_f32, f32::max);
            let font_size = (base_size * (w - 14.).max(1.) / max_width.max(1.))
                .min(base_size)
                .max(7.);
            let font = format!("{font_size}px system-ui");
            // Fit using native text metrics, including glyph ascent/descent.
            // Very narrow hosts elide rather than paint beyond their canvas.
            for line in &mut lines {
                if measure_text(&font, line).0 > (w - 14.).max(1.) {
                    while !line.is_empty()
                        && measure_text(&font, &format!("{line}…")).0 > (w - 14.).max(1.)
                    {
                        line.pop();
                    }
                    line.push('…');
                }
            }
            let metrics = text_metrics(&font, &lines[0]);
            let line_height = metrics.height.0.max(font_size + 2.);
            let measured = lines
                .iter()
                .map(|line| measure_text(&font, line).0)
                .fold(0_f32, f32::max)
                + 8.;
            let box_height = line_height * lines.len() as f32 + 4.;
            let (tip_x, tip_y, tip_w, tip_h) = tooltip_bounds(*x, measured, w, box_height);
            p = p
                .fill_color(rgb(0x232329))
                .begin_path()
                .round_rect(
                    Bounds {
                        origin: Point::new(px(tip_x), px(tip_y)),
                        size: Size::new(px(tip_w), px(tip_h)),
                    },
                    px(4.),
                )
                .fill()
                .font(&font)
                .fill_color(rgb(0xeeeef4));
            for (line_index, line) in lines.iter().enumerate() {
                let baseline = tip_y + 2. + metrics.ascent.0 + line_index as f32 * line_height;
                p = p.fill_text(line, tip_x + 4., baseline);
            }
        } else if !compact {
            p = p.font("10px system-ui").fill_color(accent).fill_text(
                if options.units == PlotUnits::BytesPerSecond {
                    if options.secondary.is_some() {
                        "↓ DOWNLOAD  ↑ UPLOAD"
                    } else {
                        "TRANSFER RATE"
                    }
                } else if kind == PlotKind::Bars {
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
    .when(options.fit_height, |v| v.flex_1().min_h(px(0.)))
    .when(!options.fit_height, |v| {
        v.h(move || px(default_height(compact, window().width.0)))
    })
    .on_layout(move |bounds: &Bounds<Points>| {
        // Native layout/exit callbacks may already be queued when a page owner
        // retires; measured signals must never resurrect an unmounted chart.
        measured_width.try_set(bounds.size.width.0);
        measured_height.try_set(bounds.size.height.0);
    })
    .on_mouse_move(move |point: &Point<Points>| mouse_select(point.x.0))
    .on_touch(move |event: &nsh_core::TouchEvent| {
        if matches!(
            event.phase,
            nsh_core::TouchPhase::Start | nsh_core::TouchPhase::Move
        ) {
            if let Some(point) = event.points.iter().find(|point| point.changed) {
                touch_select(point.location.x.0);
            }
        } else {
            cursor.try_set(None);
        }
    })
    .claim_touches(nsh_core::ui::TouchClaim::on_pan(
        nsh_core::PanActivation::horizontal(px(10.), px(10.)),
    ))
    .on_responder_grant(move |event: &nsh_core::ui::ResponderEvent| {
        grant_select(event.location.x.0)
    })
    .on_responder_move(move |event: &nsh_core::ui::ResponderEvent| select_at(event.location.x.0))
    .on_responder_release(move |_: &nsh_core::ui::ResponderEvent| {
        cursor.try_set(None);
    })
    .on_responder_terminate(move |_: &nsh_core::ui::ResponderEvent| {
        cursor.try_set(None);
    })
    .on_hover_out(move || {
        cursor.try_set(None);
    });
    view()
        .w_full()
        .when(options.fit_height, |v| v.flex_1().min_h(px(0.)))
        .child(drawing)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocated_chart_height_preserves_axis_and_minimum_trace_region() {
        for height in [64_f32, 80., 94., 154., 176., 320., 640.] {
            for reserved in [28_f32, 42., 56.] {
                let (top, bottom) = vertical_bounds(height, reserved);
                assert!(top >= 0. && bottom < height);
                assert!(bottom - top >= 16.);
                assert_eq!(bottom, height - if height < 84. { 19. } else { 25. });
                let pulse = if height < 84. { 5. } else { 9. };
                assert!(bottom + pulse <= height - 13.);
            }
        }
        assert_eq!(default_height(true, 400.), 94.);
        assert_eq!(default_height(false, 400.), 154.);
        assert_eq!(default_height(false, 1200.), 176.);
        assert_eq!(vertical_bounds(320., 28.), (28., 295.));
    }
    #[test]
    fn tooltip_stays_inside_canvas_at_both_endpoints_and_narrow_widths() {
        for width in [80_f32, 120., 250.] {
            for anchor in [0., width * 0.5, width] {
                let (x, y, w, h) = tooltip_bounds(anchor, 180., width, 16.);
                assert!(x >= 2. && x + w <= width - 2.);
                assert!(y >= 0. && y + h < 28. - 9.);
            }
        }
    }
    #[test]
    fn transfer_observations_keep_actual_units_and_share_both_series_domain() {
        let values = observations(
            &[f32::NAN, -1., 125., 2_000_000.],
            PlotUnits::BytesPerSecond,
        );
        assert_eq!(values, vec![0., 125., 2_000_000.]);
        assert_eq!(
            domain_for(
                &[1_000.],
                &[4_000.],
                PlotKind::Bars,
                PlotUnits::BytesPerSecond
            ),
            (0., 4_480.)
        );
        assert_eq!(rate_label(125.), "125 B/s");
        assert_eq!(rate_label(2_000.), "2.0 kB/s");
        assert_eq!(rate_label(2_000_000.), "2.0 MB/s");
    }
    #[test]
    fn timestamps_preserve_pause_gaps_and_hover_uses_nearest_observation() {
        let ages = sample_ages(&[1., 3., 63., 65.], 4);
        assert_eq!(ages, vec![64., 62., 2., 0.]);
        assert_eq!(time_x(62., 64., 0., 640.), 20.);
        assert_eq!(nearest_sample(&ages, 0.95, 64.), Some(2));
        assert_eq!(nearest_sample(&ages, 1., 64.), Some(3));
        assert_eq!(segments(&ages), vec![0..2, 2..4]);
        assert_eq!(sample_ages(&[3., 1.], 2), vec![2., 0.]);
        assert!(sample_ages(&[], 0).is_empty());
    }
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
