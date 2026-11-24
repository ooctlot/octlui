use crate::{
    Cx, Gallery,
    spring_demo::{Target, demo},
};
use octlui::Node;

pub(crate) const CATEGORY: &str = "Motion";
pub(crate) const TITLE: &str = "Corner radius spring";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    demo(
        state,
        cx,
        Target::CornerRadius,
        "Change the four corner radii together.",
    )
}
