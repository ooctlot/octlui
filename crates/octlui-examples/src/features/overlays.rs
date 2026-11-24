use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::{Floating, Tooltip},
    geometry::{Anchor, Vec2},
    node,
    style::*,
};
use std::time::Duration;

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "Overlays and tooltips";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let tip = label("This tooltip follows its owner.", 14., 0xf2f7f7)
        .width(fit())
        .padding(10.)
        .background(NAV)
        .corner_radius(6.);
    page("Hover to open a delayed tooltip. Change clipping to see how the badge is anchored to its parent.")
        .child(control(if state.overlay_clipped { "Clip overlay" } else { "Allow overflow" }, cx, |s| {
            s.overlay_clipped = !s.overlay_clipped;
        }))
        .child(card("Top-right anchor").child(
            node().size(grow(), 170.).padding((12., 18.)).child(
                node().size(214., 100.).padding(18.).corner_radius(8.).background(0xc3e3da)
                    .tooltip(Tooltip::new(tip).delay(Duration::from_millis(350)))
                    .child(label("Hover here", 19., INK))
                    .child(label("Badge", 13., 0xf2f7f7).width(fit()).padding(7.)
                        .background(ACCENT).corner_radius(7.)
                        .floating(Floating::floating().parent_anchor(Anchor::TOP_RIGHT)
                            .anchor(Anchor::CENTER).offset(Vec2::new(-8., 3.))
                            .clip_to_parent(state.overlay_clipped).z_index(5))),
            ),
        ))
}
