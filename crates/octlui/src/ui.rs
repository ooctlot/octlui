//! The windowless core: owns the node arena, retained per-node state and input routing.
//! Everything here is testable without winit or a GPU.

use crate::action::{Action, Key, Keymap, Keystroke};
use crate::anim::{PaintSpring, SpringState};
use crate::components::SpringProps;
use crate::ctx::ErasedFn;
use crate::events::*;
use crate::geometry::{CornerRadius, Padding, Rect, Vec2};
use crate::id::{Id, KeyMap};
use crate::image::ImageStore;
use crate::layout::{Arena, LayoutEnv, NONE, Reflow, layout, position, resolve_sizes};
use crate::node::Node;
use crate::paint::Painter;
use crate::render::RenderList;
use crate::style::Axis;
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

/// A region that can be hit by the pointer, in paint order.
pub(crate) struct Hit {
    pub idx: u32,
    pub rect: Rect,
    pub clip: Rect,
}

/// A visible scroll bar (for dragging).
pub(crate) struct BarHit {
    #[allow(dead_code)]
    pub idx: u32,
    pub key: u64,
    pub axis: Axis,
    pub thumb: Rect,
    pub track: Rect,
    pub max: f32,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Visual {
    pub delta: Vec2,
    pub size: Option<Vec2>,
    pub corners: Option<CornerRadius>,
    pub border_width: Option<f32>,
}

pub(crate) struct SpringSet {
    pos: SpringState<2>,
    size: SpringState<2>,
    corners: Option<SpringState<4>>,
    border_width: Option<SpringState<1>>,
    floating_offset: Option<SpringState<2>>,
    pub paints: PaintSprings,
    pub visual: Visual,
}

#[derive(Default)]
pub(crate) struct PaintSprings {
    pub background: Option<Box<PaintSpring>>,
    pub border: Option<Box<PaintSpring>>,
    pub tint: Option<Box<PaintSpring>>,
    pub text: Option<Box<PaintSpring>>,
}

struct ReflowSpring {
    target: [f32; 7],
    motion: SpringState<7>,
}

#[derive(Default)]
pub(crate) struct NodeState {
    seen: u64,
    target: Vec2,
    current: Vec2,
    max: Vec2,
    declared: Option<Vec2>,
    pub bar_alpha: f32,
    bar_hold: f32,
    pub spring: Option<SpringSet>,
    reflow: Option<Box<ReflowSpring>>,
}

struct HoverEntry {
    key: u64,
    handler: Option<ErasedFn>,
}

struct BarDrag {
    key: u64,
    axis: Axis,
    grab: f32,
    track_start: f32,
    track_len: f32,
    thumb_len: f32,
    max: f32,
}

#[derive(Default)]
struct TooltipState {
    target: Option<u64>,
    owner: Option<u64>,
    hover_time: f32,
    alpha: f32,
    delay: f32,
    fade_in: f32,
    fade_out: f32,
    gap: f32,
    content: Option<Node>,
    live: bool,
    arena: Arena,
    owner_rect: Option<Rect>,
}

/// Borrowed handles passed to listeners.
pub(crate) struct Env<'a> {
    pub state: &'a mut dyn Any,
    pub cx: &'a mut dyn Any,
}
impl Env<'_> {
    fn call(&mut self, f: &ErasedFn, ev: &dyn Any) -> bool {
        f(&mut *self.state, &mut *self.cx, ev)
    }
}

pub(crate) struct Ui {
    pub arena: Arena,
    states: KeyMap<NodeState>,
    frame_no: u64,
    pub size: Vec2,
    pub scale: f32,
    pub list: RenderList,
    hits: Vec<Hit>,
    bars: Vec<BarHit>,
    scroll_nodes: Vec<u32>,
    spring_nodes: Vec<u32>,
    reflow_nodes: Vec<u32>,
    reflow_active: bool,
    mouse: Option<Vec2>,
    hover: Vec<HoverEntry>,
    tooltip: TooltipState,
    pressed: Option<(u64, MouseButton)>,
    drag: Option<BarDrag>,
    pub focus: Option<u64>,
    pub mods: crate::action::Modifiers,
    layout_dirty: bool,
    position_dirty: bool,
    /// Actions nobody on the focus path handled; the window routes them to global handlers.
    pub unhandled: Vec<Box<dyn Action>>,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

fn step_to(cur: f32, target: f32, dt: f32, dur: f32) -> f32 {
    let step = dt / dur.max(1e-3);
    if target > cur { (cur + step).min(target) } else { (cur - step).max(target) }
}

impl Ui {
    pub fn new() -> Self {
        Self {
            arena: Arena::default(),
            states: KeyMap::default(),
            frame_no: 0,
            size: Vec2::new(800.0, 600.0),
            scale: 1.0,
            list: RenderList::default(),
            hits: Vec::new(),
            bars: Vec::new(),
            scroll_nodes: Vec::new(),
            spring_nodes: Vec::new(),
            reflow_nodes: Vec::new(),
            reflow_active: false,
            mouse: None,
            hover: Vec::new(),
            tooltip: TooltipState::default(),
            pressed: None,
            drag: None,
            focus: None,
            mods: Default::default(),
            layout_dirty: true,
            position_dirty: false,
            unhandled: Vec::new(),
        }
    }

    pub fn set_size(&mut self, logical: Vec2, scale: f32) {
        if self.size != logical || self.scale != scale {
            self.size = logical;
            self.scale = scale;
            self.layout_dirty = true;
        }
    }

    /// Installs a new node tree (the result of a view call).
    pub fn set_root(&mut self, node: Node) {
        self.arena.build(node);
        self.frame_no += 1;
        let frame = self.frame_no;
        self.scroll_nodes.clear();
        self.spring_nodes.clear();
        self.reflow_nodes.clear();
        let Ui { arena, states, scroll_nodes, spring_nodes, reflow_nodes, .. } = self;
        for (i, n) in arena.nodes.iter_mut().enumerate() {
            if n.scroll.is_none() && n.spring.is_none() {
                continue;
            }
            let st = states.entry(n.key).or_default();
            st.seen = frame;
            if let Some(s) = &n.scroll {
                scroll_nodes.push(i as u32);
                match s.offset {
                    Some(d) if st.declared != Some(d) => {
                        st.declared = Some(d);
                        st.target = d;
                        if !s.smooth {
                            st.current = d;
                        }
                    }
                    Some(_) => {}
                    None => st.declared = None,
                }
                n.scroll_offset = st.current;
            }
            if n.spring.is_some() {
                spring_nodes.push(i as u32);
            }
            if n.spring.as_ref().is_some_and(|s| s.properties.intersects(SpringProps::LAYOUT)) {
                reflow_nodes.push(i as u32);
            } else {
                st.reflow = None;
            }
        }
        if frame % 300 == 0 {
            self.states.retain(|_, s| s.seen + 300 >= frame);
        }
        self.tooltip.live = false;
        self.tooltip.content = None;
        self.layout_dirty = true;
    }

    pub fn has_root(&self) -> bool {
        !self.arena.is_empty()
    }

    pub fn invalidate_layout(&mut self) {
        self.layout_dirty = true;
        self.tooltip.live = false;
    }

    pub fn hover_keys(&self) -> Vec<u64> {
        self.hover.iter().map(|h| h.key).collect()
    }

    pub fn set_focus(&mut self, id: Option<&Id>) {
        self.focus = id.map(|i| i.hash);
    }

    fn focus_idx(&self) -> Option<usize> {
        self.focus.and_then(|k| self.arena.nodes.iter().position(|n| n.key == k))
    }

    // ------------------------------------------------------------------ frame

    /// Advances animations, lays out if needed and paints. Returns whether animation is
    /// ongoing and whether hit geometry moved.
    pub fn frame(&mut self, dt: f32, elapsed: f32, env: &mut LayoutEnv) -> (bool, bool) {
        if !self.has_root() {
            self.list.clear();
            return (false, false);
        }
        let layout_dirty = std::mem::take(&mut self.layout_dirty);
        if layout_dirty {
            for &i in &self.reflow_nodes {
                self.arena.nodes[i as usize].reflow = None;
            }
            resolve_sizes(&mut self.arena, self.size, env);
        }
        let reflow = (layout_dirty || self.reflow_active) && self.tick_reflow(layout_dirty, dt);
        if reflow {
            resolve_sizes(&mut self.arena, self.size, env);
        }
        if layout_dirty || reflow {
            position(&mut self.arena);
            self.sync_after_layout();
            self.position_dirty = false;
        }
        let mut animating = self.tick_scroll(dt);
        let moved = std::mem::take(&mut self.position_dirty);
        if moved {
            position(&mut self.arena);
        }
        let (springs, visual_geometry) = self.update_springs(dt);
        animating |= springs | self.reflow_active;
        animating |= self.tick_tooltip(dt, elapsed, env);
        self.paint(env.images);
        (animating, layout_dirty || reflow || moved || visual_geometry)
    }

    fn tick_reflow(&mut self, retarget: bool, dt: f32) -> bool {
        let mut changed = false;
        self.reflow_active = false;
        for &i in &self.reflow_nodes {
            let n = &mut self.arena.nodes[i as usize];
            let spec = n.spring.as_ref().unwrap();
            let props = spec.properties;
            let st = self.states.get_mut(&n.key).unwrap();
            let Padding { left, top, right, bottom } = n.pad;
            let target = [n.size.x, n.size.y, left, top, right, bottom, n.gap];
            let spring = st.reflow.get_or_insert_with(|| Box::new(ReflowSpring {
                target,
                motion: SpringState::at(target),
            }));
            if retarget {
                spring.target = target;
                for (k, prop) in [
                    SpringProps::WIDTH, SpringProps::HEIGHT, SpringProps::PADDING,
                    SpringProps::PADDING, SpringProps::PADDING, SpringProps::PADDING, SpringProps::CHILD_GAP,
                ].into_iter().enumerate() {
                    if !props.contains(prop) {
                        spring.motion.x[k] = target[k];
                        spring.motion.v[k] = 0.;
                    }
                }
            }
            self.reflow_active |= spring.motion.step(spring.target, &spec.params(), dt);
            let [w, h, l, t, r, b, gap] = spring.motion.x;
            let value = (spring.motion.x != spring.target).then_some(Reflow {
                width: props.contains(SpringProps::WIDTH).then_some(w.max(0.)),
                height: props.contains(SpringProps::HEIGHT).then_some(h.max(0.)),
                padding: Padding::new(l, t, r, b),
                gap,
            });
            if n.reflow.as_deref() != value.as_ref() {
                match (value, n.reflow.as_mut()) {
                    (Some(value), Some(current)) => **current = value,
                    (Some(value), None) => n.reflow = Some(Box::new(value)),
                    (None, _) => n.reflow = None,
                }
                changed = true;
            }
        }
        changed
    }

    fn sync_after_layout(&mut self) {
        for &i in &self.scroll_nodes {
            let n = &self.arena.nodes[i as usize];
            if let Some(st) = self.states.get_mut(&n.key) {
                st.max = n.scroll_max;
                st.current = n.scroll_offset;
                st.target = Vec2::new(st.target.x.clamp(0.0, st.max.x), st.target.y.clamp(0.0, st.max.y));
            }
        }
    }

    fn tick_scroll(&mut self, dt: f32) -> bool {
        let mut moving = false;
        for &i in &self.scroll_nodes {
            let n = &mut self.arena.nodes[i as usize];
            let s = n.scroll.as_ref().expect("scroll node");
            let Some(st) = self.states.get_mut(&n.key) else { continue };
            st.target = Vec2::new(st.target.x.clamp(0.0, st.max.x), st.target.y.clamp(0.0, st.max.y));
            if s.smooth {
                let k = 1.0 - (-s.smooth_rate * dt).exp();
                st.current = st.current.lerp(st.target, k);
                if (st.target - st.current).length() < 0.05 {
                    st.current = st.target;
                }
            } else {
                st.current = st.target;
            }
            if st.current != n.scroll_offset {
                n.scroll_offset = st.current;
                self.position_dirty = true;
            }
            if st.current != st.target {
                moving = true;
            }
            if let Some(bar) = &s.scroll_bar {
                let hovered = self.hover.iter().any(|h| h.key == n.key);
                let dragging = self.drag.as_ref().is_some_and(|d| d.key == n.key);
                st.bar_hold = (st.bar_hold - dt).max(0.0);
                let want = bar.always_visible || hovered || dragging || st.bar_hold > 0.0;
                let target = if want { 1.0 } else { 0.0 };
                if (st.bar_alpha - target).abs() > f32::EPSILON {
                    let dur = if target > st.bar_alpha { bar.fade_in } else { bar.fade_out };
                    st.bar_alpha = step_to(st.bar_alpha, target, dt, dur.as_secs_f32());
                    moving = true;
                }
                if st.bar_hold > 0.0 {
                    moving = true;
                }
            }
        }
        moving
    }

    fn update_springs(&mut self, dt: f32) -> (bool, bool) {
        let mut moving = false;
        let mut geometry_changed = false;
        for &i in &self.spring_nodes {
            let n = &self.arena.nodes[i as usize];
            let spec = n.spring.as_ref().expect("spring node");
            let params = spec.params();
            let props = spec.properties;
            let Some(st) = self.states.get_mut(&n.key) else { continue };

            let pos_t = [n.rect.x, n.rect.y];
            let size_t = [n.rect.w, n.rect.h];
            let sp = st.spring.get_or_insert_with(|| SpringSet {
                pos: SpringState::at(pos_t),
                size: SpringState::at(size_t),
                corners: None,
                border_width: None,
                floating_offset: None,
                paints: Default::default(),
                visual: Visual::default(),
            });

            let mut m = false;
            let mut v = Visual::default();
            if props.contains(SpringProps::POSITION) {
                m |= sp.pos.step(pos_t, &params, dt);
                v.delta = Vec2::new(sp.pos.x[0] - pos_t[0], sp.pos.x[1] - pos_t[1]);
            } else {
                sp.pos = SpringState::at(pos_t);
            }
            if props.contains(SpringProps::SIZE) {
                m |= sp.size.step(size_t, &params, dt);
                v.size = Some(Vec2::new(sp.size.x[0].max(0.0), sp.size.x[1].max(0.0)));
            } else {
                sp.size = SpringState::at(size_t);
            }
            if props.contains(SpringProps::CORNER_RADIUS) {
                let CornerRadius { top_left, top_right, bottom_right, bottom_left } = n.corner;
                let target = [top_left, top_right, bottom_right, bottom_left];
                let spring = sp.corners.get_or_insert_with(|| SpringState::at(target));
                m |= spring.step(target, &params, dt);
                v.corners = Some(spring.x.map(|r| r.max(0.)).into());
            } else {
                sp.corners = None;
            }
            if props.contains(SpringProps::BORDER_WIDTH) && let Some(a) = &n.appearance {
                let target = [a.border_width];
                let spring = sp.border_width.get_or_insert_with(|| SpringState::at(target));
                m |= spring.step(target, &params, dt);
                v.border_width = Some(spring.x[0].max(0.));
            } else {
                sp.border_width = None;
            }
            if props.contains(SpringProps::FLOATING_OFFSET) && n.is_floating && let Some(f) = &n.floating {
                let target = [f.offset.x, f.offset.y];
                let spring = sp.floating_offset.get_or_insert_with(|| SpringState::at(target));
                m |= spring.step(target, &params, dt);
                v.delta += Vec2::from(spring.x) - f.offset;
            } else {
                sp.floating_offset = None;
            }
            let targets = [
                (&mut sp.paints.background, SpringProps::BACKGROUND, n.appearance.as_ref().map(|a| a.background)),
                (&mut sp.paints.border, SpringProps::BORDER, n.appearance.as_ref().map(|a| a.border_color)),
                (&mut sp.paints.tint, SpringProps::TINT, n.image.as_ref().map(|i| i.tint)),
                (&mut sp.paints.text, SpringProps::TEXT_COLOR, n.text.as_ref().map(|t| t.color)),
            ];
            for (slot, property, target) in targets {
                if let Some(target) = target.filter(|_| props.contains(property)) {
                    let spring = slot.get_or_insert_with(|| Box::new(PaintSpring::at(target)));
                    m |= spring.step(target, &params, dt);
                } else {
                    *slot = None;
                }
            }
            geometry_changed |= sp.visual.delta != v.delta || sp.visual.size != v.size;
            sp.visual = v;
            moving |= m;
        }
        (moving, geometry_changed)
    }

    pub fn tooltip_wait(&self) -> Option<Duration> {
        let t = &self.tooltip;
        if t.target.is_some() && t.target == t.owner && t.hover_time < t.delay {
            Duration::try_from_secs_f32(t.delay - t.hover_time).ok()
        } else {
            None
        }
    }

    fn tick_tooltip(&mut self, dt: f32, elapsed: f32, env: &mut LayoutEnv) -> bool {
        let t = &mut self.tooltip;
        match (t.target, t.owner) {
            (Some(a), Some(b)) if a == b => t.hover_time += elapsed,
            (Some(a), _) => {
                t.owner = Some(a);
                t.hover_time = 0.0;
                t.alpha = 0.0;
                t.live = false;
                t.content = None;
            }
            (None, Some(_)) => {
                t.alpha = step_to(t.alpha, 0.0, dt, t.fade_out);
                if t.alpha <= 0.0 {
                    t.owner = None;
                    t.live = false;
                    t.content = None;
                    return false;
                }
                return true;
            }
            (None, None) => return false,
        }
        let Some(owner) = t.owner else { return false };

        if t.content.is_none() {
            // (Re)read the owner's tooltip config; the owner may have been rebuilt.
            match self.arena.nodes.iter().find(|n| n.key == owner).and_then(|n| n.tooltip.as_ref()) {
                Some(cfg) => {
                    t.delay = cfg.delay.as_secs_f32();
                    t.fade_in = cfg.fade_in.as_secs_f32();
                    t.fade_out = cfg.fade_out.as_secs_f32();
                    t.gap = cfg.gap;
                    t.content = Some(cfg.content.clone());
                    t.live = false;
                }
                None => {
                    t.owner = None;
                    return false;
                }
            }
        }
        if t.hover_time < t.delay {
            return false;
        }
        if !t.live {
            if let Some(content) = t.content.clone() {
                t.arena.build(content);
                layout(&mut t.arena, self.size, env);
                t.live = true;
            }
        }
        t.alpha = step_to(t.alpha, 1.0, dt, t.fade_in);
        t.alpha < 1.0
    }

    fn paint(&mut self, images: &mut ImageStore) {
        self.list.clear();
        self.hits.clear();
        self.bars.clear();
        let Ui { arena, states, list, hits, bars, tooltip, size, .. } = self;
        {
            let mut p = Painter::new(arena, states, list, images);
            p.hits = Some(hits);
            p.bars = Some(bars);
            p.watch = tooltip.owner;
            p.paint_tree(Vec2::ZERO, 1.0);
            if let Some(r) = p.watched_rect {
                tooltip.owner_rect = Some(r);
            }
        }
        if tooltip.live && tooltip.alpha > 0.0 {
            if let (Some(owner), Some(root)) = (tooltip.owner_rect, tooltip.arena.nodes.first()) {
                let ts = root.size;
                let mut x = owner.center().x - ts.x * 0.5;
                let mut y = owner.bottom() + tooltip.gap;
                if y + ts.y > size.y {
                    y = owner.y - tooltip.gap - ts.y;
                }
                x = x.min(size.x - ts.x).max(0.0);
                y = y.max(0.0);
                Painter::new(&tooltip.arena, states, list, images).paint_tree(Vec2::new(x, y), tooltip.alpha);
            }
        }
    }

    // ------------------------------------------------------------------ input

    fn hit_test(&self, p: Vec2) -> Option<usize> {
        self.hits.iter().rev().find(|h| h.clip.contains(p) && h.rect.contains(p)).map(|h| h.idx as usize)
    }

    fn parent(&self, i: usize) -> Option<usize> {
        let p = self.arena.nodes[i].parent;
        (p != NONE).then_some(p as usize)
    }

    /// Pointer moved (`Some`) or hover should be re-evaluated against the current layout (`None`).
    pub fn pointer_moved(&mut self, pos: Option<Vec2>, env: &mut Env) -> bool {
        if let Some(p) = pos {
            self.mouse = Some(p);
        }
        let Some(p) = self.mouse else { return false };
        if self.drag.is_some() {
            if pos.is_some() {
                self.drag_to(p);
                return true;
            }
            return false;
        }
        let mut new = Vec::new();
        let mut tip = None;
        let mut cur = self.hit_test(p);
        while let Some(i) = cur {
            let n = &self.arena.nodes[i];
            if tip.is_none() && n.tooltip.is_some() {
                tip = Some(n.key);
            }
            new.push(HoverEntry { key: n.key, handler: n.handlers.hover.clone() });
            cur = self.parent(i);
        }
        for e in &self.hover {
            if !new.iter().any(|x| x.key == e.key) {
                if let Some(f) = &e.handler {
                    env.call(f, &HoverEvent { hovered: false });
                }
            }
        }
        for e in &new {
            if !self.hover.iter().any(|x| x.key == e.key) {
                if let Some(f) = &e.handler {
                    env.call(f, &HoverEvent { hovered: true });
                }
            }
        }
        let changed = self.hover.len() != new.len() || self.hover.iter().zip(&new).any(|(a, b)| a.key != b.key);
        self.hover = new;
        self.tooltip.target = tip;
        changed
    }

    pub fn pointer_left(&mut self, env: &mut Env) {
        self.mouse = None;
        for e in std::mem::take(&mut self.hover) {
            if let Some(f) = &e.handler {
                env.call(f, &HoverEvent { hovered: false });
            }
        }
        self.tooltip.target = None;
    }

    fn click_target(&self, from: Option<usize>) -> Option<usize> {
        let mut cur = from;
        while let Some(i) = cur {
            let h = &self.arena.nodes[i].handlers;
            if h.click.is_some() || h.click_action.is_some() {
                return Some(i);
            }
            cur = self.parent(i);
        }
        None
    }

    fn drag_to(&mut self, p: Vec2) {
        let Some(d) = &self.drag else { return };
        let travel = (d.track_len - d.thumb_len).max(0.0001);
        let frac = ((p.get(d.axis) - d.grab - d.track_start) / travel).clamp(0.0, 1.0);
        if let Some(st) = self.states.get_mut(&d.key) {
            let offset = frac * d.max;
            st.target.set(d.axis, offset);
            st.bar_hold = 0.8;
            // Reflect immediately so the next layout uses it.
            if st.current.get(d.axis) != offset {
                st.current.set(d.axis, offset);
                self.position_dirty = true;
                if let Some(n) = self.arena.nodes.iter_mut().find(|n| n.key == d.key) {
                    n.scroll_offset = st.current;
                }
            }
        }
    }

    pub fn pointer_button(&mut self, button: MouseButton, pressed: bool, pos: Option<Vec2>, env: &mut Env) {
        if let Some(p) = pos {
            self.mouse = Some(p);
        }
        let Some(p) = self.mouse else { return };
        let left = button == MouseButton::Left;

        if !pressed {
            let was_dragging = self.drag.take().is_some();
            let press = self.pressed.take();
            if was_dragging || !left {
                return;
            }
            let target = self.click_target(self.hit_test(p));
            if let (Some(i), Some((key, _))) = (target, press) {
                if self.arena.nodes[i].key == key {
                    let h = &self.arena.nodes[i].handlers;
                    let (click, action) = (h.click.clone(), h.click_action.clone());
                    if let Some(f) = click {
                        env.call(&f, &ClickEvent { position: p, button, modifiers: self.mods });
                    }
                    if let Some(a) = action {
                        if !self.dispatch_action(&*a, env) {
                            self.unhandled.push(a);
                        }
                    }
                }
            }
            return;
        }

        // ---- press ----
        if left {
            if let Some(b) = self.bars.iter().rev().find(|b| b.track.contains(p)) {
                let (start, len, ts, tl) = match b.axis {
                    Axis::X => (b.thumb.x, b.thumb.w, b.track.x, b.track.w),
                    Axis::Y => (b.thumb.y, b.thumb.h, b.track.y, b.track.h),
                };
                let grab = if b.thumb.contains(p) { p.get(b.axis) - start } else { len * 0.5 };
                self.drag = Some(BarDrag { key: b.key, axis: b.axis, grab, track_start: ts, track_len: tl, thumb_len: len, max: b.max });
                self.drag_to(p);
                return;
            }
        }
        let hit = self.hit_test(p);
        let mut cur = hit;
        let mut focus = None;
        let mut down: Option<ErasedFn> = None;
        while let Some(i) = cur {
            let n = &self.arena.nodes[i];
            if focus.is_none() && n.focusable {
                focus = Some(n.key);
            }
            if down.is_none() {
                down = n.handlers.mouse_down.clone();
            }
            cur = self.parent(i);
        }
        if left {
            self.focus = focus;
        }
        if let Some(f) = down {
            env.call(&f, &MouseDownEvent { position: p, button, modifiers: self.mods });
        }
        self.pressed = self.click_target(hit).map(|i| (self.arena.nodes[i].key, button));
    }

    /// Scroll wheel / track pad. `delta` is in logical pixels, positive = scroll up/left
    /// (content moves down/right). Returns `true` if a scroll container consumed it.
    pub fn wheel(&mut self, mut delta: Vec2) -> bool {
        let Some(p) = self.mouse else { return false };
        let mut cur = self.hit_test(p);
        let mut consumed = false;
        while let Some(i) = cur {
            let n = &self.arena.nodes[i];
            if let Some(s) = &n.scroll
                && s.wheel
                && let Some(st) = self.states.get_mut(&n.key)
            {
                let before = st.target;
                let remap = delta.x == 0.0 && (self.mods.shift() || !s.axes.y()) && s.axes.x();
                if s.axes.x() {
                    let dx = if remap { delta.y } else { delta.x };
                    st.target.x = (before.x - dx).clamp(0.0, st.max.x);
                }
                if s.axes.y() && !remap {
                    st.target.y = (before.y - delta.y).clamp(0.0, st.max.y);
                }
                if st.target != before {
                    let used = before - st.target;
                    if remap {
                        delta.y -= used.x;
                    } else {
                        delta -= used;
                    }
                    st.bar_hold = 0.8;
                    consumed = true;
                    if delta == Vec2::ZERO {
                        break;
                    }
                }
            }
            cur = self.parent(i);
        }
        consumed
    }

    pub fn drop_target(&self, pos: Vec2) -> Option<u64> {
        let mut cur = self.hit_test(pos);
        while let Some(i) = cur {
            let n = &self.arena.nodes[i];
            if n.handlers.file_drop.is_some() || n.handlers.file_drop_action.is_some() {
                return Some(n.key);
            }
            cur = self.parent(i);
        }
        None
    }

    pub fn file_drop(&mut self, target: u64, event: FileDropEvent, env: &mut Env) {
        let Some(i) = self.arena.nodes.iter().position(|n| n.key == target) else { return };
        let h = &self.arena.nodes[i].handlers;
        if let Some(f) = &h.file_drop {
            env.call(f, &event);
        }
        if let Some(map) = &h.file_drop_action {
            let action = map(&event);
            if !self.dispatch_from(Some(i), &*action, env) {
                self.unhandled.push(action);
            }
        }
    }

    /// Delivers `action` to the innermost node (from the focus, else the root) that handles it.
    pub fn dispatch_action(&mut self, a: &dyn Action, env: &mut Env) -> bool {
        let origin = self.focus_idx().or(if self.has_root() { Some(0) } else { None });
        self.dispatch_from(origin, a, env)
    }

    fn dispatch_from(&self, mut cur: Option<usize>, a: &dyn Action, env: &mut Env) -> bool {
        let tid = a.type_id_of();
        let mut handled = false;
        while let Some(i) = cur {
            if let Some((_, f)) = self.arena.nodes[i].handlers.actions.iter().find(|(t, _)| *t == tid) {
                handled = true;
                if !env.call(f, a.as_any()) {
                    break;
                }
            }
            cur = self.parent(i);
        }
        handled
    }

    /// Key press: resolve key bindings against the context stack of the focus path; if none
    /// matches, give the innermost `on_key_down` handler the event.
    pub fn key_down(&mut self, key: Option<Key>, text: Option<Arc<str>>, repeat: bool, keymap: &Keymap, env: &mut Env) {
        if !self.has_root() {
            return;
        }
        let start = self.focus_idx().or(Some(0));
        if let Some(k) = key {
            let action = {
                let mut stack: Vec<&str> = Vec::new();
                let mut cur = start;
                while let Some(i) = cur {
                    if let Some(c) = &self.arena.nodes[i].key_context {
                        stack.push(c);
                    }
                    cur = self.parent(i);
                }
                stack.reverse();
                keymap.resolve(Keystroke::new(k, self.mods), &stack)
            };
            if let Some(a) = action {
                if !self.dispatch_action(&*a, env) {
                    self.unhandled.push(a);
                }
                return;
            }
        }
        let mut cur = start;
        while let Some(i) = cur {
            if let Some(f) = self.arena.nodes[i].handlers.key_down.clone() {
                if let Some(k) = key {
                    env.call(&f, &KeyDownEvent { key: k, modifiers: self.mods, text, repeat });
                }
                return;
            }
            cur = self.parent(i);
        }
    }
}

#[allow(dead_code)]
pub(crate) fn dur(d: Duration) -> f32 {
    d.as_secs_f32()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{KeyBinding, Modifiers};
    use crate::components::*;
    use crate::ctx::Context;
    use crate::geometry::{Anchor, Color, Fill};
    use crate::node::node;
    use crate::render::DrawOp;
    use crate::style::*;
    use crate::text::PlaceholderMeasurer;

    #[octlui::action]
    struct Reset;

    #[octlui::action]
    struct Change {
        amount: i32,
    }

    #[derive(Default)]
    struct State {
        value: i32,
        clicks: u32,
        hovered: bool,
        dropped: usize,
    }

    // By-value action handler, as in the design sketch.
    fn reset(state: &mut State, _cx: &mut Context<State>, _: Reset) {
        state.value = 0;
    }
    // By-reference handler.
    fn change(state: &mut State, _cx: &mut Context<State>, a: &Change) {
        state.value += a.amount;
    }
    fn clicked(state: &mut State, _cx: &mut Context<State>, _: &ClickEvent) {
        state.clicks += 1;
    }
    fn hover(state: &mut State, _cx: &mut Context<State>, e: &HoverEvent) {
        state.hovered = e.hovered;
    }
    fn dropped(state: &mut State, _cx: &mut Context<State>, e: &FileDropEvent) {
        state.dropped += e.paths.len();
    }

    struct Harness {
        ui: Ui,
        state: State,
        cx: Context<State>,
        images: ImageStore,
        keymap: Keymap,
    }
    impl Harness {
        fn new(view: impl FnOnce(&Context<State>) -> Node) -> Self {
            let cx = Context::<State>::new();
            let mut ui = Ui::new();
            ui.set_size(Vec2::new(400.0, 300.0), 1.0);
            ui.set_root(view(&cx));
            let mut h = Harness { ui, state: State::default(), cx, images: ImageStore::default(), keymap: Keymap::default() };
            h.frame(0.016);
            h
        }
        fn frame(&mut self, dt: f32) -> bool {
            let mut tm = PlaceholderMeasurer;
            self.ui.frame(dt, dt, &mut LayoutEnv { text: &mut tm, images: &mut self.images }).0
        }
        fn env(&mut self) -> (&mut Ui, Env<'_>) {
            (&mut self.ui, Env { state: &mut self.state, cx: &mut self.cx })
        }
        fn mv(&mut self, x: f32, y: f32) {
            let (ui, mut env) = self.env();
            ui.pointer_moved(Some(Vec2::new(x, y)), &mut env);
        }
        fn click(&mut self, x: f32, y: f32) {
            let (ui, mut env) = self.env();
            let p = Some(Vec2::new(x, y));
            ui.pointer_button(MouseButton::Left, true, p, &mut env);
            ui.pointer_button(MouseButton::Left, false, p, &mut env);
        }
        fn key(&mut self, k: Key) {
            let km = std::mem::take(&mut self.keymap);
            {
                let (ui, mut env) = self.env();
                ui.key_down(Some(k), None, false, &km, &mut env);
            }
            self.keymap = km;
        }
    }

    #[test]
    fn click_calls_listener() {
        let mut h = Harness::new(|cx| {
            node().size(grow(), grow()).child(node().size(fixed(100.0), fixed(100.0)).on_click(cx.listener(clicked)))
        });
        h.click(50.0, 50.0);
        assert_eq!(h.state.clicks, 1);
        h.click(200.0, 50.0); // outside the button
        assert_eq!(h.state.clicks, 1);
    }

    #[test]
    fn key_binding_dispatches_action_by_context() {
        let mut h = Harness::new(|cx| {
            node()
                .size(grow(), grow())
                .key_context("counter")
                .on_action(cx.listener(reset))
                .on_action(cx.listener(change))
        });
        h.keymap.add(KeyBinding::new(Key::R, Reset, Some("counter")));
        h.keymap.add(KeyBinding::new(Key::Up, Change { amount: 5 }, Some("counter")));
        h.keymap.add(KeyBinding::new(Key::Up.with(Modifiers::SHIFT), Change { amount: 50 }, None));
        h.key(Key::Up);
        h.key(Key::Up);
        assert_eq!(h.state.value, 10);
        h.key(Key::R);
        assert_eq!(h.state.value, 0);
        h.ui.mods = Modifiers::SHIFT;
        h.key(Key::Up);
        assert_eq!(h.state.value, 50);
    }

    #[test]
    fn binding_with_wrong_context_does_not_fire() {
        let mut h = Harness::new(|cx| node().size(grow(), grow()).key_context("other").on_action(cx.listener(reset)));
        h.state.value = 7;
        h.keymap.add(KeyBinding::new(Key::R, Reset, Some("counter")));
        h.key(Key::R);
        assert_eq!(h.state.value, 7);
    }

    #[test]
    fn click_action_and_unhandled_routing() {
        let mut h = Harness::new(|cx| {
            node()
                .size(grow(), grow())
                .on_action(cx.listener(change))
                .child(node().size(fixed(50.0), fixed(50.0)).on_click_action(Change { amount: 3 }))
                .child(node().size(fixed(50.0), fixed(50.0)).on_click_action(Reset))
        });
        h.click(10.0, 10.0);
        assert_eq!(h.state.value, 3);
        // `Reset` has no handler on the path: it must be handed back for global routing.
        h.click(60.0, 10.0);
        assert_eq!(h.ui.unhandled.len(), 1);
    }

    #[test]
    fn focus_moves_action_dispatch_start() {
        let mut h = Harness::new(|cx| {
            node()
                .size(grow(), grow())
                .on_action(cx.listener(change))
                .child(
                    node().id("inner").focusable(true).size(fixed(100.0), fixed(100.0)).on_action(cx.listener(reset)),
                )
        });
        h.state.value = 9;
        h.click(10.0, 10.0); // focuses "inner"
        assert_eq!(h.ui.focus, Some(Id::new("inner").hash));
        let (ui, mut env) = h.env();
        assert!(ui.dispatch_action(&Reset, &mut env)); // handled by the focused node
        assert_eq!(h.state.value, 0);
        h.click(300.0, 250.0); // click away: blur
        assert_eq!(h.ui.focus, None);
    }

    #[test]
    fn hover_enter_and_leave() {
        let mut h = Harness::new(|cx| {
            node().size(grow(), grow()).child(node().size(fixed(100.0), fixed(100.0)).on_hover(cx.listener(hover)))
        });
        h.mv(10.0, 10.0);
        assert!(h.state.hovered);
        h.mv(300.0, 10.0);
        assert!(!h.state.hovered);
    }

    #[test]
    fn file_drop_reaches_node_under_pointer() {
        let mut h = Harness::new(|cx| {
            node().size(grow(), grow()).child(node().size(fixed(100.0), fixed(100.0)).on_file_drop(cx.listener(dropped)))
        });
        let (ui, mut env) = h.env();
        let position = Vec2::new(10.0, 10.0);
        let target = ui.drop_target(position).unwrap();
        ui.file_drop(target, FileDropEvent {
            paths: vec!["a.png".into(), "b.png".into()],
            position,
        }, &mut env);
        assert_eq!(ui.drop_target(Vec2::new(300.0, 10.0)), None);
        assert_eq!(h.state.dropped, 2);
    }

    #[test]
    fn file_drop_action_routes_from_the_drop_target_and_can_propagate() {
        let mut h = Harness::new(|cx| {
            node().fill_parent().on_action(cx.listener(change))
                .child(node().size(100., 100.)
                    .on_file_drop_action(|event| Change { amount: event.paths.len() as i32 })
                    .on_action(cx.listener(|state: &mut State, cx: &mut Context<State>, a: &Change| {
                        state.value += a.amount * 10;
                        cx.propagate_action();
                    })))
                .child(node().id("other").size(100., 100.).focusable(true))
        });
        h.ui.set_focus(Some(&Id::new("other")));
        let (ui, mut env) = h.env();
        let position = Vec2::new(10., 10.);
        let target = ui.drop_target(position).unwrap();
        ui.file_drop(target, FileDropEvent { paths: vec!["a.png".into()], position }, &mut env);
        assert_eq!(h.state.value, 11);
        assert!(h.ui.unhandled.is_empty());
    }

    fn scroller(bar: bool, smooth: bool) -> Node {
        let mut s = Scroll::scroll().smooth(smooth);
        if bar {
            s = s.scroll_bar(ScrollBar::scroll_bar().always_visible(true));
        }
        node()
            .id("list")
            .column()
            .size(fixed(200.0), fixed(100.0))
            .scroll(s)
            .child(node().size(grow(), fixed(1000.0)).background(0x3366ff))
    }

    #[test]
    fn wheel_scrolls_and_clamps() {
        let mut h = Harness::new(|_| scroller(false, false));
        h.mv(10.0, 10.0);
        assert!(h.ui.wheel(Vec2::new(0.0, -300.0))); // scroll down by 300
        h.frame(0.016);
        assert_eq!(h.ui.arena.nodes[1].rect.y, -300.0);
        assert!(h.ui.wheel(Vec2::new(0.0, -5000.0)));
        h.frame(0.016);
        assert_eq!(h.ui.arena.nodes[1].rect.y, -900.0); // clamped to content - viewport
        assert!(!h.ui.wheel(Vec2::new(0.0, -5000.0))); // already at the end: not consumed
        // Content is clipped to the viewport.
        let quad = h.ui.list.ops.iter().find_map(|o| if let DrawOp::Quad(q) = o { Some(q) } else { None }).unwrap();
        assert_eq!((quad.clip[1], quad.clip[3]), (0.0, 100.0)); // clipped on Y only
    }

    #[test]
    fn nested_scroll_uses_only_the_available_delta_and_skips_size_measurement() {
        struct NoMeasure;
        impl crate::text::TextMeasurer for NoMeasure {
            fn measure(&mut self, _: &Text, _: Option<f32>) -> Vec2 {
                panic!("wheel input must reuse resolved sizes");
            }
            fn min_content_width(&mut self, _: &Text) -> f32 {
                panic!("wheel input must reuse resolved sizes");
            }
        }
        let mut h = Harness::new(|_| {
            let scroll = Scroll::scroll().smooth(false);
            node().column().size(200., 100.).scroll(scroll.clone())
                .child(node().column().size(200., 50.)
                    .scroll(scroll.offset(Vec2::new(0., 5.)))
                    .child(node().size(200., 70.).text("Inner")))
                .child(node().size(200., 200.).text("Outer"))
        });
        assert_eq!(h.ui.arena.nodes[1].scroll_offset.y, 5.);
        h.mv(10., 10.);
        assert!(h.ui.wheel(Vec2::new(0., -40.)));
        let (moving, geometry_changed) = h.ui.frame(0.016, 0.016, &mut LayoutEnv {
            text: &mut NoMeasure,
            images: &mut h.images,
        });
        assert!(!moving && geometry_changed);
        assert_eq!(h.ui.arena.nodes[1].scroll_offset.y, 20.);
        assert_eq!(h.ui.arena.nodes[0].scroll_offset.y, 25.);
    }

    #[test]
    fn smooth_scroll_glides() {
        let mut h = Harness::new(|_| scroller(false, true));
        h.mv(10.0, 10.0);
        h.ui.wheel(Vec2::new(0.0, -200.0));
        let moving = h.frame(0.016);
        let y1 = -h.ui.arena.nodes[1].rect.y;
        assert!(moving && y1 > 0.0 && y1 < 200.0, "y1 = {y1}");
        let mut n = 0;
        while h.frame(0.016) {
            n += 1;
            assert!(n < 300);
        }
        assert_eq!(-h.ui.arena.nodes[1].rect.y, 200.0);
    }

    #[test]
    fn scrollbar_thumb_drag() {
        let mut h = Harness::new(|_| scroller(true, false));
        h.frame(0.5); // let the bar fade in
        assert!(!h.ui.bars.is_empty());
        let thumb = h.ui.bars[0].thumb;
        let (x, y) = (thumb.center().x, thumb.center().y);
        let (ui, mut env) = h.env();
        ui.pointer_button(MouseButton::Left, true, Some(Vec2::new(x, y)), &mut env);
        ui.pointer_moved(Some(Vec2::new(x, y + 40.0)), &mut env);
        ui.pointer_button(MouseButton::Left, false, Some(Vec2::new(x, y + 40.0)), &mut env);
        h.frame(0.016);
        assert!(h.ui.arena.nodes[1].rect.y < -100.0, "dragging the thumb scrolled the content");
    }

    #[test]
    fn spring_lags_then_settles() {
        let make = |x: f32| {
            node()
                .size(grow(), grow())
                .child(node().width(fixed(x)).height(fixed(1.0)))
                .child(
                    node()
                        .id("s")
                        .size(fixed(20.0), fixed(20.0))
                        .background(0xff0000)
                        .spring(Spring::new(SpringProps::POSITION | SpringProps::BACKGROUND)),
                )
        };
        let mut h = Harness::new(|_| make(0.0));
        h.ui.set_root(make(100.0).background(Fill::NONE)); // spacer grows: spring node target moves by +100
        h.frame(0.016);
        let sp = h.ui.states[&Id::new("s").hash].spring.as_ref().unwrap();
        let d = sp.visual.delta.x;
        assert!(d < 0.0 && d > -100.0, "visual lags behind declared position, delta = {d}");
        let mut n = 0;
        while h.frame(0.016) {
            n += 1;
            assert!(n < 600);
        }
        let sp = h.ui.states[&Id::new("s").hash].spring.as_ref().unwrap();
        assert_eq!(sp.visual.delta, Vec2::ZERO);
    }

    #[test]
    fn reflow_spring_moves_siblings_and_preserves_hit_geometry() {
        let view = |width, cx: &Context<State>| {
            node().row().fill_parent()
                .child(node().id("resize").size(width, 40.).spring(Spring::new(SpringProps::WIDTH)))
                .child(node().size(35., 40.).on_click(cx.listener(clicked)))
        };
        let mut h = Harness::new(|cx| view(40., cx));
        h.ui.set_root(view(140., &h.cx));
        assert!(h.frame(0.016));
        let edge = h.ui.arena.nodes[1].rect.right();
        assert!(edge > 40. && edge < 140.);
        assert_eq!(h.ui.arena.nodes[2].rect.x, edge);
        h.click(edge + 5., 20.);
        assert_eq!(h.state.clicks, 1);
        for _ in 0..240 {
            if !h.frame(0.016) {
                break;
            }
        }
        assert_eq!(h.ui.arena.nodes[2].rect.x, 140.);
    }

    #[test]
    fn tooltip_appears_after_delay_and_fades() {
        let mut h = Harness::new(|_| {
            node().size(grow(), grow()).child(
                node().id("t").size(fixed(60.0), fixed(30.0)).tooltip(
                    Tooltip::new(node().size(fixed(80.0), fixed(20.0)).background(0x222222))
                        .delay(Duration::from_millis(300))
                        .fade_in(Duration::from_millis(100))
                        .fade_out(Duration::from_millis(100)),
                ),
            )
        });
        h.mv(10.0, 10.0);
        let base = h.ui.list.ops.len();
        assert!(!h.frame(0.1));
        assert!(h.ui.tooltip_wait().is_some());
        assert_eq!(h.ui.list.ops.len(), base, "not yet visible");
        h.frame(0.25);
        h.frame(0.05);
        assert_eq!(h.ui.list.ops.len(), base + 1, "tooltip quad drawn");
        let DrawOp::Quad(q) = h.ui.list.ops.last().unwrap() else { panic!() };
        assert!(q.fill[3] > 0.0 && q.fill[3] <= 1.0);
        // Placed below the owner and horizontally centered on it.
        assert_eq!(q.rect[1], 30.0 + 6.0);
        h.mv(300.0, 250.0);
        let mut n = 0;
        while h.frame(0.05) {
            n += 1;
            assert!(n < 50);
        }
        assert_eq!(h.ui.list.ops.len(), base, "tooltip gone after fade out");
    }

    #[test]
    fn floating_paints_above_siblings_and_hits_first() {
        let mut h = Harness::new(|cx| {
            node()
                .size(fixed(200.0), fixed(200.0))
                .child(node().size(fixed(200.0), fixed(200.0)).on_click(cx.listener(clicked)).background(0x111111))
                .child(
                    node()
                        .size(fixed(50.0), fixed(50.0))
                        .background(0xffffff)
                        .on_hover(cx.listener(hover))
                        .floating(Floating::floating().parent_anchor(Anchor::TOP_LEFT).z_index(5)),
                )
        });
        // Floating node overlaps the first child; hovering there hits the floating node.
        h.mv(10.0, 10.0);
        assert!(h.state.hovered);
        let last = h.ui.list.ops.last().unwrap();
        let DrawOp::Quad(q) = last else { panic!() };
        assert_eq!(q.rect, [0.0, 0.0, 50.0, 50.0]);
    }

    #[test]
    fn gradients_are_uploaded() {
        let h = Harness::new(|_| {
            node().size(fixed(100.0), fixed(100.0)).background(Fill::linear(0.0, &[Color::hex(0xff0000), Color::hex(0x0000ff)]))
        });
        assert_eq!(h.ui.list.gradients.len(), 1);
        let DrawOp::Quad(q) = &h.ui.list.ops[0] else { panic!() };
        assert_eq!(q.extra[1], 0.0); // gradient index 0
        assert_eq!(h.ui.list.gradients[0].params[1], 2.0); // two stops
    }

    #[test]
    fn image_fit_modes() {
        let mut store = ImageStore::default();
        let id = store.insert_rgba(200, 100, vec![255; 200 * 100 * 4]);
        let mut ui = Ui::new();
        ui.set_size(Vec2::new(400.0, 300.0), 1.0);
        let mut quads = Vec::new();
        for fit in [ImageFit::Contain, ImageFit::Cover, ImageFit::Fill, ImageFit::None] {
            ui.set_root(node().size(fixed(100.0), fixed(100.0)).image(Image::new(id).fit(fit)));
            let mut tm = PlaceholderMeasurer;
            ui.frame(0.016, 0.016, &mut LayoutEnv { text: &mut tm, images: &mut store });
            let DrawOp::Image(_, q) = ui.list.ops.last().unwrap().clone() else { panic!() };
            quads.push(q);
        }
        assert_eq!(quads[0].rect, [0.0, 25.0, 100.0, 50.0]); // contain: letterboxed
        assert_eq!(quads[1].rect, [0.0, 0.0, 100.0, 100.0]); // cover: crops via uv
        assert!((quads[1].uv[2] - quads[1].uv[0] - 0.5).abs() < 1e-6);
        assert_eq!(quads[2].uv, [0.0, 0.0, 1.0, 1.0]); // fill: stretched
        assert_eq!(quads[3].rect, [0.0, 0.0, 100.0, 100.0]); // none: cropped at natural size
    }
}
