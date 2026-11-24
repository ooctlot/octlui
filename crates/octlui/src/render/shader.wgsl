// octlui quad shader: rounded rectangles with per-corner radii, borders, gradients,
// image sampling with tint and per-instance clip rects. All geometry is in logical pixels.

struct Globals {
    viewport: vec2<f32>, // logical size
    scale: f32,          // physical pixels per logical pixel
    srgb_target: f32,    // 1.0 if the surface format is sRGB (convert to linear on output)
};

struct Gradient {
    colors: array<vec4<f32>, 8>,
    offsets: array<vec4<f32>, 2>,
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> gradients: array<Gradient>;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,
    @location(1) clip: vec4<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) fill: vec4<f32>,
    @location(4) border: vec4<f32>,
    @location(5) uv: vec4<f32>,
    @location(6) tint: vec4<f32>,
    @location(7) extra: vec4<f32>,
};

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec2<f32>,
    @location(1) @interpolate(flat) rect: vec4<f32>,
    @location(2) @interpolate(flat) clip: vec4<f32>,
    @location(3) @interpolate(flat) radii: vec4<f32>,
    @location(4) @interpolate(flat) fill: vec4<f32>,
    @location(5) @interpolate(flat) border: vec4<f32>,
    @location(6) @interpolate(flat) uv: vec4<f32>,
    @location(7) @interpolate(flat) tint: vec4<f32>,
    @location(8) @interpolate(flat) extra: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VOut {
    // Triangle strip: (0,0) (1,0) (0,1) (1,1)
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let margin = 1.0 / globals.scale; // one physical pixel for the anti-aliasing fringe
    let p = inst.rect.xy - vec2<f32>(margin) + corner * (inst.rect.zw + vec2<f32>(2.0 * margin));
    let ndc = vec2<f32>(p.x / globals.viewport.x * 2.0 - 1.0, 1.0 - p.y / globals.viewport.y * 2.0);
    var o: VOut;
    o.pos = vec4<f32>(ndc, 0.0, 1.0);
    o.world = p;
    o.rect = inst.rect;
    o.clip = inst.clip;
    o.radii = inst.radii;
    o.fill = inst.fill;
    o.border = inst.border;
    o.uv = inst.uv;
    o.tint = inst.tint;
    o.extra = inst.extra;
    return o;
}

fn sd_rounded_box(p: vec2<f32>, half_size: vec2<f32>, r: vec4<f32>) -> f32 {
    var rr = r.x; // top-left
    if (p.x > 0.0) {
        if (p.y > 0.0) { rr = r.z; } else { rr = r.y; }
    } else {
        if (p.y > 0.0) { rr = r.w; }
    }
    let q = abs(p) - half_size + vec2<f32>(rr);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - rr;
}

fn gradient_color(idx: u32, p: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    let g = gradients[idx];
    let count = u32(g.params.y);
    var t: f32;
    if (g.params.x < 0.5) {
        let dir = g.params.zw;
        let len = abs(size.x * dir.x) + abs(size.y * dir.y);
        t = 0.5 + dot(p, dir) / max(len, 0.0001);
    } else {
        t = length(p / max(size * 0.5, vec2<f32>(0.0001))) * 0.70710678;
    }
    t = clamp(t, 0.0, 1.0);
    var c = g.colors[0];
    for (var i = 1u; i < count; i = i + 1u) {
        let o0 = g.offsets[(i - 1u) / 4u][(i - 1u) % 4u];
        let o1 = g.offsets[i / 4u][i % 4u];
        if (t > o0) {
            let f = clamp((t - o0) / max(o1 - o0, 0.00001), 0.0, 1.0);
            c = mix(g.colors[i - 1u], g.colors[i], f);
        }
    }
    return c;
}

fn premul(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb * c.a, c.a);
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    if (in.world.x < in.clip.x || in.world.y < in.clip.y
        || in.world.x > in.clip.x + in.clip.z || in.world.y > in.clip.y + in.clip.w) {
        discard;
    }
    let size = in.rect.zw;
    let local = in.world - (in.rect.xy + size * 0.5);
    let d = sd_rounded_box(local, size * 0.5, in.radii);
    let outer = clamp(0.5 - d * globals.scale, 0.0, 1.0);
    if (outer <= 0.0) { discard; }

    // Background fill.
    var fill = in.fill;
    if (in.extra.y >= 0.0) { fill = gradient_color(u32(in.extra.y), local, size); }
    var col = premul(fill) * outer;

    // Image (sampled unconditionally to stay in uniform control flow), multiplied by tint.
    let uv = mix(in.uv.xy, in.uv.zw, (in.world - in.rect.xy) / max(size, vec2<f32>(0.0001)));
    let texel = textureSampleLevel(tex, samp, uv, 0.0);
    if (in.uv.z != in.uv.x) {
        var tint = in.tint;
        if (in.extra.w >= 0.0) { tint = gradient_color(u32(in.extra.w), local, size); }
        let img = premul(texel * tint) * outer;
        col = img + col * (1.0 - img.a);
    }

    // Border, drawn inside the outer edge.
    let bw = in.extra.x;
    if (bw > 0.0) {
        let inner = clamp(0.5 - (d + bw) * globals.scale, 0.0, 1.0);
        var bc = in.border;
        if (in.extra.z >= 0.0) { bc = gradient_color(u32(in.extra.z), local, size); }
        let b = premul(bc) * clamp(outer - inner, 0.0, 1.0);
        col = b + col * (1.0 - b.a);
    }

    if (globals.srgb_target > 0.5 && col.a > 0.0) {
        col = vec4<f32>(to_linear(col.rgb / col.a) * col.a, col.a);
    }
    return col;
}
