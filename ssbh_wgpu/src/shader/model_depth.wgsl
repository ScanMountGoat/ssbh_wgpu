struct CameraTransforms {
    mvp_matrix: mat4x4<f32>;
};

[[group(0), binding(0)]]
var<uniform> camera: CameraTransforms;

// TODO: Move this into model.wgsl to avoid duplicating definitions.
struct VertexInput0 {
    [[location(0)]] position0: vec4<f32>;
    [[location(1)]] normal0: vec4<f32>;
    [[location(2)]] tangent0: vec4<f32>;
};

struct VertexInput1 {
    [[location(3)]] map1_uvset: vec4<f32>;
    [[location(4)]] uv_set1_uv_set2: vec4<f32>;
    [[location(5)]] bake1: vec4<f32>;
    [[location(6)]] color_set1: vec4<f32>;
    [[location(7)]] color_set2_combined: vec4<f32>;
    [[location(8)]] color_set3: vec4<f32>;
    [[location(9)]] color_set4: vec4<f32>;
    [[location(10)]] color_set5: vec4<f32>;
    [[location(11)]] color_set6: vec4<f32>;
    [[location(12)]] color_set7: vec4<f32>;
};

[[stage(vertex)]]
fn vs_main(
    buffer0: VertexInput0,
    buffer1: VertexInput1
) -> [[builtin(position)]] vec4<f32> {
    return camera.mvp_matrix * vec4<f32>(buffer0.position0.xyz, 1.0);
}