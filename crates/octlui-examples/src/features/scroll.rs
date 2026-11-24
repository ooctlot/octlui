use crate::{Cx, Gallery, ui::*};
use octlui::{Node, components::Scroll, geometry::Vec2, node, style::*};

fn item(text: String, index: usize) -> Node {
    label(text, 15., INK)
        .height(32.)
        .padding((10., 6.))
        .corner_radius(5.)
        .background(if index.is_multiple_of(2) {
            0xe4eeeb
        } else {
            0xf2f6f5
        })
}

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "Nested scrolling";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let smooth = !state.no_smooth;
    let inner = node()
        .column()
        .id("inner-list")
        .size(grow(), 182.)
        .padding(8.)
        .background(0xd2e3df)
        .corner_radius(6.)
        .scroll(
            Scroll::scroll()
                .smooth(smooth)
                .scroll_bar(bar().always_visible(true)),
        )
        .children((0..14).map(|i| item(format!("Inner item {}", i + 1), i)));
    let outer = node()
        .column()
        .id("outer-list")
        .size(grow(), 320.)
        .padding(8.)
        .child_gap(4.)
        .background(0xf4f7f6)
        .scroll(
            Scroll::scroll()
                .smooth(smooth)
                .offset(Vec2::new(0., state.scroll_offset as f32))
                .scroll_bar(bar().always_visible(true).reserve_space(true)),
        )
        .child(inner)
        .children((0..18).map(|i| item(format!("Outer item {}", i + 1), i)));
    page("Scroll the inner list to its edge. The remaining wheel movement carries into the outer list. Thumb dragging and declared offsets use the same retained scroll state.")
        .child(row().children([
            control(if smooth { "Smooth" } else { "Immediate" }, cx, |s| s.no_smooth = !s.no_smooth),
            control("Change outer offset", cx, |s| s.scroll_offset = if s.scroll_offset == 0 { 96 } else { 0 }),
        ]))
        .child(card("Nested viewports").child(outer))
}
