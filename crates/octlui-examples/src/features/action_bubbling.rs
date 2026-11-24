use crate::{Cx, Gallery, ui::*};
use octlui::{Node, action, node, style::*};

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "Action propagation";

#[action]
struct Record(bool);

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    page("The focused child handles the action first. It can continue dispatch to the parent.")
        .on_action(cx.listener(|s: &mut Gallery, cx: &mut Cx, _: &Record| {
            s.parent_actions += 1;
            cx.notify();
        }))
        .child(
            card("Dispatch from the focused child").child(
                node()
                    .column()
                    .width(grow())
                    .child_gap(10.)
                    .focusable(true)
                    .on_action(
                        cx.listener(|s: &mut Gallery, cx: &mut Cx, action: &Record| {
                            s.child_actions += 1;
                            if action.0 {
                                cx.propagate_action();
                            }
                            cx.notify();
                        }),
                    )
                    .child(row().children([
                        button("Handle here").on_click_action(Record(false)),
                        button("Continue to parent").on_click_action(Record(true)),
                    ]))
                    .child(note(format!(
                        "Child: {} · Parent: {}",
                        state.child_actions, state.parent_actions
                    ))),
            ),
        )
}
