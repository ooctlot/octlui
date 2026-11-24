use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::{Text, TextInput},
    events::TextChangeEvent,
};

pub(crate) const CATEGORY: &str = "Text";
pub(crate) const TITLE: &str = "Text input";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let shortcuts = if cfg!(target_os = "macos") {
        "Use Cmd+A to select all, Cmd+C or Cmd+X to copy or cut, and Cmd+V to paste. Undo with Cmd+Z and redo with Cmd+Shift+Z."
    } else {
        "Use Ctrl+A to select all, Ctrl+C or Ctrl+X to copy or cut, and Ctrl+V to paste. Undo with Ctrl+Z and redo with Ctrl+Y."
    };
    let value = if state.input_value.is_empty() {
        "Current value: empty".into()
    } else {
        format!("Current value: {}", state.input_value)
    };

    page("Click a field or press Tab to focus it. Drag to select text, or hold Shift while moving the caret.")
        .child(
            card("Editing and change events")
                .child(
                    input_field(
                        "input-value",
                        42.,
                        Text::new(state.input_value.clone()).input(
                            TextInput::new()
                                .prompt("Try typing café, 日本語, or 👩🏽‍💻")
                                .caret_color(ACCENT),
                        ),
                    )
                    .on_text_change(cx.listener(
                        |state: &mut Gallery, cx: &mut Cx, event: TextChangeEvent| {
                            state.input_value = event.content;
                            cx.notify();
                        },
                    )),
                )
                .child(note(value)),
        )
        .child(
            card("Clipboard")
                .child(input_field(
                    "input-clipboard",
                    42.,
                    Text::new("").input(
                        TextInput::new().prompt("Paste here").caret_color(ACCENT),
                    ),
                ))
                .child(note(shortcuts)),
        )
}
