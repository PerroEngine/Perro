use super::*;

pub fn simplify_trimesh_data(
    vertices: Vec<na3::Point3<f32>>,
    triangles: Vec<[u32; 3]>,
) -> Option<TriMeshData> {
    let (vertices, triangles) = weld_and_filter_mesh(vertices, triangles)?;
    if let Some((reduced_vertices, reduced_triangles)) =
        simplify_coplanar_mesh(&vertices, &triangles)
    {
        return weld_and_filter_mesh(reduced_vertices, reduced_triangles);
    }
    Some((vertices, triangles))
}

pub fn weld_and_filter_mesh(
    vertices: Vec<na3::Point3<f32>>,
    triangles: Vec<[u32; 3]>,
) -> Option<TriMeshData> {
    let mut remap = vec![0u32; vertices.len()];
    let mut map = AHashMap::<(i64, i64, i64), u32>::default();
    let mut out_vertices = Vec::<na3::Point3<f32>>::new();
    let eps = 0.0001f32;
    for (idx, v) in vertices.iter().enumerate() {
        let key = (
            (v.x / eps).round() as i64,
            (v.y / eps).round() as i64,
            (v.z / eps).round() as i64,
        );
        let out_idx = if let Some(existing) = map.get(&key) {
            *existing
        } else {
            let next = out_vertices.len() as u32;
            map.insert(key, next);
            out_vertices.push(*v);
            next
        };
        remap[idx] = out_idx;
    }

    let mut unique = AHashSet::<(u32, u32, u32)>::default();
    let mut out_triangles = Vec::<[u32; 3]>::new();
    for tri in triangles {
        let a = remap.get(tri[0] as usize).copied()?;
        let b = remap.get(tri[1] as usize).copied()?;
        let c = remap.get(tri[2] as usize).copied()?;
        if a == b || b == c || a == c {
            continue;
        }
        let pa = out_vertices[a as usize];
        let pb = out_vertices[b as usize];
        let pc = out_vertices[c as usize];
        if triangle_area_sq(pa, pb, pc) <= 1.0e-12 {
            continue;
        }
        let mut ord = [a, b, c];
        ord.sort_unstable();
        if !unique.insert((ord[0], ord[1], ord[2])) {
            continue;
        }
        out_triangles.push([a, b, c]);
    }

    if out_vertices.len() < 3 || out_triangles.is_empty() {
        return None;
    }
    Some((out_vertices, out_triangles))
}

pub fn simplify_coplanar_mesh(
    vertices: &[na3::Point3<f32>],
    triangles: &[[u32; 3]],
) -> Option<TriMeshData> {
    if triangles.len() < 16 {
        return None;
    }
    let first = triangles[0];
    let p0 = vertices[first[0] as usize];
    let p1 = vertices[first[1] as usize];
    let p2 = vertices[first[2] as usize];
    let n = (p1 - p0).cross(&(p2 - p0));
    let n_len = n.norm();
    if n_len <= 1.0e-6 {
        return None;
    }
    let n = n / n_len;
    let plane_d = n.dot(&p0.coords);
    let plane_eps = 0.0025f32;
    for p in vertices {
        let dist = (n.dot(&p.coords) - plane_d).abs();
        if dist > plane_eps {
            return None;
        }
    }

    let axis = dominant_axis_3d(n.x, n.y, n.z);
    let mut pts2d = Vec::<[f32; 2]>::with_capacity(vertices.len());
    for p in vertices {
        pts2d.push(project_axis_3d(*p, axis));
    }

    let mut unique_2d = pts2d.clone();
    unique_2d.sort_by(|a, b| {
        a[0].partial_cmp(&b[0])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a[1].partial_cmp(&b[1]).unwrap_or(std::cmp::Ordering::Equal))
    });
    unique_2d.dedup_by(|a, b| (a[0] - b[0]).abs() <= 1.0e-5 && (a[1] - b[1]).abs() <= 1.0e-5);
    if unique_2d.len() < 3 {
        return None;
    }

    let hull = convex_hull_2d(&unique_2d);
    if hull.len() < 3 {
        return None;
    }

    let hull_area = polygon_area_abs(&hull);
    if hull_area <= 1.0e-6 {
        return None;
    }
    let mut tri_area_sum = 0.0f32;
    for tri in triangles {
        let a = pts2d[tri[0] as usize];
        let b = pts2d[tri[1] as usize];
        let c = pts2d[tri[2] as usize];
        tri_area_sum += ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5;
    }
    if tri_area_sum <= 1.0e-6 {
        return None;
    }
    if hull_area > tri_area_sum * 1.1 {
        return None;
    }

    let mut new_vertices = Vec::<na3::Point3<f32>>::with_capacity(hull.len());
    for p in &hull {
        new_vertices.push(unproject_axis_on_plane(*p, axis, n, plane_d));
    }
    let mut new_triangles = Vec::<[u32; 3]>::new();
    for i in 1..hull.len() - 1 {
        new_triangles.push([0, i as u32, (i + 1) as u32]);
    }
    Some((new_vertices, new_triangles))
}

pub fn dominant_axis_3d(x: f32, y: f32, z: f32) -> usize {
    let ax = x.abs();
    let ay = y.abs();
    let az = z.abs();
    if ax >= ay && ax >= az {
        0
    } else if ay >= az {
        1
    } else {
        2
    }
}

pub fn project_axis_3d(p: na3::Point3<f32>, axis: usize) -> [f32; 2] {
    match axis {
        0 => [p.y, p.z],
        1 => [p.x, p.z],
        _ => [p.x, p.y],
    }
}

pub fn unproject_axis_on_plane(
    p: [f32; 2],
    axis: usize,
    n: na3::Vector3<f32>,
    d: f32,
) -> na3::Point3<f32> {
    match axis {
        0 => {
            let y = p[0];
            let z = p[1];
            let x = (d - n.y * y - n.z * z) / n.x.max(1.0e-6).copysign(n.x);
            na3::Point3::new(x, y, z)
        }
        1 => {
            let x = p[0];
            let z = p[1];
            let y = (d - n.x * x - n.z * z) / n.y.max(1.0e-6).copysign(n.y);
            na3::Point3::new(x, y, z)
        }
        _ => {
            let x = p[0];
            let y = p[1];
            let z = (d - n.x * x - n.y * y) / n.z.max(1.0e-6).copysign(n.z);
            na3::Point3::new(x, y, z)
        }
    }
}

pub fn convex_hull_2d(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| {
        a[0].partial_cmp(&b[0])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a[1].partial_cmp(&b[1]).unwrap_or(std::cmp::Ordering::Equal))
    });
    if pts.len() <= 3 {
        return pts;
    }
    let mut lower = Vec::<[f32; 2]>::new();
    for p in &pts {
        while lower.len() >= 2
            && cross2(
                sub2(lower[lower.len() - 1], lower[lower.len() - 2]),
                sub2(*p, lower[lower.len() - 1]),
            ) <= 0.0
        {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper = Vec::<[f32; 2]>::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2
            && cross2(
                sub2(upper[upper.len() - 1], upper[upper.len() - 2]),
                sub2(*p, upper[upper.len() - 1]),
            ) <= 0.0
        {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

pub fn polygon_area_abs(poly: &[[f32; 2]]) -> f32 {
    if poly.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0f32;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        area += a[0] * b[1] - a[1] * b[0];
    }
    area.abs() * 0.5
}

pub fn sub2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn cross2(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}

pub fn triangle_area_sq(a: na3::Point3<f32>, b: na3::Point3<f32>, c: na3::Point3<f32>) -> f32 {
    let ab = b - a;
    let ac = c - a;
    ab.cross(&ac).norm_squared() * 0.25
}

pub fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return (source, None);
    };
    if path.is_empty() {
        return (source, None);
    }
    if selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return (path, Some(selector));
    }
    (source, None)
}

pub fn parse_fragment_index(fragment: Option<&str>, key: &str) -> Option<usize> {
    let fragment = fragment?;
    let (name, rest) = fragment.split_once('[')?;
    if name.trim() != key {
        return None;
    }
    let value = rest.strip_suffix(']')?.trim();
    value.parse::<usize>().ok()
}

pub fn triangle_points_2d(
    kind: Triangle2DKind,
    width: f32,
    height: f32,
) -> Option<[na2::Point2<f32>; 3]> {
    let w = width.abs().max(0.0001);
    let mut h = height.abs().max(0.0001);
    let points = match kind {
        Triangle2DKind::Equilateral => {
            h = h.max((3.0f32).sqrt() * 0.5 * w);
            [
                na2::Point2::new(-w * 0.5, -h / 3.0),
                na2::Point2::new(w * 0.5, -h / 3.0),
                na2::Point2::new(0.0, 2.0 * h / 3.0),
            ]
        }
        Triangle2DKind::Right => [
            na2::Point2::new(-w / 3.0, -h / 3.0),
            na2::Point2::new(2.0 * w / 3.0, -h / 3.0),
            na2::Point2::new(-w / 3.0, 2.0 * h / 3.0),
        ],
        Triangle2DKind::Isosceles => [
            na2::Point2::new(-w * 0.5, -h * 0.5),
            na2::Point2::new(w * 0.5, -h * 0.5),
            na2::Point2::new(0.0, h * 0.5),
        ],
    };
    Some(points)
}

pub fn tri_prism_points(width: f32, height: f32, depth: f32) -> Vec<na3::Point3<f32>> {
    let hw = width.abs().max(0.0001) * 0.5;
    let hh = height.abs().max(0.0001) * 0.5;
    let hd = depth.abs().max(0.0001) * 0.5;
    vec![
        na3::Point3::new(-hw, -hh, -hd),
        na3::Point3::new(hw, -hh, -hd),
        na3::Point3::new(0.0, hh, -hd),
        na3::Point3::new(-hw, -hh, hd),
        na3::Point3::new(hw, -hh, hd),
        na3::Point3::new(0.0, hh, hd),
    ]
}

pub fn triangular_pyramid_points(width: f32, height: f32, depth: f32) -> Vec<na3::Point3<f32>> {
    let hw = width.abs().max(0.0001) * 0.5;
    let hh = height.abs().max(0.0001) * 0.5;
    let hd = depth.abs().max(0.0001) * 0.5;
    vec![
        na3::Point3::new(-hw, -hh, -hd),
        na3::Point3::new(hw, -hh, -hd),
        na3::Point3::new(0.0, -hh, hd),
        na3::Point3::new(0.0, hh, 0.0),
    ]
}

pub fn square_pyramid_points(width: f32, height: f32, depth: f32) -> Vec<na3::Point3<f32>> {
    let hw = width.abs().max(0.0001) * 0.5;
    let hh = height.abs().max(0.0001) * 0.5;
    let hd = depth.abs().max(0.0001) * 0.5;
    vec![
        na3::Point3::new(-hw, -hh, -hd),
        na3::Point3::new(hw, -hh, -hd),
        na3::Point3::new(hw, -hh, hd),
        na3::Point3::new(-hw, -hh, hd),
        na3::Point3::new(0.0, hh, 0.0),
    ]
}

pub fn transform_to_iso2(transform: Transform2D) -> na2::Isometry2<f32> {
    na2::Isometry2::new(
        na2::Vector2::new(transform.position.x, transform.position.y),
        transform.rotation,
    )
}

pub fn transform_to_iso3(transform: Transform3D) -> na3::Isometry3<f32> {
    let rotation = na3::UnitQuaternion::from_quaternion(na3::Quaternion::new(
        transform.rotation.w,
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
    ));
    na3::Isometry3::from_parts(
        na3::Translation3::new(
            transform.position.x,
            transform.position.y,
            transform.position.z,
        ),
        rotation,
    )
}
