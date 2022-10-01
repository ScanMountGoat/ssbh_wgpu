use criterion::{criterion_group, criterion_main, Criterion};
use ssbh_data::prelude::*;

use criterion::BenchmarkId;
use criterion::Throughput;
use ssbh_data::skel_data::BillboardType;
use ssbh_data::skel_data::BoneData;
use ssbh_wgpu::animation::animate_skel;
use ssbh_wgpu::animation::AnimationTransforms;

fn identity_bone(name: &str, parent_index: Option<usize>) -> BoneData {
    BoneData {
        name: name.to_string(),
        // Start with the identity to make this simpler.
        transform: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        parent_index,
        billboard_type: BillboardType::Disabled,
    }
}

fn animate_skel_roots_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("animate_skel_roots");
    for count in [1, 64, 128, 256, 512] {
        // This should just be a simple conversion between types.
        let skel = SkelData {
            major_version: 1,
            minor_version: 0,
            bones: vec![identity_bone("A", None); count],
        };

        let anim = AnimData {
            major_version: 2,
            minor_version: 0,
            final_frame_index: 0.0,
            groups: Vec::new(),
        };

        let mut transforms = AnimationTransforms::identity();

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                animate_skel(
                    &mut transforms,
                    &skel,
                    std::iter::once(&anim),
                    None,
                    0.0,
                    false,
                )
            });
        });
    }
    group.finish();
}

fn animate_skel_chain_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("animate_skel_chain");
    for count in [1, 64, 128, 256, 512] {
        // Test a worse case scenario for accumulating transforms.
        // TODO: Check with scale inheritance/compensation?
        let mut bones = Vec::new();
        bones.push(identity_bone("A", None));
        for i in 1..count {
            bones.push(identity_bone("A", Some(i - 1)));
        }
        let skel = SkelData {
            major_version: 1,
            minor_version: 0,
            bones,
        };

        let anim = AnimData {
            major_version: 2,
            minor_version: 0,
            final_frame_index: 0.0,
            groups: Vec::new(),
        };

        let mut transforms = AnimationTransforms::identity();

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                animate_skel(
                    &mut transforms,
                    &skel,
                    std::iter::once(&anim),
                    None,
                    0.0,
                    false,
                )
            });
        });
    }
    group.finish();
}

// TODO: Is this still O(N^2) if bones occur before their parents?

// TODO: Benchmark constraints.

criterion_group!(
    benches,
    animate_skel_roots_benchmark,
    animate_skel_chain_benchmark
);
criterion_main!(benches);
