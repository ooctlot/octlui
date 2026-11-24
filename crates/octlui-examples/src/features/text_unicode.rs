use crate::{Cx, Gallery, ui::*};
use octlui::Node;

pub(crate) const CATEGORY: &str = "Text";
pub(crate) const TITLE: &str = "Text Unicode";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    page("Unicode shaping combines characters into glyphs. Font fallback selects a face for each script.")
        .child(card("Shaping and fallback").children([
            label("العربية · हिन्दी · 日本語 · 👩🏽‍💻", 30., INK),
            label("Order 123 · שלום עולם · 456", 24., INK),
            label("Combining marks · cafe\u{0301} · a\u{030a}", 24., INK),
        ]))
}
