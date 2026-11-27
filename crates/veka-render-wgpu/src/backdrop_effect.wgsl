// Composition des effets d'arrière-plan (la pyramide : `backdrop_pass.wgsl`). Formules reprises de
// VekaUI (`VekaBackdrop.usf`).

// ---------------------------------------------------------------- Composition

struct Globals {
    viewport: vec2f,
    _pad: vec2f,
};

@group(0) @binding(0) var<uniform> globals: Globals;

@group(1) @binding(0) var level0: texture_2d<f32>;
@group(1) @binding(1) var level1: texture_2d<f32>;
@group(1) @binding(2) var level2: texture_2d<f32>;
@group(1) @binding(3) var level3: texture_2d<f32>;
@group(1) @binding(4) var level4: texture_2d<f32>;
@group(1) @binding(5) var level5: texture_2d<f32>;
@group(1) @binding(6) var level6: texture_2d<f32>;
@group(1) @binding(7) var level7: texture_2d<f32>;
@group(1) @binding(8) var level_sampler: sampler;

struct EffectIn {
    @location(0) rect: vec4f,     // repère local, points
    @location(1) radii: vec4f,
    @location(2) xform: vec4f,    // a, b, c, d (local → pixels)
    @location(3) extra: vec4f,    // tx, ty, sorte (0 boîte, 1 ellipse), opacité
    @location(4) params: vec4f,   // niveau de flou (log2 du rayon en pixels), saturation, grain, qualité réduite
    @location(5) tint: vec4f,     // linéaire, a = part de la teinte
    @location(6) region: vec4f,   // zone capturée, pixels : min.xy, max.xy
};

struct EffectOut {
    @builtin(position) pos: vec4f,
    @location(0) local: vec2f,
    @location(1) @interpolate(flat) rect: vec4f,
    @location(2) @interpolate(flat) radii: vec4f,
    @location(3) @interpolate(flat) extra: vec4f,
    @location(4) @interpolate(flat) params: vec4f,
    @location(5) @interpolate(flat) tint: vec4f,
    @location(6) @interpolate(flat) region: vec4f,
};

fn corner(vi: u32) -> vec2f {
    var c = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    return c[vi];
}

@vertex
fn effect_vs(@builtin(vertex_index) vi: u32, i: EffectIn) -> EffectOut {
    let a = i.xform.x; let b = i.xform.y; let c = i.xform.z; let d = i.xform.w;
    let px_scale = max(sqrt(abs(a * d - b * c)), 1e-4);
    let margin = 2.0 / px_scale;
    let lo = i.rect.xy - vec2f(margin);
    let size = i.rect.zw + vec2f(2.0 * margin);
    let local = lo + corner(vi) * size;
    let px = vec2f(a * local.x + c * local.y + i.extra.x, b * local.x + d * local.y + i.extra.y);
    let n = px / globals.viewport * 2.0 - 1.0;
    var o: EffectOut;
    o.pos = vec4f(n.x, -n.y, 0.0, 1.0);
    o.local = local;
    o.rect = i.rect;
    o.radii = i.radii;
    o.extra = i.extra;
    o.params = i.params;
    o.tint = i.tint;
    o.region = i.region;
    return o;
}

fn sd_round_box(p: vec2f, half: vec2f, radii: vec4f) -> f32 {
    var r = radii.x;
    if (p.x >= 0.0 && p.y < 0.0) { r = radii.y; }
    if (p.x >= 0.0 && p.y >= 0.0) { r = radii.z; }
    if (p.x < 0.0 && p.y >= 0.0) { r = radii.w; }
    let q = abs(p) - half + vec2f(r);
    return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn sd_ellipse(p: vec2f, half: vec2f) -> f32 {
    if (half.x <= 0.0 || half.y <= 0.0) { return length(p); }
    return (length(p / half) - 1.0) * min(half.x, half.y);
}

// Le niveau `k` de la pyramide au pixel `px` de la fenêtre, sans sortir de la zone capturée.
fn level(k: i32, px: vec2f, region: vec4f) -> vec4f {
    let f = exp2(f32(k));
    let lo = floor(region.xy / f) + vec2f(0.5);
    let hi = ceil(region.zw / f) - vec2f(0.5);
    let t = clamp(px / f, lo, max(lo, hi));
    switch k {
        case 0: { return textureSampleLevel(level0, level_sampler, t / vec2f(textureDimensions(level0)), 0.0); }
        case 1: { return textureSampleLevel(level1, level_sampler, t / vec2f(textureDimensions(level1)), 0.0); }
        case 2: { return textureSampleLevel(level2, level_sampler, t / vec2f(textureDimensions(level2)), 0.0); }
        case 3: { return textureSampleLevel(level3, level_sampler, t / vec2f(textureDimensions(level3)), 0.0); }
        case 4: { return textureSampleLevel(level4, level_sampler, t / vec2f(textureDimensions(level4)), 0.0); }
        case 5: { return textureSampleLevel(level5, level_sampler, t / vec2f(textureDimensions(level5)), 0.0); }
        case 6: { return textureSampleLevel(level6, level_sampler, t / vec2f(textureDimensions(level6)), 0.0); }
        default: { return textureSampleLevel(level7, level_sampler, t / vec2f(textureDimensions(level7)), 0.0); }
    }
}

// Grain stable par pixel, dans [-0,5, 0,5].
fn grain(px: vec2f) -> f32 {
    return fract(sin(dot(floor(px), vec2f(12.9898, 78.233))) * 43758.5453) - 0.5;
}

@fragment
fn effect_fs(i: EffectOut) -> @location(0) vec4f {
    let half = i.rect.zw * 0.5;
    let p = i.local - (i.rect.xy + half);
    var d: f32;
    if (i.extra.z > 0.5) { d = sd_ellipse(p, half); } else { d = sd_round_box(p, half, i.radii); }
    let w = max(fwidth(d), 1e-4);
    let coverage = clamp(0.5 - d / w, 0.0, 1.0) * i.extra.w;
    if (coverage <= 0.0) {
        discard;
    }
    let px = i.pos.xy;
    let lf = clamp(i.params.x, 0.0, 7.0);
    var k = i32(floor(lf));
    var mixv = lf - floor(lf);
    if (i.params.w > 0.5) {
        k = i32(round(lf));
        mixv = 0.0;
    }
    var b = level(k, px, i.region);
    if (mixv > 0.001) {
        b = mix(b, level(k + 1, px, i.region), mixv);
    }
    // Couleur non prémultipliée, saturée, teintée, puis le grain.
    var c = b.rgb / max(b.a, 1e-4);
    let luma = dot(c, vec3f(0.2126, 0.7152, 0.0722));
    c = mix(vec3f(luma), c, i.params.y);
    c = mix(c, i.tint.rgb, i.tint.a);
    c += grain(px) * i.params.z;
    let a = mix(b.a, 1.0, i.tint.a);
    return vec4f(max(c, vec3f(0.0)) * a, a) * coverage;
}
