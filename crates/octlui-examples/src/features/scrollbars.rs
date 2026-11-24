use crate::{Cx, Gallery, ui::*};
use octlui::{Node, components::Scroll, node, style::*};

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "Scrollbars";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    let list = |id, side, reserve_space| {
        node()
            .id(id)
            .column()
            .size(grow(), 218.)
            .child_gap(4.)
            .scroll(
                Scroll::scroll().scroll_bar(
                    bar()
                        .always_visible(true)
                        .side(side)
                        .reserve_space(reserve_space),
                ),
            )
            .children((1..=14).map(|i| {
                label(format!("Item {i}"), 15., INK)
                    .height(28.)
                    .padding(6.)
                    .background(0xe5efed)
            }))
    };
    page("Drag either thumb or click the track. Compare a reserved scrollbar with one that overlays the content.")
        .child(row().children([
            card("Start · reserved").child(list("reserved-bar", Side::Start, true)),
            card("End · overlay").child(list("overlay-bar", Side::End, false)),
        ]))
}
