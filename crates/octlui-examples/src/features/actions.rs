use crate::{Add, Cx, Gallery, Reset, ui::*};
use octlui::{Node, node, style::*};

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "Actions and key bindings";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    page("Click inside the counter to focus its key context. R resets it. Ctrl+P (Cmd+P on macOS) adds ten.")
        .child(card("Scoped counter").child(
            node().column().id("counter-scope").width(grow()).padding(12.).child_gap(8.)
                .background(0xe5efed).corner_radius(7.).focusable(true).key_context("gallery-counter")
                .on_action(cx.listener(|s: &mut Gallery, cx: &mut Cx, action: &Add| {
                    s.count += action.0;
                    cx.notify();
                }))
                .on_action(cx.listener(|s: &mut Gallery, cx: &mut Cx, _: Reset| {
                    s.count = 0;
                    cx.notify();
                }))
                .child(label(state.count.to_string(), 29., INK))
                .child(row().children([
                    button("Add one").on_click_action(Add(1)),
                    button("Reset").on_click_action(Reset),
                ])),
        ))
}
