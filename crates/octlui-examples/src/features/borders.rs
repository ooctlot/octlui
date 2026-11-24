use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    geometry::{Color, Fill},
    node,
    style::*,
};

pub(crate) const CATEGORY: &str = "Rendering";
pub(crate) const TITLE: &str = "Borders";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    let sample = |name, paint, width| {
        node()
            .size(grow(), 125.)
            .center()
            .background(CARD)
            .border(paint, width)
            .child(label(name, 18., INK).width(fit()))
    };
    page("Borders are painted inside the node's bounds and accept the same fills as backgrounds.")
        .child(card("Border width and paint").child(row().children([
            sample("Solid · 2 px", Fill::solid(ACCENT), 2.),
            sample(
                "Gradient · 8 px",
                Fill::linear(0.6, &[Color::hex(0x5e79cd), Color::hex(0xc9785d)]),
                8.,
            ),
        ])))
}
