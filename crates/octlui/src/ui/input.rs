use super::{Env, NodeState, Ui};
use crate::action::Key;
use crate::events::TextChangeEvent;
use crate::geometry::{Rect, Vec2};
use crate::id::KeyMap;
use crate::text_input::{ClipboardCommand, Command};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub(crate) struct InputHit {
    pub area: Rect,
    pub clip: Rect,
    pub scroll: Vec2,
    pub caret: Option<Rect>,
}

struct Click {
    key: u64,
    point: Vec2,
    at: Instant,
    count: u8,
}

#[derive(Clone, Copy)]
struct Drag {
    key: u64,
    count: u8,
    selection: (usize, usize),
}

pub(super) struct InputState {
    pub(super) hits: KeyMap<InputHit>,
    window_focused: bool,
    generation: u64,
    click: Option<Click>,
    drag: Option<Drag>,
    wait: Option<Duration>,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            hits: KeyMap::default(),
            window_focused: true,
            generation: 0,
            click: None,
            drag: None,
            wait: None,
        }
    }
}

impl InputState {
    pub(super) fn reconcile(&mut self, states: &mut KeyMap<NodeState>, focus: Option<u64>, frame: u64) {
        let had_input = focus.is_some_and(|key| self.hits.contains_key(&key));
        for state in states.values_mut() {
            if state.seen != frame {
                state.editor = None;
            }
        }
        if self.drag.is_some_and(|drag| focus != Some(drag.key)
            || states.get(&drag.key).is_none_or(|state| state.editor.is_none()))
        {
            self.drag = None;
        }
        if had_input && focus.is_some_and(|key| states.get(&key).is_none_or(|state| state.editor.is_none()))
        {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub(super) fn end_drag(&mut self) {
        self.drag = None;
    }

    pub(super) fn cancel_drag(&mut self, key: u64) {
        if self.drag.is_some_and(|drag| drag.key == key) {
            self.end_drag();
        }
    }
}

pub(crate) enum InputKey {
    Unhandled,
    Handled,
    Clipboard(ClipboardCommand),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct PasteTarget {
    key: u64,
    generation: u64,
    revision: u64,
}

#[derive(PartialEq)]
pub(crate) struct ImeTarget {
    pub key: u64,
    pub rect: Rect,
    pub text: Arc<str>,
    pub caret: usize,
    pub anchor: usize,
}

impl Ui {
    pub(super) fn input_focus(&self) -> Option<u64> {
        self.input.window_focused.then_some(self.focus).flatten()
    }

    pub(super) fn focus_to(&mut self, key: Option<u64>) {
        if self.focus == key {
            return;
        }
        let previous_focus = self.focus_snapshot();
        self.reset_input_focus();
        self.focus = key;
        if let Some(editor) = key.and_then(|key| self.states.get_mut(&key))
            .and_then(|state| state.editor.as_mut())
        {
            editor.touch();
        }
        self.queue_focus_change(previous_focus);
    }

    fn reset_input_focus(&mut self) {
        if let Some(editor) = self.focus.and_then(|key| self.states.get_mut(&key))
            .and_then(|state| state.editor.as_mut())
        {
            editor.blur();
        }
        self.input.drag = None;
        self.input.click = None;
        self.input.generation = self.input.generation.wrapping_add(1);
    }

    pub(crate) fn set_window_focus(&mut self, focused: bool) {
        if !focused {
            self.pressed_style = None;
        }
        if !focused && self.drag_source_press.take().is_some() {
            self.pressed = None;
        }
        if self.input.window_focused != focused {
            let previous_focus = self.focus_snapshot();
            self.reset_input_focus();
            self.input.window_focused = focused;
            self.queue_focus_change(previous_focus);
        }
    }

    fn editor_index(&self) -> Option<usize> {
        self.input_focus()?;
        self.focus_idx().filter(|&i| self.arena.nodes[i].text.as_ref().is_some_and(|t| t.input.is_some()))
    }

    pub(crate) fn ime_target(&self) -> Option<ImeTarget> {
        let node = &self.arena.nodes[self.editor_index()?];
        node.text.as_ref()?.input.as_ref()?.enabled.then_some(())?;
        let hit = self.input.hits.get(&node.key)?;
        let editor = self.states.get(&node.key)?.editor.as_ref()?;
        let (caret, anchor) = editor.cursor_bytes();
        // IMEs may hide the caret during a composition. Keep the candidate window anchored.
        Some(ImeTarget { key: node.key, rect: hit.caret.unwrap_or(hit.area), text: editor.value().clone(), caret, anchor })
    }

    pub(super) fn tick_input(&mut self, elapsed: f32) -> bool {
        let (mut animating, wait) = self.input_focus().and_then(|key| self.states.get_mut(&key))
            .and_then(|state| state.editor.as_mut())
            .map_or((false, None), |editor| editor.tick(elapsed));
        self.input.wait = wait;
        if let Some(point) = self.mouse
            && let Some(drag) = self.input.drag
            && self.input.hits.get(&drag.key).is_some_and(|hit| !hit.clip.contains(point))
        {
            animating |= self.drag_input(point);
        }
        animating
    }

    pub(crate) fn input_wait(&self) -> Option<Duration> {
        self.input.wait
    }

    pub(crate) fn text_cursor(&self) -> bool {
        self.mouse.and_then(|p| self.hit_test(p)).is_some_and(|i| {
            self.input.hits.contains_key(&self.arena.nodes[i].key)
        })
    }

    pub(super) fn click_input(&mut self, point: Vec2) -> bool {
        let Some(i) = self.editor_index() else { return false };
        let node = &self.arena.nodes[i];
        let Some(hit) = self.input.hits.get(&node.key) else { return false };
        let Some(editor) = self.states.get_mut(&node.key).and_then(|s| s.editor.as_mut()) else { return false };
        let text = node.text.as_ref().unwrap();
        let input = text.input.as_ref().unwrap();
        let now = Instant::now();
        let extend = self.mods.shift();
        let count = self.input.click.as_ref().filter(|click| {
            !extend && click.key == node.key && now.duration_since(click.at) <= Duration::from_millis(500)
                && (click.point.x - point.x).abs() <= 4.0 && (click.point.y - point.y).abs() <= 4.0
        }).map_or(1, |click| click.count % 3 + 1);
        self.input.click = Some(Click { key: node.key, point, at: now, count });
        editor.cancel_preedit();
        let view = editor.view(&self.input_text, text, input, hit.area);
        editor.click(&view, point, extend, count);
        self.input.drag = Some(Drag { key: node.key, count, selection: editor.drag_anchor() });
        true
    }

    pub(super) fn drag_input(&mut self, point: Vec2) -> bool {
        let Some(drag) = self.input.drag else { return false };
        let Some(hit) = self.input.hits.get(&drag.key) else { return false };
        let Some(i) = self.editor_index() else { return false };
        let node = &self.arena.nodes[i];
        let Some(editor) = self.states.get_mut(&drag.key).and_then(|s| s.editor.as_mut()) else { return false };
        if node.key != drag.key || editor.composing() {
            return false;
        }
        let text = node.text.as_ref().unwrap();
        let view = editor.view(&self.input_text, text, text.input.as_ref().unwrap(), hit.area);
        let revision = editor.revision;
        editor.drag(&view, point, drag.count, drag.selection);
        let changed = revision != editor.revision;
        if changed {
            self.pressed = None;
        }
        changed
    }

    fn tab_focus(&mut self, reverse: bool) -> bool {
        if self.input.hits.is_empty() || !self.input.window_focused {
            return false;
        }
        let mut keys = self.hits.iter().filter_map(|hit| {
            let node = &self.arena.nodes[hit.idx as usize];
            (node.focusable && !hit.rect.intersect(&hit.clip).is_empty()).then_some(node.key)
        }).collect::<Vec<_>>();
        if keys.is_empty() {
            return false;
        }
        if reverse {
            keys.reverse();
        }
        let next = self.focus.and_then(|focus| keys.iter().position(|&key| key == focus))
            .map_or(0, |i| (i + 1) % keys.len());
        self.focus_to(Some(keys[next]));
        true
    }

    pub(crate) fn input_key(&mut self, key: Option<Key>, text: Option<&str>, env: &mut Env) -> InputKey {
        let Some(i) = self.editor_index() else {
            return if self.input_tab(key) { InputKey::Handled } else { InputKey::Unhandled };
        };
        let node = &self.arena.nodes[i];
        let config = node.text.as_ref().unwrap();
        let input = config.input.as_ref().unwrap();
        let Some(editor) = self.states.get_mut(&node.key).and_then(|s| s.editor.as_mut()) else {
            return InputKey::Unhandled;
        };
        if editor.composing() {
            // While the IME owns the composing range, it also owns its navigation keys.
            return InputKey::Handled;
        }
        let mac = cfg!(target_os = "macos");
        let old = editor.value().clone();
        if let Some(command) = key.and_then(|key| Command::for_key(key, self.mods, mac, input.multiline)) {
            if matches!(command, Command::Cancel) && !editor.has_selection() {
                return InputKey::Unhandled;
            }
            self.input.end_drag();
            let view = self.input.hits.get(&node.key)
                .map(|hit| editor.view(&self.input_text, config, input, hit.area));
            let clipboard = editor.command(command, self.mods.shift(), input, view.as_ref());
            self.input_changed(i, old, env);
            return clipboard.map_or(InputKey::Handled, InputKey::Clipboard);
        }
        let printable = !self.mods.meta() && (self.mods.alt() || !self.mods.ctrl())
            && (mac || !self.mods.alt() || self.mods.ctrl());
        if let Some(text) = text.filter(|t| printable && t.chars().any(|c| !c.is_control())) {
            self.input.end_drag();
            editor.insert(text, input, true);
            self.input_changed(i, old, env);
            return InputKey::Handled;
        }
        if self.input_tab(key) { InputKey::Handled } else { InputKey::Unhandled }
    }

    fn input_tab(&mut self, key: Option<Key>) -> bool {
        key == Some(Key::Tab) && !self.mods.ctrl() && !self.mods.alt() && !self.mods.meta()
            && self.tab_focus(self.mods.shift())
    }

    fn input_changed(&mut self, i: usize, old: Arc<str>, env: &mut Env) {
        let node = &mut self.arena.nodes[i];
        let value = self.states.get(&node.key).unwrap().editor.as_ref().unwrap().value();
        if *value == old {
            return;
        }
        node.text.as_mut().unwrap().content = value.clone();
        self.layout_dirty = true;
        self.input.end_drag();
        if let Some(handler) = &node.handlers.text_change {
            env.call(handler, &TextChangeEvent { content: value.clone() });
        }
    }

    pub(crate) fn paste_target(&self) -> Option<PasteTarget> {
        let node = &self.arena.nodes[self.editor_index()?];
        node.text.as_ref()?.input.as_ref()?.enabled.then_some(())?;
        let editor = self.states.get(&node.key)?.editor.as_ref()?;
        Some(PasteTarget { key: node.key, generation: self.input.generation, revision: editor.revision })
    }

    pub(crate) fn paste(&mut self, target: PasteTarget, value: &str, env: &mut Env) -> bool {
        self.paste_target() == Some(target) && self.commit_input(value, env)
    }

    pub(crate) fn commit_input(&mut self, value: &str, env: &mut Env) -> bool {
        let Some(i) = self.editor_index() else { return false };
        let node = &self.arena.nodes[i];
        let config = node.text.as_ref().unwrap().input.as_ref().unwrap();
        let Some(editor) = self.states.get_mut(&node.key).and_then(|state| state.editor.as_mut()) else {
            return false;
        };
        let old = editor.value().clone();
        editor.insert(value, config, false);
        self.input_changed(i, old, env);
        true
    }

    pub(crate) fn cut(&mut self, env: &mut Env) {
        let Some(i) = self.editor_index() else { return };
        let key = self.arena.nodes[i].key;
        let Some(editor) = self.states.get_mut(&key).and_then(|state| state.editor.as_mut()) else { return };
        let old = editor.value().clone();
        editor.cut();
        self.input_changed(i, old, env);
    }

    pub(crate) fn preedit(&mut self, value: &str, cursor: Option<(usize, usize)>) {
        if let Some(i) = self.editor_index() {
            let node = &self.arena.nodes[i];
            let input = node.text.as_ref().unwrap().input.as_ref().unwrap();
            if let Some(editor) = self.states.get_mut(&node.key).and_then(|state| state.editor.as_mut()) {
                editor.preedit(value, cursor, input);
            }
        }
    }

    pub(crate) fn delete_surrounding(&mut self, before: usize, after: usize, env: &mut Env) {
        let Some(i) = self.editor_index() else { return };
        let node = &self.arena.nodes[i];
        if !node.text.as_ref().unwrap().input.as_ref().unwrap().enabled {
            return;
        }
        if let Some(editor) = self.states.get_mut(&node.key).and_then(|state| state.editor.as_mut()) {
            let old = editor.value().clone();
            editor.delete_surrounding(before, after);
            self.input_changed(i, old, env);
        }
    }
}
