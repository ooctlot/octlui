use crate::{Cx, Gallery, ui::*};
use octlui::{Node, node, style::*};

pub(crate) const CATEGORY: &str = "Layout";
pub(crate) const TITLE: &str = "Flow direction";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let (direction, name) = [
        (Direction::LeftToRight, "Left to right"),
        (Direction::RightToLeft, "Right to left"),
        (Direction::TopToBottom, "Top to bottom"),
        (Direction::BottomToTop, "Bottom to top"),
    ][state.direction];
    page("Cycle through the four directions to see where the first, second and third children appear.")
        .child(control(name, cx, |s| s.direction = (s.direction + 1) % 4))
        .child(
            card("Children in declaration order").child(
                node()
                    .size(grow(), 210.)
                    .padding(10.)
                    .child_gap(10.)
                    .direction(direction)
                    .background(0xe5efed)
                    .corner_radius(8.)
                    .children((1..=3).map(|i| {
                        label(i.to_string(), 16., INK)
                            .size(38. + i as f32 * 16., 38.)
                            .padding((10., 8.))
                            .corner_radius(5.)
                            .background([0xc4ddea, 0xc3e3da, 0xf3dfb6][i - 1])
                    })),
            ),
        )
}
