fn main() {
    // TODO: Only rerun if the shader change?
    let wgsl_source = std::fs::read_to_string("src/shader.wgsl").unwrap();

    // Generate the Rust bindings and write to a file.
    let text = &wgsl_to_wgpu::create_shader_module_embedded(
        &wgsl_source,
        wgsl_to_wgpu::WriteOptions {
            derive_bytemuck: true,
            ..Default::default()
        },
    )
    .unwrap();

    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{out_dir}/shader.rs"), text.as_bytes()).unwrap();
}
