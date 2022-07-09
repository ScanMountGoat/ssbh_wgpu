struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uvs: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    // A fullscreen triangle using index calculations.
    var out: VertexOutput;
    let x = f32((i32(in_vertex_index) << 1u) & 2);
    let y = f32(i32(in_vertex_index & 2u));
    out.position = vec4(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    out.uvs = vec2(x, 1.0 - y);
    return out;
}

@group(0) @binding(0)
var color_texture: texture_2d<f32>;
@group(0) @binding(1)
var color_sampler: sampler;

@group(0) @binding(2)
var color_lut: texture_3d<f32>;
@group(0) @binding(3)
var color_lut_sampler: sampler;

@group(0) @binding(4)
var bloom_texture: texture_2d<f32>;
@group(0) @binding(5)
var bloom_sampler: sampler;

fn GetPostProcessingResult(colorLinear: vec3<f32>) -> vec3<f32>
{
    let srgb = pow(colorLinear, vec3(0.4545449912548065));
    var result = srgb * 0.9375 + 0.03125;

    // Color Grading.
    result = textureSample(color_lut, color_lut_sampler, result).rgb;

    // Post Processing.
    result = (result - srgb) * 0.99961 + srgb;
    result = result * 1.3703;
    result = pow(result, vec3(2.2));
    return result;
}

// TODO: Is this the same computation as in game?
fn GetSrgb(colorLinear: f32) -> f32
{
    if (colorLinear <= 0.00031308) {
        return 12.92 * colorLinear;
    } else {
        return 1.055 * pow(colorLinear, (1.0 / 2.4)) - 0.055;
    }
}

fn GetSrgbVec3(colorLinear: vec3<f32>) -> vec3<f32> {
    return vec3(GetSrgb(colorLinear.x), GetSrgb(colorLinear.y), GetSrgb(colorLinear.z));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(color_texture, color_sampler, in.uvs);

    let bloom = textureSample(bloom_texture, bloom_sampler, in.uvs).rgb;
    var output = color.rgb + bloom;

    // Don't post process the background but still allow bloom.
    // TODO: Investigate how this is handled in game.
    output = mix(output, GetPostProcessingResult(output.rgb), color.a);

    // Assume an sRGB frame buffer and don't gamma correct here.
    return vec4(output, 1.0);
}