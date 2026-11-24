use crate::components::{Appearance};
use crate::geometry::Color;
use crate::style::InteractionState;

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
