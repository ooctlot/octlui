use crate::{Cx, Gallery, ui::*};
use octlui::{Node, geometry::Padding, node, style::*};

pub(crate) const CATEGORY: &str = "Layout";
pub(crate) const TITLE: &str = "Padding and gaps";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    let sample = |padding: Padding, gap| {
        node()
            .width(grow())
            .column()
            .background(0xc4ddea)
            .padding(padding)
            .child_gap(gap)
            .children(
                ["First child", "Second child"]
                    .map(|title| label(title, 15., INK).padding(7.).background(CARD)),
            )
    };
    page("Blue shows both the space around children and the gap between them.").child(
        row().children([
            card("Uniform · 8 px gap").child(sample(Padding::all(12.), 8.)),
            card("Asymmetric · 22 px gap").child(sample(Padding::new(8., 22., 16., 10.), 22.)),
        ]),
    )
}
