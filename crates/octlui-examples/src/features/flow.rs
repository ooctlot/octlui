use crate::{Cx, Gallery, ui::*};
use octlui::{Node, node, style::*};

pub(crate) const CATEGORY: &str = "Layout";
pub(crate) const TITLE: &str = "Flow and alignment";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let (direction, name) = [
        (Direction::LeftToRight, "Left to right"),
        (Direction::RightToLeft, "Right to left"),
        (Direction::TopToBottom, "Top to bottom"),
        (Direction::BottomToTop, "Bottom to top"),
    ][state.direction];
    let (align, align_name) = [
        (Align::Start, "Start"),
        (Align::Center, "Center"),
        (Align::End, "End"),
    ][state.align];
    page("Change the flow and alignment. Start and End refer to the physical edges.")
        .child(row().children([
            control(name, cx, |s| s.direction = (s.direction + 1) % 4),
            control(align_name, cx, |s| s.align = (s.align + 1) % 3),
        ]))
        .child(
            card("Children in declaration order").child(
                node()
                    .size(grow(), 210.)
                    .padding(10.)
                    .align(align, align)
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
