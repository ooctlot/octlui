use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    geometry::{Color, Fill},
    style::*,
};

pub(crate) const CATEGORY: &str = "Rendering";
pub(crate) const TITLE: &str = "Image fit";

pub(crate) fn view(state: &mut Gallery, _: &mut Cx) -> Node {
    let sample =
        |title, fit| card(title).child(image_preview(state.image, fit, Fill::solid(Color::WHITE)));
    page("Each box displays the same image using a different fit mode.")
        .child(row().children([
            sample("Contain", ImageFit::Contain),
            sample("Cover", ImageFit::Cover),
        ]))
        .child(row().children([
            sample("Fill", ImageFit::Fill),
            sample("None", ImageFit::None),
        ]))
}
