// Draw an infinite grid on the XZ-plane based on the blogpost here:
// http://asliceofrendering.com/scene%20helper/2020/01/05/InfiniteGrid/
struct VertexInput {
    @location(0) position: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) near_point: vec4<f32>,
    @location(1) far_point: vec4<f32>,
};

// TODO: Investigate using encase to set the depth here.
struct FragmentOutput {
    @builtin(frag_depth) depth: f32,
    @location(0) color: vec4<f32>
};

struct CameraTransforms {
    model_view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    mvp_matrix: mat4x4<f32>,
    mvp_inv_matrix: mat4x4<f32>,
    camera_pos: vec4<f32>,
    screen_dimensions: vec4<f32>, // width, height, scale, _
};

@group(0) @binding(0)
var<uniform> camera: CameraTransforms;

// TODO: use consistent naming conventions for functions
fn unproject_point(x: f32, y: f32, z: f32) -> vec4<f32> {
    let unprojected_point = camera.mvp_inv_matrix * vec4(x, y, z, 1.0);
    return vec4(unprojected_point.xyz / unprojected_point.w, 1.0);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4(in.position.xyz, 1.0);
    out.near_point = unproject_point(in.position.x, in.position.y, 0.0);
    out.far_point = unproject_point(in.position.x, in.position.y, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    // TODO: Can this all be moved to the vertex shader?
    let t = -in.near_point.y / (in.far_point.y - in.near_point.y);

    let position = in.near_point.xyz + t * (in.far_point.xyz - in.near_point.xyz);

    // TODO: Set the depth so this can render with the model pass.

    // Calculate linear depth values.
    // https://learnopengl.com/Advanced-OpenGL/Depth-Testing
    // TODO: Add these to the camera uniforms.
    let near = 1.0;
    let far = 400000.0;
    let clip_pos = camera.mvp_matrix * vec4(position, 1.0);
    let clip_depth = (clip_pos.z / clip_pos.w);
    let clip_depth_remapped = clip_depth * 2.0 - 1.0;
    let linear_depth = (2.0 * near * far) / (far + near - clip_depth_remapped * (far - near));
    let linear_depth_normalized = linear_depth / far;

    // TODO: Should this scale with the screen scale factor?
    let scale = 10.0;
    let coord = position.xz / scale;
    let derivative = fwidth(coord);
    let grid = abs(fract(coord - 0.5) - 0.5) / derivative;
    let grid_line = min(grid.x, grid.y);
    let minz = min(derivative.y, 1.0);
    let minx = min(derivative.x, 1.0);

    var out: FragmentOutput;
    out.color = vec4(0.0);
    out.depth = clip_depth;

    // Restrict the grid above the XZ-plane.
    if t > 0.0 {
        // Increase the depth fade since the far clip distance is so large.
        let depth_fade = max(1.0 - linear_depth_normalized * 500.0, 0.0);
        var alpha = max(1.0 - grid_line, 0.0) * depth_fade * 0.5;
        var color = vec3(1.0);

        // Remove a bright line at the horizon when t approaches 1.0.
        alpha = alpha * (1.0 - abs(t));

        // x-axis
        if position.z > -scale * minz && position.z < scale * minz {
            color = vec3(1.0, 0.0, 0.0);
        }

        // z-axis.
        if position.x > -scale * minx && position.x < scale * minx {
            color = vec3(0.0, 0.0, 1.0);
        }

        // Premultiplied alpha.
        out.color = vec4(color * alpha, alpha);
    }

    return out;
}