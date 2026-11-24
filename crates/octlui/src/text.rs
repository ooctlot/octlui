//! Text measurement and line layout.

mod native;
pub use native::NativeTextMeasurer;

use crate::components::Text;
use crate::geometry::Vec2;
use crate::style::TextWrap;

/// Measures text for layout.
///
/// Layout calls it twice per text node: once without a width limit (natural size, drives
/// `Fit` and the horizontal grow/shrink passes) and, once the final width is known, with
/// `Some(width)` to get the wrapped height.
pub trait TextMeasurer {
    /// Size of `text` when wrapped at `max_width` (`None` = unconstrained).
    fn measure(&mut self, text: &Text, max_width: Option<f32>) -> Vec2;
    /// Narrowest width the text can be squeezed to without clipping a word/letter.
    /// Clipping modes (`Clip`, `Fade`) report `0`.
    fn min_content_width(&mut self, text: &Text) -> f32;
}

/// Stand-in measurer: pretends every glyph is `0.55 * size` wide and a line is `1.2 * size`
/// tall. Good enough to exercise wrapping logic in layout until real shaping exists.
#[derive(Default, Clone, Copy, Debug)]
pub struct PlaceholderMeasurer;

impl PlaceholderMeasurer {
    fn glyph_w(t: &Text) -> f32 {
        t.size * 0.55
    }
    fn line_h(t: &Text) -> f32 {
        t.size * 1.2
    }
}

impl TextMeasurer for PlaceholderMeasurer {
    fn measure(&mut self, t: &Text, max_width: Option<f32>) -> Vec2 {
        let gw = Self::glyph_w(t);
        let lh = Self::line_h(t);
        let content: &str = &t.content;
        let cols = max_width.map(|w| ((w / gw).floor() as usize).max(1));
        let mut lines = 0usize;
        let mut widest = 0usize;
        match t.wrap {
            TextWrap::None | TextWrap::Clip { .. } | TextWrap::Fade { .. } => {
                lines = 1;
                widest = content.chars().filter(|c| *c != '\n').count();
            }
            TextWrap::NewLines => {
                for l in content.split('\n') {
                    lines += 1;
                    widest = widest.max(l.chars().count());
                }
            }
            TextWrap::Letters => {
                for l in content.split('\n') {
                    let n = l.chars().count();
                    let c = cols.unwrap_or(usize::MAX);
                    lines += n.div_ceil(c).max(1);
                    widest = widest.max(n.min(c));
                }
            }
            TextWrap::Words => {
                for para in content.split('\n') {
                    let mut cur = 0usize;
                    let mut first = true;
                    lines += 1;
                    for word in para.split_whitespace() {
                        let wl = word.chars().count();
                        let need = if first { wl } else { cur + 1 + wl };
                        if !first && cols.is_some_and(|c| need > c) {
                            lines += 1;
                            cur = wl;
                        } else {
                            cur = need;
                        }
                        first = false;
                        widest = widest.max(cur);
                    }
                }
            }
        }
        Vec2::new(widest as f32 * gw, lines as f32 * lh)
    }

    fn min_content_width(&mut self, t: &Text) -> f32 {
        let gw = Self::glyph_w(t);
        match t.wrap {
            TextWrap::Clip { .. } | TextWrap::Fade { .. } => 0.0,
            TextWrap::Letters => gw,
            TextWrap::Words => {
                t.content.split_whitespace().map(|w| w.chars().count()).max().unwrap_or(0) as f32 * gw
            }
            TextWrap::None | TextWrap::NewLines => self.measure(t, None).x,
        }
    }
}
