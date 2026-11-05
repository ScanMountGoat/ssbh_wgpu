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

@group(0) @binding(2)
var outline_texture1: texture_2d<f32>;
@group(0) @binding(3)
var outline_texture2: texture_2d<f32>;

@group(0) @binding(4)
var material_mask_texture: texture_2d<f32>;

@group(0) @binding(5)
var skel_texture: texture_2d<f32>;

struct OverlaySettings {
    is_srgb: vec4<u32>
}

@group(0) @binding(6)
var<uniform> settings: OverlaySettings;

fn GetSrgb(colorLinear: f32) -> f32 {
    if colorLinear <= 0.00031308 {
        return 12.92 * colorLinear;
    } else {
        return 1.055 * pow(colorLinear, (1.0 / 2.4)) - 0.055;
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(color_texture, color_sampler, in.uvs.xy);
    let outline1 = textureSample(outline_texture1, color_sampler, in.uvs.xy).r;
    // TODO: Find a better way to handle the outline channels.
    let outline2 = textureSample(outline_texture2, color_sampler, in.uvs.xy).r;

    let material_mask = textureSample(material_mask_texture, color_sampler, in.uvs.xy);
    let skel = textureSample(skel_texture, color_sampler, in.uvs.xy);

    var output = color.rgb;
    output = mix(output, material_mask.rgb, material_mask.a);
    output = mix(output, skel.rgb, skel.a);
    // TODO: Set outline color?
    output = mix(output, vec3(0.0, 1.0, 1.0), outline1);
    output = mix(output, vec3(0.0, 0.0, 0.0), outline2);

    // The framebuffer won't always have an sRGB format.
    if settings.is_srgb.x != 1u {
        output = vec3(GetSrgb(output.x), GetSrgb(output.y), GetSrgb(output.z));
    }

    return vec4(output, color.a);
}