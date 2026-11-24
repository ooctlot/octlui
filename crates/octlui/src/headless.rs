//! Offscreen rendering without a window: useful for screenshot tests, CI and tooling.

use crate::geometry::{Color, Vec2};
use crate::image::ImageStore;
use crate::layout::LayoutEnv;
use crate::node::Node;
use crate::render::{GpuContext, GpuError, Renderer};
use crate::text::{PlaceholderMeasurer, TextMeasurer};
use crate::ui::Ui;

/// Rendered pixels, RGBA8, row-major, straight (non-premultiplied) alpha.
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
impl Screenshot {
    /// Binary PPM (P6); alpha is dropped.
    pub fn to_ppm(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        for px in self.rgba.chunks_exact(4) {
            out.extend_from_slice(&px[..3]);
        }
        out
    }
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
}

pub struct HeadlessRenderer {
    gpu: GpuContext,
    renderer: Renderer,
    ui: Ui,
    images: ImageStore,
    measurer: Box<dyn TextMeasurer>,
    target: wgpu::Texture,
    size_px: (u32, u32),
    scale: f32,
    clear: Color,
}

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl HeadlessRenderer {
    /// `width`/`height` are physical pixels; layout happens in logical pixels (`px / scale`).
    pub fn new(width: u32, height: u32, scale: f32) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let gpu = GpuContext::new(instance, None)?;
        let renderer = Renderer::new(&gpu, FORMAT);
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("octlui headless target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut ui = Ui::new();
        ui.set_size(Vec2::new(width as f32 / scale, height as f32 / scale), scale);
        Ok(Self {
            gpu,
            renderer,
            ui,
            images: ImageStore::default(),
            measurer: Box::new(PlaceholderMeasurer),
            target,
            size_px: (width, height),
            scale,
            clear: Color::hex(0x1e1e2e),
        })
    }

    pub fn images(&mut self) -> &mut ImageStore {
        &mut self.images
    }
    pub fn set_clear_color(&mut self, c: impl Into<Color>) {
        self.clear = c.into();
    }
    pub fn adapter_name(&self) -> String {
        self.gpu.adapter.get_info().name
    }

    pub fn set_root(&mut self, node: Node) {
        self.ui.set_root(node);
    }
    /// Advances animations by `dt` seconds and re-lays out. Returns `true` while animating.
    pub fn frame(&mut self, dt: f32) -> bool {
        let mut env = LayoutEnv { text: &mut *self.measurer, images: &mut self.images };
        self.ui.frame(dt, &mut env)
    }

    /// Renders the current frame and reads it back.
    pub fn read_pixels(&mut self) -> Screenshot {
        let (w, h) = self.size_px;
        let view = self.target.create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer.render(&self.gpu, &view, (w, h), self.scale, &self.ui.list, self.clear, &mut self.images);

        let bpr = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("octlui readback"),
            size: bpr as u64 * h as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.gpu.queue.submit(Some(enc.finish()));
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback buffer"));
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll device");
        let data = slice.get_mapped_range().expect("mapped range");
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for row in data.chunks_exact(bpr as usize).take(h as usize) {
            rgba.extend_from_slice(&row[..(w * 4) as usize]);
        }
        drop(data);
        buf.unmap();
        // Convert premultiplied-over-clear output to straight alpha (alpha is 1 unless the clear
        // color is transparent).
        for px in rgba.chunks_exact_mut(4) {
            let a = px[3] as f32 / 255.0;
            if a > 0.0 && a < 1.0 {
                for c in &mut px[..3] {
                    *c = ((*c as f32 / a).min(255.0)) as u8;
                }
            }
        }
        Screenshot { width: w, height: h, rgba }
    }

    /// `set_root` + one frame + `read_pixels`.
    pub fn render(&mut self, node: Node) -> Screenshot {
        self.set_root(node);
        self.frame(0.0);
        self.read_pixels()
    }
}
