use crate::{
    Cx, Gallery,
    spring_demo::{Target, demo},
};
use octlui::Node;

pub(crate) const CATEGORY: &str = "Motion";
pub(crate) const TITLE: &str = "Floating offset spring";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    demo(
        state,
        cx,
        Target::FloatingOffset,
        "Move a floating badge relative to a fixed anchor.",
    )
}
