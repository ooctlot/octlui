//! Actions, keys and key bindings.

use std::any::{Any, TypeId};
use std::fmt;
use std::ops::BitOr;

/// Marker for values that can be delivered to handlers (clicks, hovers, actions...).
pub trait Event: Any + Clone {}

/// A user-defined command, declared with `#[octlui::action]`.
///
/// Actions are bound to keys with [`KeyBinding`] and dispatched along the focus path
/// (innermost node to root). A matching `on_action` handler may continue to its ancestors
/// with [`crate::Context::propagate_action`].
pub trait Action: Any + fmt::Debug {
    /// Fully qualified name (module path + type name).
    fn name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn boxed_clone(&self) -> Box<dyn Action>;
}
impl dyn Action {
    pub fn type_id_of(&self) -> TypeId {
        self.as_any().type_id()
    }
    pub fn downcast_ref<A: Action>(&self) -> Option<&A> {
        self.as_any().downcast_ref()
    }
}
impl Clone for Box<dyn Action> {
    fn clone(&self) -> Self {
        self.boxed_clone()
    }
}

macro_rules! keys {
    ($($variant:ident => $name:expr),* $(,)?) => {
        /// A physical, layout-independent key.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Key { $($variant),* }
        impl Key {
            pub const ALL: &'static [Key] = &[$(Key::$variant),*];
            /// Lowercase name used by [`Keystroke::parse`].
            pub fn name(self) -> &'static str { match self { $(Key::$variant => $name),* } }
            pub fn from_name(s: &str) -> Option<Key> {
                let s = s.to_ascii_lowercase();
                match s.as_str() { $($name => Some(Key::$variant),)* _ => None }
            }
        }
    };
}
keys! {
    A => "a", B => "b", C => "c", D => "d", E => "e", F => "f", G => "g", H => "h", I => "i",
    J => "j", K => "k", L => "l", M => "m", N => "n", O => "o", P => "p", Q => "q", R => "r",
    S => "s", T => "t", U => "u", V => "v", W => "w", X => "x", Y => "y", Z => "z",
    Num0 => "0", Num1 => "1", Num2 => "2", Num3 => "3", Num4 => "4",
    Num5 => "5", Num6 => "6", Num7 => "7", Num8 => "8", Num9 => "9",
    F1 => "f1", F2 => "f2", F3 => "f3", F4 => "f4", F5 => "f5", F6 => "f6",
    F7 => "f7", F8 => "f8", F9 => "f9", F10 => "f10", F11 => "f11", F12 => "f12",
    Enter => "enter", Escape => "escape", Space => "space", Tab => "tab",
    Backspace => "backspace", Delete => "delete", Insert => "insert",
    Home => "home", End => "end", PageUp => "pageup", PageDown => "pagedown",
    Up => "up", Down => "down", Left => "left", Right => "right",
    Minus => "-", Equal => "=", Comma => ",", Period => ".", Slash => "/",
    Backslash => "\\", Semicolon => ";", Quote => "'", Backquote => "`",
    BracketLeft => "[", BracketRight => "]",
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(u8);
impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);
    pub const SHIFT: Modifiers = Modifiers(1);
    pub const CTRL: Modifiers = Modifiers(2);
    pub const ALT: Modifiers = Modifiers(4);
    /// Command on macOS, Windows/Super key elsewhere.
    pub const META: Modifiers = Modifiers(8);

    /// The platform's "primary" shortcut modifier: `META` on macOS, `CTRL` elsewhere.
    pub const fn primary() -> Modifiers {
        if cfg!(target_os = "macos") { Modifiers::META } else { Modifiers::CTRL }
    }
    pub const fn contains(self, o: Modifiers) -> bool {
        self.0 & o.0 == o.0
    }
    pub const fn shift(self) -> bool {
        self.contains(Modifiers::SHIFT)
    }
    pub const fn ctrl(self) -> bool {
        self.contains(Modifiers::CTRL)
    }
    pub const fn alt(self) -> bool {
        self.contains(Modifiers::ALT)
    }
    pub const fn meta(self) -> bool {
        self.contains(Modifiers::META)
    }
}
impl BitOr for Modifiers {
    type Output = Modifiers;
    fn bitor(self, o: Modifiers) -> Modifiers {
        Modifiers(self.0 | o.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Keystroke {
    pub key: Key,
    pub modifiers: Modifiers,
}
impl Keystroke {
    pub const fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
    /// Parses strings like `"ctrl-shift-s"`, `"alt-enter"`, `"f5"`, `"primary-z"`.
    pub fn parse(s: &str) -> Option<Keystroke> {
        let mut mods = Modifiers::NONE;
        let mut parts: Vec<&str> = s.split('-').collect();
        // A trailing empty part means the key itself is "-".
        let key = if s.ends_with("--") || s == "-" {
            parts.truncate(parts.len().saturating_sub(2));
            Key::Minus
        } else {
            Key::from_name(parts.pop()?)?
        };
        for p in parts {
            mods = mods
                | match p.to_ascii_lowercase().as_str() {
                    "shift" => Modifiers::SHIFT,
                    "ctrl" | "control" => Modifiers::CTRL,
                    "alt" | "option" => Modifiers::ALT,
                    "meta" | "cmd" | "super" | "win" => Modifiers::META,
                    "primary" => Modifiers::primary(),
                    _ => return None,
                };
        }
        Some(Keystroke::new(key, mods))
    }
}
impl From<Key> for Keystroke {
    fn from(key: Key) -> Self {
        Keystroke::new(key, Modifiers::NONE)
    }
}
impl Key {
    /// `Key::S.with(Modifiers::CTRL)`
    pub fn with(self, m: Modifiers) -> Keystroke {
        Keystroke::new(self, m)
    }
}

/// A boolean expression over the active key contexts, e.g. `"editor && !readonly"`.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextPredicate {
    terms: Vec<(bool, String)>, // (negated, identifier), all AND-ed
}
impl ContextPredicate {
    pub fn parse(s: &str) -> Self {
        let terms = s
            .split("&&")
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| match t.strip_prefix('!') {
                Some(rest) => (true, rest.trim().to_string()),
                None => (false, t.to_string()),
            })
            .collect();
        Self { terms }
    }
    /// Returns the depth (index in `stack`) of the deepest matching positive term, or `None`
    /// when the predicate does not hold. Deeper matches win when bindings conflict.
    pub fn matches(&self, stack: &[&str]) -> Option<i32> {
        let mut depth = -1;
        for (neg, id) in &self.terms {
            let pos = stack.iter().rposition(|c| c == id);
            match (neg, pos) {
                (true, Some(_)) | (false, None) => return None,
                (false, Some(p)) => depth = depth.max(p as i32),
                (true, None) => {}
            }
        }
        Some(depth)
    }
}

pub struct KeyBinding {
    pub keystroke: Keystroke,
    pub action: Box<dyn Action>,
    pub context: Option<ContextPredicate>,
}
impl KeyBinding {
    /// ```ignore
    /// KeyBinding::new(Key::R, Reset, Some("counter"))
    /// KeyBinding::new(Key::S.with(Modifiers::primary()), Save, None)
    /// ```
    pub fn new(keystroke: impl Into<Keystroke>, action: impl Action, context: Option<&str>) -> Self {
        Self {
            keystroke: keystroke.into(),
            action: Box::new(action),
            context: context.map(ContextPredicate::parse),
        }
    }
}

#[derive(Default)]
pub struct Keymap {
    bindings: Vec<KeyBinding>,
}
impl Keymap {
    pub fn add(&mut self, b: KeyBinding) {
        self.bindings.push(b);
    }
    pub fn len(&self) -> usize {
        self.bindings.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
    /// Finds the best binding for `ks` given the active contexts (outermost first).
    /// Deeper contexts win; ties go to the binding added last.
    pub fn resolve(&self, ks: Keystroke, stack: &[&str]) -> Option<Box<dyn Action>> {
        let mut best: Option<(i32, usize)> = None;
        for (i, b) in self.bindings.iter().enumerate() {
            if b.keystroke != ks {
                continue;
            }
            let depth = match &b.context {
                None => -1,
                Some(p) => match p.matches(stack) {
                    Some(d) => d,
                    None => continue,
                },
            };
            if best.is_none_or(|(bd, _)| depth >= bd) {
                best = Some((depth, i));
            }
        }
        best.map(|(_, i)| self.bindings[i].action.boxed_clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct A1;
    impl Action for A1 {
        fn name(&self) -> &'static str { "A1" }
        fn as_any(&self) -> &dyn Any { self }
        fn boxed_clone(&self) -> Box<dyn Action> { Box::new(A1) }
    }

    #[test]
    fn parse_keystrokes() {
        assert_eq!(Keystroke::parse("ctrl-shift-s"), Some(Key::S.with(Modifiers::CTRL | Modifiers::SHIFT)));
        assert_eq!(Keystroke::parse("f5"), Some(Key::F5.into()));
        assert_eq!(Keystroke::parse("ctrl--"), Some(Key::Minus.with(Modifiers::CTRL)));
        assert_eq!(Keystroke::parse("bogus-a"), None);
    }

    #[test]
    fn context_matching() {
        let p = ContextPredicate::parse("editor && !readonly");
        assert_eq!(p.matches(&["app", "editor"]), Some(1));
        assert_eq!(p.matches(&["editor", "readonly"]), None);
        assert_eq!(p.matches(&["app"]), None);
    }

    #[test]
    fn keymap_prefers_deeper_context() {
        let mut km = Keymap::default();
        km.add(KeyBinding::new(Key::R, A1, None));
        km.add(KeyBinding::new(Key::R, A1, Some("x")));
        assert!(km.resolve(Key::R.into(), &["x"]).is_some());
        assert!(km.resolve(Key::R.into(), &[]).is_some());
        assert!(km.resolve(Key::Q.into(), &[]).is_none());
    }
}
