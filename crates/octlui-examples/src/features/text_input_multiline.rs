use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::{Text, TextInput},
    style::TextWrap,
};

pub(crate) const CATEGORY: &str = "Text";
pub(crate) const TITLE: &str = "Multiline input";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let wrap = state.input_wrap;
    let wrap_name = match wrap {
        TextWrap::Letters => "Letter wrapping",
        TextWrap::NewLines => "Manual line breaks",
        _ => "Word wrapping",
    };

    page("Press Enter to insert a line break. Try Up and Down, Home and End, or Page Up and Page Down with Shift held down.")
        .child(
            card(wrap_name)
                .child(row().children([
                    control("Words", cx, |state| state.input_wrap = TextWrap::Words),
                    control("Letters", cx, |state| state.input_wrap = TextWrap::Letters),
                    control("No soft wrap", cx, |state| state.input_wrap = TextWrap::NewLines),
                ]))
                .child(input_field(
                    "input-multiline",
                    156.,
                    Text::new(concat!(
                        "Change the wrap mode while editing this paragraph. ",
                        "Try resizing the window or add a word that is wider than the field.",
                        "\n\nUse the arrow keys to move through the second paragraph.",
                    ))
                    .wrap(wrap)
                    .input(
                        TextInput::new()
                            .multiline(true)
                            .prompt("Write some notes")
                            .caret_color(ACCENT),
                    ),
                )),
        )
}
