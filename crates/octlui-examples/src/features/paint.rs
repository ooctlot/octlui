use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    geometry::{Color, Fill},
    node,
    style::*,
};

pub(crate) const CATEGORY: &str = "Rendering";
pub(crate) const TITLE: &str = "Background fills";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    let colors = [
        Color::hex(0xd7eee1),
        Color::hex(0x78b5ae),
        Color::hex(0x396b7c),
    ];
    let swatch = |name, fill| {
        node()
            .size(grow(), 145.)
            .center()
            .corner_radius(8.)
            .background(fill)
            .child(label(name, 18., INK).width(fit()))
    };
    page("Compare a solid background with linear and radial gradients.").child(card("Fills").child(
        row().children([
            swatch("Solid", Fill::solid(0xc3e3da)),
            swatch("Linear", Fill::linear(0.5, &colors)),
            swatch("Radial", Fill::radial(&colors)),
        ]),
    ))
}
