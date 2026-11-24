use crate::{Cx, Gallery, ui::*};
use octlui::{Node, node, style::*};

pub(crate) const CATEGORY: &str = "Layout";
pub(crate) const TITLE: &str = "Aspect ratio";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    page("Resize the window. Each sample derives its height from the available width.").child(
        card("Width-driven ratios").child(
            row().children([
                node()
                    .width(fraction(0.55))
                    .aspect_ratio(3.)
                    .center()
                    .corner_radius(8.)
                    .background(0xc4ddea)
                    .child(label("3 : 1", 18., INK).width(fit())),
                node()
                    .width(fraction(0.3))
                    .aspect_ratio(1.)
                    .center()
                    .corner_radius(8.)
                    .background(0xc3e3da)
                    .child(label("1 : 1", 18., INK).width(fit())),
            ]),
        ),
    )
}
