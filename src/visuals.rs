//! Small native vector primitives shared by navigation, cards and resource rows.
//! Icons come from Lucide through nsh-icon and are drawn by NSH rather than
//! platform fonts.
use nsh_core::prelude::*;
use nsh_icon_lucide::{Icon, Lucide};

/// One icon per section, in the application's navigation order.
pub const SECTION_ICONS: [Icon; 13] = [
    Lucide::LayoutGrid,
    Lucide::Cpu,
    Lucide::MemoryStick,
    Lucide::HardDrive,
    Lucide::Globe,
    Lucide::Gpu,
    Lucide::Brain,
    Lucide::Battery,
    Lucide::Volume2,
    Lucide::Bluetooth,
    Lucide::Folder,
    Lucide::Bot,
    Lucide::Package,
];

/// A decorative line icon. `index` follows the application's navigation order.
pub fn icon(index: usize, color: u32, size: f32) -> impl IntoElement {
    glyph(SECTION_ICONS.get(index).copied().unwrap_or(SECTION_ICONS[0]), color, size)
}

/// A decorative icon of any kind, `size` points square, tinted `color`.
pub fn glyph(icon: Icon, color: u32, size: f32) -> impl IntoElement {
    let size = if size.is_finite() { size.max(0.) } else { 20. };
    nsh_icon_lucide::icon(icon).size(px(size)).text_color(rgb(color))
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
