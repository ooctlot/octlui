use crate::{Cx, Gallery, ui::*};
use octlui::{Node, node, style::*};

pub(crate) const CATEGORY: &str = "Layout";
pub(crate) const TITLE: &str = "Axis clipping";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let (clip, name) = [
        (Clip::None, "None"),
        (Clip::X, "X"),
        (Clip::Y, "Y"),
        (Clip::XY, "Both axes"),
    ][state.clip];
    page("The colored child is larger than its outlined parent. Cycle the clip axis to reveal its bounds.")
        .child(control(name, cx, |s| s.clip = (s.clip + 1) % 4))
        .child(card("Overflow").child(
            node().size(grow(), 200.).padding(8.).child(
                node().size(220., 120.).border(ACCENT, 2.).padding(10.).clip(clip).child(
                    label("Oversized child", 18., INK).size(306., 154.).padding(14.)
                        .corner_radius(7.).background(0xe9bf8a),
                ),
            ),
        ))
}
