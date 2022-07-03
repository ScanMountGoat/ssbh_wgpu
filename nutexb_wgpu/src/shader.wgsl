struct VertexInput {
    [[location(0)]] position: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] tex_coords: vec2<f32>;
};

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] in_vertex_index: u32) -> VertexOutput {
    // A fullscreen triangle using index calculations.
    var out: VertexOutput;
    let x = f32((i32(in_vertex_index) << 1u) & 2);
    let y = f32(i32(in_vertex_index & 2u));
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    out.tex_coords = vec2<f32>(x, 1.0 - y);
    return out;
}

struct RenderSettings {
    render_rgba: vec4<f32>;
    mipmap: vec4<f32>;
    layer: vec4<f32>;
};

// TODO: Separate wgsl files for cube, 2d, etc.
[[group(0), binding(0)]]
var t_diffuse: texture_2d<f32>;
[[group(0), binding(1)]]
var s_diffuse: sampler;
[[group(0), binding(2)]]
var<uniform> render_settings: RenderSettings;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    // TODO: Sample array layers for cube maps in a separate entry point.
    let outColor = textureSampleLevel(t_diffuse, s_diffuse, in.tex_coords, render_settings.mipmap.x);

    // Use grayscale for single channels.
    let rgba = render_settings.render_rgba;
    if (rgba.r == 1.0 && rgba.g == 0.0 && rgba.b == 0.0) {
        return vec4<f32>(outColor.rrr, 1.0);
    }

    if (rgba.r == 0.0 && rgba.g == 1.0 && rgba.b == 0.0) {
        return vec4<f32>(outColor.ggg, 1.0);
    }

    if (rgba.r == 0.0 && rgba.g == 0.0 && rgba.b == 1.0) {
        return vec4<f32>(outColor.bbb, 1.0);
    }

    if (rgba.a == 1.0 && rgba.r == 0.0 && rgba.g == 0.0 && rgba.b == 0.0) {
        return vec4<f32>(outColor.aaa, 1.0);
    }

    return vec4<f32>(outColor.rgb * rgba.rgb, 1.0);
}