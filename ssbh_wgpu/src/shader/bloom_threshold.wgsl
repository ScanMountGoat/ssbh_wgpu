struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] uvs: vec2<f32>;
};

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] in_vertex_index: u32) -> VertexOutput {
    // A fullscreen triangle using index calculations.
    var out: VertexOutput;
    let x = f32((i32(in_vertex_index) << 1u) & 2);
    let y = f32(i32(in_vertex_index & 2u));
    out.position = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    out.uvs = vec2<f32>(x, 1.0 - y);
    return out;
}

[[group(0), binding(0)]]
var color_texture: texture_2d<f32>;
[[group(0), binding(1)]]
var color_sampler: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    // Ported bloom code from fighter shaders.
    // Uniform values are hardcoded for now.
    // TODO: Where do these uniform buffer values come from?
    let color = textureSample(color_texture, color_sampler, in.uvs);
    let componentMax = max(max(color.r, max(color.g, color.b)), 0.001);
    let scale = 1.0 / componentMax;
    let scale2 = max(0.925 * -0.5 + componentMax, 0.0);

    return vec4<f32>(color.rgb * scale * scale2 * 6.0, color.a);
}