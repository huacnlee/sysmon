//! Small native vector primitives shared by navigation, cards and resource rows.
//! Icons use a 24-point grid and are drawn by NSH rather than platform fonts.
use nsh_core::prelude::*;

const ICON_PATHS: [&str; 13] = [
    // Overview: four softly rounded tiles.
    "M5 3.5H8.5Q10 3.5 10 5V8.5Q10 10 8.5 10H5Q3.5 10 3.5 8.5V5Q3.5 3.5 5 3.5Z M15.5 3.5H19Q20.5 3.5 20.5 5V8.5Q20.5 10 19 10H15.5Q14 10 14 8.5V5Q14 3.5 15.5 3.5Z M5 14H8.5Q10 14 10 15.5V19Q10 20.5 8.5 20.5H5Q3.5 20.5 3.5 19V15.5Q3.5 14 5 14Z M15.5 14H19Q20.5 14 20.5 15.5V19Q20.5 20.5 19 20.5H15.5Q14 20.5 14 19V15.5Q14 14 15.5 14Z",
    // CPU: package, die and four sides of pins.
    "M7 5.5H17Q18.5 5.5 18.5 7V17Q18.5 18.5 17 18.5H7Q5.5 18.5 5.5 17V7Q5.5 5.5 7 5.5Z M9 9H15V15H9Z M9 2.5V5.5 M15 2.5V5.5 M9 18.5V21.5 M15 18.5V21.5 M2.5 9H5.5 M2.5 15H5.5 M18.5 9H21.5 M18.5 15H21.5",
    // Memory: DIMM body, chips and contacts.
    "M4 6H20Q21 6 21 7V15Q21 16 20 16H4Q3 16 3 15V7Q3 6 4 6Z M7 9V13 M12 9V13 M17 9V13 M6 16V19 M10 16V19 M14 16V19 M18 16V19",
    // Disk: enclosure, horizontal split and indicator.
    "M5 4.5H19Q21 4.5 21 6.5V17.5Q21 19.5 19 19.5H5Q3 19.5 3 17.5V6.5Q3 4.5 5 4.5Z M3 14H21 M6.5 17H9",
    // Network: globe with latitude and longitude.
    "M21 12A9 9 0 1 1 3 12A9 9 0 1 1 21 12Z M3 12H21 M12 3C7.5 7.5 7.5 16.5 12 21C16.5 16.5 16.5 7.5 12 3Z",
    // GPU: layered compute boards.
    "M12 3L21 8L12 13L3 8Z M3 12L12 17L21 12 M3 16L12 21L21 16",
    // Neural: paired cortex lobes and their central connection.
    "M12 6C10 2 6 3 6 6C2 6 2 11 4 12C1.5 15 4 19 7 18C8 22 12 21 12 18V6Z M12 6C14 2 18 3 18 6C22 6 22 11 20 12C22.5 15 20 19 17 18C16 22 12 21 12 18 M6 6C6 9 8 10 9 9 M4 12C6 11 8 13 7 15 M18 6C18 9 16 10 15 9 M20 12C18 11 16 13 17 15",
    // Battery: body, terminal and charge mark.
    "M4 7H18Q20 7 20 9V15Q20 17 18 17H4Q2 17 2 15V9Q2 7 4 7Z M22 10V14 M6 10H14V14H6Z",
    // Sound: speaker cone and two sound waves.
    "M3 9H7L12 5V19L7 15H3Z M16 8C18.5 10 18.5 14 16 16 M19 5C23 8.5 23 15.5 19 19",
    // Bluetooth: standard crossing angular monogram.
    "M7 7L17 17L12 21V3L17 7L7 17",
    // Projects: tabbed folder.
    "M3 7Q3 5 5 5H9L12 8H19Q21 8 21 10V18Q21 20 19 20H5Q3 20 3 18Z",
    // Agents: antenna, face plate, eyes and short mouth.
    "M12 3V6 M10.5 3H13.5 M6 7H18Q20 7 20 9V17Q20 19 18 19H6Q4 19 4 17V9Q4 7 6 7Z M2 11V15 M22 11V15 M8 11V13 M16 11V13 M9 16H15",
    // Containers: isometric package, lid seam and central vertical.
    "M12 2.5L21 7.5V17L12 22L3 17V7.5Z M3 7.5L12 12.5L21 7.5 M12 12.5V22 M7.5 5L16.5 10",
];

/// A decorative line icon. `index` follows the application's navigation order.
pub fn icon(index: usize, color: u32, size: f32) -> impl IntoElement {
    let size = if size.is_finite() { size.max(0.) } else { 20. };
    let drawing = Painter::new()
        .scale(size / 24., size / 24.)
        .stroke_color(rgb(color))
        .line_width(1.65)
        .stroke_svg(ICON_PATHS.get(index).copied().unwrap_or(ICON_PATHS[0]));
    canvas(drawing).w(px(size)).h(px(size)).accessible(false)
}

/// A 26-point tinted tile for metric-card headings.
pub fn badge_icon(index: usize, color: u32) -> impl IntoElement {
    view()
        .w(px(26.))
        .h(px(26.))
        .rounded(px(7.))
        .bg(rgb(color).opacity(0.13))
        .items_center()
        .justify_center()
        .child(icon(index, color, 16.))
}

/// Five-point horizontal track. `value` is a percentage in the range 0..100.
pub fn progress(value: f32, color: u32, width: f32) -> impl IntoElement {
    let width = if width.is_finite() { width.max(0.) } else { 0. };
    let value = if value.is_finite() {
        value.clamp(0., 100.)
    } else {
        0.
    };
    view()
        .w(px(width))
        .h(px(5.))
        .rounded(px(2.5))
        .bg(rgb(color).opacity(0.15))
        .child(
            view()
                .w(px(width * value / 100.))
                .h(px(5.))
                .rounded(px(2.5))
                .bg(rgb(color)),
        )
}
