use super::{Fonts, NativeTextMeasurer, TextMeasurer};
use crate::{
    components::{FontId, Text},
    geometry::Vec2,
    style::TextWrap,
};
use cosmic_text::{
    Attrs, Buffer, CacheKey, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, LayoutGlyph,
    Metrics, Shaping, SwashCache, SwashImage, Wrap, fontdb,
};
use std::{
    cell::{OnceCell, RefCell},
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::Arc,
};

#[derive(Clone)]
pub(crate) struct ShapedText(Rc<EngineResources>);

struct EngineResources {
    fonts: Fonts,
    default_font: FontId,
    engine: OnceCell<RefCell<Engine>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    content: Arc<str>,
    font: FontId,
    size: u32,
    breaks: Breaks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Breaks {
    Single,
    Hard,
    Glyph,
    Word,
    Ellipsis,
}

impl From<TextWrap> for Breaks {
    fn from(wrap: TextWrap) -> Self {
        match wrap {
            TextWrap::None | TextWrap::Clip { ellipsis: false } | TextWrap::Fade { .. } => {
                Self::Single
            }
            TextWrap::NewLines => Self::Hard,
            TextWrap::Letters => Self::Glyph,
            TextWrap::Words => Self::Word,
            TextWrap::Clip { ellipsis: true } => Self::Ellipsis,
        }
    }
}

struct CachedText {
    buffer: Buffer,
    widths: VecDeque<(Option<u32>, Rc<Layout>)>,
    last_used: u64,
}

struct Engine {
    fonts: FontSystem,
    native: NativeTextMeasurer,
    families: HashMap<FontId, Option<FontStyle>>,
    rasterizer: SwashCache,
    layouts: HashMap<Key, CachedText>,
    clock: u64,
}

struct FontStyle {
    family: String,
    weight: fontdb::Weight,
    style: fontdb::Style,
    stretch: fontdb::Stretch,
}

pub(crate) struct Line {
    pub glyphs: Vec<LayoutGlyph>,
    pub width: f32,
    pub baseline: f32,
    pub top: f32,
    pub height: f32,
    pub offset: usize,
    pub hard_end: usize,
    pub rtl: bool,
}

#[derive(Default)]
pub(crate) struct Layout {
    pub lines: Vec<Line>,
    pub size: Vec2,
    pub input_lines: OnceCell<Vec<crate::text_input::InputLine>>,
}

impl ShapedText {
    pub(crate) fn new(fonts: Fonts, default_font: FontId) -> Self {
        Self(Rc::new(EngineResources {
            fonts,
            default_font,
            engine: OnceCell::new(),
        }))
    }

    fn engine(&self) -> &RefCell<Engine> {
        self.0.engine.get_or_init(|| {
            RefCell::new(Engine {
                fonts: FontSystem::new(),
                native: NativeTextMeasurer::with_fonts(self.0.fonts.clone(), self.0.default_font),
                families: HashMap::new(),
                rasterizer: SwashCache::new(),
                layouts: HashMap::new(),
                clock: 0,
            })
        })
    }

    pub fn layout(
        &self,
        content: &Arc<str>,
        font: FontId,
        size: f32,
        wrap: TextWrap,
        width: Option<f32>,
    ) -> Rc<Layout> {
        if !size.is_finite() || size <= 0. {
            return Rc::new(Layout::default());
        }
        let key = Key {
            content: content.clone(),
            font,
            size: size.to_bits(),
            breaks: wrap.into(),
        };
        let width = match key.breaks {
            Breaks::Word | Breaks::Glyph | Breaks::Ellipsis => {
                width.filter(|v| v.is_finite()).map(|v| v.max(0.))
            }
            Breaks::Single | Breaks::Hard => None,
        };
        self.engine().borrow_mut().layout(key, width)
    }

    pub fn raster(&self, key: CacheKey) -> Option<SwashImage> {
        let mut engine = self.engine().borrow_mut();
        let Engine {
            rasterizer, fonts, ..
        } = &mut *engine;
        rasterizer.get_image_uncached(fonts, key)
    }
}

impl Default for ShapedText {
    fn default() -> Self {
        Self::new(Fonts::default(), FontId::DEFAULT)
    }
}

impl Engine {
    fn layout(&mut self, key: Key, width: Option<f32>) -> Rc<Layout> {
        const TEXT_LIMIT: usize = 512;
        const WIDTH_LIMIT: usize = 6;
        self.clock += 1;
        if !self.layouts.contains_key(&key) {
            if self.layouts.len() >= TEXT_LIMIT {
                let oldest = self
                    .layouts
                    .iter()
                    .min_by_key(|(_, v)| v.last_used)
                    .map(|(key, _)| key.clone())
                    .unwrap();
                self.layouts.remove(&oldest);
            }
            let cached = self.shape(&key);
            self.layouts.insert(key.clone(), cached);
        }
        let cached = self.layouts.get_mut(&key).unwrap();
        cached.last_used = self.clock;
        let width_key = width.map(f32::to_bits);
        if let Some(index) = cached.widths.iter().position(|(key, _)| *key == width_key) {
            let entry = cached.widths.remove(index).unwrap();
            let layout = entry.1.clone();
            cached.widths.push_front(entry);
            return layout;
        }

        cached.buffer.set_size(width, None);
        cached.buffer.shape_until_scroll(&mut self.fonts, false);
        let mut size = Vec2::ZERO;
        let mut offsets = cached.buffer.lines.iter().scan(0, |offset, line| {
            let start = *offset;
            *offset += line.text().len() + line.ending().as_str().len();
            Some(start)
        });
        let mut line_index = 0;
        let mut offset = offsets.next().unwrap_or(0);
        let lines = cached
            .buffer
            .layout_runs()
            .map(|line| {
                while line_index < line.line_i {
                    line_index += 1;
                    offset = offsets.next().unwrap_or(offset);
                }
                size.x = size.x.max(line.line_w);
                size.y = size.y.max(line.line_top + line.line_height);
                Line {
                    glyphs: line.glyphs.to_vec(),
                    width: line.line_w,
                    baseline: line.line_y,
                    top: line.line_top,
                    height: line.line_height,
                    offset,
                    hard_end: offset + line.text.len(),
                    rtl: line.rtl,
                }
            })
            .collect();
        let layout = Rc::new(Layout {
            lines,
            size,
            input_lines: OnceCell::new(),
        });
        if cached.widths.len() == WIDTH_LIMIT {
            cached.widths.pop_back();
        }
        cached.widths.push_front((width_key, layout.clone()));
        layout
    }

    fn shape(&mut self, key: &Key) -> CachedText {
        let style = self.families.entry(key.font).or_insert_with(|| {
            let face = self.native.face(key.font)?;
            let name = face.font.postscript_name();
            let find_face = |db: &fontdb::Database| {
                db.faces()
                    .find(|f| Some(&f.post_script_name) == name.as_ref())
                    .cloned()
            };
            let mut info = find_face(self.fonts.db());
            if face.id.0 >= 3
                && let Some(bytes) = face.font.copy_font_data()
            {
                let mut source = fontdb::Database::new();
                source.load_font_source(fontdb::Source::Binary(bytes));
                if let Some(mut registered) = find_face(&source)
                    && let Some((_, language)) = registered.families.first()
                {
                    // A registered face can share its family and style with an installed font.
                    registered.families = vec![(format!("octlui font {}", face.id.0), *language)];
                    self.fonts.db_mut().push_face_info(registered.clone());
                    info = Some(registered);
                }
            }
            let info = info?;
            Some(FontStyle {
                family: info.families.first()?.0.clone(),
                weight: info.weight,
                style: info.style,
                stretch: info.stretch,
            })
        });
        let attrs = if let Some(style) = style {
            Attrs::new()
                .family(Family::Name(&style.family))
                .weight(style.weight)
                .style(style.style)
                .stretch(style.stretch)
        } else {
            Attrs::new().family(match key.font {
                FontId::MONOSPACE => Family::Monospace,
                FontId::SERIF => Family::Serif,
                _ => Family::SansSerif,
            })
        };
        let size = f32::from_bits(key.size);
        let mut buffer = Buffer::new_empty(Metrics::new(size, size * 1.25));
        let single;
        let content: &str = if matches!(key.breaks, Breaks::Single | Breaks::Ellipsis)
            && key.content.contains(['\n', '\r'])
        {
            single = key.content.replace("\r\n", " ").replace(['\r', '\n'], " ");
            &single
        } else {
            &key.content
        };
        buffer.set_text(
            content,
            &attrs,
            Shaping::Advanced,
            Some(cosmic_text::Align::Left),
        );
        buffer.set_wrap(match key.breaks {
            Breaks::Word => Wrap::Word,
            Breaks::Glyph => Wrap::Glyph,
            _ => Wrap::None,
        });
        if key.breaks == Breaks::Ellipsis {
            buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
        }
        CachedText {
            buffer,
            widths: VecDeque::new(),
            last_used: self.clock,
        }
    }
}

impl TextMeasurer for ShapedText {
    fn measure(&mut self, text: &Text, max_width: Option<f32>) -> Vec2 {
        self.layout(&text.content, text.font, text.size, text.wrap, max_width)
            .size
    }

    fn min_content_width(&mut self, text: &Text) -> f32 {
        let width = match text.wrap {
            TextWrap::Clip { .. } | TextWrap::Fade { .. } => return 0.,
            TextWrap::Words | TextWrap::Letters => Some(0.),
            TextWrap::None | TextWrap::NewLines => None,
        };
        self.measure(text, width).x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shaping_reuses_widths_and_applies_bidirectional_order() {
        let engine = ShapedText::new(Fonts::default(), FontId::DEFAULT);
        let text = Text::new("abc אבג DEF");
        let layout = engine.layout(&text.content, text.font, text.size, TextWrap::Words, None);
        if layout.lines.is_empty() || layout.lines[0].glyphs.is_empty() {
            return;
        }
        let glyphs = &layout.lines[0].glyphs;
        assert!(glyphs.windows(2).any(|pair| pair[1].start < pair[0].start));
        let narrow = engine.layout(
            &text.content,
            text.font,
            text.size,
            TextWrap::Words,
            Some(1.),
        );
        assert!(narrow.lines.len() > 1);
        let cached = engine.layout(&text.content, text.font, text.size, TextWrap::Words, None);
        assert!(Rc::ptr_eq(&layout, &cached));
    }
}
