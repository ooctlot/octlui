use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::{Text, TextInput},
};

pub(crate) const CATEGORY: &str = "Text";
pub(crate) const TITLE: &str = "Read-only input";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let read_only = state.input_read_only;
    let toggle = if read_only { "Allow editing" } else { "Make read-only" };

    page("Switch editing on and off. In read-only mode you can still move the caret, select text, and copy it.")
        .child(
            card(if read_only { "Read-only" } else { "Editable" })
                .child(input_field(
                    "input-enabled",
                    42.,
                    Text::new("Edit this text, then switch modes.").input(
                        TextInput::new()
                            .enabled(!read_only)
                            .prompt("Enter some text")
                            .caret_color(ACCENT),
                    ),
                ))
                .child(control(toggle, cx, |state| {
                    state.input_read_only = !state.input_read_only;
                })),
        )
}
