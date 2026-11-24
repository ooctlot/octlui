use crate::{Cx, Gallery, ui::*};
use octlui::{
    Node,
    components::{Text, TextInput},
    geometry::Color,
};

pub(crate) const CATEGORY: &str = "Text";
pub(crate) const TITLE: &str = "Input prompt and colors";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    page("Focus a field to see its caret fade in and out. Type and select a few characters to see the selection color, then clear the field to see the prompt again.")
        .child(
            card("Blue caret and selection").child(input_field(
                "input-style-blue",
                42.,
                Text::new("").input(
                    TextInput::new()
                        .prompt("Search the gallery")
                        .prompt_color(0x36547b)
                        .caret_color(0x1756b0)
                        .selection_color(Color::rgba(0.33, 0.60, 0.92, 0.35)),
                ),
            )),
        )
        .child(
            card("Plum caret and selection").child(input_field(
                "input-style-plum",
                42.,
                Text::new("").input(
                    TextInput::new()
                        .prompt("Leave a comment")
                        .prompt_color(0x684261)
                        .caret_color(0x832667)
                        .selection_color(Color::rgba(0.74, 0.35, 0.63, 0.32)),
                ),
            )),
        )
}
