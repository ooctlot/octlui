use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    geometry::{Color, Fill},
    style::*,
};

pub(crate) const CATEGORY: &str = "Rendering";
pub(crate) const TITLE: &str = "Image tint";

pub(crate) fn view(state: &mut Gallery, _: &mut Cx) -> Node {
    page("The renderer multiplies the same image by each tint.").child(row().children([
        card("White").child(image_preview(
            state.image,
            ImageFit::Cover,
            Fill::solid(Color::WHITE),
        )),
        card("Solid").child(image_preview(
            state.image,
            ImageFit::Cover,
            Fill::solid(0xfac386),
        )),
        card("Gradient").child(image_preview(
            state.image,
            ImageFit::Cover,
            Fill::linear(0., &[Color::WHITE, Color::hex(0xd18e82)]),
        )),
    ]))
}
