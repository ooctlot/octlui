use crate::{Cx, Gallery, ui::*};
use octlui::{Node, builder};

pub(crate) const CATEGORY: &str = "Application";
pub(crate) const TITLE: &str = "Custom builders";

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
    page("Use builder macros to declare constructors and chainable setters for your own types.")
        .child(
            card("Custom builder")
                .child(note(
                    "#[builder] generates the constructor. #[property] generates the setters.",
                ))
                .child(
                    label(sample.label, 17., INK)
                        .width(sample.width)
                        .padding(10.)
                        .background(0xc3e3da)
                        .corner_radius(6.),
                ),
        )
}
