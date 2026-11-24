use super::TextMeasurer;
use crate::components::{FontId, Text};
use crate::geometry::Vec2;
use crate::style::TextWrap;
use font_kit::{family_name::FamilyName, font::Font, properties::Properties, source::SystemSource};
use std::collections::HashMap;

#[derive(Default)]
pub struct NativeTextMeasurer {
    faces: HashMap<FontId, Option<Face>>,
}

pub(crate) struct Face {
    pub font: Font,
    advances: HashMap<char, (u32, f32)>,
}

#[derive(Default)]
pub(crate) struct Line {
    pub glyphs: Vec<(u32, f32)>,
    pub width: f32,
}

pub(crate) struct Layout {
    pub lines: Vec<Line>,
    pub ascent: f32,
    pub height: f32,
    pub min_width: f32,
}

impl NativeTextMeasurer {
    pub(crate) fn face(&mut self, id: FontId) -> Option<&mut Face> {
        let (id, family) = match id {
            FontId::SERIF => (id, FamilyName::Serif),
            FontId::MONOSPACE => (id, FamilyName::Monospace),
            _ => (FontId::DEFAULT, FamilyName::SansSerif),
        };
        self.faces
            .entry(id)
            .or_insert_with(|| {
                let font = SystemSource::new()
                    .select_best_match(&[family], &Properties::new())
                    .ok()?
                    .load()
                    .ok()?;
                Some(Face {
                    font,
                    advances: HashMap::new(),
                })
            })
            .as_mut()
    }
}

impl Face {
    fn glyph(&mut self, ch: char, size: f32) -> (u32, f32) {
        let (id, advance) = *self.advances.entry(ch).or_insert_with(|| {
            let id = self.font.glyph_for_char(ch).unwrap_or(0);
            let advance = self.font.advance(id).map_or(0., |v| v.x())
                / self.font.metrics().units_per_em as f32;
            (id, advance)
        });
        (id, advance * size)
    }

    pub fn layout(&mut self, content: &str, size: f32, wrap: TextWrap, limit: f32) -> Layout {
        let metrics = self.font.metrics();
        let unit = size / metrics.units_per_em as f32;
        let mut lines = Vec::new();
        let mut line = Line::default();
        let mut min_width = 0f32;
        let mut append = |line: &mut Line, ch| {
            let (id, advance) = self.glyph(ch, size);
            line.glyphs.push((id, line.width));
            line.width += advance;
            advance
        };
        if wrap == TextWrap::Words {
            for paragraph in content.split('\n') {
                for word in paragraph.split_whitespace() {
                    let mut part = Line::default();
                    for ch in word.chars() {
                        append(&mut part, ch);
                    }
                    min_width = min_width.max(part.width);
                    if !line.glyphs.is_empty() {
                        let old = line.width;
                        append(&mut line, ' ');
                        if line.width + part.width > limit {
                            line.glyphs.pop();
                            line.width = old;
                            lines.push(std::mem::take(&mut line));
                        }
                    }
                    line.glyphs
                        .extend(part.glyphs.into_iter().map(|(id, x)| (id, line.width + x)));
                    line.width += part.width;
                }
                lines.push(std::mem::take(&mut line));
            }
        } else {
            for ch in content.chars() {
                if ch == '\n' {
                    if !wrap.single_line() {
                        lines.push(std::mem::take(&mut line));
                    }
                    continue;
                }
                let advance = append(&mut line, ch);
                if wrap == TextWrap::Letters {
                    min_width = min_width.max(advance);
                    if line.width > limit && line.glyphs.len() > 1 {
                        let (id, old_width) = line.glyphs.pop().unwrap();
                        line.width = old_width;
                        lines.push(std::mem::take(&mut line));
                        line.glyphs.push((id, 0.));
                        line.width = advance;
                    }
                }
            }
            if matches!(wrap, TextWrap::Clip { ellipsis: true }) && line.width > limit {
                let (id, advance) = self.glyph('…', size);
                while line.width + advance > limit {
                    let Some((_, x)) = line.glyphs.pop() else {
                        break;
                    };
                    line.width = x;
                }
                if advance <= limit {
                    line.glyphs.push((id, line.width));
                    line.width += advance;
                }
            }
            lines.push(line);
            if matches!(wrap, TextWrap::None | TextWrap::NewLines) {
                min_width = lines.iter().map(|l| l.width).fold(0., f32::max);
            }
        }
        Layout {
            lines,
            min_width,
            ascent: metrics.ascent * unit,
            height: (metrics.ascent - metrics.descent + metrics.line_gap) * unit,
        }
    }
}

impl TextMeasurer for NativeTextMeasurer {
    fn measure(&mut self, t: &Text, max_width: Option<f32>) -> Vec2 {
        let Some(face) = self.face(t.font) else {
            return Vec2::default();
        };
        let layout = face.layout(
            &t.content,
            t.size.max(0.),
            t.wrap,
            max_width.unwrap_or(f32::INFINITY),
        );
        Vec2::new(
            layout.lines.iter().map(|l| l.width).fold(0., f32::max),
            layout.lines.len() as f32 * layout.height,
        )
    }

    fn min_content_width(&mut self, t: &Text) -> f32 {
        self.face(t.font).map_or(0., |face| {
            face.layout(&t.content, t.size.max(0.), t.wrap, f32::INFINITY)
                .min_width
        })
    }
}
