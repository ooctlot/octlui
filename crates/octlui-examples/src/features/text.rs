use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::{FontId, Text},
    node,
    style::*,
};

pub(crate) const CATEGORY: &str = "Rendering";
pub(crate) const TITLE: &str = "Text and Unicode";

pub(crate) fn view(state: &mut Gallery, _: &mut Cx) -> Node {
    let sample = "Spacing and line breaks adapt as the width changes.";
    let wrap = |name, mode| {
        node()
            .column()
            .width(grow())
            .padding((12., 5.))
            .child_gap(3.)
            .child(label(name, 13., ACCENT))
            .child(
                node()
                    .width(grow())
                    .text(Text::new(sample).size(18.).color(INK).wrap(mode)),
            )
    };
    page("The gallery enables Unicode shaping and system font fallback. Compare scripts and wrap modes below.")
        .child(card("Shaping and fallback").children([
            label("العربية · हिन्दी · 日本語 · 👩🏽‍💻", 30., INK),
            label("Order 123 · שלום עולם · 456", 24., INK),
            node().width(grow()).text(Text::new("Registered font • Sphinx of black quartz.")
                .font(state.loaded_font.unwrap_or(FontId::SERIF)).size(21.).color(INK)),
        ]))
        .child(card("Wrap modes").child(row().children([
            wrap("Word wrap", TextWrap::Words),
            wrap("Letter wrap", TextWrap::Letters),
        ])).child(row().children([
            wrap("Ellipsis", TextWrap::Clip { ellipsis: true }),
            wrap("Fade", TextWrap::Fade { length: 55. }),
        ])))
}
