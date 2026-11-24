use crate::{Cx, Gallery, ui::*};
use octlui::{Node, builder, style::*};

pub(crate) const CATEGORY: &str = "Application";
pub(crate) const TITLE: &str = "Windows and builders";

#[derive(Default)]
#[builder(name = "new")]
struct ExampleCardConfig {
    #[octlui::property]
    width: f32,
    #[octlui::property(into)]
    label: String,
}

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    let sample = ExampleCardConfig::new()
        .width(235.)
        .label("Built with #[property]");
    page("The gallery and counter each own a window and separate state. Press Esc in either window to close it.")
        .child(card("Custom builder").child(note(
            "#[builder] generates the constructor. #[property] generates the setters.",
        )).child(label(sample.label, 17., INK).width(sample.width)
            .padding(10.).background(0xc3e3da).corner_radius(6.)))
        .child(card("Run the other examples").children([
            note("cargo run -p octlui --example counter"),
            note("cargo run -p octlui --example showcase"),
            note("cargo run -p octlui --example headless"),
        ]).width(grow()))
}
