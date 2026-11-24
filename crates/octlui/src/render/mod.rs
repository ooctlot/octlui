//! Render commands and the wgpu renderer.

mod atlas;
mod gpu;
mod text;

pub use gpu::{GpuContext, GpuError, Renderer};
pub use text::{NoopTextRenderer, TextRenderer};

use crate::components::FontId;
use crate::geometry::{Color, Fill, Rect};
use crate::image::ImageId;
use crate::style::{Align, TextWrap};
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;

/// One rounded rectangle / image quad, as consumed by the shader. 128 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct QuadInstance {
    /// x, y, w, h (logical pixels)
    pub rect: [f32; 4],
    /// Clip rect x, y, w, h; fragments outside are discarded.
    pub clip: [f32; 4],
    /// Corner radii: top-left, top-right, bottom-right, bottom-left.
    pub radii: [f32; 4],
    pub fill: [f32; 4],
    pub border: [f32; 4],
    /// Image uv min.xy, max.zw. `min.x == max.x` means "no image".
    pub uv: [f32; 4],
    pub tint: [f32; 4],
    /// border width, fill gradient, border gradient, tint gradient (-1 = none).
    pub extra: [f32; 4],
}

/// GPU gradient description (176 bytes), stored in a storage buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuGradient {
    pub colors: [[f32; 4]; 8],
    pub offsets: [[f32; 4]; 2],
    /// kind (0 linear, 1 radial), stop count, direction x, direction y.
    pub params: [f32; 4],
}

/// A run of text to draw. Drawn by the native renderer or a custom [`TextRenderer`] hook.
#[derive(Clone, Debug)]
pub struct TextRun {
    pub rect: Rect,
    pub clip: Rect,
    pub content: Arc<str>,
    pub color: Fill,
    pub size: f32,
    pub font: FontId,
    pub align: Align,
    pub wrap: TextWrap,
}

#[derive(Clone, Debug)]
pub enum DrawOp {
    Quad(QuadInstance),
    /// A quad sampling the given image.
    Image(ImageId, QuadInstance),
    Text(TextRun),
    /// Editable text drawn with the shaping backend used for caret placement.
    InputText(TextRun),
}

/// Everything needed to draw one frame, in back-to-front order.
#[derive(Default)]
pub struct RenderList {
    pub ops: Vec<DrawOp>,
    pub gradients: Vec<GpuGradient>,
}

impl RenderList {
    pub fn clear(&mut self) {
        self.ops.clear();
        self.gradients.clear();
    }

    /// Uploads `fill` as a gradient if needed. Returns `(solid color, gradient index or -1)`.
    pub(crate) fn encode_fill(&mut self, fill: &Fill) -> ([f32; 4], f32) {
        match fill {
            Fill::Solid(c) => (c.to_array(), -1.0),
            Fill::Linear { angle, stops } => {
                let idx = self.push_gradient(0.0, stops.as_slice(), (angle.cos(), angle.sin()));
                (Color::TRANSPARENT.to_array(), idx)
            }
            Fill::Radial { stops } => {
                let idx = self.push_gradient(1.0, stops.as_slice(), (0.0, 0.0));
                (Color::TRANSPARENT.to_array(), idx)
            }
        }
    }

    fn push_gradient(
        &mut self,
        kind: f32,
        stops: &[crate::geometry::ColorStop],
        dir: (f32, f32),
    ) -> f32 {
        let mut g = GpuGradient::zeroed();
        for (i, s) in stops.iter().enumerate() {
            g.colors[i] = s.color.to_array();
            g.offsets[i / 4][i % 4] = s.offset;
        }
        g.params = [kind, stops.len() as f32, dir.0, dir.1];
        self.gradients.push(g);
        (self.gradients.len() - 1) as f32
    }
}
