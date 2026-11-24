use crate::{Cx, Gallery, ui::*};
use octlui::{Node, node, style::*};

pub(crate) const CATEGORY: &str = "Layout";
pub(crate) const TITLE: &str = "Dimensions";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    let sample = |title, width, color| {
        label(title, 15., INK)
            .width(width)
            .padding((10., 12.))
            .corner_radius(6.)
            .background(color)
    };
    page("Resize the window to see how fixed, fractional, fit, and grow widths share space.")
        .child(card("Width in a row").child(row().children([
            sample("120 px", fixed(120.), 0xc7deb7u32),
            sample("25%", fraction(0.25), 0xc4ddea),
            sample("Grow", grow().min(85.).max(240.), 0xc3e3da),
            sample("Fit", fit(), 0xf3dfb6),
        ])))
        .child(
            card("Aspect ratio").child(
                node()
                    .width(fraction(0.55))
                    .aspect_ratio(3.)
                    .center()
                    .corner_radius(8.)
                    .background(0xc4ddea)
                    .child(label("3 : 1", 18., INK).width(fit())),
            ),
        )
}
