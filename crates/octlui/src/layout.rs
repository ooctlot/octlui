//! Layout engine.
//!
//! The node tree is flattened breadth-first into an [`Arena`], so every node's children are a
//! contiguous index range and a parent always has a smaller index than its children. That lets
//! each pass be a plain loop (forward = top-down, reverse = bottom-up) without recursion.
//!
//! Passes: fit widths (bottom-up) -> grow/shrink widths (top-down) -> fit heights (bottom-up,
//! text wraps here) -> grow/shrink heights (top-down) -> position.

use crate::components::*;
use crate::geometry::{Padding, Rect, Vec2};
use crate::id::{Id, mix};
use crate::image::ImageStore;
use crate::node::{Handlers, Node};
use crate::style::*;
use crate::text::TextMeasurer;
use std::sync::Arc;

pub(crate) const NONE: u32 = u32::MAX;
const EPS: f32 = 0.01;

pub(crate) struct LNode {
    pub key: u64,
    #[allow(dead_code)] // kept for debugging/inspection tooling
    pub id: Option<Id>,
    pub parent: u32,
    pub first_child: u32,
    pub child_count: u32,

    pub width: Sizing,
    pub height: Sizing,
    pub direction: Direction,
    pub align_x: Align,
    pub align_y: Align,
    pub gap: f32,
    /// Padding including space reserved for scroll bars.
    pub pad: Padding,
    pub corner: crate::geometry::CornerRadius,
    pub aspect: Option<f32>,
    pub clip: Clip,
    pub key_context: Option<Arc<str>>,
    pub focusable: bool,
    pub interactive: bool,
    pub handlers: Handlers,

    pub appearance: Option<Box<Appearance>>,
    pub text: Option<Box<Text>>,
    pub image: Option<Box<Image>>,
    pub tooltip: Option<Box<Tooltip>>,
    pub scroll: Option<Box<Scroll>>,
    pub floating: Option<Box<Floating>>,
    pub spring: Option<Box<Spring>>,
    pub reflow: Option<Box<Reflow>>,

    // ---- computed ----
    pub is_floating: bool,
    pub size: Vec2,
    pub min_size: Vec2,
    /// Declared position/size (before springs), scroll offsets of ancestors included.
    pub rect: Rect,
    /// Content size including padding.
    pub content: Vec2,
    /// Requested scroll offset (set by the UI from retained state before layout).
    pub scroll_offset: Vec2,
    /// Maximum scroll offset (set by layout).
    pub scroll_max: Vec2,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Reflow {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub padding: Padding,
    pub gap: f32,
}

impl LNode {
    #[inline]
    fn sizing(&self, a: Axis) -> Sizing {
        match a {
            Axis::X => self.reflow.as_ref().and_then(|r| r.width).map_or(self.width, fixed),
            Axis::Y => self.reflow.as_ref().and_then(|r| r.height).map_or(self.height, fixed),
        }
    }
    pub fn padding(&self) -> Padding {
        self.reflow.as_ref().map_or(self.pad, |r| r.padding)
    }
    fn gap(&self) -> f32 {
        self.reflow.as_ref().map_or(self.gap, |r| r.gap)
    }
    #[inline]
    fn pad_along(&self, a: Axis) -> f32 {
        match a {
            Axis::X => self.padding().horizontal(),
            Axis::Y => self.padding().vertical(),
        }
    }
    #[inline]
    pub fn children(&self) -> std::ops::Range<usize> {
        self.first_child as usize..(self.first_child + self.child_count) as usize
    }
}

#[derive(Default)]
pub(crate) struct Arena {
    pub nodes: Vec<LNode>,
}

fn convert(node: Node, parent: u32, key: u64) -> (LNode, Vec<Node>) {
    let Node {
        id, width, height, direction, child_align_x, child_align_y, child_gap, mut padding, corner_radius,
        aspect_ratio, clip, key_context, focusable, appearance, text, image, tooltip, scroll, floating,
        spring, handlers, children,
    } = node;

    let mut clip = clip;
    if let Some(s) = &scroll {
        // Scrolling implies clipping on the scrolled axes.
        clip = match (clip.x() || s.axes.x(), clip.y() || s.axes.y()) {
            (true, true) => Clip::XY,
            (true, false) => Clip::X,
            (false, true) => Clip::Y,
            _ => Clip::None,
        };
        if let Some(bar) = &s.scroll_bar {
            if bar.reserve_space {
                if s.axes.y() {
                    match bar.side {
                        Side::End => padding.right += bar.thickness,
                        Side::Start => padding.left += bar.thickness,
                    }
                }
                if s.axes.x() {
                    match bar.side {
                        Side::End => padding.bottom += bar.thickness,
                        Side::Start => padding.top += bar.thickness,
                    }
                }
            }
        }
    }

    let n = LNode {
        key,
        is_floating: parent != NONE && floating.is_some(),
        id,
        parent,
        first_child: NONE,
        child_count: 0,
        width,
        height,
        direction,
        align_x: child_align_x,
        align_y: child_align_y,
        gap: child_gap,
        pad: padding,
        corner: corner_radius,
        aspect: aspect_ratio.filter(|r| *r > 0.0),
        clip,
        key_context,
        focusable,
        interactive: false,
        handlers,
        appearance,
        text,
        image,
        tooltip,
        scroll,
        floating,
        spring,
        reflow: None,
        size: Vec2::ZERO,
        min_size: Vec2::ZERO,
        rect: Rect::default(),
        content: Vec2::ZERO,
        scroll_offset: Vec2::ZERO,
        scroll_max: Vec2::ZERO,
    };
    (n, children)
}

impl Arena {
    /// Flattens `root` (breadth-first, children contiguous). Reuses the allocation.
    pub fn build(&mut self, root: Node) {
        self.nodes.clear();
        let mut pending: Vec<Vec<Node>> = Vec::new();
        let root_key = root.id.as_ref().map_or(mix(0, 0x0c71), |i| i.hash);
        let (mut n, kids) = convert(root, NONE, root_key);
        n.interactive = n.handlers_or_flags();
        self.nodes.push(n);
        pending.push(kids);

        let mut i = 0;
        while i < self.nodes.len() {
            let kids = std::mem::take(&mut pending[i]);
            let parent_key = self.nodes[i].key;
            self.nodes[i].first_child = self.nodes.len() as u32;
            self.nodes[i].child_count = kids.len() as u32;
            for (idx, kid) in kids.into_iter().enumerate() {
                let key = kid.id.as_ref().map_or_else(|| mix(parent_key, idx as u64 + 1), |i| i.hash);
                let (mut ln, grand) = convert(kid, i as u32, key);
                ln.interactive = ln.handlers_or_flags();
                self.nodes.push(ln);
                pending.push(grand);
            }
            i += 1;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl LNode {
    fn handlers_or_flags(&self) -> bool {
        let h = &self.handlers;
        h.click.is_some()
            || h.click_action.is_some()
            || h.mouse_down.is_some()
            || h.hover.is_some()
            || h.key_down.is_some()
            || h.file_drop.is_some()
            || h.file_drop_action.is_some()
            || !h.actions.is_empty()
            || self.focusable
            || self.tooltip.is_some()
            || self.scroll.is_some()
    }
}

pub(crate) struct LayoutEnv<'a> {
    pub text: &'a mut dyn TextMeasurer,
    pub images: &'a mut ImageStore,
}

/// Runs all layout passes. `viewport` is in logical pixels.
pub(crate) fn layout(arena: &mut Arena, viewport: Vec2, env: &mut LayoutEnv) {
    if arena.is_empty() {
        return;
    }
    resolve_sizes(arena, viewport, env);
    position(arena);
}

pub(crate) fn resolve_sizes(arena: &mut Arena, viewport: Vec2, env: &mut LayoutEnv) {
    fit_pass(arena, Axis::X, env);
    resolve_pass(arena, Axis::X, viewport);
    fit_pass(arena, Axis::Y, env);
    resolve_pass(arena, Axis::Y, viewport);
}

fn leaf_content(node: &LNode, axis: Axis, env: &mut LayoutEnv) -> Option<(f32, f32)> {
    let pad = node.pad_along(axis);
    if let Some(t) = &node.text {
        return Some(match axis {
            Axis::X => {
                let nat = env.text.measure(t, None).x;
                (nat + pad, env.text.min_content_width(t).min(nat) + pad)
            }
            Axis::Y => {
                let w = (node.size.x - node.pad_along(Axis::X)).max(0.0);
                let h = env.text.measure(t, Some(w)).y + pad;
                (h, h)
            }
        });
    }
    if node.child_count == 0 {
        if let Some(img) = &node.image {
            if let Some((w, h)) = env.images.size(img.source) {
                let v = if axis == Axis::X { w } else { h } as f32;
                return Some((v + pad, pad));
            }
        }
    }
    None
}

fn fit_pass(arena: &mut Arena, axis: Axis, env: &mut LayoutEnv) {
    for i in (0..arena.nodes.len()).rev() {
        let (size, min) = {
            let node = &arena.nodes[i];
            let sizing = node.sizing(axis);
            let pad = node.pad_along(axis);
            let (content, min_content) = match leaf_content(node, axis, env) {
                Some(v) => v,
                None => {
                    let main = node.direction.axis() == axis;
                    let (mut sum, mut msum, mut mx, mut mmx, mut count) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0usize);
                    for c in node.children() {
                        let ch = &arena.nodes[c];
                        if ch.is_floating {
                            continue;
                        }
                        let s = ch.size.get(axis);
                        let m = ch.min_size.get(axis);
                        sum += s;
                        msum += m;
                        mx = mx.max(s);
                        mmx = mmx.max(m);
                        count += 1;
                    }
                    if main {
                        let gaps = node.gap() * count.saturating_sub(1) as f32;
                        (sum + gaps + pad, msum + gaps + pad)
                    } else {
                        (mx + pad, mmx + pad)
                    }
                }
            };
            match sizing.kind {
                SizeKind::Fixed(v) => {
                    let v = sizing.clamp(v);
                    (v, v)
                }
                _ => {
                    let size = sizing.clamp(content);
                    // A scroll viewport can shrink independently of the content it exposes.
                    let scrolls = node.scroll.as_ref().is_some_and(|s| match axis {
                        Axis::X => s.axes.x(),
                        Axis::Y => s.axes.y(),
                    });
                    let min_content = if scrolls { pad } else { min_content };
                    let min = sizing.min.max(min_content).min(size);
                    (size, min)
                }
            }
        };
        let node = &mut arena.nodes[i];
        let mut size = size;
        let mut min = min;
        match axis {
            // Width derived from a fixed height + aspect ratio.
            Axis::X => {
                let (w, h) = (node.sizing(Axis::X), node.sizing(Axis::Y));
                if let (Some(r), SizeKind::Fixed(height)) = (node.aspect, h.kind)
                    && !matches!(w.kind, SizeKind::Fixed(_))
                {
                    size = w.clamp(h.clamp(height) * r);
                    min = min.min(size);
                }
            }
            // Height derived from the (now final) width + aspect ratio.
            Axis::Y => {
                if let Some(r) = node.aspect {
                    let height = node.sizing(Axis::Y);
                    if !matches!(height.kind, SizeKind::Fixed(_)) {
                        size = height.clamp(node.size.x / r);
                        min = size;
                    }
                }
            }
        }
        node.size.set(axis, size);
        node.min_size.set(axis, min);
    }
}

fn resolve_pass(arena: &mut Arena, axis: Axis, viewport: Vec2) {
    // Root sizing against the viewport.
    {
        let root = &mut arena.nodes[0];
        let s = root.sizing(axis);
        let vp = viewport.get(axis);
        match s.kind {
            SizeKind::Grow => root.size.set(axis, s.clamp(vp)),
            SizeKind::Fraction(f) => root.size.set(axis, s.clamp(vp * f)),
            _ => {}
        }
    }

    let mut flow: Vec<usize> = Vec::new();
    let mut tmp: Vec<usize> = Vec::new();
    for i in 0..arena.nodes.len() {
        if arena.nodes[i].child_count == 0 {
            continue;
        }
        let (range, dir, gap, parent_size, inner) = {
            let n = &arena.nodes[i];
            let size = n.size.get(axis);
            (n.children(), n.direction, n.gap(), size, (size - n.pad_along(axis)).max(0.0))
        };
        flow.clear();
        for c in range.clone() {
            if !arena.nodes[c].is_floating {
                flow.push(c);
            }
        }

        if dir.axis() == axis {
            // ---- main axis: fractions, then grow or shrink ----
            for &c in &flow {
                let n = &mut arena.nodes[c];
                if let SizeKind::Fraction(f) = n.sizing(axis).kind {
                    let v = n.sizing(axis).clamp(inner * f);
                    n.size.set(axis, v);
                }
            }
            let gaps = gap * flow.len().saturating_sub(1) as f32;
            let used: f32 = flow.iter().map(|&c| arena.nodes[c].size.get(axis)).sum::<f32>() + gaps;
            let mut remaining = inner - used;

            if remaining > EPS {
                tmp.clear();
                tmp.extend(flow.iter().copied().filter(|&c| matches!(arena.nodes[c].sizing(axis).kind, SizeKind::Grow)));
                grow_smallest_first(arena, axis, &mut tmp, &mut remaining);
            } else if remaining < -EPS {
                tmp.clear();
                tmp.extend(flow.iter().copied().filter(|&c| !matches!(arena.nodes[c].sizing(axis).kind, SizeKind::Fixed(_))));
                shrink_largest_first(arena, axis, &mut tmp, &mut remaining);
            }
        } else {
            // ---- cross axis ----
            for &c in &flow {
                let n = &mut arena.nodes[c];
                let s = n.sizing(axis);
                match s.kind {
                    SizeKind::Grow => n.size.set(axis, s.clamp(inner)),
                    SizeKind::Fraction(f) => n.size.set(axis, s.clamp(inner * f)),
                    SizeKind::Fit => {
                        let cur = n.size.get(axis);
                        if cur > inner + EPS {
                            let v = inner.max(n.min_size.get(axis));
                            n.size.set(axis, v.min(cur));
                        }
                    }
                    SizeKind::Fixed(_) => {}
                }
            }
        }

        // Floating children size against their parent's full box.
        for c in range {
            if !arena.nodes[c].is_floating {
                continue;
            }
            let n = &mut arena.nodes[c];
            let s = n.sizing(axis);
            match s.kind {
                SizeKind::Grow => n.size.set(axis, s.clamp(parent_size)),
                SizeKind::Fraction(f) => n.size.set(axis, s.clamp(parent_size * f)),
                _ => {}
            }
        }
    }
}

/// Distributes `remaining` over `idx` by repeatedly growing the smallest nodes.
fn grow_smallest_first(arena: &mut Arena, axis: Axis, idx: &mut Vec<usize>, remaining: &mut f32) {
    let mut guard = 0;
    while *remaining > EPS && !idx.is_empty() && guard < 1024 {
        guard += 1;
        let mut smallest = f32::INFINITY;
        let mut second = f32::INFINITY;
        for &c in idx.iter() {
            let s = arena.nodes[c].size.get(axis);
            if s < smallest - EPS {
                second = smallest;
                smallest = s;
            } else if s > smallest + EPS && s < second {
                second = s;
            }
        }
        let at_smallest = idx.iter().filter(|&&c| (arena.nodes[c].size.get(axis) - smallest).abs() <= EPS).count();
        let step = (second - smallest).min(*remaining / at_smallest as f32);
        for &c in idx.iter() {
            let n = &mut arena.nodes[c];
            let s = n.size.get(axis);
            if (s - smallest).abs() <= EPS {
                let max = n.sizing(axis).max;
                let new = (s + step).min(max.max(s));
                *remaining -= new - s;
                n.size.set(axis, new);
            }
        }
        idx.retain(|&c| {
            let n = &arena.nodes[c];
            n.size.get(axis) < n.sizing(axis).max - EPS
        });
    }
}

/// Takes `-remaining` away from `idx` by repeatedly shrinking the largest nodes down to their
/// minimum sizes.
fn shrink_largest_first(arena: &mut Arena, axis: Axis, idx: &mut Vec<usize>, remaining: &mut f32) {
    idx.retain(|&c| arena.nodes[c].size.get(axis) > arena.nodes[c].min_size.get(axis) + EPS);
    let mut guard = 0;
    while *remaining < -EPS && !idx.is_empty() && guard < 1024 {
        guard += 1;
        let mut largest = f32::NEG_INFINITY;
        let mut second = f32::NEG_INFINITY;
        for &c in idx.iter() {
            let s = arena.nodes[c].size.get(axis);
            if s > largest + EPS {
                second = largest;
                largest = s;
            } else if s < largest - EPS && s > second {
                second = s;
            }
        }
        let at_largest = idx.iter().filter(|&&c| (arena.nodes[c].size.get(axis) - largest).abs() <= EPS).count();
        let step = if second.is_finite() { (largest - second).min(-*remaining / at_largest as f32) } else { -*remaining / at_largest as f32 };
        for &c in idx.iter() {
            let n = &mut arena.nodes[c];
            let s = n.size.get(axis);
            if (s - largest).abs() <= EPS {
                let min = n.min_size.get(axis);
                let new = (s - step).max(min);
                *remaining += s - new;
                n.size.set(axis, new);
            }
        }
        idx.retain(|&c| {
            let n = &arena.nodes[c];
            n.size.get(axis) > n.min_size.get(axis) + EPS
        });
    }
}

pub(crate) fn position(arena: &mut Arena) {
    {
        let r = &mut arena.nodes[0];
        r.rect = Rect::new(0.0, 0.0, r.size.x, r.size.y);
    }
    for i in 0..arena.nodes.len() {
        let (range, dir, gap, align_x, align_y, pad, rect, scroll_axes, req_offset) = {
            let n = &mut arena.nodes[i];
            n.rect.w = n.size.x;
            n.rect.h = n.size.y;
            let axes = n.scroll.as_ref().map_or(Clip::None, |s| s.axes);
            (n.children(), n.direction, n.gap(), n.align_x, n.align_y, n.padding(), n.rect, axes, n.scroll_offset)
        };
        if arena.nodes[i].child_count == 0 {
            arena.nodes[i].content = Vec2::new(pad.horizontal(), pad.vertical());
            arena.nodes[i].scroll_max = Vec2::ZERO;
            continue;
        }
        let main = dir.axis();
        let cross = main.other();
        let (mut total, mut max_cross, mut count) = (0.0f32, 0.0f32, 0usize);
        for c in range.clone() {
            let ch = &arena.nodes[c];
            if ch.is_floating {
                continue;
            }
            total += ch.size.get(main);
            max_cross = max_cross.max(ch.size.get(cross));
            count += 1;
        }
        total += gap * count.saturating_sub(1) as f32;

        let (pad_main, pad_cross) = match main {
            Axis::X => (pad.horizontal(), pad.vertical()),
            Axis::Y => (pad.vertical(), pad.horizontal()),
        };
        let mut content = Vec2::ZERO;
        content.set(main, total + pad_main);
        content.set(cross, max_cross + pad_cross);

        // Scroll offset clamped to the scrollable overflow.
        let mut max_off = Vec2::ZERO;
        max_off.x = if scroll_axes.x() { (content.x - rect.w).max(0.0) } else { 0.0 };
        max_off.y = if scroll_axes.y() { (content.y - rect.h).max(0.0) } else { 0.0 };
        let off = Vec2::new(req_offset.x.clamp(0.0, max_off.x), req_offset.y.clamp(0.0, max_off.y));
        {
            let n = &mut arena.nodes[i];
            n.content = content;
            n.scroll_max = max_off;
            n.scroll_offset = off;
        }

        let inner_origin = Vec2::new(rect.x + pad.left, rect.y + pad.top);
        let inner_size = Vec2::new((rect.w - pad.horizontal()).max(0.0), (rect.h - pad.vertical()).max(0.0));
        let (align_main, align_cross) = match main {
            Axis::X => (align_x, align_y),
            Axis::Y => (align_y, align_x),
        };
        let slack = (inner_size.get(main) - total).max(0.0);
        let mut cursor = inner_origin.get(main) + slack * align_main.factor() - off.get(main);

        let mut place = |arena: &mut Arena, c: usize| {
            let ch = &mut arena.nodes[c];
            let csz = ch.size;
            let mut o = Vec2::ZERO;
            o.set(main, cursor);
            let cslack = (inner_size.get(cross) - csz.get(cross)).max(0.0);
            o.set(cross, inner_origin.get(cross) + cslack * align_cross.factor() - off.get(cross));
            ch.rect = Rect::from_origin_size(o, csz);
            cursor += csz.get(main) + gap;
        };
        if dir.reversed() {
            for c in range.clone().rev() {
                if !arena.nodes[c].is_floating {
                    place(arena, c);
                }
            }
        } else {
            for c in range.clone() {
                if !arena.nodes[c].is_floating {
                    place(arena, c);
                }
            }
        }

        for c in range {
            let ch = &mut arena.nodes[c];
            if !ch.is_floating {
                continue;
            }
            let f = ch.floating.as_ref().expect("floating flag implies component");
            let p = rect.anchor_point(f.parent_anchor) + f.offset;
            let origin = Vec2::new(p.x - ch.size.x * f.anchor.x.factor(), p.y - ch.size.y * f.anchor.y.factor());
            ch.rect = Rect::from_origin_size(origin, ch.size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Anchor;
    use crate::node::node;
    use crate::text::PlaceholderMeasurer;

    fn run(root: Node, w: f32, h: f32) -> Arena {
        let mut a = Arena::default();
        a.build(root);
        let mut tm = PlaceholderMeasurer;
        let mut imgs = ImageStore::default();
        layout(&mut a, Vec2::new(w, h), &mut LayoutEnv { text: &mut tm, images: &mut imgs });
        a
    }
    fn r(a: &Arena, i: usize) -> Rect {
        a.nodes[i].rect
    }

    #[test]
    fn row_fixed_and_grow() {
        let a = run(
            node().size(grow(), grow()).child(node().size(fixed(100.0), fixed(50.0))).child(node().size(grow(), grow())),
            400.0,
            300.0,
        );
        assert_eq!(r(&a, 1), Rect::new(0.0, 0.0, 100.0, 50.0));
        assert_eq!(r(&a, 2), Rect::new(100.0, 0.0, 300.0, 300.0));
    }

    #[test]
    fn grow_shares_equally_and_respects_max() {
        let a = run(
            node()
                .size(grow(), grow())
                .child(node().width(grow().max(50.0)).height(grow()))
                .child(node().size(grow(), grow()))
                .child(node().size(grow(), grow())),
            350.0,
            10.0,
        );
        assert_eq!(r(&a, 1).w, 50.0);
        assert_eq!(r(&a, 2).w, 150.0);
        assert_eq!(r(&a, 3).w, 150.0);
        assert_eq!(r(&a, 3).x, 200.0);
    }

    #[test]
    fn column_gap_padding_fit() {
        let a = run(
            node().column().padding(10.0).child_gap(5.0)
                .child(node().size(fixed(40.0), fixed(20.0)))
                .child(node().size(fixed(60.0), fixed(30.0))),
            500.0,
            500.0,
        );
        assert_eq!((r(&a, 0).w, r(&a, 0).h), (80.0, 75.0));
        assert_eq!(r(&a, 1), Rect::new(10.0, 10.0, 40.0, 20.0));
        assert_eq!(r(&a, 2), Rect::new(10.0, 35.0, 60.0, 30.0));
    }

    #[test]
    fn alignment_center_and_end() {
        let a = run(
            node().size(fixed(100.0), fixed(100.0)).center().child(node().size(fixed(20.0), fixed(20.0))),
            200.0, 200.0,
        );
        assert_eq!(r(&a, 1), Rect::new(40.0, 40.0, 20.0, 20.0));
        let a = run(
            node().size(fixed(100.0), fixed(100.0)).align(Align::End, Align::End).child(node().size(fixed(20.0), fixed(20.0))),
            200.0, 200.0,
        );
        assert_eq!(r(&a, 1), Rect::new(80.0, 80.0, 20.0, 20.0));
    }

    #[test]
    fn reversed_direction() {
        let a = run(
            node().size(fixed(100.0), fixed(10.0)).direction(Direction::RightToLeft)
                .child(node().size(fixed(10.0), fixed(10.0)))
                .child(node().size(fixed(20.0), fixed(10.0))),
            200.0, 200.0,
        );
        assert_eq!(r(&a, 2).x, 0.0);
        assert_eq!(r(&a, 1).x, 20.0);
    }

    #[test]
    fn fraction_sizes() {
        let a = run(
            node().size(fixed(200.0), fixed(100.0)).padding(10.0)
                .child(node().width(fraction(0.5)).height(fraction(1.0))),
            400.0, 400.0,
        );
        assert_eq!((r(&a, 1).w, r(&a, 1).h), (90.0, 80.0));
    }

    #[test]
    fn shrink_to_min() {
        // Two Grow children with min widths in a too-small parent shrink to their minimums.
        let a = run(
            node().size(fixed(100.0), fixed(10.0))
                .child(node().width(fixed(80.0)).height(fixed(10.0)))
                .child(node().width(grow().min(30.0)).height(fixed(10.0)).child(node().size(fixed(60.0), fixed(10.0)))),
            400.0, 400.0,
        );
        assert_eq!(r(&a, 1).w, 80.0);
        assert!(r(&a, 2).w >= 30.0);
    }

    #[test]
    fn aspect_ratio_from_width() {
        let a = run(node().width(fixed(160.0)).aspect_ratio(16.0 / 9.0), 400.0, 400.0);
        assert!((r(&a, 0).h - 90.0).abs() < 0.01);
        let a = run(node().height(fixed(90.0)).aspect_ratio(16.0 / 9.0), 400.0, 400.0);
        assert!((r(&a, 0).w - 160.0).abs() < 0.01);
    }

    #[test]
    fn text_wraps_and_grows_height() {
        let t = Text::new("aaaa bbbb cccc dddd").size(10.0); // glyph 5.5, line 12
        let a = run(node().column().width(fixed(50.0)).child(node().width(grow()).text(t)), 400.0, 400.0);
        // 9 columns per line -> "aaaa bbbb" / "cccc dddd" = 2 lines
        assert_eq!(r(&a, 1).h, 24.0);
        assert_eq!(r(&a, 0).h, 24.0);
    }

    #[test]
    fn text_clip_can_shrink() {
        let t = Text::new("a very long single line of text").size(10.0).wrap(TextWrap::Clip { ellipsis: true });
        let a = run(node().width(fixed(40.0)).child(node().width(grow()).text(t)), 400.0, 400.0);
        assert_eq!(r(&a, 1).w, 40.0);
    }

    #[test]
    fn floating_does_not_affect_flow() {
        let a = run(
            node().size(fixed(100.0), fixed(100.0))
                .child(node().size(fixed(10.0), fixed(10.0)))
                .child(
                    node().size(fixed(30.0), fixed(20.0)).floating(
                        Floating::floating().parent_anchor(Anchor::BOTTOM_RIGHT).anchor(Anchor::TOP_RIGHT).offset(Vec2::new(0.0, 4.0)),
                    ),
                ),
            200.0, 200.0,
        );
        assert_eq!(r(&a, 2), Rect::new(70.0, 104.0, 30.0, 20.0));
        assert_eq!(a.nodes[2].is_floating, true);
    }

    #[test]
    fn scroll_content_and_offset_clamp() {
        let mut a = Arena::default();
        a.build(
            node().column().size(fixed(100.0), fixed(100.0)).scroll(Scroll::scroll())
                .child(node().size(fixed(100.0), fixed(300.0))),
        );
        a.nodes[0].scroll_offset = Vec2::new(0.0, 500.0);
        let mut tm = PlaceholderMeasurer;
        let mut imgs = ImageStore::default();
        layout(&mut a, Vec2::new(400.0, 400.0), &mut LayoutEnv { text: &mut tm, images: &mut imgs });
        assert_eq!(a.nodes[0].scroll_max, Vec2::new(0.0, 200.0));
        assert_eq!(a.nodes[0].scroll_offset.y, 200.0);
        assert_eq!(a.nodes[1].rect.y, -200.0);
    }

    #[test]
    fn scrollbar_reserves_space() {
        let a = run(
            node().size(fixed(100.0), fixed(100.0)).scroll(Scroll::scroll().scroll_bar(ScrollBar::scroll_bar().reserve_space(true).thickness(10.0)))
                .child(node().width(grow()).height(fixed(10.0))),
            400.0, 400.0,
        );
        assert_eq!(r(&a, 1).w, 90.0);
    }

    #[test]
    fn many_nodes_are_contiguous() {
        let kids: Vec<Node> = (0..1000).map(|_| node().size(grow(), fixed(1.0))).collect();
        let a = run(node().column().size(grow(), grow()).children(kids), 100.0, 5000.0);
        assert_eq!(a.nodes[0].child_count, 1000);
        assert_eq!(r(&a, 1000).y, 999.0);
    }
}
