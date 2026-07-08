//! Skinning-palette build hot path.
//!
//! Mirrors `build_skeleton_palette` (crate-internal) using only public API so
//! the two per-frame paths can be compared directly:
//!   - `cached`   — reads the precomputed `Skeleton3D::inv_bind_mats` lane
//!   - `fallback` — recomputes `inv_bind.to_mat4()` per bone every frame
//!
//! Both produce the same 3-row affine palette the GPU consumes verbatim.
//! Run: cargo bench -p perro_runtime --bench skeletal_palette

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use glam::Mat4;
use perro_nodes::{Bone3D, Skeleton3D};
use perro_structs::{Quaternion, Transform3D, Vector3};

/// A parent-before-child bone chain with non-trivial pose + bind transforms.
fn make_skeleton(bone_count: usize) -> Skeleton3D {
    let mut skeleton = Skeleton3D::default();
    skeleton.bones = (0..bone_count)
        .map(|i| {
            let parent = if i == 0 { -1 } else { i as i32 - 1 };
            let pose = Transform3D::new(
                Vector3::new(0.13 * i as f32, 0.2, -0.1),
                Quaternion::new(0.0, 0.0, 0.382_683_43, 0.923_879_5),
                Vector3::ONE,
            );
            let inv_bind = Transform3D::new(
                Vector3::new(-0.05 * i as f32, 0.1, 0.2),
                Quaternion::new(0.0, 0.130_526_2, 0.0, 0.991_444_9),
                Vector3::new(1.0, 1.0, 1.0),
            );
            Bone3D {
                name: format!("bone{i}").into(),
                parent,
                rest: Transform3D::IDENTITY,
                pose,
                inv_bind,
            }
        })
        .collect();
    skeleton
}

#[inline]
fn pack_affine_rows(joint: &Mat4) -> [[f32; 4]; 3] {
    let c = joint.to_cols_array_2d();
    [
        [c[0][0], c[1][0], c[2][0], c[3][0]],
        [c[0][1], c[1][1], c[2][1], c[3][1]],
        [c[0][2], c[1][2], c[2][2], c[3][2]],
    ]
}

#[inline]
fn accumulate_global(skeleton: &Skeleton3D, global: &mut Vec<Mat4>) {
    global.clear();
    global.resize(skeleton.bones.len(), Mat4::IDENTITY);
    for (i, bone) in skeleton.bones.iter().enumerate() {
        let local = bone.pose.to_mat4();
        global[i] = if bone.parent >= 0 {
            let parent = bone.parent as usize;
            if parent < global.len() {
                global[parent] * local
            } else {
                local
            }
        } else {
            local
        };
    }
}

fn build_cached(skeleton: &Skeleton3D, global: &mut Vec<Mat4>, out: &mut Vec<[[f32; 4]; 3]>) {
    accumulate_global(skeleton, global);
    out.clear();
    let inv_bind_mats = skeleton.inv_bind_mats();
    for (i, _bone) in skeleton.bones.iter().enumerate() {
        out.push(pack_affine_rows(&(global[i] * inv_bind_mats[i].0)));
    }
}

fn build_fallback(skeleton: &Skeleton3D, global: &mut Vec<Mat4>, out: &mut Vec<[[f32; 4]; 3]>) {
    accumulate_global(skeleton, global);
    out.clear();
    for (i, bone) in skeleton.bones.iter().enumerate() {
        out.push(pack_affine_rows(&(global[i] * bone.inv_bind.to_mat4())));
    }
}

fn bench_palette(c: &mut Criterion) {
    let mut group = c.benchmark_group("skeletal_palette_build");
    for &bone_count in &[24usize, 64, 160] {
        let mut skeleton = make_skeleton(bone_count);
        skeleton.refresh_inv_bind_cache();
        let mut global = Vec::new();
        let mut out = Vec::new();

        group.bench_with_input(
            BenchmarkId::new("cached_inv_bind_lane", bone_count),
            &bone_count,
            |b, _| {
                b.iter(|| {
                    build_cached(black_box(&skeleton), &mut global, &mut out);
                    black_box(out.len())
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("inline_to_mat4", bone_count),
            &bone_count,
            |b, _| {
                b.iter(|| {
                    build_fallback(black_box(&skeleton), &mut global, &mut out);
                    black_box(out.len())
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_palette);
criterion_main!(benches);
