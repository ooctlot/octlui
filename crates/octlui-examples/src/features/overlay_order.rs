use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::Floating,
    geometry::{Anchor, Vec2},
    node,
};

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "Overlay stacking";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    let layer = |title, x, y, z, color: u32| {
        label(title, 16., INK)
            .size(155., 80.)
            .padding(10.)
            .background(color)
            .corner_radius(8.)
            .floating(
                Floating::floating()
                    .parent_anchor(Anchor::TOP_LEFT)
                    .anchor(Anchor::TOP_LEFT)
                    .offset(Vec2::new(x, y))
                    .z_index(z),
            )
    };
    page("The front layer is declared first and has the highest z-index.").child(
        card("Three overlapping layers").child(node().size(330., 190.).children([
            layer("Front · z 10", 90., 85., 10, 0xc3e3da),
            layer("Middle · z 0", 55., 55., 0, 0xc4ddea),
            layer("Back · z -5", 20., 25., -5, 0xf3dfb6),
        ])),
    )
}
