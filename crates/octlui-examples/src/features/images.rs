use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::Image,
    geometry::{Color, Fill},
    image::ImageId,
    node,
    style::*,
};

pub(crate) const CATEGORY: &str = "Rendering";
pub(crate) const TITLE: &str = "Images and loading";

pub(crate) fn view(state: &mut Gallery, _: &mut Cx) -> Node {
    let preview = |title: &str, id: Option<ImageId>, mode, tint| {
        let mut image = node()
            .size(grow(), 132.)
            .corner_radius(8.)
            .background(0xe5efed);
        if let Some(id) = id {
            image = image.image(Image::new(id).fit(mode).tint(tint));
        }
        node()
            .column()
            .width(grow())
            .child_gap(6.)
            .child(label(title, 14., INK))
            .child(image)
    };
    let white = Fill::solid(Color::WHITE);
    page("PNG bytes and a one-shot supplier decode on demand. Decoded images trigger layout and redraw automatically.")
        .child(card("Registered PNG").child(row().children([
            preview("Contain", state.image, ImageFit::Contain, white),
            preview("Cover", state.image, ImageFit::Cover, white),
        ])))
        .child(card("Worker supplier").child(row().children([
            preview("Stretch", state.lazy_image, ImageFit::Fill, white),
            preview("Tinted", state.lazy_image, ImageFit::Cover,
                Fill::linear(0., &[Color::WHITE, Color::hex(0xfac386)])),
        ])))
}
