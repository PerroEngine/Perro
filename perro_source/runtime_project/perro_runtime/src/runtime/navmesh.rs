use perro_resource_api::sub_apis::NavMesh3D;
use perro_runtime_api::sub_apis::{NavMeshPath3D, NavMeshPathOptions, NavMeshPathStatus};
use perro_structs::{BitMask, Vector3};

#[derive(Clone, Copy)]
struct ProjectedPoint {
    point: Vector3,
    triangle: usize,
    distance2: f32,
}

pub(crate) fn project_point_3d(
    navmesh: &NavMesh3D,
    point: Vector3,
    max_distance: f32,
    layers: BitMask,
) -> Option<Vector3> {
    nearest_triangle_point(navmesh, point, max_distance, layers).map(|projected| projected.point)
}

pub(crate) fn find_path_3d(
    navmesh: &NavMesh3D,
    start: Vector3,
    end: Vector3,
    opts: NavMeshPathOptions,
) -> NavMeshPath3D {
    if navmesh.is_empty() || opts.layers.is_empty() {
        return NavMeshPath3D::failed();
    }
    let start = match nearest_triangle_point(navmesh, start, opts.max_snap_distance, opts.layers) {
        Some(projected) => projected,
        None => return NavMeshPath3D::failed(),
    };
    let end = match nearest_triangle_point(navmesh, end, opts.max_snap_distance, opts.layers) {
        Some(projected) => projected,
        None => return NavMeshPath3D::failed(),
    };
    if start.triangle == end.triangle {
        return path_from_points(vec![start.point, end.point], opts);
    }

    let adjacency = build_adjacency(navmesh, opts.layers);
    let Some(tri_path) = astar(
        navmesh,
        &adjacency,
        start.triangle,
        end.triangle,
        opts.layers,
    ) else {
        return NavMeshPath3D::failed();
    };
    let mut points = Vec::with_capacity(tri_path.len() + 1);
    points.push(start.point);
    for pair in tri_path.windows(2) {
        if let Some(mid) = shared_edge_midpoint(navmesh, pair[0], pair[1]) {
            points.push(mid);
        }
    }
    points.push(end.point);
    path_from_points(points, opts)
}

fn path_from_points(mut points: Vec<Vector3>, opts: NavMeshPathOptions) -> NavMeshPath3D {
    dedup_points(&mut points);
    if opts.simplify {
        simplify_collinear(&mut points);
    }
    if opts.max_points > 1 && points.len() > opts.max_points as usize {
        let last = *points.last().unwrap();
        points.truncate(opts.max_points as usize);
        if let Some(slot) = points.last_mut() {
            *slot = last;
        }
    }
    let distance = points
        .windows(2)
        .map(|pair| pair[0].distance_to(pair[1]))
        .sum();
    NavMeshPath3D {
        status: NavMeshPathStatus::Complete,
        points,
        distance,
    }
}

fn nearest_triangle_point(
    navmesh: &NavMesh3D,
    point: Vector3,
    max_distance: f32,
    layers: BitMask,
) -> Option<ProjectedPoint> {
    let max_distance = max_distance.max(0.0);
    let max_distance2 = max_distance * max_distance;
    let mut best: Option<ProjectedPoint> = None;
    for (triangle_index, triangle) in navmesh.triangles.iter().enumerate() {
        if !triangle.layers.intersects(layers) {
            continue;
        }
        let a = navmesh.vertices[triangle.vertices[0] as usize];
        let b = navmesh.vertices[triangle.vertices[1] as usize];
        let c = navmesh.vertices[triangle.vertices[2] as usize];
        let projected = closest_point_on_triangle_xz(point, a, b, c);
        let distance2 = distance2_xz(point, projected);
        if distance2 <= max_distance2 && best.is_none_or(|current| distance2 < current.distance2) {
            best = Some(ProjectedPoint {
                point: projected,
                triangle: triangle_index,
                distance2,
            });
        }
    }
    best
}

fn closest_point_on_triangle_xz(point: Vector3, a: Vector3, b: Vector3, c: Vector3) -> Vector3 {
    if let Some((u, v, w)) = barycentric_xz(point, a, b, c)
        && u >= -0.0001
        && v >= -0.0001
        && w >= -0.0001
    {
        return a * u + b * v + c * w;
    }

    let ab = closest_point_on_segment_xz(point, a, b);
    let bc = closest_point_on_segment_xz(point, b, c);
    let ca = closest_point_on_segment_xz(point, c, a);
    [ab, bc, ca]
        .into_iter()
        .min_by(|left, right| {
            distance2_xz(point, *left)
                .partial_cmp(&distance2_xz(point, *right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(a)
}

fn barycentric_xz(point: Vector3, a: Vector3, b: Vector3, c: Vector3) -> Option<(f32, f32, f32)> {
    let v0 = (b.x - a.x, b.z - a.z);
    let v1 = (c.x - a.x, c.z - a.z);
    let v2 = (point.x - a.x, point.z - a.z);
    let den = v0.0 * v1.1 - v1.0 * v0.1;
    if den.abs() <= f32::EPSILON {
        return None;
    }
    let v = (v2.0 * v1.1 - v1.0 * v2.1) / den;
    let w = (v0.0 * v2.1 - v2.0 * v0.1) / den;
    let u = 1.0 - v - w;
    Some((u, v, w))
}

fn closest_point_on_segment_xz(point: Vector3, a: Vector3, b: Vector3) -> Vector3 {
    let ab = (b.x - a.x, b.z - a.z);
    let ap = (point.x - a.x, point.z - a.z);
    let len2 = ab.0 * ab.0 + ab.1 * ab.1;
    if len2 <= f32::EPSILON {
        return a;
    }
    let t = ((ap.0 * ab.0 + ap.1 * ab.1) / len2).clamp(0.0, 1.0);
    a + (b - a) * t
}

fn distance2_xz(a: Vector3, b: Vector3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

fn build_adjacency(navmesh: &NavMesh3D, layers: BitMask) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new(); navmesh.triangles.len()];
    for i in 0..navmesh.triangles.len() {
        if !navmesh.triangles[i].layers.intersects(layers) {
            continue;
        }
        for j in (i + 1)..navmesh.triangles.len() {
            if !navmesh.triangles[j].layers.intersects(layers) {
                continue;
            }
            if shared_vertex_count(navmesh.triangles[i].vertices, navmesh.triangles[j].vertices)
                >= 2
            {
                out[i].push(j);
                out[j].push(i);
            }
        }
    }
    out
}

fn astar(
    navmesh: &NavMesh3D,
    adjacency: &[Vec<usize>],
    start: usize,
    end: usize,
    layers: BitMask,
) -> Option<Vec<usize>> {
    let mut open = vec![start];
    let mut came_from = vec![usize::MAX; navmesh.triangles.len()];
    let mut g_score = vec![f32::INFINITY; navmesh.triangles.len()];
    let mut f_score = vec![f32::INFINITY; navmesh.triangles.len()];
    g_score[start] = 0.0;
    f_score[start] = centroid(navmesh, start).distance_to(centroid(navmesh, end));

    while !open.is_empty() {
        let best_pos = open
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                f_score[**left]
                    .partial_cmp(&f_score[**right])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(pos, _)| pos)
            .unwrap();
        let current = open.swap_remove(best_pos);
        if current == end {
            return Some(reconstruct(came_from, current));
        }
        for &next in &adjacency[current] {
            if !navmesh.triangles[next].layers.intersects(layers) {
                continue;
            }
            let tentative =
                g_score[current] + centroid(navmesh, current).distance_to(centroid(navmesh, next));
            if tentative < g_score[next] {
                came_from[next] = current;
                g_score[next] = tentative;
                f_score[next] =
                    tentative + centroid(navmesh, next).distance_to(centroid(navmesh, end));
                if !open.contains(&next) {
                    open.push(next);
                }
            }
        }
    }
    None
}

fn reconstruct(came_from: Vec<usize>, mut current: usize) -> Vec<usize> {
    let mut path = vec![current];
    while came_from[current] != usize::MAX {
        current = came_from[current];
        path.push(current);
    }
    path.reverse();
    path
}

fn centroid(navmesh: &NavMesh3D, triangle: usize) -> Vector3 {
    let tri = navmesh.triangles[triangle].vertices;
    (navmesh.vertices[tri[0] as usize]
        + navmesh.vertices[tri[1] as usize]
        + navmesh.vertices[tri[2] as usize])
        / 3.0
}

fn shared_vertex_count(a: [u32; 3], b: [u32; 3]) -> usize {
    a.into_iter()
        .filter(|left| b.into_iter().any(|right| *left == right))
        .count()
}

fn shared_edge_midpoint(navmesh: &NavMesh3D, a: usize, b: usize) -> Option<Vector3> {
    let mut shared = Vec::new();
    for left in navmesh.triangles[a].vertices {
        if navmesh.triangles[b]
            .vertices
            .into_iter()
            .any(|right| left == right)
        {
            shared.push(navmesh.vertices[left as usize]);
        }
    }
    (shared.len() >= 2).then(|| (shared[0] + shared[1]) * 0.5)
}

fn dedup_points(points: &mut Vec<Vector3>) {
    points.dedup_by(|a, b| a.distance_to(*b) <= 0.0001);
}

fn simplify_collinear(points: &mut Vec<Vector3>) {
    if points.len() <= 2 {
        return;
    }
    let mut out = Vec::with_capacity(points.len());
    out.push(points[0]);
    for i in 1..points.len() - 1 {
        let prev = *out.last().unwrap();
        let current = points[i];
        let next = points[i + 1];
        let a = (current.x - prev.x, current.z - prev.z);
        let b = (next.x - current.x, next.z - current.z);
        let cross = a.0 * b.1 - a.1 * b.0;
        if cross.abs() > 0.0001 {
            out.push(current);
        }
    }
    out.push(*points.last().unwrap());
    *points = out;
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_resource_api::sub_apis::{NavMesh3D, NavMeshTriangle3D};

    #[test]
    fn same_poly_returns_direct_path() {
        let nav = single_tri();
        let path = find_path_3d(
            &nav,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(0.2, 0.0, 0.2),
            NavMeshPathOptions::default(),
        );
        assert_eq!(path.status, NavMeshPathStatus::Complete);
        assert_eq!(path.points.len(), 2);
    }

    #[test]
    fn corridor_turn_returns_midpoint() {
        let nav = NavMesh3D {
            vertices: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(1.0, 0.0, 1.0),
                Vector3::new(2.0, 0.0, 1.0),
            ],
            triangles: vec![tri([0, 1, 2], 1), tri([1, 3, 2], 1), tri([1, 4, 3], 1)],
        };
        let path = find_path_3d(
            &nav,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(1.8, 0.0, 0.9),
            NavMeshPathOptions::default(),
        );
        assert_eq!(path.status, NavMeshPathStatus::Complete);
        assert!(path.points.len() >= 3);
    }

    #[test]
    fn disconnected_returns_failed() {
        let mut nav = single_tri();
        nav.vertices.extend([
            Vector3::new(4.0, 0.0, 4.0),
            Vector3::new(5.0, 0.0, 4.0),
            Vector3::new(4.0, 0.0, 5.0),
        ]);
        nav.triangles.push(tri([3, 4, 5], 1));
        let path = find_path_3d(
            &nav,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(4.1, 0.0, 4.1),
            NavMeshPathOptions {
                max_snap_distance: 2.0,
                ..Default::default()
            },
        );
        assert_eq!(path.status, NavMeshPathStatus::Failed);
    }

    #[test]
    fn layer_mask_blocks_path() {
        let nav = NavMesh3D {
            vertices: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(1.0, 0.0, 1.0),
            ],
            triangles: vec![tri([0, 1, 2], 1), tri([1, 3, 2], 2)],
        };
        let path = find_path_3d(
            &nav,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(0.9, 0.0, 0.9),
            NavMeshPathOptions {
                layers: BitMask::layer(1),
                max_snap_distance: 0.05,
                ..Default::default()
            },
        );
        assert_eq!(path.status, NavMeshPathStatus::Failed);
    }

    fn single_tri() -> NavMesh3D {
        NavMesh3D {
            vertices: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
            ],
            triangles: vec![tri([0, 1, 2], 1)],
        }
    }

    fn tri(vertices: [u32; 3], layer: u8) -> NavMeshTriangle3D {
        NavMeshTriangle3D {
            vertices,
            layers: BitMask::layer(layer),
        }
    }
}
