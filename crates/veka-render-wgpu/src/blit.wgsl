// Copie de la texture intermédiaire vers la fenêtre.


@group(0) @binding(0) var blit_src: texture_2d<f32>;
@group(0) @binding(1) var blit_sampler: sampler;

struct BlitOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn blit_vs(@builtin(vertex_index) vi: u32) -> BlitOut {
    // Un seul triangle qui couvre l'écran.
    let uv = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u));
    var o: BlitOut;
    o.pos = vec4f(uv * vec2f(2.0, -2.0) + vec2f(-1.0, 1.0), 0.0, 1.0);
    o.uv = uv;
    return o;
}

@fragment
fn blit_fs(i: BlitOut) -> @location(0) vec4f {
    return textureSample(blit_src, blit_sampler, i.uv);
}
