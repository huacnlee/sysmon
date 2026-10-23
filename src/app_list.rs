//! Application resource lists. Search, ordering and expanded details live outside
//! the sample-dependent region so a live refresh does not reset the controls.
use nsh_core::ui::prelude::*;
use std::rc::Rc;

use crate::{COLORS, MUTED, PANEL, Sample, apps::AppUsage, gb};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
    Cpu,
    Memory,
    Name,
}

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
fn avatar(name: &str, size: f32) -> impl IntoElement {
    let color = color_for(name);
    view()
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        .rounded(px(7.))
        .bg(rgb(color).opacity(0.17))
        .items_center()
        .justify_center()
        .child(
            text(initials(name))
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(color)),
        )
}
fn bar(fraction: f32, color: u32) -> impl IntoElement {
    view()
        .w_full()
        .h(px(3.))
        .rounded_full()
        .bg(rgb(color).opacity(0.13))
        .overflow_hidden()
        .child(
            view()
                .h(px(3.))
                .w(relative(fraction.clamp(0., 1.)))
                .rounded_full()
                .bg(rgb(color)),
        )
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

/// Overview list. Values and bars use the same real application aggregates.
pub fn compact(s: &Sample, memory: bool) -> impl IntoElement {
    let apps = ordered(
        s.apps.clone(),
        "",
        if memory { Order::Memory } else { Order::Cpu },
        true,
    );
    let peak = apps
        .iter()
        .map(|app| {
            if memory {
                app.memory as f64
            } else {
                app.cpu as f64
            }
        })
        .fold(1_f64, f64::max);
    let color = COLORS[if memory { 2 } else { 1 }];
    view()
        .w_full()
        .min_w(px(260.))
        .bg(rgb(PANEL))
        .rounded(px(16.))
        .p(px(12.))
        .gap(px(10.))
        .child(heading(
            if memory {
                "⊞  Memory by app"
            } else {
                "⊞  CPU by app"
            },
            color,
        ))
        .when(apps.is_empty(), |v| {
            v.child(muted(if cfg!(target_os = "ios") {
                "Other applications are restricted by iOS."
            } else {
                "No application metrics available."
            }))
        })
        .children(apps.iter().take(4).map(|app| {
            let value = if memory {
                gb(app.memory)
            } else {
                format!("{:.1}%", app.cpu)
            };
            let fraction = if memory {
                app.memory as f64 / peak
            } else {
                app.cpu as f64 / peak
            };
            view()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .child(avatar(&app.name, 24.))
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.))
                        .gap(px(5.))
                        .child(
                            view()
                                .flex_row()
                                .justify_between()
                                .gap(px(5.))
                                .child(
                                    text(app.name.clone())
                                        .text_size(px(12.))
                                        .flex_1()
                                        .min_w(px(0.)),
                                )
                                .child(text(value).text_size(px(12.)).text_color(rgb(MUTED))),
                        )
                        .child(bar(fraction as f32, color)),
                )
        }))
        .child(note(if memory {
            "Resident memory · shared pages may overlap"
        } else {
            "CPU 100% = one core"
        }))
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
    let color = COLORS[if memory { 2 } else { 1 }];
    let count_source = source.clone();
    view().bg(rgb(PANEL)).rounded(px(16.)).p(px(14.)).gap(px(12.))
        .child(view().flex_row().flex_wrap().items_center().justify_between().gap_3()
            .child(heading(if memory {"⊞  Memory by app"} else {"⊞  CPU by app"},color))
            .child(text(move || {
                let all=count_source();
                let count=ordered(all.clone(),&query.get(),order.get(),descending.get()).len();
                if count==all.len(){format!("{} applications",count)}else{format!("{} of {} applications",count,all.len())}
            }).text_size(px(11.)).text_color(rgb(MUTED))))
        .child(view().flex_row().flex_wrap().items_center().gap_3()
            .child(input().flex_1().min_w(px(180.)).h(px(30.)).px_3().rounded(px(11.)).bg(rgb(0x19191c))
                .border_1().border_color(rgb(0x333339)).text_color(rgb(0xf2f2f5)).text_size(px(12.))
                .placeholder("Search applications").placeholder_color(rgb(MUTED)).value(query)
                .test_id("apps-search").label("Search applications")
                .on_change(move |value: &str| query.set(value.to_owned())))
            .child(view().flex_row().gap_1().p_1().rounded(px(11.)).bg(rgb(0x19191c))
                .children([(Order::Cpu,"CPU","sort-cpu"),(Order::Memory,"Memory","sort-memory"),(Order::Name,"Name","sort-name")].into_iter().map(move |(by,name,id)| {
                    view().h(px(28.)).justify_center().px(px(10.)).rounded(px(7.)).test_id(id).role(AccessibilityRole::Button)
                        .label(format!("Sort by {}",name))
                        .bg(move || if order.get()==by {rgb(color).opacity(0.16)} else {rgb(0x19191c).into()})
                        .on_press(move || {if order.get_untracked()==by{descending.update(|v|*v=!*v);}else{order.set(by);descending.set(by!=Order::Name);}})
                        .child(text(move || if order.get()==by {format!("{} {}",name,if descending.get(){"↓"}else{"↑"})}else{name.into()})
                            .text_size(px(11.)).text_color(move || rgb(if order.get()==by{color}else{MUTED})))
                })))
            .child(dynamic(move || if query.get().is_empty(){view().into_element()}else{
                view().px_2().py_2().role(AccessibilityRole::Button).label("Clear application search").test_id("apps-clear-search")
                    .on_press(move || query.set(String::new())).child(text("Clear").text_size(px(11.)).text_color(rgb(MUTED))).into_element()
            })))
        .child(view().flex_row().justify_between().px_2().child(muted("APPLICATION"))
            .child(muted(if memory {"MEMORY / CPU"}else{"CPU / MEMORY"})))
        .child(dynamic(move || {
            let all=source();
            let visible=ordered(all,&query.get(),order.get(),descending.get());
            let peak=visible.iter().map(|app| if memory {app.memory as f64}else{app.cpu as f64}).fold(1_f64,f64::max);
            view().gap_1()
                .when(visible.is_empty(),|v|v.child(view().py_6().gap_2()
                    .child(text(if query.get().is_empty(){"Application metrics unavailable"}else{"No matching applications"}).text_size(px(12.)))
                    .child(muted(if !query.get().is_empty(){"Try a shorter name or clear the search."}else if cfg!(target_os="ios"){ "iOS does not permit inspection of other applications." }else{"No application resource readings are available yet."}))))
                .children(visible.iter().map(|app|{
                    let app=app.clone();
                    let open=expanded.get()==app.id;
                    let selected=app.id.clone();
                    let fraction=if memory{app.memory as f64/peak}else{app.cpu as f64/peak};
                    let rowid=format!("app-row-{}",app.id);
                    view().rounded(px(12.)).bg(rgb(if open {0x25252b}else{0x202023})).overflow_hidden()
                        .child(view().flex_row().items_center().gap(px(10.)).px(px(10.)).py(px(8.)).min_h(px(36.))
                            .test_id(&rowid).label(format!("{} details for {}",if open{"Close"}else{"Open"},app.name)).role(AccessibilityRole::Button)
                            .hover(|v|v.bg(rgb(0x28282d)))
                            .on_press(move || if expanded.get_untracked()==selected{expanded.set(String::new())}else{expanded.set(selected.clone())})
                            .child(avatar(&app.name,26.))
                            .child(view().flex_1().min_w(px(0.)).gap_1().child(text(app.name.clone()).text_size(px(12.)).font_weight(FontWeight::MEDIUM))
                                .child(muted(format!("{} {}",app.processes,if app.processes==1{"process"}else{"processes"}))))
                            .child(view().w(px(100.)).gap_2()
                                .child(view().items_end().gap_1()
                                    .child(text(if memory{gb(app.memory)}else{format!("{:.1}%",app.cpu)}).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD))
                                    .child(muted(if memory{format!("{:.1}% CPU",app.cpu)}else{gb(app.memory)})))
                                .child(bar(fraction as f32,color)))
                            .child(text(if open{"⌄"}else{"›"}).text_size(px(13.)).text_color(rgb(MUTED))))
                        .when(open,|v|v.child(view().px(px(10.)).pb(px(10.)).gap(px(10.))
                            .child(view().h(px(1.)).bg(rgb(0x35353c)))
                            .child(view().flex_row().flex_wrap().gap(px(14.))
                                .child(detail_value("CPU",format!("{:.1}%",app.cpu)))
                                .child(detail_value("Resident memory",gb(app.memory)))
                                .child(detail_value("Processes",app.processes.to_string())))
                            .child(note("Application total across the grouped processes. CPU 100% equals one full core; shared memory pages may overlap."))))
                })).into_element()
        }))
        .child(note("Bars compare the visible applications. Select an application to inspect its resource totals."))
}
fn detail_value(name: &str, value: String) -> impl IntoElement {
    view().gap_1().child(muted(name)).child(
        text(value)
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD),
    )
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

fn muted(value: impl Into<String>) -> impl IntoElement {
    text(value.into()).text_size(px(11.)).text_color(rgb(MUTED))
}
fn note(value: impl Into<String>) -> impl IntoElement {
    text(value.into()).text_size(px(10.)).text_color(rgb(MUTED))
}
fn heading(value: impl Into<String>, color: u32) -> impl IntoElement {
    text(value.into())
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(color))
}
