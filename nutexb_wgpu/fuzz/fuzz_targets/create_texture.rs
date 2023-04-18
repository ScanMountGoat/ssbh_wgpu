#![no_main]
use arbitrary::Unstructured;
use futures::executor::block_on;
use libfuzzer_sys::fuzz_target;
use nutexb::NutexbFile;
use nutexb_wgpu::create_texture;
use once_cell::sync::Lazy;
use wgpu::{Device, DeviceDescriptor, Limits, PowerPreference, Queue, RequestAdapterOptions};

static SHARED: Lazy<(Device, Queue)> = Lazy::new(|| {
    // Load models in headless mode without a surface.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .unwrap();

    let (device, queue) = block_on(adapter.request_device(
        &DeviceDescriptor {
            label: None,
            features: wgpu::Features::TEXTURE_COMPRESSION_BC,
            limits: Limits::default(),
        },
        None,
    ))
    .unwrap();

    (device, queue)
});

fuzz_target!(|data: &[u8]| {
    let device = &SHARED.0;
    let queue = &SHARED.1;

    // TODO: arbitrary format?
    let mut u = Unstructured::new(data);
    let nutexb = NutexbFile {
        data: u.arbitrary().unwrap(),
        layer_mipmaps: Vec::new(),
        footer: nutexb::NutexbFooter {
            string: Vec::new().into(),
            width: u.arbitrary().unwrap(),
            height: u.arbitrary().unwrap(),
            depth: u.arbitrary().unwrap(),
            image_format: nutexb::NutexbFormat::B8G8R8A8Srgb,
            unk2: 1,
            mipmap_count: u.arbitrary().unwrap(),
            unk3: 1,
            layer_count: u.arbitrary().unwrap(),
            data_size: u.arbitrary().unwrap(),
            version: (1, 2),
        },
    };

    // TODO: How to free up WGPU memory?
    let _texture = create_texture(&nutexb, &device, &queue);

    device.poll(wgpu::Maintain::Wait);
});
