use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    events::{FocusChangeEvent, KeyDownEvent},
    id::Id,
};

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "Focus and raw keys";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let focused = cx.is_focused(&Id::new("raw-key-target"));
    page("Click the target and press keys to inspect their physical key names.").child(
        card("Focusable target")
            .child(
                label(if focused { "Focused" } else { "Click to focus" }, 20., INK)
                    .id("raw-key-target")
                    .width(245.)
                    .focusable(true)
                    .padding(20.)
                    .background(if focused { 0xc3e3da } else { 0xe5efed })
                    .on_focus_change(
                        cx.listener(|_: &mut Gallery, cx: &mut Cx, _: &FocusChangeEvent| cx.notify()),
                    )
                    .on_key_down(
                        cx.listener(|s: &mut Gallery, cx: &mut Cx, e: &KeyDownEvent| {
                            s.last_key = Some(e.key);
                            cx.notify();
                        }),
                    ),
            )
            .child(note(state.last_key.map_or_else(
                || "Press a key.".into(),
                |key| format!("Last key: {}", key.name()),
            ))),
    )
}
