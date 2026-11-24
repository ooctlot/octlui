//! Event payloads delivered to node handlers.

use crate::action::{Event, Key, Modifiers};
use crate::geometry::Vec2;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

#[derive(Clone, Debug)]
pub struct ClickEvent {
    pub position: Vec2,
    pub button: MouseButton,
    pub modifiers: Modifiers,
}
impl Event for ClickEvent {}

#[derive(Clone, Debug)]
pub struct MouseDownEvent {
    pub position: Vec2,
    pub button: MouseButton,
    pub modifiers: Modifiers,
}
impl Event for MouseDownEvent {}

#[derive(Clone, Debug)]
pub struct HoverEvent {
    /// `true` when the pointer entered the node, `false` when it left.
    pub hovered: bool,
}
impl Event for HoverEvent {}

#[derive(Clone, Debug)]
pub struct KeyDownEvent {
    pub key: Key,
    pub modifiers: Modifiers,
    /// Text produced by the key press, if any.
    pub text: Option<Arc<str>>,
    pub repeat: bool,
}
impl Event for KeyDownEvent {}

/// An edit to a node's [text input](crate::components::TextInput).
#[derive(Clone, Debug)]
pub struct TextChangeEvent {
    pub content: Arc<str>,
}
impl Event for TextChangeEvent {}

#[derive(Clone, Debug)]
pub struct FileDropEvent {
    pub paths: Vec<PathBuf>,
    pub position: Vec2,
}
impl Event for FileDropEvent {}
