use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    events::{HoverEvent, MouseDownEvent},
};

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "Pointer events";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    page("Move into the target, then press a mouse button to trigger its mouse-down listener.")
        .child(
            card("Hover and mouse down")
                .child(
                    label(
                        if state.hovering {
                            "Pointer inside"
                        } else {
                            "Pointer outside"
                        },
                        20.,
                        INK,
                    )
                    .width(250.)
                    .padding(24.)
                    .corner_radius(9.)
                    .background(if state.hovering { 0xc3e3da } else { 0xe5efed })
                    .on_hover(cx.listener(|s: &mut Gallery, cx: &mut Cx, e: &HoverEvent| {
                        s.hovering = e.hovered;
                        cx.notify();
                    }))
                    .on_mouse_down(cx.listener(
                        |s: &mut Gallery, cx: &mut Cx, _: &MouseDownEvent| {
                            s.presses += 1;
                            cx.notify();
                        },
                    )),
                )
                .child(note(format!("Mouse-down events: {}", state.presses))),
        )
}
