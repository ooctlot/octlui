use crate::{Close, Cx, Gallery, ui::*};
use octlui::Node;

pub(crate) const CATEGORY: &str = "Application";
pub(crate) const TITLE: &str = "Independent windows";

pub(crate) fn view(_: &mut Gallery, _: &mut Cx) -> Node {
    page("The gallery and counter each own their window and state. Closing one leaves the other running.")
        .child(card("Window controls")
            .child(note("Press Esc in the counter window to close it, or close the gallery with the button below."))
            .child(button("Close gallery").on_click_action(Close)))
}
