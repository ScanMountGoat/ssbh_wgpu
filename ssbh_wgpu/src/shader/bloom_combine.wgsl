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
var bloom0_texture: texture_2d<f32>;
@group(0) @binding(1)
var bloom1_texture: texture_2d<f32>;
@group(0) @binding(2)
var bloom2_texture: texture_2d<f32>;
@group(0) @binding(3)
var bloom3_texture: texture_2d<f32>;
@group(0) @binding(4)
var bloom_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let bloom0 = textureSample(bloom0_texture, bloom_sampler, in.uvs.xy);
    let bloom1 = textureSample(bloom1_texture, bloom_sampler, in.uvs.xy);
    let bloom2 = textureSample(bloom2_texture, bloom_sampler, in.uvs.xy);
    let bloom3 = textureSample(bloom3_texture, bloom_sampler, in.uvs.xy);

    let weights = array<f32, 4>(0.32, 0.10, 0.20, 0.25);
    let bloom_total = bloom0.rgb * weights[0] + bloom1.rgb * weights[1] + bloom2.rgb * weights[2] + bloom3.rgb * weights[3];
    let clamped_bloom = clamp(bloom_total, vec3(0.0), vec3(1.0));
    let bloom_contribution = pow(clamped_bloom, vec3(2.2));

    return vec4(bloom_contribution, 1.0);
}