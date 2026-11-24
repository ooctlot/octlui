use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    geometry::{Color, Fill},
    node,
    style::*,
};

pub(crate) const CATEGORY: &str = "Rendering";
pub(crate) const TITLE: &str = "Paint and borders";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    let colors = [
        Color::hex(0xd7eee1),
        Color::hex(0x78b5ae),
        Color::hex(0x396b7c),
    ];
    let swatch = |name, fill, corners| {
        node()
            .size(grow(), 145.)
            .center()
            .corner_radius(corners)
            .background(fill)
            .border(Fill::linear(1.4, &colors), 4.)
            .child(label(name, 18., INK).width(fit()))
    };
    page("Apply solid, linear, and radial fills to backgrounds, borders, text, and image tints.")
        .child(card("Fills").child(row().children([
            swatch("Solid", Fill::solid(0xc3e3da), [12.; 4]),
            swatch("Linear", Fill::linear(0.5, &colors), [22., 4., 22., 4.]),
            swatch("Radial", Fill::radial(&colors), [24.; 4]),
        ])))
}
