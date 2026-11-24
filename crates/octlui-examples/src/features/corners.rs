use crate::{Cx, Gallery, ui::*};
use octlui::{Node, node, style::*};

pub(crate) const CATEGORY: &str = "Rendering";
pub(crate) const TITLE: &str = "Corner radii";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    let sample = |radii, color: u32| {
        node()
            .size(grow(), 110.)
            .corner_radius(radii)
            .background(color)
    };
    page("Set individual radii in top-left, top-right, bottom-right, bottom-left order.").child(
        card("Uniform and per-corner radii").child(row().children([
            sample([14.; 4], 0xc3e3da),
            sample([34., 3., 34., 3.], 0xc4ddea),
            sample([0., 38., 10., 24.], 0xf3dfb6),
        ])),
    )
}
