use crate::components::{Appearance, Text};
use crate::geometry::Color;
use crate::style::InteractionState;
use crate::{Node, node};

/// Builds a button whose background lightens on hover and darkens while pressed.
/// Attach a click listener or action to define what the button does.
pub fn button(text: impl Into<Text>) -> Node {
    node()
        .text(text)
        .padding((4.0, 4.0))
        .corner_radius(4.0)
        .background(Color::WHITE)
        .style_when(button_style)
}

pub fn button_style(style: Appearance, state: InteractionState) -> Appearance {
    let prev_background = style.background.clone();
    if state.pressed {
        style.background(
            prev_background.interpolate(crate::geometry::Fill::Solid(Color::WHITE), 0.3),
        )
    } else if state.hovered {
        style.background(
            prev_background.interpolate(crate::geometry::Fill::Solid(Color::WHITE), 0.25),
        )
    } else {
        style
    }
}
