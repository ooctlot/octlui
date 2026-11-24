use super::{GpuContext, QuadInstance, RenderList, TextRun};
use crate::{
    components::FontId,
    style::TextWrap,
    text::{NativeTextMeasurer, ShapedText},
};
use cosmic_text::{CacheKey, SwashContent};
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
    color: bool,
}

impl Page {
    fn new(
        gpu: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        side: u32,
        color: bool,
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
            format: if color {
                wgpu::TextureFormat::Rgba8Unorm
            } else {
                wgpu::TextureFormat::R8Unorm
            },
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
            color,
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
    color: bool,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum GlyphKey {
    Native(FontId, u32, u32),
    Shaped(CacheKey),
}

struct Bitmap<'a> {
    bounds: [i32; 4],
    stride: u32,
    pixels: &'a [u8],
    color: bool,
}

#[derive(Default)]
struct GlyphCache {
    pages: Vec<Page>,
    entries: HashMap<GlyphKey, Option<Entry>>,
}

impl GlyphCache {
    fn native(
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
        self.upload(
            gpu,
            layout,
            sampler,
            Bitmap {
                bounds: [bounds.origin_x(), bounds.origin_y(), w as i32, h as i32],
                stride: canvas.stride as u32,
                pixels: &canvas.pixels,
                color: false,
            },
        )
    }

    fn upload(
        &mut self,
        gpu: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        bitmap: Bitmap<'_>,
    ) -> Option<Entry> {
        let [left, top, w, h] = bitmap.bounds;
        let (w, h) = (u32::try_from(w).ok()?, u32::try_from(h).ok()?);
        let max = gpu.device.limits().max_texture_dimension_2d;
        if w == 0 || h == 0 || w + 2 > max || h + 2 > max {
            return None;
        }
        let pos = self
            .pages
            .iter_mut()
            .enumerate()
            .rev()
            .find(|(_, page)| page.color == bitmap.color)
            .and_then(|(i, page)| page.reserve(w, h).map(|pos| (i, pos)));
        let (index, (x, y)) = pos.unwrap_or_else(|| {
            let side = 1024.max(w + 2).max(h + 2).next_power_of_two().min(max);
            let mut page = Page::new(gpu, layout, sampler, side, bitmap.color);
            let pos = page.reserve(w, h).unwrap();
            let index = self.pages.len();
            self.pages.push(page);
            (index, pos)
        });
        let page = &self.pages[index];
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &page.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            bitmap.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bitmap.stride),
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
            page: index,
            uv: [
                x as f32 / side,
                y as f32 / side,
                (x + w) as f32 / side,
                (y + h) as f32 / side,
            ],
            bounds: [left as f32, top as f32, w as f32, h as f32],
            color: bitmap.color,
        })
    }

    fn shaped(
        &mut self,
        gpu: &GpuContext,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        text: &ShapedText,
        key: CacheKey,
    ) -> Option<Entry> {
        let image = text.raster(key)?;
        let placement = image.placement;
        let color = image.content == SwashContent::Color;
        let mut pixels = image.data;
        if image.content == SwashContent::SubpixelMask {
            pixels = pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| c[0].max(c[1]).max(c[2]))
                .collect();
        }
        self.upload(
            gpu,
            layout,
            sampler,
            Bitmap {
                bounds: [
                    placement.left,
                    -placement.top,
                    placement.width as i32,
                    placement.height as i32,
                ],
                stride: placement.width * if color { 4 } else { 1 },
                pixels: &pixels,
                color,
            },
        )
    }
}

#[derive(Default)]
pub(super) struct NativeText {
    pub(super) fonts: NativeTextMeasurer,
    shaped: Option<ShapedText>,
    cache: GlyphCache,
}

impl NativeText {
    pub(super) fn set_shaped(&mut self, text: ShapedText) {
        self.shaped = Some(text);
        self.cache = GlyphCache::default();
    }

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
        let (tint, gradient) = colors.encode_fill(&run.color);
        let area = run.rect;
        let clip = run.clip;
        let line_origin = |width: f32| {
            let remaining = area.w - width;
            let offset = if matches!(run.wrap, TextWrap::Fade { .. } | TextWrap::Clip { .. }) {
                remaining.max(0.)
            } else {
                remaining
            };
            let start = area.x + run.align.factor() * offset;
            let fade = match run.wrap {
                TextWrap::Fade { length } if remaining < 0. => length.max(0.),
                _ => 0.,
            };
            (start, fade)
        };
        let mut push = |entry: Entry, x: f32, y: f32, fade: f32| {
            let [left, top, w, h] = entry.bounds;
            let rect = [(x + left) / scale, (y + top) / scale, w / scale, h / scale];
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
                    extra: [if entry.color { -2. } else { -1. }, -1., -1., gradient],
                },
            ));
        };
        if let Some(shaped) = &self.shaped {
            let lines = shaped.layout(&run.content, run.font, run.size, run.wrap, Some(area.w));
            for line in &lines.lines {
                let (start, fade) = line_origin(line.width);
                let baseline = area.y + line.baseline;
                for glyph in &line.glyphs {
                    let physical = glyph.physical((start * scale, baseline * scale), scale);
                    let key = GlyphKey::Shaped(physical.cache_key);
                    let entry = match self.cache.entries.get(&key) {
                        Some(entry) => *entry,
                        None => {
                            let entry =
                                self.cache
                                    .shaped(gpu, layout, sampler, shaped, physical.cache_key);
                            self.cache.entries.insert(key, entry);
                            entry
                        }
                    };
                    if let Some(entry) = entry {
                        push(entry, physical.x as f32, physical.y as f32, fade);
                    }
                }
            }
            return;
        }
        let Some(face) = self.fonts.face(run.font) else {
            return;
        };
        let lines = face.layout(&run.content, run.size, run.wrap, area.w);
        for (i, line) in lines.lines.iter().enumerate() {
            let (start, fade) = line_origin(line.width);
            let baseline = area.y + lines.ascent + i as f32 * lines.height;
            for &(id, x) in &line.glyphs {
                let key = GlyphKey::Native(face.id, id, px.to_bits());
                let entry = match self.cache.entries.get(&key) {
                    Some(entry) => *entry,
                    None => {
                        let entry = self.cache.native(gpu, layout, sampler, &face.font, id, px);
                        self.cache.entries.insert(key, entry);
                        entry
                    }
                };
                if let Some(entry) = entry {
                    push(
                        entry,
                        ((start + x) * scale).round(),
                        (baseline * scale).round(),
                        fade,
                    );
                }
            }
        }
    }
}
