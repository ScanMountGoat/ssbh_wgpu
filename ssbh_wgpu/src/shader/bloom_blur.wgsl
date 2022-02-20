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

fn Blur(uvs: vec2<f32>) -> vec3<f32> {
    // Get a single texel offset.
    let offset = vec2<f32>(1.0) / vec2<f32>(textureDimensions(color_texture));

    // The blur kernel used for the first blur pass.
    // 1 2 1
    // 2 4 1
    // 1 2 1
    var result = textureSample(color_texture, color_sampler, uvs).rgb * 4.0;
    result = result + textureSample(color_texture, color_sampler, uvs + vec2<f32>(offset.x, 0.0)).rgb * 2.0;
    result = result + textureSample(color_texture, color_sampler, uvs + vec2<f32>(-offset.x, 0.0)).rgb * 2.0;
    result = result + textureSample(color_texture, color_sampler, uvs + vec2<f32>(0.0, offset.y)).rgb * 2.0;
    result = result + textureSample(color_texture, color_sampler, uvs + vec2<f32>(0.0, -offset.y)).rgb * 2.0;
    result = result + textureSample(color_texture, color_sampler, uvs + vec2<f32>(offset.x, offset.y)).rgb;
    result = result + textureSample(color_texture, color_sampler, uvs + vec2<f32>(offset.x, -offset.y)).rgb;
    result = result + textureSample(color_texture, color_sampler, uvs + vec2<f32>(-offset.x, offset.y)).rgb;
    result = result + textureSample(color_texture, color_sampler, uvs + vec2<f32>(-offset.x, -offset.y)).rgb;

    // The kernel weights are normalized to sum to 1.0.
    return result / 16.0;
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let color = textureSample(color_texture, color_sampler, in.uvs);
    return vec4<f32>(Blur(in.uvs), color.a);
}