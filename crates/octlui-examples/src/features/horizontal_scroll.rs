use crate::{Cx, Gallery, ui::*};
use octlui::{Node, components::Scroll, node, style::*};

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "Horizontal scrolling";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    page("Use the wheel, a horizontal trackpad gesture, or the scrollbar thumb to move along the row.")
        .child(card("Horizontal viewport").child(
            node().id("horizontal-scroll").row().size(grow(), 120.).padding(8.).child_gap(8.)
                .scroll(Scroll::scroll().axes(Clip::X).scroll_bar(bar().always_visible(true).reserve_space(true)))
                .children((1..=16).map(|i| label(format!("Tile {i}"), 16., INK).size(110., 82.)
                    .padding(12.).corner_radius(7.).background(if i % 2 == 0 { 0xc4ddea } else { 0xc3e3da }))),
        ))
}
