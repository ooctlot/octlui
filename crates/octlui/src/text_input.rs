//! Retained state, editing commands and shaped geometry for text inputs.

use crate::action::{Key, Modifiers};
use crate::components::{Text, TextInput};
use crate::geometry::{Rect, Vec2};
use crate::style::TextWrap;
use crate::text::ShapedText;
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

mod geometry;

pub(crate) use geometry::{InputLine, input_line_x};
use geometry::InputView;

pub(crate) fn input_wrap(text: &Text, input: &TextInput) -> TextWrap {
    match (input.multiline, text.wrap) {
        (false, _) => TextWrap::None,
        (true, wrap @ (TextWrap::Words | TextWrap::Letters)) => wrap,
        (true, _) => TextWrap::NewLines,
    }
}

pub(crate) fn normalize(value: &Arc<str>, multiline: bool) -> Arc<str> {
    if value.chars().any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\t'))) {
        Arc::from(clean_text(value, multiline))
    } else {
        value.clone()
    }
}

fn clean_text(value: &str, multiline: bool) -> String {
    let mut clean = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                clean.push(if multiline { '\n' } else { ' ' });
            }
            '\n' | '\t' if !multiline => clean.push(' '),
            '\n' | '\t' => clean.push(c),
            c if !c.is_control() => clean.push(c),
            _ => {}
        }
    }
    clean
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Caret {
    byte: usize,
    trailing: bool,
}

impl Caret {
    fn leading(byte: usize) -> Self {
        Self { byte, trailing: false }
    }

    fn trailing(byte: usize) -> Self {
        Self { byte, trailing: true }
    }
}

#[derive(Clone)]
struct Snapshot {
    value: Arc<str>,
    caret: Caret,
    anchor: Caret,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Group {
    Typing,
    Backspace,
    Delete,
}

#[derive(Clone, Copy)]
pub(crate) enum Motion {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    LineStart,
    LineEnd,
    DocumentStart,
    DocumentEnd,
    ParagraphStart,
    ParagraphEnd,
    NextParagraphStart,
    Word { left: bool, next_start: bool },
    PreviousWord,
    NextWord { to_start: bool },
    Previous,
    Next,
}

pub(crate) enum Command {
    Move(Motion),
    Delete(Motion, Option<Group>),
    SelectAll,
    Cancel,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Overwrite,
    Transpose,
    Kill(Motion),
    Yank,
    Newline,
    OpenLine,
    Center,
}

impl Command {
    pub(crate) fn for_key(key: Key, mods: Modifiers, mac: bool, multiline: bool) -> Option<Self> {
        use Command::*;
        use Key as K;
        use Motion as M;
        match key {
            K::Copy => return Some(Copy),
            K::Cut => return Some(Cut),
            K::Paste => return Some(Paste),
            K::Undo => return Some(Undo),
            K::Redo => return Some(Redo),
            _ => {}
        }
        let (ctrl, alt, meta, shift) = (mods.ctrl(), mods.alt(), mods.meta(), mods.shift());
        if mac && meta && alt && shift && !ctrl && key == K::V {
            return Some(Paste);
        }
        let primary = if mac { meta && !ctrl && !alt } else { ctrl && !alt && !meta };
        if primary {
            match key {
                K::A => return Some(SelectAll),
                K::C => return Some(Copy),
                K::X => return Some(Cut),
                K::V => return Some(Paste),
                K::Z => return Some(if shift { Redo } else { Undo }),
                K::Y if !mac => return Some(Redo),
                K::Insert if !mac && !shift => return Some(Copy),
                _ => {}
            }
        }
        if mac && ctrl && !alt && !meta {
            return Some(match key {
                K::A => Move(M::LineStart),
                K::E => Move(M::LineEnd),
                K::B => Move(M::Previous),
                K::F => Move(M::Next),
                K::P => Move(M::Up),
                K::N => Move(M::Down),
                K::H => Delete(M::Previous, Some(Group::Backspace)),
                K::D => Delete(M::Next, Some(Group::Delete)),
                K::K => Kill(M::ParagraphEnd),
                K::U => Kill(M::ParagraphStart),
                K::W => Kill(M::PreviousWord),
                K::Y => Yank,
                K::T => Transpose,
                K::L => Center,
                K::V => Move(M::PageDown),
                K::O if multiline => OpenLine,
                _ => return None,
            });
        }
        if !mac && alt && !ctrl && !meta && key == K::Backspace {
            return Some(if shift { Redo } else { Undo });
        }
        // Ctrl+Alt can be AltGr and must still be able to enter printable characters.
        if (mac && ctrl) || (mac && meta && alt) || (!mac && (meta || alt)) {
            return None;
        }
        let word = if mac { alt } else { ctrl };
        let document = primary;
        Some(match key {
            K::Left if mac && primary => Move(M::LineStart),
            K::Right if mac && primary => Move(M::LineEnd),
            K::Left | K::Right if word => Move(M::Word { left: key == K::Left, next_start: !mac }),
            K::Left => Move(M::Left),
            K::Right => Move(M::Right),
            K::Up if mac && primary => Move(M::DocumentStart),
            K::Down if mac && primary => Move(M::DocumentEnd),
            K::Up if word => Move(M::ParagraphStart),
            K::Down if word => Move(if mac { M::ParagraphEnd } else { M::NextParagraphStart }),
            K::Up => Move(M::Up),
            K::Down => Move(M::Down),
            K::Home if document => Move(M::DocumentStart),
            K::End if document => Move(M::DocumentEnd),
            K::Home => Move(M::LineStart),
            K::End => Move(M::LineEnd),
            K::PageUp => Move(M::PageUp),
            K::PageDown => Move(M::PageDown),
            K::Backspace if mac && primary => Delete(M::LineStart, None),
            K::Delete if mac && primary => Delete(M::LineEnd, None),
            K::Backspace if word => Delete(M::PreviousWord, None),
            K::Delete if word => Delete(M::NextWord { to_start: !mac }, None),
            K::Backspace => Delete(M::Previous, Some(Group::Backspace)),
            K::Delete if !mac && shift => Cut,
            K::Delete => Delete(M::Next, Some(Group::Delete)),
            K::Insert if !mac && shift => Paste,
            K::Insert if !mac => Overwrite,
            K::Escape if !primary && !alt => Cancel,
            K::Enter if multiline && !primary && !alt => Newline,
            _ => return None,
        })
    }
}

pub(crate) enum ClipboardCommand {
    Copy(Arc<str>),
    Cut(Arc<str>),
    Paste,
}

pub(crate) struct Editor {
    declared: Arc<str>,
    enabled: bool,
    current: Snapshot,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_edit: Option<(Group, Instant)>,
    kill: Arc<str>,
    overwrite: bool,
    preferred_x: Option<f32>,
    preedit: Option<(Arc<str>, Option<(usize, usize)>)>,
    pub scroll: Vec2,
    pub revision: u64,
    blink: f32,
    blink_reset: bool,
}

impl Editor {
    pub fn new(text: &Text, input: &TextInput) -> Self {
        let value = normalize(&text.content, input.multiline);
        let caret = Caret::trailing(value.len());
        Self {
            declared: text.content.clone(),
            enabled: input.enabled,
            current: Snapshot { value, caret, anchor: caret },
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            kill: Arc::from(""),
            overwrite: false,
            preferred_x: None,
            preedit: None,
            scroll: Vec2::ZERO,
            revision: 0,
            blink: 0.0,
            blink_reset: true,
        }
    }

    pub fn reconcile(&mut self, text: &Text, input: &TextInput) {
        let declared_changed = self.declared != text.content;
        self.declared = text.content.clone();
        let value = normalize(
            if declared_changed { &text.content } else { self.value() },
            input.multiline,
        );
        if value != self.current.value {
            let revision = self.revision.wrapping_add(1);
            let mut replacement = Self::new(text, input);
            let caret = Caret::trailing(value.len());
            replacement.current = Snapshot { value, caret, anchor: caret };
            replacement.revision = revision;
            *self = replacement;
        }
        if !input.enabled {
            self.preedit = None;
            self.last_edit = None;
        }
        if self.enabled != input.enabled {
            self.enabled = input.enabled;
            self.touch();
        }
    }

    pub fn value(&self) -> &Arc<str> {
        &self.current.value
    }

    fn selection(&self) -> Range<usize> {
        let (anchor, caret) = (self.current.anchor.byte, self.current.caret.byte);
        anchor.min(caret)..anchor.max(caret)
    }

    pub fn has_selection(&self) -> bool {
        self.current.caret.byte != self.current.anchor.byte
    }

    pub fn touch(&mut self) {
        self.blink = 0.0;
        self.blink_reset = true;
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn blur(&mut self) {
        self.preedit = None;
        self.last_edit = None;
        self.preferred_x = None;
        self.touch();
    }

    pub fn tick(&mut self, elapsed: f32) -> (bool, Option<Duration>) {
        const PERIOD: f32 = 1.2;
        if !std::mem::take(&mut self.blink_reset) {
            self.blink = (self.blink + elapsed.max(0.0)) % PERIOD;
        }
        if self.preedit.is_some() {
            return (false, None);
        }
        let next = match self.blink {
            t if t < 0.52 => 0.52 - t,
            t if t < 0.68 => return (true, None),
            t if t < 1.04 => 1.04 - t,
            _ => return (true, None),
        };
        (false, Some(Duration::from_secs_f32(next)))
    }

    pub fn caret_alpha(&self) -> f32 {
        if self.preedit.is_some() {
            return 1.0;
        }
        let t = self.blink;
        let opacity = if t < 0.68 { ((0.68 - t) / 0.16).clamp(0.0, 1.0) }
            else { ((t - 1.04) / 0.16).clamp(0.0, 1.0) };
        opacity * opacity * (3.0 - 2.0 * opacity)
    }

    fn set_caret(&mut self, caret: Caret, extend: bool) {
        self.current.caret = caret;
        if !extend {
            self.current.anchor = caret;
        }
        self.last_edit = None;
        self.touch();
    }

    fn range_history(&mut self, group: Option<Group>) {
        let now = Instant::now();
        let merge = group.zip(self.last_edit).is_some_and(|(group, (previous, time))| {
            group == previous && now.duration_since(time) < Duration::from_secs(1)
        });
        if !merge {
            if self.undo.len() == 100 {
                self.undo.remove(0);
            }
            self.undo.push(self.current.clone());
        }
        self.last_edit = group.map(|group| (group, now));
        self.redo.clear();
    }

    fn replace(&mut self, range: Range<usize>, text: &str, group: Option<Group>) {
        if self.current.value[range.clone()] == *text {
            self.set_caret(Caret::trailing(range.start + text.len()), false);
            return;
        }
        self.range_history(group);
        let mut value = String::with_capacity(self.current.value.len() - range.len() + text.len());
        value.push_str(&self.current.value[..range.start]);
        value.push_str(text);
        value.push_str(&self.current.value[range.end..]);
        let byte = boundary_after(&value, range.start + text.len());
        let caret = Caret::trailing(byte);
        self.current = Snapshot { value: Arc::from(value), caret, anchor: caret };
        self.preferred_x = None;
        self.touch();
    }

    pub fn insert(&mut self, value: &str, input: &TextInput, typing: bool) {
        if !input.enabled {
            return;
        }
        let value = clean_text(value, input.multiline);
        self.preedit = None;
        if value.is_empty() {
            return;
        }
        let mut range = self.selection();
        if typing && self.overwrite && range.is_empty() {
            for _ in value.graphemes(true) {
                if self.value()[range.end..].starts_with('\n') {
                    break;
                }
                range.end = next_grapheme(self.value(), range.end);
            }
        }
        let group = (typing && self.selection().is_empty()).then_some(Group::Typing);
        self.replace(range, &value, group);
    }

    pub fn preedit(&mut self, value: &str, cursor: Option<(usize, usize)>, input: &TextInput) {
        let clean = clean_text(value, input.multiline);
        let cursor = cursor.map(|(start, end)| {
            let start = char_boundary(value, start);
            let end = char_boundary(value, end);
            if value == clean {
                (start, end)
            } else {
                (clean_text(&value[..start], input.multiline).len(),
                 clean_text(&value[..end], input.multiline).len())
            }
        });
        self.preedit = (input.enabled && !clean.is_empty()).then(|| (Arc::from(clean), cursor));
        self.touch();
    }

    pub fn composing(&self) -> bool {
        self.preedit.is_some()
    }

    pub fn cancel_preedit(&mut self) {
        if self.preedit.take().is_some() {
            self.touch();
        }
    }

    pub fn cursor_bytes(&self) -> (usize, usize) {
        (self.current.caret.byte, self.current.anchor.byte)
    }

    pub fn delete_surrounding(&mut self, before: usize, after: usize) {
        let selection = self.selection();
        let start = boundary_before(self.value(), char_boundary(self.value(), selection.start.saturating_sub(before)));
        let end = boundary_after(self.value(), self.value().ceil_char_boundary(selection.end.saturating_add(after).min(self.value().len())));
        if start == selection.start && end == selection.end {
            return;
        }
        let removed_before = selection.start - start;
        let (caret, anchor) = self.cursor_bytes();
        let keep = self.value()[selection].to_owned();
        self.replace(start..end, &keep, None);
        self.current.caret = Caret::trailing(boundary_after(self.value(), caret - removed_before));
        self.current.anchor = Caret::leading(boundary_after(self.value(), anchor - removed_before));
    }

    fn motion(&mut self, motion: Motion, view: Option<&InputView>) -> Caret {
        use Motion::*;
        let caret = self.current.caret;
        let byte = caret.byte;
        let value = &self.current.value;
        let vertical = matches!(motion, Up | Down | PageUp | PageDown);
        if !vertical {
            self.preferred_x = None;
        }
        match motion {
            Previous => Caret::leading(previous_grapheme(value, byte)),
            Next => Caret::trailing(next_grapheme(value, byte)),
            DocumentStart => Caret::leading(0),
            DocumentEnd => Caret::trailing(value.len()),
            ParagraphStart => {
                let start = value[..byte].rfind('\n').map_or(0, |i| i + 1);
                let start = if start == byte && start > 0 {
                    value[..start - 1].rfind('\n').map_or(0, |i| i + 1)
                } else { start };
                Caret::leading(start)
            }
            ParagraphEnd => {
                let end = value[byte..].find('\n').map_or(value.len(), |i| byte + i);
                let end = if end == byte && end < value.len() {
                    value[end + 1..].find('\n').map_or(value.len(), |i| end + 1 + i)
                } else { end };
                Caret::trailing(end)
            }
            NextParagraphStart => Caret::leading(value[byte..].find('\n').map_or(value.len(), |i| byte + i + 1)),
            PreviousWord => Caret::leading(previous_word(value, byte)),
            NextWord { to_start: true } => Caret::leading(next_word_start(value, byte)),
            NextWord { to_start: false } => Caret::trailing(next_word(value, byte)),
            Word { left, next_start } => {
                let rtl = view.is_some_and(|v| v.rtl_at(caret));
                if left != rtl {
                    Caret::leading(previous_word(value, byte))
                } else if next_start {
                    Caret::leading(next_word_start(value, byte))
                } else {
                    Caret::trailing(next_word(value, byte))
                }
            }
            Left | Right => view.map_or_else(
                || if matches!(motion, Left) { Caret::leading(previous_grapheme(value, byte)) }
                    else { Caret::trailing(next_grapheme(value, byte)) },
                |view| view.horizontal(caret, matches!(motion, Left)),
            ),
            LineStart | LineEnd => {
                let start = matches!(motion, LineStart);
                view.map_or_else(
                    || if start { Caret::leading(0) } else { Caret::trailing(value.len()) },
                    |view| view.line_edge(caret, start),
                )
            }
            Up | Down | PageUp | PageDown => {
                let up = matches!(motion, Up | PageUp);
                view.map_or_else(
                    || if up { Caret::leading(0) } else { Caret::trailing(value.len()) },
                    |view| view.vertical(caret, up, matches!(motion, PageUp | PageDown), &mut self.preferred_x),
                )
            }
        }
    }

    pub fn command(
        &mut self,
        command: Command,
        extend: bool,
        input: &TextInput,
        view: Option<&InputView>,
    ) -> Option<ClipboardCommand> {
        let selection = self.selection();
        match command {
            Command::Move(motion) => {
                let caret = if !extend && !selection.is_empty() && matches!(motion, Motion::Left | Motion::Right) {
                    let left = matches!(motion, Motion::Left);
                    self.preferred_x = None;
                    view.map_or_else(
                        || if left { Caret::leading(selection.start) } else { Caret::trailing(selection.end) },
                        |view| view.selection_edge(self.current.anchor, self.current.caret, left),
                    )
                } else {
                    self.motion(motion, view)
                };
                self.set_caret(caret, extend);
            }
            Command::SelectAll => {
                self.preferred_x = None;
                self.current.anchor = Caret::leading(0);
                self.set_caret(Caret::trailing(self.value().len()), true);
            }
            Command::Center => {
                if let Some(view) = view
                    && let Some(caret) = view.caret_rect()
                {
                    let delta = caret.center() - view.area.center();
                    self.scroll = view.scroll + delta;
                    self.touch();
                }
            }
            Command::Cancel => {
                self.preedit = None;
                self.set_caret(self.current.caret, false);
            }
            Command::Copy | Command::Cut if !selection.is_empty() => {
                self.last_edit = None;
                let value = Arc::from(&self.value()[selection]);
                return Some(if input.enabled && matches!(command, Command::Cut) {
                    ClipboardCommand::Cut(value)
                } else {
                    ClipboardCommand::Copy(value)
                });
            }
            _ if !input.enabled => {}
            Command::Paste => return Some(ClipboardCommand::Paste),
            Command::Overwrite => self.overwrite = !self.overwrite,
            Command::Delete(motion, group) => {
                let (range, group) = if selection.is_empty() {
                    let other = self.motion(motion, view).byte;
                    let byte = self.current.caret.byte;
                    (byte.min(other)..byte.max(other), group)
                } else { (selection, None) };
                if !range.is_empty() {
                    self.replace(range, "", group);
                }
            }
            Command::Undo | Command::Redo => {
                let (from, to) = if matches!(command, Command::Undo) {
                    (&mut self.undo, &mut self.redo)
                } else { (&mut self.redo, &mut self.undo) };
                if let Some(snapshot) = from.pop() {
                    to.push(std::mem::replace(&mut self.current, snapshot));
                    self.preferred_x = None;
                    self.last_edit = None;
                    self.touch();
                }
            }
            Command::Transpose if selection.is_empty() => {
                let value = self.value();
                let mut middle = self.current.caret.byte;
                if middle == value.len() || value[middle..].starts_with('\n') {
                    middle = previous_grapheme(value, middle);
                }
                let start = previous_grapheme(value, middle);
                let end = next_grapheme(value, middle);
                if start < middle && middle < end && !value[start..end].contains('\n') {
                    let swapped = format!("{}{}", &value[middle..end], &value[start..middle]);
                    self.replace(start..end, &swapped, None);
                }
            }
            Command::Kill(motion) => {
                let range = if selection.is_empty() {
                    let byte = self.current.caret.byte;
                    let other = match motion {
                        Motion::ParagraphStart => self.value()[..byte].rfind('\n').map_or(0, |i| i + 1),
                        Motion::ParagraphEnd if self.value()[byte..].starts_with('\n') => byte + 1,
                        Motion::ParagraphEnd => self.value()[byte..].find('\n').map_or(self.value().len(), |i| byte + i),
                        _ => previous_word(self.value(), byte),
                    };
                    byte.min(other)..byte.max(other)
                } else { selection };
                if !range.is_empty() {
                    self.kill = Arc::from(&self.value()[range.clone()]);
                    self.replace(range, "", None);
                }
            }
            Command::Yank => self.insert(&self.kill.clone(), input, false),
            Command::Newline => self.insert("\n", input, false),
            Command::OpenLine => {
                let at = self.current.caret.byte;
                self.replace(at..at, "\n", None);
                self.set_caret(Caret::trailing(at), false);
            }
            _ => {}
        }
        None
    }

    pub fn cut(&mut self) {
        self.replace(self.selection(), "", None);
    }

    pub fn click(&mut self, view: &InputView, point: Vec2, extend: bool, count: u8) {
        self.preedit = None;
        self.preferred_x = None;
        let caret = view.hit(point);
        if extend || count == 1 {
            self.set_caret(caret, extend);
        } else if count == 2 {
            let range = word_at(self.value(), caret);
            self.current.anchor = Caret::leading(range.start);
            self.set_caret(Caret::trailing(range.end), true);
        } else {
            let text = self.value();
            let start = text[..caret.byte].rfind('\n').map_or(0, |i| i + 1);
            let end = text[caret.byte..].find('\n').map_or(text.len(), |i| caret.byte + i + 1);
            self.current.anchor = Caret::leading(start);
            self.set_caret(Caret::trailing(end), true);
        }
    }

    pub fn drag_anchor(&self) -> (usize, usize) {
        let range = self.selection();
        (range.start, range.end)
    }

    pub fn drag(&mut self, view: &InputView, point: Vec2, count: u8, initial: (usize, usize)) {
        let old_anchor = self.current.anchor;
        let mut caret = view.hit(point);
        let (start, end) = initial;
        if count > 1 {
            let text = self.value();
            let range = if count == 2 {
                word_at(text, caret)
            } else {
                let start = text[..caret.byte].rfind('\n').map_or(0, |i| i + 1);
                let end = text[caret.byte..].find('\n').map_or(text.len(), |i| caret.byte + i + 1);
                start..end
            };
            if range.start < start {
                self.current.anchor = Caret::trailing(end);
                caret = Caret::leading(range.start);
            } else {
                self.current.anchor = Caret::leading(start);
                caret = Caret::trailing(range.end.max(end));
            }
        }
        if self.current.caret != caret || old_anchor != self.current.anchor {
            self.set_caret(caret, true);
        }
    }

    pub fn view(&self, shaping: &ShapedText, text: &Text, input: &TextInput, area: Rect) -> InputView {
        let mut caret = Some(self.current.caret);
        let mut selection = self.selection();
        let mut composition = None;
        let value = if let Some((preedit, cursor)) = &self.preedit {
            let start = selection.start;
            let mut value = self.value().to_string();
            value.replace_range(selection, preedit);
            caret = cursor.map(|(_, end)| Caret::trailing(boundary_after(&value, start + end.min(preedit.len()))));
            selection = cursor.map_or(start..start, |(a, b)| {
                (start + a.min(b).min(preedit.len()))..(start + a.max(b).min(preedit.len()))
            });
            composition = Some(start..start + preedit.len());
            Arc::from(value)
        } else {
            self.value().clone()
        };
        InputView::new(shaping, text, input, area, value, caret, selection, composition, self.scroll)
    }
}

fn char_boundary(value: &str, byte: usize) -> usize {
    value.floor_char_boundary(byte.min(value.len()))
}

fn boundary_after(value: &str, byte: usize) -> usize {
    let mut cursor = GraphemeCursor::new(byte, value.len(), true);
    if cursor.is_boundary(value, 0).unwrap() {
        byte
    } else {
        cursor.next_boundary(value, 0).unwrap().unwrap_or(value.len())
    }
}

fn boundary_before(value: &str, byte: usize) -> usize {
    let mut cursor = GraphemeCursor::new(byte, value.len(), true);
    if cursor.is_boundary(value, 0).unwrap() {
        byte
    } else {
        cursor.prev_boundary(value, 0).unwrap().unwrap_or(0)
    }
}

fn previous_grapheme(value: &str, byte: usize) -> usize {
    value[..byte].grapheme_indices(true).next_back().map_or(0, |(i, _)| i)
}

fn next_grapheme(value: &str, byte: usize) -> usize {
    byte + value[byte..].graphemes(true).next().map_or(0, str::len)
}

fn previous_word(value: &str, byte: usize) -> usize {
    let index = value.unicode_word_indices().take_while(|&(i, _)| i < byte).last().map_or(0, |(i, _)| i);
    boundary_before(value, index)
}

fn next_word(value: &str, byte: usize) -> usize {
    let index = value.unicode_word_indices().map(|(i, word)| i + word.len())
        .find(|&end| end > byte).unwrap_or(value.len());
    boundary_after(value, index)
}

fn next_word_start(value: &str, byte: usize) -> usize {
    let index = value.unicode_word_indices().map(|(i, _)| i)
        .find(|&start| start > byte).unwrap_or(value.len());
    boundary_after(value, index)
}

fn word_at(value: &str, caret: Caret) -> Range<usize> {
    let byte = if caret.trailing { previous_grapheme(value, caret.byte) } else { caret.byte };
    value.split_word_bound_indices().find_map(|(i, word)| {
        (i <= byte && byte < i + word.len())
            .then(|| boundary_before(value, i)..boundary_after(value, i + word.len()))
    }).unwrap_or(byte..byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_shortcuts_cover_document_selection_and_clipboard_aliases() {
        let shortcut = |key, mods, mac| Command::for_key(key, mods, mac, true);
        let ctrl = Modifiers::CTRL;
        let cmd = Modifiers::META;
        let alt = Modifiers::ALT;
        let shift = Modifiers::SHIFT;
        assert!(matches!(shortcut(Key::Home, ctrl | shift, false), Some(Command::Move(Motion::DocumentStart))));
        assert!(matches!(shortcut(Key::Down, ctrl | shift, false), Some(Command::Move(Motion::NextParagraphStart))));
        assert!(matches!(shortcut(Key::Insert, ctrl, false), Some(Command::Copy)));
        assert!(matches!(shortcut(Key::Insert, shift, false), Some(Command::Paste)));
        assert!(matches!(shortcut(Key::Delete, shift, false), Some(Command::Cut)));
        assert!(matches!(shortcut(Key::Up, cmd | shift, true), Some(Command::Move(Motion::DocumentStart))));
        assert!(matches!(shortcut(Key::Backspace, alt, true), Some(Command::Delete(Motion::PreviousWord, None))));
        assert!(matches!(shortcut(Key::L, ctrl, true), Some(Command::Center)));
        assert!(matches!(shortcut(Key::O, ctrl, true), Some(Command::OpenLine)));
        assert!(matches!(shortcut(Key::V, cmd | alt | shift, true), Some(Command::Paste)));
        assert!(shortcut(Key::A, ctrl | alt, false).is_none());
    }

    #[test]
    fn unicode_edits_and_history_preserve_whole_graphemes() {
        let input = TextInput::new();
        let mut editor = Editor::new(&Text::new("a👩🏽‍💻e\u{301}"), &input);
        let backspace = || Command::Delete(Motion::Previous, Some(Group::Backspace));
        editor.command(backspace(), false, &input, None);
        assert_eq!(&**editor.value(), "a👩🏽‍💻");
        editor.command(backspace(), false, &input, None);
        assert_eq!(&**editor.value(), "a");
        editor.command(Command::Undo, false, &input, None);
        assert_eq!(&**editor.value(), "a👩🏽‍💻e\u{301}");
        editor.command(Command::Redo, false, &input, None);
        assert_eq!(&**editor.value(), "a");
        editor.insert("b\r\nc", &input, false);
        assert_eq!(&**editor.value(), "ab c");
    }

    #[test]
    fn shaped_motions_follow_bidi_and_visual_lines() {
        let shaping = ShapedText::new(Default::default(), Default::default());
        let input = TextInput::new().multiline(true);
        let text = Text::new("alpha beta gamma\nאבג\n").wrap(TextWrap::Words);
        let mut editor = Editor::new(&text, &input);
        let area = Rect::new(0.0, 0.0, 54.0, 28.0);
        let view = editor.view(&shaping, &text, &input, area);
        assert!(view.lines().len() >= 5);
        assert!(view.scroll.y > 0.0);
        let hebrew = editor.value().find('א').unwrap();
        editor.set_caret(Caret::leading(hebrew), false);
        let view = editor.view(&shaping, &text, &input, area);
        let before = view.caret_rect().unwrap().x;
        editor.command(Command::Move(Motion::Left), true, &input, Some(&view));
        assert_eq!(&editor.value()[editor.selection()], "א");
        let view = editor.view(&shaping, &text, &input, area);
        assert!(view.caret_rect().unwrap().x < before);
        assert_eq!(view.selection_rects(false).len(), 1);
        editor.command(Command::Move(Motion::Up), false, &input, Some(&view));
        assert!(editor.current.caret.byte < hebrew);
    }

    #[test]
    fn arrow_keys_reach_every_boundary_across_wrapped_spaces() {
        use std::collections::BTreeSet;

        let shaping = ShapedText::default();
        let input = TextInput::new().multiline(true);
        let text = Text::new("abc     def").font(crate::components::FontId::MONOSPACE);
        let mut editor = Editor::new(&text, &input);
        editor.set_caret(Caret::leading(0), false);
        let area = Rect::new(0.0, 0.0, 40.0, 100.0);
        let view = editor.view(&shaping, &text, &input, area);
        assert!(view.lines().len() > 1);
        let mut reached = BTreeSet::from([0]);
        for _ in 0..=2 * text.content.len() {
            editor.command(Command::Move(Motion::Right), false, &input, Some(&view));
            reached.insert(editor.current.caret.byte);
        }
        assert_eq!(reached, (0..=text.content.len()).collect());
    }

    #[test]
    fn preedit_and_surrounding_deletion_keep_the_selected_text() {
        let input = TextInput::new();
        let text = Text::new("😀selectedé");
        let mut editor = Editor::new(&text, &input);
        editor.current.anchor = Caret::leading("😀".len());
        editor.current.caret = Caret::trailing("😀selected".len());
        editor.preedit("好", Some((3, 3)), &input);
        editor.delete_surrounding(1, 1);
        assert_eq!(&**editor.value(), "selected");
        assert_eq!(editor.selection(), 0..8);
        assert!(editor.composing());
        editor.insert("好", &input, false);
        assert_eq!(&**editor.value(), "好");
        assert!(!editor.composing());
    }
}
