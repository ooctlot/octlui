use crate::{
    Cx, Gallery,
    spring_demo::{Target, demo},
};
use octlui::Node;

pub(crate) const CATEGORY: &str = "Motion";
pub(crate) const TITLE: &str = "Height spring";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    demo(
        state,
        cx,
        Target::Height,
        "Change the height and watch the sibling below it move with the animating edge.",
    )
}
