use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::{FontId, Text},
    node,
    style::*,
};

pub(crate) const CATEGORY: &str = "Text";
pub(crate) const TITLE: &str = "Font selection";

pub(crate) fn view(state: &mut Gallery, _: &mut Cx) -> Node {
    let mut samples = card("Same text in different faces");
    for (name, font) in [
        ("Default", FontId::DEFAULT),
        ("Serif", FontId::SERIF),
        ("Monospace", FontId::MONOSPACE),
    ]
    .into_iter()
    .chain(state.loaded_font.map(|font| ("Registered face", font)))
    {
        samples = samples.child(note(name)).child(
            node().width(grow()).text(
                Text::new("Sphinx of black quartz · 0123456789")
                    .font(font)
                    .size(23.)
                    .color(INK),
            ),
        );
    }
    page("The registered face uses OCTLUI_EXAMPLES_FONT when available.").child(samples)
}
