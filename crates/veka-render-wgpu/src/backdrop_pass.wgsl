// Lecture de l'arrière-plan (étape 6) : la pyramide de flou.
//
// La pyramide couvre toute la fenêtre ; seule la zone capturée est calculée (découpage). Le niveau k a
// pour taille ceil(taille / 2^k), et son texel j correspond aux pixels [j·2^k, (j+1)·2^k) de la fenêtre.
// Niveau 0 : copie nette. Niveaux 1… : réduction (moyenne 2×2) depuis le niveau précédent, puis flou
// gaussien séparable (σ ≈ 1,5 texel, donc ≈ 1,6 · 2^k pixels en tout).

// ---------------------------------------------------------------- Passes de la pyramide

struct Pass {
    // Taille de la texture lue, en texels.
    src_size: vec2f,
    // Direction du flou gaussien (1, 0) ou (0, 1) ; (0, 0) pour une réduction.
    dir: vec2f,
    // Zone lisible dans la texture lue, en texels : min.xy, max.xy.
    bounds: vec4f,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(1) @binding(0) var<uniform> pass_u: Pass;

@vertex
fn full_vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
    let uv = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u));
    return vec4f(uv * vec2f(2.0, -2.0) + vec2f(-1.0, 1.0), 0.0, 1.0);
}

// Lit `src` au point `t` (en texels), sans sortir de la zone capturée.
fn tap(t: vec2f) -> vec4f {
    let c = clamp(t, pass_u.bounds.xy + vec2f(0.5), pass_u.bounds.zw - vec2f(0.5));
    return textureSampleLevel(src, src_sampler, c / pass_u.src_size, 0.0);
}

// Réduction : le texel de sortie couvre 2×2 texels de l'entrée (lus d'un seul tap bilinéaire au coin
// commun, plus les quatre centres pour adoucir : noyau de Kawase).
@fragment
fn down_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let c = pos.xy * 2.0;
    var sum = tap(c) * 4.0;
    sum += tap(c + vec2f(-0.5, -0.5));
    sum += tap(c + vec2f(0.5, -0.5));
    sum += tap(c + vec2f(-0.5, 0.5));
    sum += tap(c + vec2f(0.5, 0.5));
    return sum / 8.0;
}

// Flou gaussien d'un niveau (σ ≈ 1,5 texel), sur 9 prises.
@fragment
fn gauss_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let w = array<f32, 5>(0.2270, 0.1945, 0.1216, 0.0540, 0.0162);
    var sum = tap(pos.xy) * w[0];
    for (var i = 1; i < 5; i++) {
        let o = pass_u.dir * f32(i);
        sum += (tap(pos.xy + o) + tap(pos.xy - o)) * w[i];
    }
    return sum / (w[0] + 2.0 * (w[1] + w[2] + w[3] + w[4]));
}

