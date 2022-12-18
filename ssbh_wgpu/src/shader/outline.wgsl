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
var color_texture: texture_2d<f32>;
@group(0) @binding(1)
var color_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // TODO: Handle color?
    let dim = textureDimensions(color_texture);
    let texel = vec2<i32>(in.uvs.xy * vec2<f32>(dim));

    // Expand the silhouette by 2 pixel.
    // TODO: Is this more efficient as a compute shader?
    // Check alpha to avoid needing separate silhouette pipelines.
    let left2 = textureLoad(color_texture, texel + vec2(-2, 0), 0).a;
    let left1 = textureLoad(color_texture, texel + vec2(-1, 0), 0).a;
    let center = textureLoad(color_texture, texel, 0).a;
    let right1 = textureLoad(color_texture, texel + vec2(1, 0), 0).a;
    let right2 = textureLoad(color_texture, texel + vec2(2, 0), 0).a;

    let top2 = textureLoad(color_texture, texel + vec2(0, 2), 0).a;
    let top1 = textureLoad(color_texture, texel + vec2(0, 1), 0).a;
    let bottom1 = textureLoad(color_texture, texel + vec2(0, -1), 0).a;
    let bottom2 = textureLoad(color_texture, texel + vec2(0, -2), 0).a;

    let expanded = left2 + left1 + center + right1 + right2 + top2 + top1 + bottom1 + bottom2;
    return vec4(expanded);
}