use super::{GpuContext, TextRun};

/// Hook for GPU text rendering. The default [`NoopTextRenderer`] draws nothing.
///
/// Per frame the renderer first calls [`prepare`](TextRenderer::prepare) once with all runs in
/// draw order (upload glyphs, build buffers), then [`draw`](TextRenderer::draw) for each
/// contiguous batch of runs *in between* quad batches, so text keeps its z-order.
pub trait TextRenderer {
    fn prepare(&mut self, gpu: &GpuContext, runs: &[TextRun], scale_factor: f32);
    /// Draws `runs[range]` (indices into the slice given to `prepare`).
    fn draw(&mut self, pass: &mut wgpu::RenderPass<'_>, range: std::ops::Range<usize>);
}

#[derive(Default)]
pub struct NoopTextRenderer;
impl TextRenderer for NoopTextRenderer {
    fn prepare(&mut self, _gpu: &GpuContext, _runs: &[TextRun], _scale_factor: f32) {}
    fn draw(&mut self, _pass: &mut wgpu::RenderPass<'_>, _range: std::ops::Range<usize>) {}
}
