use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    geometry::{Color, Fill},
    style::*,
};

pub(crate) const CATEGORY: &str = "Rendering";
pub(crate) const TITLE: &str = "Image loading";

pub(crate) fn view(state: &mut Gallery, _: &mut Cx) -> Node {
    page("The first image uses registered PNG bytes. The second runs a one-shot supplier on the decoding worker when first displayed.")
        .child(row().children([
            card("Registered bytes").child(image_preview(state.image, ImageFit::Contain, Fill::solid(Color::WHITE))),
            card("Worker supplier").child(image_preview(state.lazy_image, ImageFit::Contain, Fill::solid(Color::WHITE))),
        ]))
}
