fn main() {
    // TODO: Only rerun if the shaders change?
    // TODO: Apply this automatically to all wgsl files in directory?
    wgsl_to_wgpu::write_module_file("src/shader/model.rs", "src/shader/model.wgsl", "model.wgsl");
    wgsl_to_wgpu::write_module_file(
        "src/shader/post_process.rs",
        "src/shader/post_process.wgsl",
        "post_process.wgsl",
    );
    wgsl_to_wgpu::write_module_file(
        "src/shader/bloom_blur.rs",
        "src/shader/bloom_blur.wgsl",
        "bloom_blur.wgsl",
    );
    wgsl_to_wgpu::write_module_file(
        "src/shader/bloom_threshold.rs",
        "src/shader/bloom_threshold.wgsl",
        "bloom_threshold.wgsl",
    );
    wgsl_to_wgpu::write_module_file(
        "src/shader/bloom_combine.rs",
        "src/shader/bloom_combine.wgsl",
        "bloom_combine.wgsl",
    );
    wgsl_to_wgpu::write_module_file(
        "src/shader/bloom_upscale.rs",
        "src/shader/bloom_upscale.wgsl",
        "bloom_upscale.wgsl",
    );
}
