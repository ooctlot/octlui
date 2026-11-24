use crate::{Cx, Gallery, ui::*};
use octlui::{Node, components::Text, node, style::*};

pub(crate) const CATEGORY: &str = "Text";
pub(crate) const TITLE: &str = "Text Wrap";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    let sample = |name, mode| {
        card(name).child(
            node().width(200.).background(0xe5efed).text(
                Text::new("Wrapping follows the width.\nThis next line keeps wrapping.")
                    .size(17.)
                    .color(INK)
                    .wrap(mode),
            ),
        )
    };
    page("Each sample has the same width and source text.")
        .child(row().children([
            sample("Words", TextWrap::Words),
            sample("Letters", TextWrap::Letters),
        ]))
        .child(row().children([
            sample("Ellipsis", TextWrap::Clip { ellipsis: true }),
            sample("Fade", TextWrap::Fade { length: 55. }),
        ]))
        .child(row().children([
            sample("Hard breaks", TextWrap::NewLines),
            sample("Clip", TextWrap::Clip { ellipsis: false }),
        ]))
}
