//! The declarative [`Node`] tree returned by view functions.

use crate::action::{Action, Event};
use crate::components::*;
use crate::ctx::{ErasedFn, Listener};
use crate::events::*;
use crate::geometry::{CornerRadius, Fill, Padding};
use crate::id::Id;
use crate::style::{Align, Clip, Direction, Sizing};
use std::any::TypeId;
use std::rc::Rc;
use std::sync::Arc;

type FileDropAction = Rc<dyn Fn(&FileDropEvent) -> Box<dyn Action>>;

#[derive(Clone, Default)]
pub(crate) struct Handlers {
    pub click: Option<ErasedFn>,
    pub mouse_down: Option<ErasedFn>,
    pub hover: Option<ErasedFn>,
    pub key_down: Option<ErasedFn>,
    pub text_change: Option<ErasedFn>,
    pub file_drop: Option<ErasedFn>,
    pub file_drop_action: Option<FileDropAction>,
    pub actions: Vec<(TypeId, ErasedFn)>,
    /// Dispatched (through the action system) when the node is clicked with the left button.
    pub click_action: Option<Box<dyn Action>>,
}

/// A member of the layout.
///
/// Every node has sizing, child alignment, direction and gap. Everything else (id, padding,
/// corners, aspect ratio, clip, handlers) is optional, and heavier features live in optional
/// components: [`Appearance`], [`Text`], [`Image`], [`Tooltip`], [`Scroll`], [`Floating`]
/// and [`Spring`].
#[derive(Clone, Default)]
#[octlui::builder(free)]
pub struct Node {
    /// Identifies the node across frames and in debug output.
    #[octlui::property(into)]
    pub(crate) id: Option<Id>,
    #[octlui::property(into)]
    pub(crate) width: Sizing,
    #[octlui::property(into)]
    pub(crate) height: Sizing,
    #[octlui::property]
    pub(crate) direction: Direction,
    #[octlui::property]
    pub(crate) child_align_x: Align,
    #[octlui::property]
    pub(crate) child_align_y: Align,
    /// Space between consecutive children.
    #[octlui::property]
    pub(crate) child_gap: f32,
    #[octlui::property(into)]
    pub(crate) padding: Padding,
    #[octlui::property(into)]
    pub(crate) corner_radius: CornerRadius,
    /// Keep `width / height` at this ratio.
    #[octlui::property]
    pub(crate) aspect_ratio: Option<f32>,
    #[octlui::property]
    pub(crate) clip: Clip,
    /// Name used to select key bindings (see `KeyBinding`).
    #[octlui::property(into)]
    pub(crate) key_context: Option<Arc<str>>,
    /// Clicking the node (or its descendants) focuses it.
    #[octlui::property]
    pub(crate) focusable: bool,
    #[octlui::property(boxed)]
    pub(crate) appearance: Option<Box<Appearance>>,
    #[octlui::property(boxed, into)]
    pub(crate) text: Option<Box<Text>>,
    #[octlui::property(boxed, into)]
    pub(crate) image: Option<Box<Image>>,
    #[octlui::property(boxed)]
    pub(crate) tooltip: Option<Box<Tooltip>>,
    #[octlui::property(boxed)]
    pub(crate) scroll: Option<Box<Scroll>>,
    #[octlui::property(boxed)]
    pub(crate) floating: Option<Box<Floating>>,
    #[octlui::property(boxed)]
    pub(crate) spring: Option<Box<Spring>>,
    pub(crate) handlers: Handlers,
    pub(crate) children: Vec<Node>,
}

impl Node {
    /// Appends a child.
    #[must_use]
    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }
    /// Appends all `children`.
    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(children);
        self
    }
    /// Sets both alignments.
    #[must_use]
    pub fn align(mut self, x: Align, y: Align) -> Self {
        self.child_align_x = x;
        self.child_align_y = y;
        self
    }
    /// Centers children on both axes.
    #[must_use]
    pub fn center(self) -> Self {
        self.align(Align::Center, Align::Center)
    }
    /// Shorthand for `.direction(Direction::LeftToRight)`.
    #[must_use]
    pub fn row(self) -> Self {
        self.direction(Direction::LeftToRight)
    }
    /// Shorthand for `.direction(Direction::TopToBottom)`.
    #[must_use]
    pub fn column(self) -> Self {
        self.direction(Direction::TopToBottom)
    }
    /// Shorthand for `width(w).height(h)`.
    #[must_use]
    pub fn size(self, w: impl Into<Sizing>, h: impl Into<Sizing>) -> Self {
        self.width(w).height(h)
    }
    /// Grow on both axes.
    #[must_use]
    pub fn fill_parent(self) -> Self {
        self.size(crate::style::grow(), crate::style::grow())
    }

    fn appearance_mut(&mut self) -> &mut Appearance {
        self.appearance.get_or_insert_with(Default::default)
    }
    /// Sets the background, creating an [`Appearance`] if needed.
    #[must_use]
    pub fn background(mut self, fill: impl Into<Fill>) -> Self {
        self.appearance_mut().background = fill.into();
        self
    }
    /// Sets the border, creating an [`Appearance`] if needed.
    #[must_use]
    pub fn border(mut self, color: impl Into<Fill>, width: f32) -> Self {
        let a = self.appearance_mut();
        a.border_color = color.into();
        a.border_width = width;
        self
    }

    // ---- handlers ----

    /// Called when the primary button is pressed and released on this node.
    #[must_use]
    pub fn on_click(mut self, l: Listener<ClickEvent>) -> Self {
        self.handlers.click = Some(l.f);
        self
    }
    /// Dispatches `action` when the node is clicked.
    #[must_use]
    pub fn on_click_action(mut self, action: impl Action) -> Self {
        self.handlers.click_action = Some(Box::new(action));
        self
    }
    #[must_use]
    pub fn on_mouse_down(mut self, l: Listener<MouseDownEvent>) -> Self {
        self.handlers.mouse_down = Some(l.f);
        self
    }
    /// Called when the pointer enters or leaves the node or its descendants.
    #[must_use]
    pub fn on_hover(mut self, l: Listener<HoverEvent>) -> Self {
        self.handlers.hover = Some(l.f);
        self
    }
    /// Called for key presses when this node is the innermost focused node with such a handler
    /// and no key binding matched.
    #[must_use]
    pub fn on_key_down(mut self, l: Listener<KeyDownEvent>) -> Self {
        self.handlers.key_down = Some(l.f);
        self
    }
    /// Called when this node's text input changes, including during undo and redo.
    #[must_use]
    pub fn on_text_change(mut self, l: Listener<TextChangeEvent>) -> Self {
        self.handlers.text_change = Some(l.f);
        self
    }
    /// Called when files are dropped onto the node.
    #[must_use]
    pub fn on_file_drop(mut self, l: Listener<FileDropEvent>) -> Self {
        self.handlers.file_drop = Some(l.f);
        self
    }
    /// Maps dropped files to an action routed from this node up to the root.
    #[must_use]
    pub fn on_file_drop_action<A: Action>(
        mut self,
        map: impl Fn(&FileDropEvent) -> A + 'static,
    ) -> Self {
        self.handlers.file_drop_action = Some(Rc::new(move |event| Box::new(map(event))));
        self
    }
    /// Handles action `A` dispatched at or below this node.
    #[must_use]
    pub fn on_action<A: Action + Event>(mut self, l: Listener<A>) -> Self {
        let tid = TypeId::of::<A>();
        self.handlers.actions.retain(|(t, _)| *t != tid);
        self.handlers.actions.push((tid, l.f));
        self
    }

}
