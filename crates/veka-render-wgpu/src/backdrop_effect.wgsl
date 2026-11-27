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
    @location(7) flow: vec4f,     // ondulation : force (pixels), 1 / taille des vagues (pixels), temps
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
    @location(7) @interpolate(flat) flow: vec4f,
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
    o.flow = i.flow;
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
    // Ondulation (deux couches de sinus croisés), qui s'efface au bord de la forme.
    var flow = vec2f(0.0);
    if (i.flow.x > 0.0) {
        let q = px * i.flow.y;
        let t = i.flow.z * 2.2;
        flow.x = sin(q.y * 6.2832 + t * 1.3) + 0.5 * sin((q.x + q.y) * 9.1 - t * 2.1);
        flow.y = cos(q.x * 6.2832 - t * 1.7) + 0.5 * cos((q.x - q.y) * 8.3 + t * 1.9);
        let d_px = d / w;
        flow *= i.flow.x * 0.66 * clamp(-d_px / max(i.flow.x * 2.0, 1.0), 0.0, 1.0);
    }
    let sp = px + flow;
    var b = level(k, sp, i.region);
    if (mixv > 0.001) {
        b = mix(b, level(k + 1, sp, i.region), mixv);
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

// ---------------------------------------------------------------- Verre liquide (façon 27)

struct GlassShapeGpu {
    rect: vec4f,    // repère local : x, y, largeur, hauteur
    radii: vec4f,
    kind: vec4f,    // x : 0 boîte, 1 ellipse
};

@group(2) @binding(0) var<storage, read> glass_shapes: array<GlassShapeGpu>;

struct GlassIn {
    @location(0) rect: vec4f,     // boîte de toutes les formes (repère local)
    @location(1) xform: vec4f,
    @location(2) extra: vec4f,    // tx, ty, première forme, nombre de formes
    @location(3) p1: vec4f,       // niveau de flou, réfraction (px), largeur du bord (px), aberration
    @location(4) p2: vec4f,       // reflet, fusion (repère local), saturation, opacité
    @location(5) tint: vec4f,
    @location(6) region: vec4f,
    @location(7) light: vec4f,    // pointeur (px), pointeur présent, qualité réduite
    @location(8) p3: vec4f,       // bord sombre, lumière interne, ombre, rayon de l'ombre (px)
};

struct GlassOut {
    @builtin(position) pos: vec4f,
    @location(0) local: vec2f,
    @location(1) @interpolate(flat) rect: vec4f,
    @location(2) @interpolate(flat) xform: vec4f,
    @location(3) @interpolate(flat) extra: vec4f,
    @location(4) @interpolate(flat) p1: vec4f,
    @location(5) @interpolate(flat) p2: vec4f,
    @location(6) @interpolate(flat) tint: vec4f,
    @location(7) @interpolate(flat) region: vec4f,
    @location(8) @interpolate(flat) light: vec4f,
    @location(9) @interpolate(flat) p3: vec4f,
};

@vertex
fn glass_vs(@builtin(vertex_index) vi: u32, i: GlassIn) -> GlassOut {
    let a = i.xform.x; let b = i.xform.y; let c = i.xform.z; let d = i.xform.w;
    let px_scale = max(sqrt(abs(a * d - b * c)), 1e-4);
    // Marge : antialiasing et ombre (décalée vers le bas).
    let margin = (2.0 + i.p3.w * 1.6) / px_scale;
    let lo = i.rect.xy - vec2f(margin);
    let size = i.rect.zw + vec2f(2.0 * margin);
    let local = lo + corner(vi) * size;
    let px = vec2f(a * local.x + c * local.y + i.extra.x, b * local.x + d * local.y + i.extra.y);
    let n = px / globals.viewport * 2.0 - 1.0;
    var o: GlassOut;
    o.pos = vec4f(n.x, -n.y, 0.0, 1.0);
    o.local = local;
    o.rect = i.rect;
    o.xform = i.xform;
    o.extra = i.extra;
    o.p1 = i.p1;
    o.p2 = i.p2;
    o.tint = i.tint;
    o.region = i.region;
    o.light = i.light;
    o.p3 = i.p3;
    return o;
}

fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if (k <= 0.0) { return min(a, b); }
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

// Distance signée (repère local) aux formes fusionnées.
fn glass_field(p: vec2f, first: u32, count: u32, k: f32) -> f32 {
    var dist = 1e6;
    for (var j = 0u; j < count; j++) {
        let s = glass_shapes[first + j];
        let half = s.rect.zw * 0.5;
        let q = p - (s.rect.xy + half);
        var dj: f32;
        if (s.kind.x > 0.5) { dj = sd_ellipse(q, half); } else { dj = sd_round_box(q, half, s.radii); }
        if (j == 0u) { dist = dj; } else { dist = smooth_min(dist, dj, k); }
    }
    return dist;
}

fn blurred(px: vec2f, k: i32, mixv: f32, region: vec4f) -> vec4f {
    var b = level(k, px, region);
    if (mixv > 0.001) {
        b = mix(b, level(k + 1, px, region), mixv);
    }
    return b;
}

@fragment
fn glass_fs(i: GlassOut) -> @location(0) vec4f {
    let a = i.xform.x; let b = i.xform.y; let c = i.xform.z; let d = i.xform.w;
    let ps = max(sqrt(abs(a * d - b * c)), 1e-4);
    let first = u32(i.extra.z + 0.5);
    let count = u32(i.extra.w + 0.5);
    let k_merge = i.p2.y;
    let dist = glass_field(i.local, first, count, k_merge) * ps;
    let alpha = i.p2.w;
    let coverage = clamp(0.5 - dist, 0.0, 1.0) * alpha;

    // Ombre portée, douce et un peu décalée vers le bas, seulement hors du verre.
    var shadow = 0.0;
    let sr = i.p3.w;
    if (i.p3.z > 0.0 && sr > 0.0) {
        let sd = glass_field(i.local - vec2f(0.0, sr * 0.35 / ps), first, count, k_merge) * ps;
        shadow = i.p3.z * (1.0 - smoothstep(-sr * 0.5, sr, sd)) * alpha;
    }
    if (coverage <= 0.0 && shadow <= 0.0) {
        discard;
    }
    let px = i.pos.xy;
    var color = vec3f(0.0);
    var cov_alpha = 0.0;
    if (coverage > 0.0) {
        // Normale vers l'extérieur, en pixels.
        let e = 1.0 / ps;
        let g = vec2f(
            glass_field(i.local + vec2f(e, 0.0), first, count, k_merge) - glass_field(i.local - vec2f(e, 0.0), first, count, k_merge),
            glass_field(i.local + vec2f(0.0, e), first, count, k_merge) - glass_field(i.local - vec2f(0.0, e), first, count, k_merge),
        );
        var n = vec2f(a * g.x + c * g.y, b * g.x + d * g.y);
        n = n / max(length(n), 1e-4);

        // 0 au bord, 1 passé le bord courbe : le bord tord le fond, surtout tout au bord.
        let bezel = max(i.p1.z, 1.0);
        let depth = clamp(-dist / bezel, 0.0, 1.0);
        let rim = 1.0 - depth;
        let lens = rim * rim;
        let offset = n * i.p1.y * lens;

        let lf = clamp(i.p1.x, 0.0, 7.0);
        var k = i32(floor(lf));
        var mixv = lf - floor(lf);
        var ab = i.p1.w;
        if (i.light.w > 0.5) {
            k = i32(round(lf));
            mixv = 0.0;
            ab = 0.0;
        }
        let center = blurred(px + offset, k, mixv, i.region);
        var col = center.rgb / max(center.a, 1e-4);
        if (ab > 0.001) {
            let r = blurred(px + offset * (1.0 + ab), k, mixv, i.region);
            let bl = blurred(px + offset * (1.0 - ab), k, mixv, i.region);
            col = vec3f(r.r / max(r.a, 1e-4), col.g, bl.b / max(bl.a, 1e-4));
        }
        let luma = dot(col, vec3f(0.2126, 0.7152, 0.0722));
        col = mix(vec3f(luma), col, i.p2.z);
        col = mix(col, i.tint.rgb, i.tint.a);

        // Ce qui est capturé peut être transparent (fenêtre posée sur Mica ou la vibrancy) : le corps du
        // verre garde l'opacité du fond (plus la teinte), sa lumière s'ajoute par-dessus.
        let body_a = mix(center.a, 1.0, i.tint.a);

        // Lumière propre au verre : plus claire en haut (27).
        let t = clamp((i.local.y - i.rect.y) / max(i.rect.w, 1e-4), 0.0, 1.0);
        let inner = i.p3.y * 0.14 * (1.0 - t);

        // Anneau sombre juste à l'intérieur du bord (27).
        // Un trait fin et net tout au bord, puis une ombre plus large qui s'estompe vers l'intérieur.
        let inside = max(-dist, 0.0);
        let band = 0.6 * exp(-inside / max(bezel * 0.12, 1.0)) + 0.4 * exp(-inside / max(bezel * 0.45, 1.0));
        let ring = clamp(i.p3.x * 0.9 * band, 0.0, 0.6);
        col = (col + vec3f(inner)) * (1.0 - ring);

        // Reflet : un trait vif tout au bord, tourné vers la lumière (le pointeur, sinon en haut à gauche).
        var to_light = normalize(vec2f(-0.55, -1.0));
        if (i.light.z > 0.5) {
            to_light = normalize(i.light.xy - px + vec2f(1e-3));
        }
        let facing = clamp(dot(n, to_light), 0.0, 1.0);
        let back = clamp(dot(n, -to_light), 0.0, 1.0);
        let edge = exp(-max(-dist, 0.0) / 1.6);
        var spec = i.p2.x * (edge * (0.25 + 0.75 * facing + 0.35 * back) + 0.18 * pow(rim, 3.0) * facing);
        // Verre interactif : une lueur douce sous le pointeur, et le bord le plus proche s'allume.
        if (i.light.z > 0.5) {
            let r = max(min(i.rect.z, i.rect.w) * ps * 0.6, 24.0);
            let near = exp(-dot(i.light.xy - px, i.light.xy - px) / (r * r));
            spec += i.p2.x * (0.16 * near + 0.9 * edge * near);
        }

        // Prémultiplié : corps, puis lumière (et anneau) là où le fond est transparent.
        let glow = spec + inner * (1.0 - body_a);
        let out_a = clamp(body_a + (1.0 - body_a) * (clamp(spec + inner, 0.0, 1.0) + ring * 0.6), 0.0, 1.0);
        cov_alpha = out_a * coverage;
        color = (max(col, vec3f(0.0)) * body_a + vec3f(glow)) * coverage;
    }
    let shade = shadow * (1.0 - coverage / max(alpha, 1e-4));
    return vec4f(color, cov_alpha) + vec4f(0.0, 0.0, 0.0, shade) * (1.0 - cov_alpha);
}
