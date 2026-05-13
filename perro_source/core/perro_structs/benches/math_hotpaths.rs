use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};

const N: usize = 16_384;

fn make_vec2s() -> Vec<Vector2> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Vector2::new(f.mul_add(0.013, 1.0), f.mul_add(0.021, 2.0))
        })
        .collect()
}

fn make_vec3s() -> Vec<Vector3> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Vector3::new(
                f.mul_add(0.011, 1.0),
                f.mul_add(0.017, 2.0),
                f.mul_add(0.023, 3.0),
            )
        })
        .collect()
}

fn make_quats() -> Vec<Quaternion> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Quaternion::from_euler_xyz(f * 0.0003, f * 0.0005, f * 0.0007)
        })
        .collect()
}

fn make_transforms_2d() -> Vec<Transform2D> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Transform2D::new(
                Vector2::new(f * 0.125, f * -0.25),
                f * 0.0009,
                Vector2::new(1.0 + (i % 7) as f32 * 0.01, 1.0 + (i % 11) as f32 * 0.01),
            )
        })
        .collect()
}

fn make_transforms_3d() -> Vec<Transform3D> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Transform3D::new(
                Vector3::new(f * 0.125, f * -0.25, f * 0.03125),
                Quaternion::from_euler_xyz(f * 0.0003, f * 0.0005, f * 0.0007),
                Vector3::new(
                    1.0 + (i % 7) as f32 * 0.01,
                    1.0 + (i % 11) as f32 * 0.01,
                    1.0 + (i % 13) as f32 * 0.01,
                ),
            )
        })
        .collect()
}

fn bench_vector2_bulk_ops(c: &mut Criterion) {
    let a = make_vec2s();
    let b = make_vec2s();

    c.bench_function("perro_structs/vector2_bulk_ops", |bench| {
        bench.iter(|| {
            let mut acc = Vector2::ZERO;
            let mut scalar = 0.0f32;
            for (&lhs, &rhs) in black_box(a.iter().zip(&b)) {
                let mixed = (lhs + rhs) * 0.5 - rhs.normalized();
                scalar += lhs.dot(rhs) + lhs.cross(rhs) + lhs.distance_to(rhs);
                acc += mixed.lerped(rhs, 0.25);
            }
            black_box((acc, scalar))
        })
    });
}

fn bench_vector3_bulk_ops(c: &mut Criterion) {
    let a = make_vec3s();
    let b = make_vec3s();

    c.bench_function("perro_structs/vector3_bulk_ops", |bench| {
        bench.iter(|| {
            let mut acc = Vector3::ZERO;
            let mut scalar = 0.0f32;
            for (&lhs, &rhs) in black_box(a.iter().zip(&b)) {
                let mixed = (lhs + rhs) * 0.5 - rhs.normalized();
                scalar += lhs.dot(rhs) + lhs.cross(rhs).length() + lhs.distance_to(rhs);
                acc += mixed.lerped(rhs.project_on(lhs), 0.25);
            }
            black_box((acc, scalar))
        })
    });
}

fn bench_quaternion_bulk_ops(c: &mut Criterion) {
    let quats = make_quats();
    let vectors = make_vec3s();

    c.bench_function("perro_structs/quaternion_bulk_ops", |bench| {
        bench.iter(|| {
            let mut acc = Quaternion::IDENTITY;
            let mut vec_acc = Vector3::ZERO;
            let mut scalar = 0.0f32;
            for (&q, &v) in black_box(quats.iter().zip(&vectors)) {
                let mixed = acc.mul_quat(q).normalized();
                vec_acc += mixed.rotate_vector3(v);
                scalar += mixed.dot(q) + mixed.inverse().dot(acc);
                acc = mixed.slerped(q, 0.35);
            }
            black_box((acc, vec_acc, scalar))
        })
    });
}

fn bench_quaternion_lerp_modes(c: &mut Criterion) {
    let quats = make_quats();
    let mut group = c.benchmark_group("perro_structs/quaternion_lerp_modes");

    group.bench_function("slerp", |bench| {
        bench.iter(|| {
            let mut acc = Quaternion::IDENTITY;
            for &q in black_box(&quats) {
                acc = acc.slerped(q, 0.35);
            }
            black_box(acc)
        })
    });

    group.bench_function("nlerp", |bench| {
        bench.iter(|| {
            let mut acc = Quaternion::IDENTITY;
            for &q in black_box(&quats) {
                acc = acc.nlerped(q, 0.35);
            }
            black_box(acc)
        })
    });

    group.finish();
}

fn bench_transform2d_mat_roundtrip(c: &mut Criterion) {
    let transforms = make_transforms_2d();

    c.bench_function("perro_structs/transform2d_mat_roundtrip", |bench| {
        bench.iter(|| {
            let mut acc = Vector2::ZERO;
            let mut rot = 0.0f32;
            for &transform in black_box(&transforms) {
                let rebuilt = Transform2D::from_mat3(transform.to_mat3());
                acc += rebuilt.position + rebuilt.scale;
                rot += rebuilt.rotation;
            }
            black_box((acc, rot))
        })
    });
}

fn bench_transform3d_mat_roundtrip(c: &mut Criterion) {
    let transforms = make_transforms_3d();

    c.bench_function("perro_structs/transform3d_mat_roundtrip", |bench| {
        bench.iter(|| {
            let mut acc = Vector3::ZERO;
            let mut scalar = 0.0f32;
            for &transform in black_box(&transforms) {
                let rebuilt = Transform3D::from_mat4(transform.to_mat4());
                acc += rebuilt.position + rebuilt.scale;
                scalar += rebuilt.rotation.dot(transform.rotation);
            }
            black_box((acc, scalar))
        })
    });
}

criterion_group!(
    benches,
    bench_vector2_bulk_ops,
    bench_vector3_bulk_ops,
    bench_quaternion_bulk_ops,
    bench_quaternion_lerp_modes,
    bench_transform2d_mat_roundtrip,
    bench_transform3d_mat_roundtrip
);
criterion_main!(benches);
