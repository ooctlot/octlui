use crate::{Cx, Gallery, ui::*};
use octlui::{Node, components::Text, node, style::*};

pub(crate) const CATEGORY: &str = "Text";
pub(crate) const TITLE: &str = "Text alignment";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    page("Text alignment positions each line within the text box.").child(
        card("Horizontal alignment").children(
            [
                ("Start", Align::Start),
                ("Center", Align::Center),
                ("End", Align::End),
            ]
            .map(|(name, align)| {
                node()
                    .width(grow())
                    .padding(10.)
                    .background(0xe5efed)
                    .text(Text::new(name).size(20.).color(INK).align(align))
            }),
        ),
    )
}
