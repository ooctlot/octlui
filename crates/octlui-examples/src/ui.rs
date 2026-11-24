use crate::{Cx, Gallery};
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use octlui::components::Text;
use octlui::widgets::button_style;
use octlui::{
    Node,
    components::{Image, ScrollBar},
    events::ClickEvent,
    geometry::Fill,
    image::ImageId,
    node,
    style::*,
};

pub(crate) const PAPER: u32 = 0xf1f5f4;
pub(crate) const CARD: u32 = 0xfcfdfd;
pub(crate) const INK: u32 = 0x172a2c;
pub(crate) const NAV: u32 = 0x172d32;
pub(crate) const MUTED: u32 = 0xb6c9ca;
pub(crate) const ACCENT: u32 = 0x196963;
pub(crate) const LINE: u32 = 0xd3dfdd;

pub(crate) fn label(text: impl Into<std::sync::Arc<str>>, size: f32, color: u32) -> Node {
    node()
        .width(grow())
        .text(Text::new(text).size(size).color(color))
}

pub(crate) fn note(text: impl Into<std::sync::Arc<str>>) -> Node {
    label(text, 15., 0x485f60)
}

pub(crate) fn page(text: &str) -> Node {
    node()
        .column()
        .width(grow())
        .child_gap(16.)
        .child(note(text))
}

pub(crate) fn card(title: &str) -> Node {
    node()
        .column()
        .width(grow())
        .child_gap(12.)
        .padding(17.)
        .corner_radius(10.)
        .background(CARD)
        .border(LINE, 1.)
        .child(label(title, 18., INK))
}

pub(crate) fn row() -> Node {
    node().width(grow()).row().child_gap(10.)
}

pub(crate) fn button(text: impl Into<std::sync::Arc<str>>) -> Node {
    node()
        .text(Text::new(text).size(14.).color(INK).wrap(TextWrap::None))
        .padding((12., 8.))
        .corner_radius(6.)
        .background(0xc3e3da)
        .style_when(button_style)
}

pub(crate) fn control(text: &str, cx: &Cx, change: impl Fn(&mut Gallery) + 'static) -> Node {
    button(text).on_click(
        cx.listener(move |s: &mut Gallery, cx: &mut Cx, _: &ClickEvent| {
            change(s);
            cx.notify();
        }),
    )
}

pub(crate) fn bar() -> ScrollBar {
    ScrollBar::scroll_bar().thickness(7.).bar_color(0x719991)
}

pub(crate) fn input_field(id: &'static str, height: f32, text: Text) -> Node {
    node()
        .id(id)
        .size(grow(), height)
        .padding((12., 8.))
        .corner_radius(6.)
        .background(PAPER)
        .border(LINE, 1.)
        .text(text.size(17.).color(INK))
}

pub(crate) fn image_preview(id: Option<ImageId>, mode: ImageFit, tint: Fill) -> Node {
    let preview = node()
        .size(grow(), 132.)
        .corner_radius(8.)
        .background(0xe5efed);
    if let Some(id) = id {
        preview.image(Image::new(id).fit(mode).tint(tint))
    } else {
        preview.child(note("Image unavailable"))
    }
}

pub(crate) fn demo_png(alternate: bool) -> std::io::Result<Vec<u8>> {
    let (width, height) = (144u32, 88u32);
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let sun = (x as i32 - 104).pow(2) + (y as i32 - 24).pow(2) < 13 * 13;
            let ridge = y > 38 + x.abs_diff(55) / 2;
            let foreground = y > 59 + x.abs_diff(95) / 5;
            let color = match (alternate, sun, foreground, ridge) {
                (_, true, _, _) => [248, 207, 123],
                (true, _, true, _) => [53, 72, 120],
                (true, _, _, true) => [101, 114, 163],
                (false, _, true, _) => [36, 91, 82],
                (false, _, _, true) => [97, 147, 126],
                (_, _, _, _) => [181, 214, 211],
            };
            pixels.extend_from_slice(&color);
        }
    }
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, width, height, ExtendedColorType::Rgb8)
        .map_err(std::io::Error::other)?;
    Ok(bytes)
}
