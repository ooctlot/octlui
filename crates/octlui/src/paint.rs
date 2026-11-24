//! Turns a laid-out [`Arena`] into [`RenderList`] draw ops and hit regions.

use crate::components::*;
use crate::geometry::{CornerRadius, Fill, Rect, Vec2};
use crate::image::ImageStore;
use crate::layout::Arena;
use crate::render::{DrawOp, QuadInstance, RenderList, TextRun};
use crate::style::*;
use crate::text::ShapedText;
use crate::text_input::{Editor, input_wrap, normalize};
use crate::ui::{BarHit, Hit, InputHit, NodeState, Visual};
use crate::id::KeyMap;

pub(crate) struct Painter<'a> {
    pub arena: &'a Arena,
    pub states: &'a KeyMap<NodeState>,
    pub list: &'a mut RenderList,
    pub hits: Option<&'a mut Vec<Hit>>,
    pub bars: Option<&'a mut Vec<BarHit>>,
    pub inputs: Option<&'a mut KeyMap<InputHit>>,
    pub input_text: &'a ShapedText,
    pub input_focus: Option<u64>,
    pub images: &'a mut ImageStore,
    /// When set, the visual rect of the node with this key is reported through `watched_rect`.
    pub watch: Option<u64>,
    pub watched_rect: Option<Rect>,
    deferred: Vec<Deferred>,
    seq: u32,
}

struct Deferred {
    z: i16,
    seq: u32,
    idx: usize,
    translate: Vec2,
    clip: Rect,
}

fn radii(c: CornerRadius, r: Rect) -> [f32; 4] {
    let m = (r.w.min(r.h) * 0.5).max(0.0);
    [c.top_left.min(m), c.top_right.min(m), c.bottom_right.min(m), c.bottom_left.min(m)]
}

impl<'a> Painter<'a> {
    pub fn new(
        arena: &'a Arena,
        states: &'a KeyMap<NodeState>,
        list: &'a mut RenderList,
        images: &'a mut ImageStore,
        input_text: &'a ShapedText,
    ) -> Self {
        Self {
            arena,
            states,
            list,
            hits: None,
            bars: None,
            inputs: None,
            input_text,
            input_focus: None,
            images,
            watch: None,
            watched_rect: None,
            deferred: Vec::new(),
            seq: 0,
        }
    }

    /// Paints the whole tree, then floating nodes in z order.
    pub fn paint_tree(&mut self, root_translate: Vec2, alpha: f32) {
        if self.arena.is_empty() {
            return;
        }
        self.node(0, root_translate, Rect::INFINITE, alpha);
        while !self.deferred.is_empty() {
            self.deferred.sort_by_key(|d| (d.z, d.seq));
            let d = self.deferred.remove(0);
            self.node(d.idx, d.translate, d.clip, alpha);
        }
    }

    fn quad(&mut self, rect: Rect, clip: Rect, radii: [f32; 4], fill: &Fill, border: &Fill, bw: f32, alpha: f32) {
        let fill = fill.mul_alpha(alpha);
        let border = border.mul_alpha(alpha);
        let (fc, fg) = self.list.encode_fill(&fill);
        let (bc, bg) = if bw > 0.0 { self.list.encode_fill(&border) } else { ([0.0; 4], -1.0) };
        self.list.ops.push(DrawOp::Quad(QuadInstance {
            rect: [rect.x, rect.y, rect.w, rect.h],
            clip: [clip.x, clip.y, clip.w, clip.h],
            radii,
            fill: fc,
            border: bc,
            uv: [0.0; 4],
            tint: [0.0; 4],
            extra: [bw, fg, bg, -1.0],
        }));
    }

    fn node(&mut self, i: usize, parent_translate: Vec2, clip: Rect, alpha: f32) {
        if clip.is_empty() {
            return;
        }
        let arena = self.arena;
        let n = &arena.nodes[i];
        let spring = if n.spring.is_some() { self.states.get(&n.key).and_then(|s| s.spring.as_ref()) } else { None };
        let vis = spring.map(|s| s.visual);
        let paints = spring.map(|s| &s.paints);
        let mut translate = parent_translate;
        if let Some(v) = &vis {
            translate += v.delta;
        }
        let mut rect = n.rect.translate(translate);
        if let Some(Visual { size: Some(s), .. }) = &vis {
            rect.w = s.x;
            rect.h = s.y;
        }
        if self.watch == Some(n.key) {
            self.watched_rect = Some(rect);
        }

        let clips = n.clip != Clip::None;
        if clips && rect.intersect(&clip).is_empty() {
            return;
        }
        if n.interactive {
            if let Some(h) = self.hits.as_mut() {
                h.push(Hit { idx: i as u32, rect, clip });
            }
        }

        let r = radii(vis.and_then(|v| v.corners).unwrap_or(n.corner), rect);

        // ---- background + border ----
        if let Some(a) = &n.appearance {
            let bg = paints.and_then(|p| p.background.as_ref()).map_or(a.background, |p| p.value());
            let bc = paints.and_then(|p| p.border.as_ref()).map_or(a.border_color, |p| p.value());
            let width = vis.and_then(|v| v.border_width).unwrap_or(a.border_width);
            let bw = if bc.is_visible() { width.max(0.0) } else { 0.0 };
            if bg.is_visible() || bw > 0.0 {
                self.quad(rect, clip, r, &bg, &bc, bw, alpha);
            }
        }

        // ---- image ----
        if let Some(img) = &n.image {
            let tint = paints.and_then(|p| p.tint.as_ref()).map_or(img.tint, |p| p.value());
            self.image(img, n.padding(), rect, clip, r, tint, alpha);
        }

        let mut child_clip = clip;
        if clips {
            let lim = Rect::new(
                if n.clip.x() { rect.x } else { -1.0e9 },
                if n.clip.y() { rect.y } else { -1.0e9 },
                if n.clip.x() { rect.w } else { 2.0e9 },
                if n.clip.y() { rect.h } else { 2.0e9 },
            );
            child_clip = clip.intersect(&lim);
        }

        // ---- text ----
        if let Some(t) = &n.text {
            let color = paints.and_then(|p| p.text.as_ref()).map_or(t.color, |p| p.value());
            let area = rect.shrink(n.padding());
            if let Some(input) = &t.input {
                self.input(n.key, t, input, area, clip.intersect(&area), color, alpha);
            } else if !t.content.is_empty() {
                self.list.ops.push(DrawOp::Text(TextRun {
                    rect: area,
                    clip: match t.wrap {
                        TextWrap::None => child_clip,
                        TextWrap::Clip { .. } | TextWrap::Fade { .. } => clip.intersect(&area),
                        _ => clip.intersect(&rect),
                    },
                    content: t.content.clone(),
                    color: color.mul_alpha(alpha),
                    size: t.size,
                    font: t.font,
                    align: t.align,
                    wrap: t.wrap,
                }));
            }
        }

        // ---- children ----
        if n.text.is_none() {
            for c in n.children() {
                let ch = &arena.nodes[c];
                if ch.is_floating {
                    let f = ch.floating.as_ref().expect("floating");
                    self.seq += 1;
                    let fclip = if f.clip_to_parent { clip.intersect(&rect) } else { Rect::INFINITE };
                    self.deferred.push(Deferred { z: f.z_index, seq: self.seq, idx: c, translate, clip: fclip });
                } else {
                    self.node(c, translate, child_clip, alpha);
                }
            }
        }

        // ---- scroll bars ----
        if let Some(s) = &n.scroll {
            if let Some(bar) = &s.scroll_bar {
                self.scroll_bars(i, s, bar, rect, clip.intersect(&rect), alpha);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn input(&mut self, key: u64, text: &Text, input: &TextInput, area: Rect, clip: Rect, color: Fill, alpha: f32) {
        if clip.is_empty() {
            return;
        }
        let fallback;
        let editor = if let Some(editor) = self.inputs.as_ref()
            .and_then(|_| self.states.get(&key))
            .and_then(|state| state.editor.as_deref())
        {
            editor
        } else {
            fallback = Editor::new(text, input);
            &fallback
        };
        let view = editor.view(self.input_text, text, input, area);
        let focused = self.input_focus == Some(key);
        let caret = view.caret_rect();
        let caret_alpha = editor.caret_alpha();
        if let Some(inputs) = &mut self.inputs {
            inputs.insert(key, InputHit { area, clip, scroll: view.scroll, caret });
        }
        if focused {
            for rect in view.selection_rects(false) {
                self.quad(rect, clip, [0.0; 4], &input.selection_color.into(), &Fill::NONE, 0.0, alpha);
            }
        }
        let (content, color) = if view.value.is_empty() {
            (normalize(&input.prompt, input.multiline), input.prompt_color.into())
        } else {
            (view.value.clone(), color)
        };
        if !content.is_empty() {
            self.list.ops.push(DrawOp::InputText(TextRun {
                rect: view.rect,
                clip,
                content,
                color: color.mul_alpha(alpha),
                size: text.size,
                font: text.font,
                align: text.align,
                wrap: input_wrap(text, input),
            }));
        }
        if focused {
            let color = input.caret_color.into();
            for rect in view.selection_rects(true) {
                self.quad(rect, clip, [0.0; 4], &color, &Fill::NONE, 0.0, alpha);
            }
            if let Some(rect) = caret
                && caret_alpha > 0.0
            {
                self.quad(rect, clip, [0.0; 4], &color, &Fill::NONE, 0.0, alpha * caret_alpha);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn image(&mut self, img: &Image, pad: crate::geometry::Padding, rect: Rect, clip: Rect, r: [f32; 4], tint: Fill, alpha: f32) {
        let Some((iw, ih)) = self.images.size(img.source) else { return };
        let (iw, ih) = (iw as f32, ih as f32);
        let area = rect.shrink(pad);
        if area.is_empty() || iw <= 0.0 || ih <= 0.0 {
            return;
        }
        let (dest, uv) = match img.fit {
            ImageFit::Fill => (area, [0.0, 0.0, 1.0, 1.0]),
            ImageFit::Contain => {
                let s = (area.w / iw).min(area.h / ih);
                let (w, h) = (iw * s, ih * s);
                (Rect::new(area.x + (area.w - w) * 0.5, area.y + (area.h - h) * 0.5, w, h), [0.0, 0.0, 1.0, 1.0])
            }
            ImageFit::Cover => {
                let s = (area.w / iw).max(area.h / ih);
                let (vw, vh) = (area.w / (iw * s), area.h / (ih * s));
                let (u0, v0) = ((1.0 - vw) * 0.5, (1.0 - vh) * 0.5);
                (area, [u0, v0, u0 + vw, v0 + vh])
            }
            ImageFit::None => {
                let (w, h) = (iw.min(area.w), ih.min(area.h));
                let (vw, vh) = (w / iw, h / ih);
                let (u0, v0) = ((1.0 - vw) * 0.5, (1.0 - vh) * 0.5);
                (Rect::new(area.x + (area.w - w) * 0.5, area.y + (area.h - h) * 0.5, w, h), [u0, v0, u0 + vw, v0 + vh])
            }
        };
        let (tc, tg) = self.list.encode_fill(&tint.mul_alpha(alpha));
        let same = dest == area;
        self.list.ops.push(DrawOp::Image(
            img.source,
            QuadInstance {
                rect: [dest.x, dest.y, dest.w, dest.h],
                clip: [clip.x, clip.y, clip.w, clip.h],
                radii: if same { r } else { [0.0; 4] },
                fill: [0.0; 4],
                border: [0.0; 4],
                uv,
                tint: tc,
                extra: [0.0, -1.0, -1.0, tg],
            },
        ));
    }

    fn scroll_bars(&mut self, idx: usize, s: &Scroll, bar: &ScrollBar, rect: Rect, clip: Rect, alpha: f32) {
        let n = &self.arena.nodes[idx];
        let Some(st) = self.states.get(&n.key) else { return };
        let a = st.bar_alpha * alpha;
        if a <= 0.004 {
            return;
        }
        let both = s.axes.x() && s.axes.y() && n.scroll_max.x > 0.0 && n.scroll_max.y > 0.0;
        for axis in [Axis::Y, Axis::X] {
            let enabled = if axis == Axis::Y { s.axes.y() } else { s.axes.x() };
            let max = n.scroll_max.get(axis);
            if !enabled || max <= 0.0 {
                continue;
            }
            let view_len = rect.size().get(axis);
            let content_len = n.content.get(axis).max(view_len);
            let track_len = (view_len - if both { bar.thickness } else { 0.0 }).max(0.0);
            let thumb_len = (track_len * view_len / content_len).clamp(bar.min_thumb_length.min(track_len), track_len);
            let off = n.scroll_offset.get(axis);
            let thumb_pos = if max > 0.0 { off / max * (track_len - thumb_len) } else { 0.0 };
            let (track, thumb) = match axis {
                Axis::Y => {
                    let x = match bar.side {
                        Side::End => rect.right() - bar.thickness,
                        Side::Start => rect.x,
                    };
                    (
                        Rect::new(x, rect.y, bar.thickness, track_len),
                        Rect::new(x, rect.y + thumb_pos, bar.thickness, thumb_len),
                    )
                }
                Axis::X => {
                    let y = match bar.side {
                        Side::End => rect.bottom() - bar.thickness,
                        Side::Start => rect.y,
                    };
                    (
                        Rect::new(rect.x, y, track_len, bar.thickness),
                        Rect::new(rect.x + thumb_pos, y, thumb_len, bar.thickness),
                    )
                }
            };
            let rad = [bar.corner_radius.min(bar.thickness * 0.5); 4];
            if bar.background_color.is_visible() {
                self.quad(track, clip, rad, &bar.background_color, &Fill::NONE, 0.0, a);
            }
            self.quad(thumb, clip, rad, &bar.bar_color, &Fill::NONE, 0.0, a);
            if let Some(b) = self.bars.as_mut() {
                if st.bar_alpha > 0.05 {
                    b.push(BarHit { idx: idx as u32, key: n.key, axis, thumb, track, max });
                }
            }
        }
    }
}
