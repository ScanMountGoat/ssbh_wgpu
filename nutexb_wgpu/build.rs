fn main() {
    println!("cargo:rerun-if-changed=src/shader.wgsl");

    let wgsl_source = std::fs::read_to_string("src/shader.wgsl").unwrap();

    // Generate the Rust bindings and write to a file.
    let text = &wgsl_to_wgpu::create_shader_modules(
        &wgsl_source,
        wgsl_to_wgpu::WriteOptions {
            derive_bytemuck_vertex: true,
            derive_bytemuck_host_shareable: true,
            ..Default::default()
        },
        wgsl_to_wgpu::demangle_identity,
    )
    .unwrap();

    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{out_dir}/shader.rs"), text.as_bytes()).unwrap();
}
