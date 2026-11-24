use super::{GpuContext, QuadInstance, RenderList, TextRun};
use crate::{components::FontId, style::TextWrap, text::NativeTextMeasurer};
use font_kit::{
    canvas::{Canvas, Format, RasterizationOptions},
    font::Font,
    hinting::HintingOptions,
};
use pathfinder_geometry::transform2d::Transform2F;
use std::collections::HashMap;

struct Page {
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
    side: u32,
    x: u32,
    y: u32,
    row: u32,
}

impl Page {
    fn new(
        gpu: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        side: u32,
    ) -> Self {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph coverage atlas"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glyph atlas"),
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
        Self {
            texture,
            group,
            side,
            x: 1,
            y: 1,
            row: 0,
        }
    }

    fn reserve(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w + 2 > self.side {
            return None;
        }
        if self.x + w >= self.side {
            self.x = 1;
            self.y += self.row;
            self.row = 0;
        }
        if self.y + h >= self.side {
            return None;
        }
        let pos = (self.x, self.y);
        self.x += w + 1;
        self.row = self.row.max(h + 1);
        Some(pos)
    }
}

#[derive(Clone, Copy)]
struct Entry {
    page: usize,
    uv: [f32; 4],
    bounds: [f32; 4],
}

#[derive(Default)]
struct GlyphCache {
    pages: Vec<Page>,
    entries: HashMap<(FontId, u32, u32), Option<Entry>>,
}

impl GlyphCache {
    fn upload(
        &mut self,
        gpu: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        font: &Font,
        glyph: u32,
        px: f32,
    ) -> Option<Entry> {
        let hint = HintingOptions::None;
        let aa = RasterizationOptions::GrayscaleAa;
        let bounds = font
            .raster_bounds(glyph, px, Transform2F::default(), hint, aa)
            .ok()?;
        let (w, h) = (
            u32::try_from(bounds.width()).ok()?,
            u32::try_from(bounds.height()).ok()?,
        );
        let max = gpu.device.limits().max_texture_dimension_2d;
        if w == 0 || h == 0 || w + 2 > max || h + 2 > max {
            return None;
        }
        let mut canvas = Canvas::new(bounds.size(), Format::A8);
        let transform = Transform2F::from_translation(-bounds.origin().to_f32());
        font.rasterize_glyph(&mut canvas, glyph, px, transform, hint, aa)
            .ok()?;
        let pos = self.pages.last_mut().and_then(|p| p.reserve(w, h));
        let (x, y) = pos.unwrap_or_else(|| {
            let side = 1024.max(w + 2).max(h + 2).next_power_of_two().min(max);
            let mut page = Page::new(gpu, layout, sampler, side);
            let pos = page.reserve(w, h).unwrap();
            self.pages.push(page);
            pos
        });
        let page = self.pages.last().unwrap();
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &page.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &canvas.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(canvas.stride as u32),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let side = page.side as f32;
        Some(Entry {
            page: self.pages.len() - 1,
            uv: [
                x as f32 / side,
                y as f32 / side,
                (x + w) as f32 / side,
                (y + h) as f32 / side,
            ],
            bounds: [
                bounds.origin_x() as f32,
                bounds.origin_y() as f32,
                w as f32,
                h as f32,
            ],
        })
    }
}

#[derive(Default)]
pub(super) struct NativeText {
    fonts: NativeTextMeasurer,
    cache: GlyphCache,
}

impl NativeText {
    pub fn begin_frame(&mut self) {
        // Only reset between frames, when no pending batch references an old page.
        if self.cache.pages.len() > 8 || self.cache.entries.len() > 65536 {
            self.cache = GlyphCache::default();
        }
    }
    pub fn group(&self, page: usize) -> &wgpu::BindGroup {
        &self.cache.pages[page].group
    }

    pub fn prepare(
        &mut self,
        gpu: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        run: &TextRun,
        scale: f32,
        colors: &mut RenderList,
        out: &mut Vec<(usize, QuadInstance)>,
    ) {
        let px = run.size * scale;
        if !px.is_finite() || px <= 0. || px > gpu.device.limits().max_texture_dimension_2d as f32 {
            return;
        }
        let Some(face) = self.fonts.face(run.font) else {
            return;
        };
        let lines = face.layout(&run.content, run.size, run.wrap, run.rect.w);
        let (tint, gradient) = colors.encode_fill(&run.color);
        let area = run.rect;
        let clip = run.clip;
        for (i, line) in lines.lines.iter().enumerate() {
            let remaining = area.w - line.width;
            let overflow = remaining < 0.;
            let offset = if matches!(run.wrap, TextWrap::Fade { .. } | TextWrap::Clip { .. }) {
                remaining.max(0.)
            } else {
                remaining
            };
            let start = area.x + run.align.factor() * offset;
            let baseline = area.y + lines.ascent + i as f32 * lines.height;
            let fade = match run.wrap {
                TextWrap::Fade { length } if overflow => length.max(0.),
                _ => 0.,
            };
            for &(id, x) in &line.glyphs {
                let key = (run.font, id, px.to_bits());
                let entry = match self.cache.entries.get(&key) {
                    Some(entry) => *entry,
                    None => {
                        let entry = self.cache.upload(gpu, layout, sampler, &face.font, id, px);
                        self.cache.entries.insert(key, entry);
                        entry
                    }
                };
                let Some(entry) = entry else { continue };
                let [left, top, w, h] = entry.bounds;
                let rect = [
                    (((start + x) * scale).round() + left) / scale,
                    ((baseline * scale).round() + top) / scale,
                    w / scale,
                    h / scale,
                ];
                out.push((
                    entry.page,
                    QuadInstance {
                        rect,
                        clip: [clip.x, clip.y, clip.w, clip.h],
                        uv: entry.uv,
                        tint,
                        fill: [area.x, area.y, area.w, area.h],
                        radii: [fade, 0., 0., 0.],
                        border: [0.; 4],
                        extra: [-1., -1., -1., gradient],
                    },
                ));
            }
        }
    }
}
