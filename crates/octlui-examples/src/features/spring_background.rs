use crate::{
    Cx, Gallery,
    spring_demo::{Target, demo},
};
use octlui::Node;

pub(crate) const CATEGORY: &str = "Motion";
pub(crate) const TITLE: &str = "Background spring";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    demo(
        state,
        cx,
        Target::Background,
        "Cycle through a solid, linear gradient, and radial gradient. Click again during motion to retarget.",
    )
}
