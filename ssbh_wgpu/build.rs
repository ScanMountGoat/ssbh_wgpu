use std::fmt::Write;

fn write_shader_module(wgsl_source: &str, shader_name: &str) {
    // Generate the Rust bindings and write to a file.
    let text = wgsl_to_wgpu::create_shader_module_embedded(
        &wgsl_source,
        wgsl_to_wgpu::WriteOptions {
            derive_bytemuck: true,
            derive_encase: true,
            matrix_vector_types: wgsl_to_wgpu::MatrixVectorTypes::Glam,
            ..Default::default()
        },
    )
    .unwrap();

    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{out_dir}/{shader_name}.rs"), text.as_bytes()).unwrap();
}

fn main() {
    // TODO: Only rerun if the shaders change?
    let mut shader_paths: Vec<_> = std::fs::read_dir("src/shader")
        .unwrap()
        .filter_map(|p| Some(p.ok()?.path()))
        .filter(|p| p.extension().unwrap().to_string_lossy() == "wgsl")
        .collect();

    // Use alphabetical order for consistency.
    shader_paths.sort();

    let mut f = String::new();
    writeln!(&mut f, "// File automatically generated by build.rs.").unwrap();
    writeln!(&mut f, "// Changes made to this file will not be saved.").unwrap();

    // Create each shader module and add it to shader.rs.
    for shader_path in shader_paths {
        let file_name = shader_path.with_extension("");
        let shader_name = file_name.file_name().unwrap().to_string_lossy().to_string();

        writeln!(&mut f, "pub mod {shader_name} {{").unwrap();
        writeln!(
            &mut f,
            r#"    include!(concat!(env!("OUT_DIR"), "/{shader_name}.rs"));"#
        )
        .unwrap();
        writeln!(&mut f, "}}").unwrap();

        let wgsl_source = std::fs::read_to_string(shader_path).unwrap();
        write_shader_module(&wgsl_source, &shader_name);
    }

    std::fs::write("src/shader.rs", f.as_bytes()).unwrap();
}
