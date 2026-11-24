use crate::{Cx, Gallery, ui::*};
use octlui::{Node, components::Tooltip, style::*};
use std::time::Duration;

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "Tooltips";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let delay = if state.tooltip_slow { 900 } else { 250 };
    let tip = label("The tooltip follows its owner.", 14., 0xf2f7f7)
        .width(fit())
        .padding(10.)
        .background(NAV)
        .corner_radius(6.);
    page("Hover over the target, wait for the tooltip, then move away to see it fade out.")
        .child(control(
            if state.tooltip_slow {
                "Delay: 900 ms"
            } else {
                "Delay: 250 ms"
            },
            cx,
            |s| {
                s.tooltip_slow = !s.tooltip_slow;
            },
        ))
        .child(
            card("Hover target").child(
                label("Hover here", 19., INK)
                    .width(200.)
                    .padding(14.)
                    .background(0xc3e3da)
                    .tooltip(
                        Tooltip::new(tip)
                            .delay(Duration::from_millis(delay))
                            .fade_in(Duration::from_millis(180))
                            .fade_out(Duration::from_millis(250)),
                    ),
            ),
        )
}
