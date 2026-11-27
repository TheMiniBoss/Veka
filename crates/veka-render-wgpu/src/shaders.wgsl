// Shaders du renderer wgpu de Veka. Les formules SDF sont les mêmes que `veka-graphics/src/shape.rs`.

struct Globals {
    // Taille de la cible en pixels.
    viewport: vec2f,
    _pad: vec2f,
};

@group(0) @binding(0) var<uniform> globals: Globals;

fn to_clip(px: vec2f) -> vec4f {
    let n = px / globals.viewport * 2.0 - 1.0;
    return vec4f(n.x, -n.y, 0.0, 1.0);
}

// Deux triangles : 0 1 2, 2 1 3 → coins (0,0) (1,0) (0,1) (1,1).
fn corner(vi: u32) -> vec2f {
    var c = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    return c[vi];
}

// ---------------------------------------------------------------- Formes

struct ShapeIn {
    @location(0) rect: vec4f,     // x, y, largeur, hauteur (repère local, points)
    @location(1) radii: vec4f,    // haut-gauche, haut-droite, bas-droite, bas-gauche
    @location(2) color: vec4f,    // linéaire, prémultiplié
    @location(3) xform: vec4f,    // a, b, c, d  (local → pixels)
    @location(4) extra: vec4f,    // tx, ty, sorte (0 boîte, 1 ellipse), épaisseur du trait (0 = remplissage)
    @location(5) fill: vec4f,     // remplissage (0 uni, 1 linéaire, 2 radial, 3 elliptique, 4 angulaire, 5 maillé), rampe x, rampe v
    @location(6) geo: vec4f,      // géométrie du dégradé (repère local)
    @location(7) geo2: vec4f,
};

struct ShapeOut {
    @builtin(position) pos: vec4f,
    @location(0) local: vec2f,
    @location(1) @interpolate(flat) rect: vec4f,
    @location(2) @interpolate(flat) radii: vec4f,
    @location(3) @interpolate(flat) color: vec4f,
    @location(4) @interpolate(flat) kind_stroke: vec2f,
    @location(5) @interpolate(flat) fill: vec4f,
    @location(6) @interpolate(flat) geo: vec4f,
    @location(7) @interpolate(flat) geo2: vec4f,
};

@group(1) @binding(0) var ramps: texture_2d<f32>;
@group(1) @binding(1) var ramps_sampler: sampler;

const RAMPS_SIZE: f32 = 1024.0;
const RAMP_LEN: f32 = 256.0;
const TAU: f32 = 6.28318530718;

@vertex
fn shape_vs(@builtin(vertex_index) vi: u32, i: ShapeIn) -> ShapeOut {
    let a = i.xform.x; let b = i.xform.y; let c = i.xform.z; let d = i.xform.w;
    // Marge pour l'antialiasing (≈ 2 px) et la moitié du trait.
    let px_scale = max(sqrt(abs(a * d - b * c)), 1e-4);
    let margin = 2.0 / px_scale + i.extra.w * 0.5;
    let lo = i.rect.xy - vec2f(margin);
    let size = i.rect.zw + vec2f(2.0 * margin);
    let local = lo + corner(vi) * size;
    let px = vec2f(a * local.x + c * local.y + i.extra.x, b * local.x + d * local.y + i.extra.y);
    var o: ShapeOut;
    o.pos = to_clip(px);
    o.local = local;
    o.rect = i.rect;
    o.radii = i.radii;
    o.color = i.color;
    o.kind_stroke = i.extra.zw;
    o.fill = i.fill;
    o.geo = i.geo;
    o.geo2 = i.geo2;
    return o;
}

// Couleur d'un dégradé au point `p` (repère local). Linéaire et prémultipliée, lue dans l'atlas.
fn gradient(p: vec2f, fill: vec4f, geo: vec4f, geo2: vec4f) -> vec4f {
    let kind = i32(fill.x + 0.5);
    if (kind == 5) {
        let uv = (p - geo.xy) / max(geo.zw, vec2f(1e-4));
        let at = mix(geo2.xy, geo2.zw, clamp(uv, vec2f(0.0), vec2f(1.0)));
        return textureSampleLevel(ramps, ramps_sampler, at, 0.0);
    }
    var t = 0.0;
    if (kind == 1) {
        let d = geo.zw - geo.xy;
        t = dot(p - geo.xy, d) / max(dot(d, d), 1e-6);
    } else if (kind == 2) {
        t = (length(p - geo.xy) - geo.z) / max(geo.w - geo.z, 1e-4);
    } else if (kind == 3) {
        let r = length((p - geo.xy) / max(geo.zw, vec2f(1e-4)));
        t = (r - geo2.x) / max(geo2.y - geo2.x, 1e-4);
    } else if (kind == 4) {
        let d = p - geo.xy;
        // Sens horaire depuis +X (y vers le bas).
        let a = atan2(d.y, d.x) - geo.z;
        let turn = a - floor(a / TAU) * TAU;
        t = turn / max(abs(geo.w), 1e-4);
    }
    t = clamp(t, 0.0, 1.0);
    let u = (fill.y + 0.5 + t * (RAMP_LEN - 1.0)) / RAMPS_SIZE;
    return textureSampleLevel(ramps, ramps_sampler, vec2f(u, fill.z), 0.0);
}

fn sd_round_box(p: vec2f, half: vec2f, radii: vec4f) -> f32 {
    var r = radii.x;                       // haut-gauche
    if (p.x >= 0.0 && p.y < 0.0) { r = radii.y; }   // haut-droite
    if (p.x >= 0.0 && p.y >= 0.0) { r = radii.z; }  // bas-droite
    if (p.x < 0.0 && p.y >= 0.0) { r = radii.w; }   // bas-gauche
    let q = abs(p) - half + vec2f(r);
    return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn sd_ellipse(p: vec2f, half: vec2f) -> f32 {
    if (half.x <= 0.0 || half.y <= 0.0) { return length(p); }
    return (length(p / half) - 1.0) * min(half.x, half.y);
}

@fragment
fn shape_fs(i: ShapeOut) -> @location(0) vec4f {
    let half = i.rect.zw * 0.5;
    let p = i.local - (i.rect.xy + half);
    var d: f32;
    if (i.kind_stroke.x > 0.5) { d = sd_ellipse(p, half); } else { d = sd_round_box(p, half, i.radii); }
    if (i.kind_stroke.y > 0.0) { d = abs(d) - i.kind_stroke.y * 0.5; }
    let w = max(fwidth(d), 1e-4);
    let coverage = clamp(0.5 - d / w, 0.0, 1.0);
    var color = i.color;
    if (i.fill.x > 0.5) {
        color = gradient(i.local, i.fill, i.geo, i.geo2) * i.color;
    }
    return color * coverage;
}

// ---------------------------------------------------------------- Glyphes

@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct GlyphIn {
    @location(0) rect: vec4f,   // pixels
    @location(1) uv: vec4f,     // u0, v0, u1, v1
    @location(2) color: vec4f,  // linéaire, prémultiplié
};

struct GlyphOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) @interpolate(flat) color: vec4f,
};

@vertex
fn glyph_vs(@builtin(vertex_index) vi: u32, i: GlyphIn) -> GlyphOut {
    let c = corner(vi);
    var o: GlyphOut;
    o.pos = to_clip(i.rect.xy + c * i.rect.zw);
    o.uv = mix(i.uv.xy, i.uv.zw, c);
    o.color = i.color;
    return o;
}

@fragment
fn glyph_fs(i: GlyphOut) -> @location(0) vec4f {
    let coverage = textureSample(atlas, atlas_sampler, i.uv).r;
    return i.color * coverage;
}

