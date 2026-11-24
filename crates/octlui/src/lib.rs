//! octlui: a retained-state, GPU-rendered UI library with a gpui-inspired API.

extern crate self as octlui;

pub mod action;
pub mod app;
pub mod anim;
pub mod components;
pub mod ctx;
pub mod events;
pub mod geometry;
pub mod headless;
pub mod id;
pub mod image;
pub(crate) mod layout;
pub mod node;
pub(crate) mod paint;
pub mod render;
pub mod style;
pub mod text;
pub(crate) mod ui;

pub use action::{Action, Event, Key, KeyBinding, Keymap, Keystroke, Modifiers};
pub use app::{AppContext, Application, WindowOptions, application};
pub use ctx::{Context, Listener};
pub use node::{Node, node};
pub use octlui_macros::{action, builder, property};
