use crate::{
    Cx, Gallery,
    spring_demo::{Target, demo},
};
use octlui::Node;

pub(crate) const CATEGORY: &str = "Motion";
pub(crate) const TITLE: &str = "Text color spring";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    demo(
        state,
        cx,
        Target::TextColor,
        "Cycle the fill used to paint the text.",
    )
}
