// Composition d'un calque (étape 8) : le contenu du groupe sur la capture de ce qui est derrière, avec un
// mode de fusion. Mêmes formules que `veka_graphics::composite` (norme W3C Compositing and Blending).

struct Globals {
    viewport: vec2f,
    _pad: vec2f,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var layer: texture_2d<f32>;
@group(1) @binding(1) var backdrop: texture_2d<f32>;

struct GroupIn {
    @location(0) rect: vec4f,   // pixels : min.xy, max.xy
    @location(1) params: vec4f, // mode, opacité
};

struct GroupOut {
    @builtin(position) pos: vec4f,
    @location(0) @interpolate(flat) params: vec4f,
};

@vertex
fn group_vs(@builtin(vertex_index) vi: u32, i: GroupIn) -> GroupOut {
    var c = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    let px = mix(i.rect.xy, i.rect.zw, c[vi]);
    let n = px / globals.viewport * 2.0 - 1.0;
    var o: GroupOut;
    o.pos = vec4f(n.x, -n.y, 0.0, 1.0);
    o.params = i.params;
    return o;
}

fn encode1(c: f32) -> f32 {
    let x = clamp(c, 0.0, 1.0);
    if (x <= 0.0031308) { return x * 12.92; }
    return 1.055 * pow(x, 1.0 / 2.4) - 0.055;
}

fn decode1(c: f32) -> f32 {
    let x = clamp(c, 0.0, 1.0);
    if (x <= 0.04045) { return x / 12.92; }
    return pow((x + 0.055) / 1.055, 2.4);
}

fn encode(c: vec3f) -> vec3f { return vec3f(encode1(c.r), encode1(c.g), encode1(c.b)); }
fn decode(c: vec3f) -> vec3f { return vec3f(decode1(c.r), decode1(c.g), decode1(c.b)); }

fn lum(c: vec3f) -> f32 { return dot(c, vec3f(0.3, 0.59, 0.11)); }

fn clip_color(c0: vec3f) -> vec3f {
    var c = c0;
    let l = lum(c);
    let n = min(c.r, min(c.g, c.b));
    let x = max(c.r, max(c.g, c.b));
    if (n < 0.0) { c = vec3f(l) + (c - vec3f(l)) * l / max(l - n, 1e-6); }
    if (x > 1.0) { c = vec3f(l) + (c - vec3f(l)) * (1.0 - l) / max(x - l, 1e-6); }
    return c;
}

fn set_lum(c: vec3f, l: f32) -> vec3f { return clip_color(c + vec3f(l - lum(c))); }

fn sat(c: vec3f) -> f32 { return max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b)); }

fn set_sat(c: vec3f, s: f32) -> vec3f {
    let mx = max(c.r, max(c.g, c.b));
    let mn = min(c.r, min(c.g, c.b));
    if (mx > mn) { return (c - vec3f(mn)) * s / (mx - mn); }
    return vec3f(0.0);
}

fn hard_light1(b: f32, s: f32) -> f32 {
    if (s <= 0.5) { return b * 2.0 * s; }
    let s2 = 2.0 * s - 1.0;
    return b + s2 - b * s2;
}

fn dodge1(b: f32, s: f32) -> f32 {
    if (b <= 0.0) { return 0.0; }
    if (s >= 1.0) { return 1.0; }
    return min(b / (1.0 - s), 1.0);
}

fn burn1(b: f32, s: f32) -> f32 {
    if (b >= 1.0) { return 1.0; }
    if (s <= 0.0) { return 0.0; }
    return 1.0 - min((1.0 - b) / s, 1.0);
}

fn soft1(b: f32, s: f32) -> f32 {
    if (s <= 0.5) { return b - (1.0 - 2.0 * s) * b * (1.0 - b); }
    var d = sqrt(b);
    if (b <= 0.25) { d = ((16.0 * b - 12.0) * b + 4.0) * b; }
    return b + (2.0 * s - 1.0) * (d - b);
}

// B(cb, cs) d'un mode de mélange, couleurs non prémultipliées en sRGB.
fn mix_mode(m: i32, b: vec3f, s: vec3f) -> vec3f {
    switch m {
        case 1: { return b * s; }
        case 2: { return b + s - b * s; }
        case 3: { return vec3f(hard_light1(s.r, b.r), hard_light1(s.g, b.g), hard_light1(s.b, b.b)); }
        case 4: { return min(b, s); }
        case 5: { return max(b, s); }
        case 6: { return vec3f(dodge1(b.r, s.r), dodge1(b.g, s.g), dodge1(b.b, s.b)); }
        case 7: { return vec3f(burn1(b.r, s.r), burn1(b.g, s.g), burn1(b.b, s.b)); }
        case 8: { return vec3f(soft1(b.r, s.r), soft1(b.g, s.g), soft1(b.b, s.b)); }
        case 9: { return vec3f(hard_light1(b.r, s.r), hard_light1(b.g, s.g), hard_light1(b.b, s.b)); }
        case 10: { return abs(b - s); }
        case 11: { return b + s - 2.0 * b * s; }
        case 12: { return set_lum(set_sat(s, sat(b)), lum(b)); }
        case 13: { return set_lum(set_sat(b, sat(s)), lum(b)); }
        case 14: { return set_lum(s, lum(b)); }
        case 15: { return set_lum(b, lum(s)); }
        default: { return s; }
    }
}

fn unpremul(p: vec4f) -> vec3f {
    if (p.a > 0.0) { return p.rgb / p.a; }
    return vec3f(0.0);
}

@fragment
fn group_fs(i: GroupOut) -> @location(0) vec4f {
    let p = vec2i(i.pos.xy);
    let src = textureLoad(layer, p, 0) * i.params.y;
    // Là où le groupe n'a rien dessiné, le fond reste tel quel (pas de réécriture).
    if (src.a <= 0.0) { discard; }
    let dst = textureLoad(backdrop, p, 0);
    let m = i32(i.params.x + 0.5);
    let a_s = clamp(src.a, 0.0, 1.0);
    let ab = clamp(dst.a, 0.0, 1.0);
    // Composition alpha : en linéaire, comme le reste du rendu.
    if (m == 0) { return src + dst * (1.0 - a_s); }
    if (m == 16) { return vec4f(src.rgb * ab + dst.rgb * (1.0 - a_s), ab); }
    if (m == 17) { return dst + src * (1.0 - ab); }
    if (m == 18) { return dst * (1.0 - a_s); }
    // Mélange des couleurs : en sRGB.
    let cb = encode(unpremul(dst));
    let cs = encode(unpremul(src));
    var rgb: vec3f;
    var ao: f32;
    if (m == 20) {
        ao = min(a_s + ab, 1.0);
        rgb = min(cs * a_s + cb * ab, vec3f(1.0));
    } else if (m == 19) {
        ao = min(a_s + ab, 1.0);
        rgb = max(vec3f(ao) - min((vec3f(a_s) - cs * a_s) + (vec3f(ab) - cb * ab), vec3f(ao)), vec3f(0.0));
    } else {
        let bl = clamp(mix_mode(m, cb, cs), vec3f(0.0), vec3f(1.0));
        let cs2 = (1.0 - ab) * cs + ab * bl;
        ao = a_s + ab * (1.0 - a_s);
        rgb = a_s * cs2 + ab * cb * (1.0 - a_s);
    }
    if (ao <= 0.0) { return vec4f(0.0); }
    return vec4f(decode(rgb / ao) * ao, ao);
}
