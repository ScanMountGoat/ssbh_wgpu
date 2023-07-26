struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uvs: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    // A fullscreen triangle using index calculations.
    var out: VertexOutput;
    let x = f32((i32(in_vertex_index) << 1u) & 2);
    let y = f32(i32(in_vertex_index & 2u));
    out.position = vec4(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    out.uvs = vec4(x, 1.0 - y, 0.0, 0.0);
    return out;
}

@group(0) @binding(0)
var texture_shadow: texture_depth_2d;
@group(0) @binding(1)
var sampler_shadow: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate an approximation of the first two moments M1 and M2.
    // M1 is the mean, and M2 is the square of M1.
    // This enables calculating smooth variance shadows in the model shader.
    let samples = textureGather(texture_shadow, sampler_shadow, in.uvs.xy);
    let m1 = (samples.x + samples.y + samples.y + samples.w) / 4.0;
    return vec4(m1, m1 * m1, 0.0, 0.0);
}
