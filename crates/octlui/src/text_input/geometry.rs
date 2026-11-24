use super::{Caret, input_wrap};
use crate::components::{Text, TextInput};
use crate::geometry::{Rect, Vec2};
use crate::style::Align;
use crate::text::{ShapedLayout, ShapedText};
use std::collections::BTreeMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy)]
struct Cell {
    start: usize,
    end: usize,
    left: f32,
    right: f32,
    rtl: bool,
}

impl Cell {
    fn stops(self) -> [(Caret, f32); 2] {
        let a = Caret::leading(self.start);
        let b = Caret::trailing(self.end);
        if self.rtl {
            [(b, self.left), (a, self.right)]
        } else {
            [(a, self.left), (b, self.right)]
        }
    }
}

pub(crate) struct InputLine {
    cells: Vec<Cell>,
    stops: Vec<(Caret, f32)>,
    newline: Option<usize>,
}

fn input_lines(layout: &ShapedLayout, value: &str) -> Vec<InputLine> {
    let boundaries: Vec<_> = value
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain(std::iter::once(value.len()))
        .collect();
    layout
        .lines
        .iter()
        .enumerate()
        .map(|(line_index, line)| {
            let mut cells = BTreeMap::<usize, Cell>::new();
            for glyph in &line.glyphs {
                let start = line.offset + glyph.start;
                let end = line.offset + glyph.end;
                let first = boundaries
                    .partition_point(|&i| i <= start)
                    .saturating_sub(1);
                let last = boundaries.partition_point(|&i| i < end);
                let rtl = glyph.level.is_rtl();
                let width = glyph.w / last.saturating_sub(first).max(1) as f32;
                for i in first..last.min(boundaries.len() - 1) {
                    let left = glyph.x + width * if rtl { last - i - 1 } else { i - first } as f32;
                    let right = left + width;
                    cells
                        .entry(i)
                        .and_modify(|cell| {
                            cell.left = cell.left.min(left);
                            cell.right = cell.right.max(right);
                        })
                        .or_insert(Cell {
                            start: boundaries[i],
                            end: boundaries[i + 1],
                            left,
                            right,
                            rtl,
                        });
                }
            }
            let previous = line_index.checked_sub(1).and_then(|i| layout.lines.get(i));
            let next = layout
                .lines
                .get(line_index + 1)
                .filter(|next| next.offset == line.offset);
            let line_start = if previous.is_some_and(|previous| previous.offset == line.offset) {
                cells
                    .first_key_value()
                    .map_or(line.hard_end, |(_, cell)| cell.start)
            } else {
                line.offset
            };
            let line_end = next
                .and_then(|next| next.glyphs.iter().map(|g| g.start).min())
                .map_or(line.hard_end, |byte| line.offset + byte);

            // Shaping can omit whitespace at a soft wrap. Keep a caret stop for every
            // grapheme even when its advance is zero and it draws no glyph.
            let first = boundaries.partition_point(|&byte| byte < line_start);
            let last = boundaries.partition_point(|&byte| byte < line_end);
            for i in first..last {
                if cells.contains_key(&i) {
                    continue;
                }
                let x = if let Some((_, cell)) = cells.range(..i).next_back() {
                    if cell.rtl { cell.left } else { cell.right }
                } else if let Some((_, cell)) = cells.range(i..).next() {
                    if cell.rtl { cell.right } else { cell.left }
                } else {
                    0.0
                };
                cells.insert(
                    i,
                    Cell {
                        start: boundaries[i],
                        end: boundaries[i + 1],
                        left: x,
                        right: x,
                        rtl: line.rtl,
                    },
                );
            }
            let newline = (next.is_none() && value.as_bytes().get(line.hard_end) == Some(&b'\n'))
                .then_some(line.hard_end);
            let mut cells: Vec<_> = cells.into_values().collect();
            cells.sort_by(|a, b| {
                a.left
                    .total_cmp(&b.left)
                    .then_with(|| a.rtl.cmp(&b.rtl))
                    .then_with(|| {
                        if a.rtl {
                            b.start.cmp(&a.start)
                        } else {
                            a.start.cmp(&b.start)
                        }
                    })
            });
            let mut stops: Vec<_> = cells.iter().flat_map(|c| c.stops()).collect();
            stops.sort_by(|a, b| a.1.total_cmp(&b.1));
            stops.dedup_by(|a, b| a.0.byte == b.0.byte && (a.1 - b.1).abs() < 0.01);
            if stops.is_empty() {
                stops.push((Caret::leading(line.offset), 0.0));
            }
            InputLine {
                cells,
                stops,
                newline,
            }
        })
        .collect()
}

pub(crate) struct InputView {
    pub value: Arc<str>,
    pub scroll: Vec2,
    pub rect: Rect,
    pub(super) area: Rect,
    layout: Rc<ShapedLayout>,
    align: Align,
    caret: Option<Caret>,
    selection: Range<usize>,
    composition: Option<Range<usize>>,
}

impl InputView {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        shaping: &ShapedText,
        text: &Text,
        input: &TextInput,
        area: Rect,
        value: Arc<str>,
        caret: Option<Caret>,
        selection: Range<usize>,
        composition: Option<Range<usize>>,
        old_scroll: Vec2,
    ) -> Self {
        let layout = shaping.layout(
            &value,
            text.font,
            text.size,
            input_wrap(text, input),
            Some(area.w),
        );
        layout
            .input_lines
            .get_or_init(|| input_lines(&layout, &value));
        let top = if input.multiline {
            0.0
        } else {
            (area.h - layout.size.y).max(0.0) / 2.0
        };
        let mut view = Self {
            value,
            layout,
            caret,
            selection,
            composition,
            area,
            align: text.align,
            scroll: Vec2::ZERO,
            rect: area.translate(Vec2::new(0.0, top)),
        };
        let caret_width = 1.5;
        let max_x = (view.layout.size.x + caret_width - area.w).max(0.0);
        let max_y = (view.layout.size.y - area.h).max(0.0);
        let mut scroll = Vec2::new(
            old_scroll.x.clamp(0.0, max_x),
            old_scroll.y.clamp(0.0, max_y),
        );
        if let Some(caret) = view.caret_rect() {
            scroll.x = scroll.x.clamp(
                (caret.right() - area.right()).max(0.0),
                (caret.x - area.x).max(0.0),
            );
            if caret.h <= area.h {
                scroll.y = scroll.y.clamp(
                    (caret.bottom() - area.bottom()).max(0.0),
                    (caret.y - area.y).max(0.0),
                );
            }
        }
        view.scroll = scroll;
        view.rect = view.rect.translate(Vec2::new(-scroll.x, -scroll.y));
        view
    }

    pub(super) fn lines(&self) -> &[InputLine] {
        self.layout.input_lines.get().map_or(&[], Vec::as_slice)
    }

    fn line_x(&self, line: usize) -> f32 {
        input_line_x(self.area.w, self.layout.lines[line].width, self.align)
    }

    fn location(&self, caret: Caret) -> Option<(usize, usize)> {
        let mut closest = None;
        let mut distance = usize::MAX;
        for (li, line) in self.lines().iter().enumerate() {
            for (si, &(stop, _)) in line.stops.iter().enumerate() {
                if stop == caret {
                    return Some((li, si));
                }
                let diff = stop.byte.abs_diff(caret.byte);
                if diff < distance {
                    closest = Some((li, si));
                    distance = diff;
                }
            }
        }
        closest
    }

    pub fn caret_rect(&self) -> Option<Rect> {
        let (line, stop) = self.location(self.caret?)?;
        let x = self.rect.x + self.line_x(line) + self.lines()[line].stops[stop].1;
        let shape = &self.layout.lines[line];
        Some(Rect::new(x, self.rect.y + shape.top, 1.5, shape.height))
    }

    fn closest_stop(&self, line: usize, x: f32) -> Caret {
        let x = x - self.line_x(line);
        let line = &self.lines()[line];
        if let Some(cell) = line
            .cells
            .iter()
            .find(|cell| cell.left <= x && x < cell.right)
        {
            let stops = cell.stops();
            return stops[usize::from(x - cell.left >= cell.right - x)].0;
        }
        line.stops
            .iter()
            .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
            .map_or(Caret::default(), |&(caret, _)| caret)
    }

    pub(super) fn hit(&self, point: Vec2) -> Caret {
        let y = point.y - self.rect.y;
        let line = self
            .layout
            .lines
            .partition_point(|l| l.top + l.height <= y)
            .min(self.lines().len().saturating_sub(1));
        if self.lines().is_empty() {
            Caret::default()
        } else {
            self.closest_stop(line, point.x - self.rect.x)
        }
    }

    pub(super) fn horizontal(&self, caret: Caret, left: bool) -> Caret {
        let Some((line, stop)) = self.location(caret) else {
            return caret;
        };
        let stops = &self.lines()[line].stops;
        let next_stop = if left {
            stop.checked_sub(1)
        } else {
            Some(stop + 1)
        };
        if let Some(next) = next_stop.and_then(|next| stops.get(next)) {
            return next.0;
        }
        let previous_line = left != self.layout.lines[line].rtl;
        let next_line = if previous_line {
            line.checked_sub(1)
        } else {
            Some(line + 1)
        };
        next_line
            .filter(|&next| next < self.lines().len())
            .map_or(caret, |next| self.edge(next, !previous_line))
    }

    pub(super) fn vertical(
        &self,
        caret: Caret,
        up: bool,
        page: bool,
        preferred: &mut Option<f32>,
    ) -> Caret {
        let Some((line, stop)) = self.location(caret) else {
            return caret;
        };
        let lines = self.lines();
        let shape = &self.layout.lines[line];
        let step = if page {
            (self.area.h / shape.height).floor().max(1.0) as usize
        } else {
            1
        };
        let next = if up {
            line.saturating_sub(step)
        } else {
            (line + step).min(lines.len() - 1)
        };
        if next == line {
            return self.line_edge(caret, up);
        }
        let x = *preferred.get_or_insert_with(|| self.line_x(line) + lines[line].stops[stop].1);
        self.closest_stop(next, x)
    }

    pub(super) fn line_edge(&self, caret: Caret, start: bool) -> Caret {
        let Some((line, _)) = self.location(caret) else {
            return caret;
        };
        self.edge(line, start)
    }

    fn edge(&self, line: usize, start: bool) -> Caret {
        let stops = &self.lines()[line].stops;
        if start != self.layout.lines[line].rtl {
            stops[0].0
        } else {
            stops.last().unwrap().0
        }
    }

    pub(super) fn rtl_at(&self, caret: Caret) -> bool {
        self.location(caret).is_some_and(|(line, _)| {
            self.lines()[line]
                .cells
                .iter()
                .find(|c| {
                    if caret.trailing {
                        c.end == caret.byte
                    } else {
                        c.start == caret.byte
                    }
                })
                .is_some_and(|c| c.rtl)
        })
    }

    pub(super) fn selection_edge(&self, anchor: Caret, caret: Caret, left: bool) -> Caret {
        if self.location(anchor) < self.location(caret) {
            if left { anchor } else { caret }
        } else if left {
            caret
        } else {
            anchor
        }
    }

    pub fn selection_rects(&self, underline: bool) -> Vec<Rect> {
        let range = if underline {
            let Some(range) = &self.composition else {
                return Vec::new();
            };
            range
        } else {
            &self.selection
        };
        let mut rects = Vec::<Rect>::new();
        if range.is_empty() {
            return rects;
        }
        for (i, line) in self.lines().iter().enumerate() {
            let origin = self.rect.x + self.line_x(i);
            let shape = &self.layout.lines[i];
            let y = self.rect.y
                + shape.top
                + if underline {
                    (shape.height - 2.0).max(0.0)
                } else {
                    0.0
                };
            let height = if underline {
                shape.height.min(1.0)
            } else {
                shape.height
            };
            for cell in &line.cells {
                if cell.end <= range.start || cell.start >= range.end {
                    continue;
                }
                let left = origin + cell.left;
                let right = origin + cell.right;
                if let Some(last) = rects.last_mut()
                    && last.y == y
                    && left <= last.right() + 0.01
                {
                    last.w = last.w.max(right - last.x);
                } else {
                    rects.push(Rect::new(left, y, right - left, height));
                }
            }
            if line.newline.is_some_and(|byte| range.contains(&byte)) {
                rects.push(Rect::new(
                    origin + shape.width,
                    y,
                    shape.height * 0.3,
                    height,
                ));
            }
        }
        rects
    }
}

pub(crate) fn input_line_x(width: f32, line_width: f32, align: Align) -> f32 {
    align.factor() * (width - line_width).max(0.0)
}
