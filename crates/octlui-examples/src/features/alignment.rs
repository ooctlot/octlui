use crate::{Cx, Gallery, ui::*};
use octlui::{Node, node, style::*};

pub(crate) const CATEGORY: &str = "Layout";
pub(crate) const TITLE: &str = "Child alignment";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let (align, name) = [
        (Align::Start, "Start"),
        (Align::Center, "Center"),
        (Align::End, "End"),
    ][state.align];
    page("Change the horizontal and vertical alignment of the same child.")
        .child(control(name, cx, |s| s.align = (s.align + 1) % 3))
        .child(
            card("Parent bounds").child(
                node()
                    .size(grow(), 165.)
                    .padding(8.)
                    .align(align, align)
                    .background(0xe5efed)
                    .child(
                        label("Child", 17., INK)
                            .size(98., 48.)
                            .padding(10.)
                            .background(0xc3e3da),
                    ),
            ),
        )
}
