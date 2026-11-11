//! Application resource lists. Search, ordering and expanded details live outside
//! the sample-dependent region so a live refresh does not reset the controls.
use crate::PanelSurface;
use nsh_core::prelude::*;
use nsh_core::{LayoutAnimation, LayoutCurve, LayoutTransition};
use std::rc::Rc;

use crate::{COLORS, MUTED, Sample, apps::AppUsage, gb};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
    Cpu,
    Memory,
    Name,
}

#[derive(Clone, Copy)]
struct TableState {
    query: Signal<String>,
    order: Signal<Order>,
    descending: Signal<bool>,
    expanded: Signal<String>,
    memory: bool,
    color: u32,
    all: Memo<Vec<AppUsage>>,
    rows: Memo<Vec<AppUsage>>,
    peak: Memo<f64>,
    list_ref: ElementRef,
}

#[derive(Clone, Copy)]
struct RowState {
    peak: Memo<f64>,
    expanded: Signal<String>,
    memory: bool,
    color: u32,
    list_ref: ElementRef,
}

#[derive(Clone, Copy)]
struct SortChoice {
    order: Order,
    name: &'static str,
    id: &'static str,
    icon: usize,
}

const SORT_CHOICES: [SortChoice; 3] = [
    SortChoice {
        order: Order::Cpu,
        name: "CPU",
        id: "sort-cpu",
        icon: 1,
    },
    SortChoice {
        order: Order::Memory,
        name: "Memory",
        id: "sort-memory",
        icon: 2,
    },
    SortChoice {
        order: Order::Name,
        name: "Name",
        id: "sort-name",
        icon: 0,
    },
];

fn color_for(name: &str) -> u32 {
    const PALETTE: [u32; 6] = [0x398ef5, 0x9885ed, 0x20b48c, 0xdc5489, 0xd89b13, 0x20b4d2];
    let hash = name.bytes().fold(0_usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    });
    PALETTE[hash % PALETTE.len()]
}

fn initials(name: &str) -> String {
    let words: Vec<_> = name.split_whitespace().take(2).collect();
    let value: String = words
        .iter()
        .filter_map(|part| part.chars().next())
        .flat_map(char::to_uppercase)
        .collect();
    if value.is_empty() { "A".into() } else { value }
}

fn ordered(mut apps: Vec<AppUsage>, query: &str, order: Order, descending: bool) -> Vec<AppUsage> {
    let query = query.trim().to_lowercase();
    apps.retain(|app| app.name.to_lowercase().contains(&query));
    apps.sort_by(|a, b| {
        let cmp = match order {
            Order::Cpu => a.cpu.total_cmp(&b.cpu),
            Order::Memory => a.memory.cmp(&b.memory),
            Order::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        };
        let cmp = if descending { cmp.reverse() } else { cmp };
        cmp.then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });
    apps
}

fn app_tint(memory: bool) -> u32 {
    COLORS[if memory { 2 } else { 1 }]
}

fn app_title(memory: bool) -> &'static str {
    if memory {
        "⊞  Memory by app"
    } else {
        "⊞  CPU by app"
    }
}

fn app_columns(memory: bool) -> &'static str {
    if memory {
        "MEMORY / CPU"
    } else {
        "CPU / MEMORY"
    }
}

fn app_value(app: &AppUsage, memory: bool) -> f64 {
    if memory {
        app.memory as f64
    } else {
        app.cpu as f64
    }
}

fn app_reading(app: &AppUsage, memory: bool) -> String {
    if memory {
        gb(app.memory)
    } else {
        format!("{:.1}%", app.cpu)
    }
}

fn app_secondary_reading(app: &AppUsage, memory: bool) -> String {
    if memory {
        format!("{:.1}% CPU", app.cpu)
    } else {
        gb(app.memory)
    }
}

fn peak_usage(rows: &[AppUsage], memory: bool) -> f64 {
    rows.iter()
        .map(|app| app_value(app, memory))
        .fold(1_f64, f64::max)
}

fn peak_memo(rows: Memo<Vec<AppUsage>>, memory: bool) -> Memo<f64> {
    computed(move || rows.with(|items| peak_usage(items, memory)))
}

fn app_panel() -> View {
    view()
        .w_full()
        .flex_1()
        .min_h(px(0.))
        .min_w(px(0.))
        .panel()
        .rounded(px(8.))
        .p(px(12.))
        .gap(px(10.))
}

/// A compact live list with four stable rank slots. Values and progress
/// transforms update in place, instead of replacing overview rows each sample.
pub fn compact_from_data(data: Signal<Sample>, memory: bool) -> impl IntoElement {
    let rows = computed(move || {
        let mut values = ordered(
            data.with(|sample| sample.apps.clone()),
            "",
            if memory { Order::Memory } else { Order::Cpu },
            true,
        );
        values.truncate(4);
        values
    });
    let peak = peak_memo(rows, memory);
    let color = app_tint(memory);
    app_panel()
        .child(heading(app_title(memory), color).flex_shrink_0())
        .child(compact_empty(rows))
        .child(compact_rows(rows, peak, memory, color))
}

fn compact_empty(rows: Memo<Vec<AppUsage>>) -> impl IntoElement {
    when(
        move || rows.with(Vec::is_empty),
        move || {
            muted(if cfg!(target_os = "ios") {
                "Other applications are restricted by iOS."
            } else {
                "No application metrics available."
            })
        },
    )
}

fn compact_rows(
    rows: Memo<Vec<AppUsage>>,
    peak: Memo<f64>,
    memory: bool,
    color: u32,
) -> impl IntoElement {
    view()
        .flex_1()
        .min_h(px(0.))
        .justify_between()
        .gap(px(6.))
        .children((0..4).map(move |index| compact_slot(rows, peak, memory, color, index)))
}

fn compact_slot(
    rows: Memo<Vec<AppUsage>>,
    peak: Memo<f64>,
    memory: bool,
    color: u32,
    index: usize,
) -> impl IntoElement {
    when(
        move || rows.with(|items| index < items.len()),
        move || compact_row(rows, peak, memory, color, index),
    )
}

fn compact_row(
    rows: Memo<Vec<AppUsage>>,
    peak: Memo<f64>,
    memory: bool,
    color: u32,
    index: usize,
) -> impl IntoElement {
    let app = computed(move || rows.with(|items| items.get(index).cloned().unwrap_or_default()));
    let fraction = computed(move || app.with(|item| app_value(item, memory) / peak.get()) as f32);
    view()
        .min_h(px(24.))
        .flex_shrink_0()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .child(live_avatar(app, 24.))
        .child(compact_row_body(app, fraction, memory, color))
}

fn compact_row_body(
    app: Memo<AppUsage>,
    fraction: Memo<f32>,
    memory: bool,
    color: u32,
) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(px(0.))
        .gap(px(5.))
        .child(compact_row_labels(app, memory))
        .child(live_bar(fraction, color))
}

fn compact_row_labels(app: Memo<AppUsage>, memory: bool) -> impl IntoElement {
    view()
        .flex_row()
        .justify_between()
        .gap(px(5.))
        .child(
            text(move || app.with(|item| item.name.clone()))
                .text_size(px(12.))
                .flex_1()
                .min_w(px(0.)),
        )
        .child(
            text(move || app.with(|item| app_reading(item, memory)))
                .text_size(px(12.))
                .text_color(rgb(MUTED)),
        )
}

/// Mount this outside a region that rebuilds on every Sample. The table tracks
/// data itself; only its rows and count change when the sample updates.
pub fn table_from_data(data: Signal<Sample>, memory: bool) -> impl IntoElement {
    table_source(Rc::new(move || data.get().apps), memory)
}

/// An alternative entry for callers that already expose application aggregates.
#[allow(dead_code)]
pub fn table(apps: Signal<Vec<AppUsage>>, memory: bool) -> impl IntoElement {
    table_source(Rc::new(move || apps.get()), memory)
}

fn table_source(source: Rc<dyn Fn() -> Vec<AppUsage>>, memory: bool) -> impl IntoElement {
    let query = signal(String::new());
    let order = signal(if memory { Order::Memory } else { Order::Cpu });
    let descending = signal(true);
    let expanded = signal(String::new());
    let all = computed(move || source());
    let rows = computed(move || ordered(all.get(), &query.get(), order.get(), descending.get()));
    let state = TableState {
        query,
        order,
        descending,
        expanded,
        memory,
        color: app_tint(memory),
        all,
        rows,
        peak: peak_memo(rows, memory),
        list_ref: element_ref(),
    };
    app_panel()
        .child(table_header(state))
        .child(table_controls(state))
        .child(table_columns(state))
        .child(table_list(state))
}

fn table_header(state: TableState) -> impl IntoElement {
    view()
        .flex_shrink_0()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_3()
        .child(heading(app_title(state.memory), state.color))
        .child(
            text(move || table_count(state))
                .text_size(px(11.))
                .text_color(rgb(MUTED)),
        )
}

fn table_count(state: TableState) -> String {
    let total = state.all.with(Vec::len);
    let count = state.rows.with(Vec::len);
    if count == total {
        format!("{count} applications")
    } else {
        format!("{count} of {total} applications")
    }
}

fn table_controls(state: TableState) -> impl IntoElement {
    view()
        .flex_shrink_0()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(search_control(state.query, state.list_ref))
        .child(sort_control(state.order, state.descending, state.list_ref))
}

fn table_columns(state: TableState) -> impl IntoElement {
    view()
        .flex_shrink_0()
        .flex_row()
        .justify_between()
        .px_2()
        .child(muted("APPLICATION"))
        .child(muted(app_columns(state.memory)))
}

fn table_list(state: TableState) -> impl IntoElement {
    let row = RowState {
        peak: state.peak,
        expanded: state.expanded,
        memory: state.memory,
        color: state.color,
        list_ref: state.list_ref,
    };
    virtual_list(
        move || state.rows.with(Vec::len),
        move |at| {
            // Recycled cells retain their view tree. Position and the memo below
            // redirect every binding and event to the current application.
            let app = computed(move || {
                state
                    .rows
                    .with(|items| items.get(at.index()).cloned().unwrap_or_default())
            });
            resource_row(app, row, at)
        },
    )
    .keys(move || {
        state
            .rows
            .with(|items| items.iter().map(|app| app.id.clone()).collect::<Vec<_>>())
    })
    .estimated_item_size(px(52.))
    .row_gap(px(2.))
    .flex_1()
    .min_h(px(0.))
    .w_full()
    .test_id("apps-list")
    .label("Application resources")
    .element_ref(&state.list_ref)
    .empty(empty_app_list(state.query))
}

fn empty_app_list(query: Signal<String>) -> impl IntoElement {
    view()
        .py_6()
        .gap_2()
        .child(
            text(move || {
                if query.get().is_empty() {
                    "Application metrics unavailable"
                } else {
                    "No matching applications"
                }
            })
            .text_size(px(12.)),
        )
        .child(
            text(move || {
                if !query.get().is_empty() {
                    "Try a shorter name or clear the search."
                } else if cfg!(target_os = "ios") {
                    "iOS does not permit inspection of other applications."
                } else {
                    "No application resource readings are available yet."
                }
            })
            .text_size(px(11.))
            .text_color(rgb(MUTED)),
        )
}

/// A single inset field keeps its clear affordance in a fixed trailing slot,
/// so typing does not move either the field or the adjacent sort control.
fn search_control(query: Signal<String>, list_ref: ElementRef) -> impl IntoElement {
    let focused = signal(false);
    let field = element_ref();
    search_frame(focused)
        .on_press(move || field.focus())
        .child(search_icon())
        .child(search_field(query, focused, field, list_ref))
        .child(clear_search_button(query, field, list_ref))
}

fn search_frame(focused: Signal<bool>) -> View {
    view()
        .flex_1()
        .min_w(px(220.))
        .h(px(32.))
        .flex_row()
        .items_center()
        .gap(px(8.))
        .px(px(10.))
        .rounded(px(9.))
        .bg(rgb(0x0f0f11).opacity(0.62))
        .border_1()
        .border_color(move || {
            if focused.get() {
                rgb(0xffffff).opacity(0.24)
            } else {
                rgb(0xffffff).opacity(0.10)
            }
        })
        .transition_colors()
        .duration(move || if reduce_motion() { 0_f32 } else { 140_f32 })
}

fn search_icon() -> impl IntoElement {
    canvas(
        Painter::new()
            .stroke_color(rgb(MUTED))
            .line_width(1.5)
            .line_cap(nsh_core::canvas::LineCap::Round)
            .stroke_svg(
                "M10.5 6.25A4.25 4.25 0 1 1 2 6.25A4.25 4.25 0 1 1 10.5 6.25 M9.3 9.3L13 13",
            ),
    )
    .w(px(14.))
    .h(px(14.))
    .flex_shrink_0()
    .accessible(false)
}

fn search_field(
    query: Signal<String>,
    focused: Signal<bool>,
    field: ElementRef,
    list_ref: ElementRef,
) -> impl IntoElement {
    nsh_base::input()
        .flex_1()
        .min_w(px(0.))
        .h(px(30.))
        .px(px(0.))
        .py(px(0.))
        .border(px(0.))
        .bg(rgb(0x000000).opacity(0.))
        .text_color(rgb(0xeeeef2))
        .text_size(px(12.))
        .placeholder("Search applications…")
        .placeholder_color(rgb(0x7e838e))
        .value(query)
        .test_id("apps-search")
        .label("Search applications")
        .element_ref(&field)
        .on_focus(move || focused.set(true))
        .on_blur(move || focused.set(false))
        .on_change(move |value: &str| {
            query.set(value.to_owned());
            list_ref.scroll_to_offset(px(0.), false);
        })
}

fn clear_search_button(
    query: Signal<String>,
    field: ElementRef,
    list_ref: ElementRef,
) -> impl IntoElement {
    view().w(px(22.)).h(px(24.)).flex_shrink_0().child(when(
        move || !query.get().is_empty(),
        move || {
            view()
                .w(px(22.))
                .h(px(24.))
                .items_center()
                .justify_center()
                .rounded(px(5.))
                .test_id("apps-clear-search")
                .label("Clear application search")
                .role(AccessibilityRole::Button)
                .tooltip(crate::hint("Clear search"))
                .hover(|s| s.bg(rgb(0xffffff).opacity(0.07)))
                .focus(|s| s.outline(px(1.)).outline_color(rgb(0xffffff).opacity(0.35)))
                .on_press(move || {
                    query.set(String::new());
                    list_ref.scroll_to_offset(px(0.), false);
                    field.focus();
                })
                .child(clear_search_icon())
        },
    ))
}

fn clear_search_icon() -> impl IntoElement {
    canvas(
        Painter::new()
            .stroke_color(rgb(0xa3a8b2))
            .line_width(1.4)
            .line_cap(nsh_core::canvas::LineCap::Round)
            .stroke_svg("M4 4L10 10 M10 4L4 10"),
    )
    .w(px(14.))
    .h(px(14.))
    .accessible(false)
    .pointer_events(PointerEvents::None)
}

fn sort_control(
    order: Signal<Order>,
    descending: Signal<bool>,
    list_ref: ElementRef,
) -> impl IntoElement {
    view()
        .flex_row()
        .h(px(32.))
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(9.))
        .bg(rgb(0x0f0f11).opacity(0.55))
        .border_1()
        .border_color(rgb(0xffffff).opacity(0.08))
        .children(
            SORT_CHOICES
                .into_iter()
                .map(move |choice| sort_option(order, descending, list_ref, choice)),
        )
}

fn sort_option(
    order: Signal<Order>,
    descending: Signal<bool>,
    list_ref: ElementRef,
    choice: SortChoice,
) -> impl IntoElement {
    let selected = computed(move || order.get() == choice.order);
    let tint = sort_tint(choice.order);
    view()
        .relative()
        .flex_row()
        .items_center()
        .justify_center()
        .h(px(26.))
        .gap(px(5.))
        .px(px(9.))
        .rounded(px(6.))
        .border_1()
        .border_color(move || {
            if selected.get() {
                rgb(0xffffff).opacity(0.16)
            } else {
                rgb(0xffffff).opacity(0.)
            }
        })
        .bg(move || rgb(0x29292c).opacity(if selected.get() { 0.9 } else { 0. }))
        .inline_style(sort_shadow(selected))
        .child(sort_highlight(selected))
        .test_id(choice.id)
        .role(AccessibilityRole::Button)
        .label(move || sort_label(choice.name, selected, descending))
        .tooltip(crate::hint(format!(
            "Sort by {}; select again to reverse",
            choice.name
        )))
        .hover(|s| s.bg(rgb(0xffffff).opacity(0.055)))
        .focus(|s| s.outline(px(1.)).outline_color(rgb(0xffffff).opacity(0.35)))
        .transition_colors()
        .duration(move || if reduce_motion() { 0_f32 } else { 140_f32 })
        .on_press(move || sort_pressed(order, descending, list_ref, choice.order))
        .child(sort_icon(choice, selected, tint))
        .child(
            text(choice.name)
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || rgb(if selected.get() { 0xe8edf4 } else { MUTED })),
        )
        .child(sort_arrow(selected, descending, tint))
}

fn sort_tint(order: Order) -> u32 {
    match order {
        Order::Cpu => COLORS[1],
        Order::Memory => COLORS[2],
        Order::Name => 0xaeb8ca,
    }
}

fn sort_shadow(selected: Memo<bool>) -> impl Fn() -> nsh_core::element::Style + 'static {
    move || {
        nsh_core::element::Style::default().with(
            nsh_core::StyleKey::BoxShadow,
            nsh_core::PropertyValue::Shadows(std::rc::Rc::from([(
                nsh_core::BoxShadow {
                    offset_x: px(0.),
                    offset_y: px(2.),
                    blur: px(3.),
                    spread: px(0.),
                    color: rgb(0x000000)
                        .opacity(if selected.get() { 0.20 } else { 0. })
                        .into(),
                    inset: false,
                },
                true,
            )])),
        )
    }
}

fn sort_highlight(selected: Memo<bool>) -> impl IntoElement {
    view()
        .absolute()
        .top(px(0.))
        .left(px(6.))
        .right(px(6.))
        .h(px(0.75))
        .bg(move || rgb(0xffffff).opacity(if selected.get() { 0.06 } else { 0. }))
        .pointer_events(PointerEvents::None)
}

fn sort_icon(choice: SortChoice, selected: Memo<bool>, tint: u32) -> impl IntoElement {
    dynamic(move || {
        if choice.order == Order::Name {
            text("A")
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(if selected.get() { tint } else { MUTED }))
                .into_element()
        } else {
            crate::visuals::icon(choice.icon, if selected.get() { tint } else { MUTED }, 12.)
                .into_element()
        }
    })
}

fn sort_label(name: &str, selected: Memo<bool>, descending: Signal<bool>) -> String {
    format!(
        "Sort by {}{}",
        name,
        if selected.get() {
            if descending.get() {
                ", descending"
            } else {
                ", ascending"
            }
        } else {
            ""
        }
    )
}

fn sort_arrow(selected: Memo<bool>, descending: Signal<bool>, tint: u32) -> impl IntoElement {
    text(move || {
        if selected.get() {
            if descending.get() { "↓" } else { "↑" }
        } else {
            ""
        }
    })
    .w(px(8.))
    .text_size(px(10.))
    .text_color(rgb(tint))
}

fn sort_pressed(order: Signal<Order>, descending: Signal<bool>, list_ref: ElementRef, by: Order) {
    animate_action();
    if order.get_untracked() == by {
        descending.update(|v| *v = !*v);
    } else {
        order.set(by);
        descending.set(by != Order::Name);
    }
    list_ref.scroll_to_offset(px(0.), false);
}

fn resource_row(app: Memo<AppUsage>, state: RowState, at: Position) -> impl IntoElement {
    let open =
        computed(move || app.with(|item| !item.id.is_empty() && state.expanded.get() == item.id));
    let fraction =
        computed(move || app.with(|item| app_value(item, state.memory) / state.peak.get()) as f32);
    view()
        .rounded(px(8.))
        .bg(move || rgb(if open.get() { 0x25252b } else { 0x202023 }))
        .overflow_hidden()
        .child(resource_summary(app, open, fraction, state, at))
        .child(resource_details(app, open))
}

fn resource_summary(
    app: Memo<AppUsage>,
    open: Memo<bool>,
    fraction: Memo<f32>,
    state: RowState,
    at: Position,
) -> impl IntoElement {
    interactive(view())
        .flex_row()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .py(px(8.))
        .min_h(px(52.))
        // test_id is static in NSH: use the row selector plus its live AX
        // label, rather than leave an obsolete app id on a recycled cell.
        .test_id("app-row")
        .role(AccessibilityRole::Button)
        .label(move || {
            app.with(|item| {
                format!(
                    "{} details for {}",
                    if open.get() { "Close" } else { "Open" },
                    item.name
                )
            })
        })
        .hover(|v| v.bg(rgb(0x28282d)))
        .on_press(move || toggle_row(app, open, state, at))
        .child(live_avatar(app, 26.))
        .child(resource_identity(app))
        .child(resource_metrics(app, fraction, state.memory, state.color))
        .child(resource_chevron(open))
}

fn toggle_row(app: Memo<AppUsage>, open: Memo<bool>, state: RowState, at: Position) {
    let id = app.with_untracked(|item| item.id.clone());
    if id.is_empty() {
        return;
    }
    animate_action();
    if open.get_untracked() {
        state.expanded.set(String::new());
    } else {
        state.expanded.set(id);
        // Recycled cells also serve keyboard/AX activation. Bring the selected
        // logical row into view before revealing its larger content, instead of
        // expanding an overscan slot.
        state
            .list_ref
            .scroll_to_index(at.index() as u32, percentage(0.), px(0.), !reduce_motion());
    }
}

fn resource_identity(app: Memo<AppUsage>) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(px(0.))
        .gap_1()
        .child(
            text(move || app.with(|item| item.name.clone()))
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM),
        )
        .child(
            text(move || app.with(process_count))
                .text_size(px(11.))
                .text_color(rgb(MUTED)),
        )
}

fn process_count(app: &AppUsage) -> String {
    format!(
        "{} {}",
        app.processes,
        if app.processes == 1 {
            "process"
        } else {
            "processes"
        }
    )
}

fn resource_metrics(
    app: Memo<AppUsage>,
    fraction: Memo<f32>,
    memory: bool,
    color: u32,
) -> impl IntoElement {
    view()
        .w(px(100.))
        .gap_2()
        .child(
            view()
                .items_end()
                .gap_1()
                .child(
                    text(move || app.with(|item| app_reading(item, memory)))
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .child(
                    text(move || app.with(|item| app_secondary_reading(item, memory)))
                        .text_size(px(11.))
                        .text_color(rgb(MUTED)),
                ),
        )
        .child(live_bar(fraction, color))
}

fn resource_chevron(open: Memo<bool>) -> impl IntoElement {
    text(move || if open.get() { "⌄" } else { "›" })
        .text_size(px(13.))
        .text_color(rgb(MUTED))
}

fn resource_details(app: Memo<AppUsage>, open: Memo<bool>) -> impl IntoElement {
    when(
        move || open.get(),
        move || {
            view()
                .px(px(10.))
                .pb(px(10.))
                .gap(px(10.))
                .child(view().h(px(1.)).bg(rgb(0x35353c)))
                .child(resource_detail_values(app))
        },
    )
}

fn resource_detail_values(app: Memo<AppUsage>) -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .gap(px(14.))
        .child(live_detail("CPU", move || {
            app.with(|item| format!("{:.1}%", item.cpu))
        }))
        .child(live_detail("Resident memory", move || {
            app.with(|item| gb(item.memory))
        }))
        .child(live_detail("Processes", move || {
            app.with(|item| item.processes.to_string())
        }))
}

fn live_avatar(app: Memo<AppUsage>, size: f32) -> impl IntoElement {
    view()
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        .rounded(px(7.))
        .bg(move || app.with(|item| rgb(color_for(&item.name)).opacity(0.17)))
        .items_center()
        .justify_center()
        .child(
            text(move || app.with(|item| initials(&item.name)))
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || app.with(|item| rgb(color_for(&item.name)))),
        )
}

fn live_bar(fraction: Memo<f32>, color: u32) -> impl IntoElement {
    view()
        .w_full()
        .h(px(4.))
        .rounded_full()
        .bg(rgb(color).opacity(0.13))
        .overflow_hidden()
        .child(
            view()
                .w_full()
                .h(px(4.))
                .rounded_full()
                .bg_linear_to_r()
                .gradient_from(rgb(color))
                .gradient_to(rgb(crate::charts::gradient_end(color)))
                .origin_left()
                .scale_x(move || fraction.get().clamp(0., 1.))
                .transition_transform()
                .duration(move || if reduce_motion() { 0_f32 } else { 220_f32 })
                .ease_out(),
        )
}

fn live_detail(name: &str, value: impl Fn() -> String + 'static) -> impl IntoElement {
    view().gap_1().child(muted(name)).child(
        text(value)
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD),
    )
}

fn interactive<S: Styled + Interactive>(s: S) -> S {
    s.transition()
        .duration(move || if reduce_motion() { 0_f32 } else { 150_f32 })
        .ease_out()
        .active(|s| s.scale(move || if reduce_motion() { 1_f32 } else { 0.985_f32 }))
}

fn animate_action() {
    if !reduce_motion() {
        animate_layout(LayoutAnimation::all(LayoutTransition::new(
            LayoutCurve::EaseOut,
            150.,
        )));
    }
}

fn muted(value: impl Into<String>) -> impl IntoElement {
    text(value.into()).text_size(px(11.)).text_color(rgb(MUTED))
}

fn heading(value: impl Into<String>, color: u32) -> impl IntoElement + Styled {
    text(value.into())
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(color))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, cpu: f32, memory: u64) -> AppUsage {
        AppUsage {
            id: format!("test-{}", name),
            name: name.into(),
            cpu,
            memory,
            processes: 2,
        }
    }

    #[test]
    fn search_is_case_insensitive_and_sort_preserves_readings() {
        let source = vec![
            app("Safari", 8., 400),
            app("Chrome", 20., 300),
            app("Terminal", 0., 100),
        ];
        let matches = ordered(source.clone(), "  SAF  ", Order::Cpu, true);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "Safari");
        assert_eq!(matches[0].memory, 400);
        let by_memory = ordered(source, "", Order::Memory, true);
        assert_eq!(by_memory[0].name, "Safari");
        assert_eq!(by_memory[2].name, "Terminal");
    }

    #[test]
    fn identical_names_keep_distinct_stable_row_keys() {
        let mut first = app("Editor", 1., 10);
        first.id = "install-a".into();
        let mut second = app("Editor", 1., 10);
        second.id = "install-b".into();
        let rows = ordered(vec![second, first], "", Order::Name, false);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "install-a");
        assert_eq!(rows[1].id, "install-b");
    }

    #[test]
    fn ordering_ties_are_stable_and_names_can_sort_both_directions() {
        let source = vec![app("Zulu", 1., 10), app("Alpha", 1., 10)];
        assert_eq!(
            ordered(source.clone(), "", Order::Cpu, true)[0].name,
            "Alpha"
        );
        assert_eq!(
            ordered(source.clone(), "", Order::Name, false)[0].name,
            "Alpha"
        );
        assert_eq!(ordered(source, "", Order::Name, true)[0].name, "Zulu");
    }
}
