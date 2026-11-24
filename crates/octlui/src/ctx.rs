//! [`Context`], listeners and effects.

use crate::action::{Action, Event};
use crate::geometry::Vec2;
use crate::id::Id;
use std::any::Any;
use std::marker::PhantomData;
use std::rc::Rc;

/// A type-erased handler. The return value requests further action propagation.
pub(crate) type ErasedFn = Rc<dyn Fn(&mut dyn Any, &mut dyn Any, &dyn Any) -> bool>;

/// A handler bound to an app state `S`, produced by [`Context::listener`].
pub struct Listener<E> {
    pub(crate) f: ErasedFn,
    _p: PhantomData<fn(&E)>,
}
impl<E> Clone for Listener<E> {
    fn clone(&self) -> Self {
        Self { f: self.f.clone(), _p: PhantomData }
    }
}

/// Marker: handler takes the event by reference.
pub struct ByRef;
/// Marker: handler takes the event by value (needs `Event`).
pub struct ByValue;

/// Anything callable as `fn(&mut S, &mut Context<S>, &E)` or `fn(&mut S, &mut Context<S>, E)`.
pub trait Handler<S, E, M>: 'static {
    fn call(&self, state: &mut S, cx: &mut Context<S>, event: &E);
}
impl<S, E, F> Handler<S, E, ByRef> for F
where
    F: Fn(&mut S, &mut Context<S>, &E) + 'static,
{
    fn call(&self, state: &mut S, cx: &mut Context<S>, event: &E) {
        self(state, cx, event)
    }
}
impl<S, E, F> Handler<S, E, ByValue> for F
where
    E: Event,
    F: Fn(&mut S, &mut Context<S>, E) + 'static,
{
    fn call(&self, state: &mut S, cx: &mut Context<S>, event: &E) {
        self(state, cx, event.clone())
    }
}

/// Side effects requested by handlers; applied by the window after the handler returns.
pub(crate) enum Effect {
    Notify,
    Quit,
    CloseWindow,
    Dispatch(Box<dyn Action>),
    Focus(Id),
    Blur,
    SetTitle(String),
}

/// Read-only window facts, refreshed before handlers run.
#[derive(Clone, Default)]
pub(crate) struct WindowInfo {
    pub size: Vec2,
    pub scale: f32,
    pub focus: Option<u64>,
    pub hover: Vec<u64>,
}

/// Passed to views and handlers. It collects requests (re-render, dispatch an action, focus...)
/// that the window applies once the handler returns.
pub struct Context<S> {
    pub(crate) effects: Vec<Effect>,
    pub(crate) info: WindowInfo,
    propagate_action: bool,
    _p: PhantomData<fn(&mut S)>,
}

impl<S: 'static> Context<S> {
    pub(crate) fn new() -> Self {
        Self {
            effects: Vec::new(),
            info: WindowInfo { scale: 1.0, ..Default::default() },
            propagate_action: false,
            _p: PhantomData,
        }
    }

    /// Wraps `handler` so it can be attached to a node (`on_click`, `on_action`, ...).
    ///
    /// The event type is inferred from the handler signature.
    pub fn listener<E: 'static, M, H: Handler<S, E, M>>(&self, handler: H) -> Listener<E> {
        let f: ErasedFn = Rc::new(move |state, cx, ev| {
            if let (Some(s), Some(c), Some(e)) =
                (state.downcast_mut::<S>(), cx.downcast_mut::<Context<S>>(), ev.downcast_ref::<E>())
            {
                c.propagate_action = false;
                handler.call(s, c, e);
                return std::mem::take(&mut c.propagate_action);
            }
            false
        });
        Listener { f, _p: PhantomData }
    }

    /// Requests a re-run of the view function.
    pub fn notify(&mut self) {
        self.effects.push(Effect::Notify);
    }
    /// Dispatches `action` from the focused node (or the root).
    pub fn dispatch(&mut self, action: impl Action) {
        self.effects.push(Effect::Dispatch(Box::new(action)));
    }
    /// Allows the action being handled to continue to matching ancestor handlers.
    /// Call this from an `on_action` listener.
    pub fn propagate_action(&mut self) {
        self.propagate_action = true;
    }
    pub fn focus(&mut self, id: impl Into<Id>) {
        self.effects.push(Effect::Focus(id.into()));
    }
    pub fn blur(&mut self) {
        self.effects.push(Effect::Blur);
    }
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.effects.push(Effect::SetTitle(title.into()));
    }
    /// Closes the application.
    pub fn quit(&mut self) {
        self.effects.push(Effect::Quit);
    }
    /// Closes the window this context belongs to.
    pub fn close_window(&mut self) {
        self.effects.push(Effect::CloseWindow);
    }
    /// Window size in logical pixels.
    pub fn window_size(&self) -> Vec2 {
        self.info.size
    }
    pub fn scale_factor(&self) -> f32 {
        self.info.scale
    }
    pub fn is_focused(&self, id: &Id) -> bool {
        self.info.focus == Some(id.hash)
    }
    /// Whether the pointer is over `id` or one of its descendants.
    pub fn is_hovered(&self, id: &Id) -> bool {
        self.info.hover.contains(&id.hash)
    }
}
