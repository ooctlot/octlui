use crate::{Cx, Dropped, Gallery, ui::*};
use octlui::{Node, events::FileDropEvent, node, style::*};

pub(crate) const CATEGORY: &str = "Interaction";
pub(crate) const TITLE: &str = "File drops";

pub(crate) fn view(state: &mut Gallery, cx: &mut Cx) -> Node {
    let result = if state.drop_paths.is_empty() {
        note("Dropped paths will appear here.")
    } else {
        node().column().width(grow()).child_gap(5.).children(
            state
                .drop_paths
                .iter()
                .map(|path| note(path.display().to_string())),
        )
    };
    page("Drop files from your file manager. Both targets display the paths, using a listener or a typed action.")
        .on_action(cx.listener(|s: &mut Gallery, cx: &mut Cx, action: Dropped| {
            s.drop_paths = action.0;
            cx.notify();
        }))
        .child(row().children([
            card("Listener").child(
                label("Drop files here", 17., ACCENT).padding(14.).background(0xe5efed)
                    .on_file_drop(cx.listener(|s: &mut Gallery, cx: &mut Cx, event: FileDropEvent| {
                        s.drop_paths = event.paths;
                        cx.notify();
                    })),
            ),
            card("Mapped action").child(
                label("Drop files here", 17., ACCENT).padding(14.).background(0xe5efed)
                    .on_file_drop_action(|e| Dropped(e.paths.clone())),
            ),
        ]))
        .child(card("Paths").child(result))
}
