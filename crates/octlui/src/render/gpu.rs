//! wgpu device setup and the quad renderer.

use super::{DrawOp, QuadInstance, RenderList, TextRenderer, TextRun};
use crate::geometry::Color;
use crate::id::KeyMap;
use crate::image::{ImageId, ImageStore};
use std::fmt;
use std::mem::size_of;

#[derive(Debug)]
pub enum GpuError {
    Adapter(String),
    Device(String),
}
impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::Adapter(e) => write!(f, "no suitable GPU adapter: {e}"),
            GpuError::Device(e) => write!(f, "failed to create GPU device: {e}"),
        }
    }
}
impl std::error::Error for GpuError {}

/// Shared wgpu objects.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    /// Creates a context. Pass the first window's surface so the adapter is guaranteed to be
    /// able to present to it.
    pub fn new(
        instance: wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, GpuError> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|e| GpuError::Adapter(e.to_string()))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("octlui device"),
            ..Default::default()
        }))
        .map_err(|e| GpuError::Device(e.to_string()))?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    viewport: [f32; 2],
    scale: f32,
    srgb_target: f32,
}

struct GpuImage {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    version: u64,
}

enum Batch {
    Glyphs {
        page: usize,
        range: std::ops::Range<u32>,
    },
    Quads {
        image: Option<ImageId>,
        range: std::ops::Range<u32>,
    },
    Text {
        range: std::ops::Range<usize>,
    },
}

/// Draws [`RenderList`]s into any texture view.
pub struct Renderer {
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    globals_layout: wgpu::BindGroupLayout,
    globals_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    white: GpuImage,
    instance_buf: wgpu::Buffer,
    instance_cap: usize,
    gradient_buf: wgpu::Buffer,
    gradient_cap: usize,
    images: KeyMap<GpuImage>,
    text: Option<Box<dyn TextRenderer>>,
    native: super::atlas::NativeText,
    glyphs: Vec<(usize, QuadInstance)>,
    colors: RenderList,
    // scratch, reused across frames
    instances: Vec<QuadInstance>,
    batches: Vec<Batch>,
    runs: Vec<TextRun>,
}

const GRADIENT_SIZE: u64 = size_of::<super::GpuGradient>() as u64;
const INSTANCE_SIZE: u64 = size_of::<QuadInstance>() as u64;

impl Renderer {
    pub fn new(gpu: &GpuContext, format: wgpu::TextureFormat) -> Self {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("octlui quad shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("octlui globals layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("octlui texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("octlui pipeline layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
            immediate_size: 0,
        });

        let attrs: Vec<wgpu::VertexAttribute> = (0..8)
            .map(|i| wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: i * 16,
                shader_location: i as u32,
            })
            .collect();
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("octlui quad pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: INSTANCE_SIZE,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attrs,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("octlui globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("octlui sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let instance_cap = 1024;
        let gradient_cap = 64;
        let instance_buf = Self::make_instance_buf(device, instance_cap);
        let gradient_buf = Self::make_gradient_buf(device, gradient_cap);
        let globals_group =
            Self::make_globals_group(device, &globals_layout, &globals_buf, &gradient_buf);

        let white = Self::upload_texture(
            gpu,
            &texture_layout,
            &sampler,
            1,
            1,
            &[255, 255, 255, 255],
            0,
        );

        Self {
            format,
            pipeline,
            globals_buf,
            globals_layout,
            globals_group,
            texture_layout,
            sampler,
            white,
            instance_buf,
            instance_cap,
            gradient_buf,
            gradient_cap,
            images: KeyMap::default(),
            text: None,
            native: Default::default(),
            glyphs: Vec::new(),
            colors: RenderList::default(),
            instances: Vec::new(),
            batches: Vec::new(),
            runs: Vec::new(),
        }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub(crate) fn set_native_fonts(&mut self, fonts: crate::text::NativeTextMeasurer) {
        self.native.fonts = fonts;
    }

    pub(crate) fn set_shaped_text(&mut self, text: crate::text::ShapedText) {
        self.native.set_shaped(text);
    }

    pub(crate) fn set_input_text(&mut self, text: crate::text::ShapedText) {
        self.native.set_input_text(text);
    }

    /// Installs a text renderer (see [`TextRenderer`]).
    pub fn set_text_renderer(&mut self, t: Box<dyn TextRenderer>) {
        self.text = Some(t);
    }

    fn make_instance_buf(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("octlui instances"),
            size: cap as u64 * INSTANCE_SIZE,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    fn make_gradient_buf(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("octlui gradients"),
            size: cap as u64 * GRADIENT_SIZE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    fn make_globals_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        globals: &wgpu::Buffer,
        gradients: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("octlui globals group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gradients.as_entire_binding(),
                },
            ],
        })
    }

    fn upload_texture(
        gpu: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        w: u32,
        h: u32,
        rgba: &[u8],
        version: u64,
    ) -> GpuImage {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("octlui image"),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Colors are treated as sRGB-encoded values everywhere; no hardware decode.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("octlui image group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        GpuImage {
            _texture: texture,
            bind_group,
            version,
        }
    }

    fn ensure_image(&mut self, gpu: &GpuContext, store: &mut ImageStore, id: ImageId) -> bool {
        let Some((data, version)) = store.get(id) else {
            return false;
        };
        if self.images.get(&id.0).is_some_and(|g| g.version == version) {
            return true;
        }
        let max = gpu.device.limits().max_texture_dimension_2d;
        if data.width == 0 || data.height == 0 || data.width > max || data.height > max {
            return false;
        }
        let img = Self::upload_texture(
            gpu,
            &self.texture_layout,
            &self.sampler,
            data.width,
            data.height,
            &data.rgba,
            version,
        );
        self.images.insert(id.0, img);
        true
    }

    /// Draws `list` into `target`.
    ///
    /// `size_px` is the target size in physical pixels and `scale` the scale factor; the list's
    /// coordinates are logical pixels.
    pub fn render(
        &mut self,
        gpu: &GpuContext,
        target: &wgpu::TextureView,
        size_px: (u32, u32),
        scale: f32,
        list: &RenderList,
        clear: Color,
        images: &mut ImageStore,
    ) {
        // ---- gather instances into batches ----
        self.instances.clear();
        self.batches.clear();
        self.runs.clear();
        self.native.begin_frame();
        self.colors.gradients.clone_from(&list.gradients);
        for op in &list.ops {
            match op {
                DrawOp::Quad(q) => self.push_quad(None, *q),
                DrawOp::Image(id, q) => {
                    if self.ensure_image(gpu, images, *id) {
                        self.push_quad(Some(*id), *q);
                    }
                }
                DrawOp::Text(t) if self.text.is_some() => {
                    let i = self.runs.len();
                    self.runs.push(t.clone());
                    match self.batches.last_mut() {
                        Some(Batch::Text { range }) => range.end = i + 1,
                        _ => self.batches.push(Batch::Text { range: i..i + 1 }),
                    }
                }
                DrawOp::InputText(t) | DrawOp::Text(t) => {
                    self.glyphs.clear();
                    let input = matches!(op, DrawOp::InputText(_));
                    self.native.prepare(
                        gpu,
                        &self.texture_layout,
                        &self.sampler,
                        t,
                        input,
                        scale,
                        &mut self.colors,
                        &mut self.glyphs,
                    );
                    for &(page, quad) in &self.glyphs {
                        let i = self.instances.len() as u32;
                        self.instances.push(quad);
                        match self.batches.last_mut() {
                            Some(Batch::Glyphs { page: prev, range }) if *prev == page => {
                                range.end = i + 1
                            }
                            _ => self.batches.push(Batch::Glyphs {
                                page,
                                range: i..i + 1,
                            }),
                        }
                    }
                }
            }
        }

        // ---- upload ----
        let globals = Globals {
            viewport: [size_px.0 as f32 / scale, size_px.1 as f32 / scale],
            scale,
            srgb_target: if self.format.is_srgb() { 1.0 } else { 0.0 },
        };
        gpu.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        let mut regroup = false;
        if self.instances.len() > self.instance_cap {
            self.instance_cap = self.instances.len().next_power_of_two();
            self.instance_buf = Self::make_instance_buf(&gpu.device, self.instance_cap);
        }
        if self.colors.gradients.len() > self.gradient_cap {
            self.gradient_cap = self.colors.gradients.len().next_power_of_two();
            self.gradient_buf = Self::make_gradient_buf(&gpu.device, self.gradient_cap);
            regroup = true;
        }
        if regroup {
            self.globals_group = Self::make_globals_group(
                &gpu.device,
                &self.globals_layout,
                &self.globals_buf,
                &self.gradient_buf,
            );
        }
        if !self.instances.is_empty() {
            gpu.queue
                .write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&self.instances));
        }
        if !self.colors.gradients.is_empty() {
            gpu.queue.write_buffer(
                &self.gradient_buf,
                0,
                bytemuck::cast_slice(&self.colors.gradients),
            );
        }
        if !self.runs.is_empty() {
            self.text.as_mut().unwrap().prepare(gpu, &self.runs, scale);
        }

        // ---- encode ----
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("octlui frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("octlui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear.r as f64,
                            g: clear.g as f64,
                            b: clear.b as f64,
                            a: clear.a as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.globals_group, &[]);
            pass.set_vertex_buffer(0, self.instance_buf.slice(..));
            for b in &self.batches {
                match b {
                    Batch::Quads { image, range } => {
                        let bg = match image {
                            Some(id) => &self.images[&id.0].bind_group,
                            None => &self.white.bind_group,
                        };
                        pass.set_bind_group(1, bg, &[]);
                        pass.draw(0..4, range.clone());
                    }
                    Batch::Glyphs { page, range } => {
                        pass.set_bind_group(1, self.native.group(*page), &[]);
                        pass.draw(0..4, range.clone());
                    }
                    Batch::Text { range } => {
                        self.text.as_mut().unwrap().draw(&mut pass, range.clone());
                        // A text renderer may have changed pipeline state.
                        pass.set_pipeline(&self.pipeline);
                        pass.set_bind_group(0, &self.globals_group, &[]);
                        pass.set_vertex_buffer(0, self.instance_buf.slice(..));
                    }
                }
            }
        }
        gpu.queue.submit(Some(encoder.finish()));
    }

    fn push_quad(&mut self, image: Option<ImageId>, q: QuadInstance) {
        let i = self.instances.len() as u32;
        self.instances.push(q);
        match self.batches.last_mut() {
            Some(Batch::Quads { image: im, range }) if *im == image => range.end = i + 1,
            _ => self.batches.push(Batch::Quads {
                image,
                range: i..i + 1,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shader_validates() {
        let src = include_str!("shader.wgsl");
        let module = naga::front::wgsl::parse_str(src).expect("wgsl parse");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("wgsl validation");
    }

    #[test]
    fn instance_layout_sizes() {
        assert_eq!(std::mem::size_of::<super::QuadInstance>(), 128);
        assert_eq!(std::mem::size_of::<super::super::GpuGradient>(), 176);
    }
}
