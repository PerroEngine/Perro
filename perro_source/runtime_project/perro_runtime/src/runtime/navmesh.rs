use perro_resource_api::sub_apis::{NavMesh3D, NavMeshResource3D};
use perro_runtime_api::sub_apis::{
    NavMeshAreaCost, NavMeshObstacle3D, NavMeshPath3D, NavMeshPathOptions, NavMeshPathStatus,
    NavMeshQueryOptions,
};
use perro_structs::{BitMask, Vector3};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

const POINT_EPSILON: f32 = 0.0001;

#[derive(Clone, Copy)]
struct ProjectedPoint {
    point: Vector3,
    triangle: usize,
    distance2: f32,
}

#[derive(Clone, Copy, Debug)]
enum Transition {
    Portal { left: Vector3, right: Vector3 },
    OffMesh { start: Vector3, end: Vector3 },
}

#[derive(Clone, Copy, Debug)]
struct SearchEdge {
    to: usize,
    transition: Transition,
    base_cost: f32,
}

pub(crate) struct SearchGraph {
    adjacency: Vec<Vec<SearchEdge>>,
    centroids: Vec<Vector3>,
    areas: Vec<u8>,
    has_off_mesh_links: bool,
}

#[derive(Clone, Copy, Debug)]
struct OpenEntry {
    triangle: usize,
    estimated_cost: f32,
}

impl PartialEq for OpenEntry {
    fn eq(&self, other: &Self) -> bool {
        self.triangle == other.triangle
            && self.estimated_cost.to_bits() == other.estimated_cost.to_bits()
    }
}

impl Eq for OpenEntry {}

impl PartialOrd for OpenEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OpenEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimated_cost
            .total_cmp(&self.estimated_cost)
            .then_with(|| other.triangle.cmp(&self.triangle))
    }
}

pub(crate) fn project_point_3d(
    navmesh: &NavMesh3D,
    point: Vector3,
    max_distance: f32,
    layers: BitMask,
) -> Option<Vector3> {
    if navmesh.validate().is_err() || !vector_is_finite(point) || max_distance.is_nan() {
        return None;
    }
    nearest_triangle_point(navmesh, point, max_distance, layers, &[])
        .map(|projected| projected.point)
}

#[cfg(test)]
fn find_path_3d(
    navmesh: &NavMesh3D,
    start: Vector3,
    end: Vector3,
    opts: NavMeshPathOptions,
) -> NavMeshPath3D {
    if navmesh.validate().is_err() {
        return NavMeshPath3D::failed();
    }
    let resource = NavMeshResource3D::from_mesh(navmesh.clone());
    let graph = SearchGraph::new(&resource, opts.layers);
    find_path_3d_prepared(&resource, &graph, start, end, opts)
}

pub(crate) fn find_path_3d_prepared(
    resource: &NavMeshResource3D,
    graph: &SearchGraph,
    start: Vector3,
    end: Vector3,
    opts: NavMeshPathOptions,
) -> NavMeshPath3D {
    find_path_query_3d_prepared(
        resource,
        graph,
        start,
        end,
        NavMeshQueryOptions {
            path: opts,
            ..Default::default()
        },
    )
}

pub(crate) fn find_path_query_3d_prepared(
    resource: &NavMeshResource3D,
    graph: &SearchGraph,
    start: Vector3,
    end: Vector3,
    query: NavMeshQueryOptions,
) -> NavMeshPath3D {
    let opts = query.path;
    if resource.validate().is_err()
        || opts.layers.is_empty()
        || !vector_is_finite(start)
        || !vector_is_finite(end)
        || opts.max_snap_distance.is_nan()
        || !query_is_valid(&query)
    {
        return NavMeshPath3D::failed();
    }

    let blocked = blocked_triangles(&resource.mesh, &query.obstacles);
    let start = match nearest_triangle_point(
        &resource.mesh,
        start,
        opts.max_snap_distance,
        opts.layers,
        &blocked,
    ) {
        Some(projected) => projected,
        None => return NavMeshPath3D::failed(),
    };
    let end = match nearest_triangle_point(
        &resource.mesh,
        end,
        opts.max_snap_distance,
        opts.layers,
        &blocked,
    ) {
        Some(projected) => projected,
        None => return NavMeshPath3D::failed(),
    };
    if start.triangle == end.triangle {
        return path_from_points(vec![start.point, end.point], opts);
    }

    let area_costs = area_cost_table(&query.area_costs);
    let Some(transitions) = astar(
        graph,
        start.triangle,
        end.triangle,
        &area_costs,
        &blocked,
        query.use_off_mesh_links,
        &query.obstacles,
    ) else {
        return NavMeshPath3D::failed();
    };
    let points = corridor_points(start.point, end.point, &transitions, opts.simplify);
    path_from_points(points, opts)
}

fn query_is_valid(query: &NavMeshQueryOptions) -> bool {
    query.area_costs.iter().all(|cost| {
        (1..=32).contains(&cost.area) && cost.multiplier.is_finite() && cost.multiplier > 0.0
    }) && query.obstacles.iter().all(|obstacle| match *obstacle {
        NavMeshObstacle3D::Circle { center, radius } => {
            vector_is_finite(center) && radius.is_finite() && radius >= 0.0
        }
        NavMeshObstacle3D::Aabb { min, max } => {
            vector_is_finite(min) && vector_is_finite(max) && min.x <= max.x && min.z <= max.z
        }
    })
}

fn area_cost_table(costs: &[NavMeshAreaCost]) -> [f32; 32] {
    let mut table = [1.0; 32];
    for cost in costs {
        table[cost.area as usize - 1] = cost.multiplier;
    }
    table
}

fn path_from_points(mut points: Vec<Vector3>, opts: NavMeshPathOptions) -> NavMeshPath3D {
    dedup_points(&mut points);
    if opts.max_points > 1
        && points.len() > opts.max_points as usize
        && let Some(last) = points.last().copied()
    {
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
    blocked: &[bool],
) -> Option<ProjectedPoint> {
    let max_distance = max_distance.max(0.0);
    let max_distance2 = max_distance * max_distance;
    let mut best: Option<ProjectedPoint> = None;
    for (triangle_index, triangle) in navmesh.triangles.iter().enumerate() {
        if blocked.get(triangle_index).copied().unwrap_or(false)
            || !triangle.layers.intersects(layers)
        {
            continue;
        }
        let [a, b, c] = triangle_points(navmesh, triangle_index);
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
        && u >= -POINT_EPSILON
        && v >= -POINT_EPSILON
        && w >= -POINT_EPSILON
    {
        return a * u + b * v + c * w;
    }

    let ab = closest_point_on_segment_xz(point, a, b);
    let bc = closest_point_on_segment_xz(point, b, c);
    let ca = closest_point_on_segment_xz(point, c, a);
    [ab, bc, ca]
        .into_iter()
        .min_by(|left, right| distance2_xz(point, *left).total_cmp(&distance2_xz(point, *right)))
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
    Some((1.0 - v - w, v, w))
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

fn vector_is_finite(value: Vector3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

impl SearchGraph {
    pub(crate) fn new(resource: &NavMeshResource3D, layers: BitMask) -> Self {
        let navmesh = &resource.mesh;
        let mut adjacency = vec![Vec::new(); navmesh.triangles.len()];
        let centroids: Vec<_> = (0..navmesh.triangles.len())
            .map(|triangle| centroid(navmesh, triangle))
            .collect();
        let mut edge_owner =
            HashMap::<(u32, u32), usize>::with_capacity(navmesh.triangles.len().saturating_mul(3));

        for (triangle_index, triangle) in navmesh.triangles.iter().enumerate() {
            if !triangle.layers.intersects(layers) {
                continue;
            }
            let [a, b, c] = triangle.vertices;
            for (left, right) in [(a, b), (b, c), (c, a)] {
                let edge = if left < right {
                    (left, right)
                } else {
                    (right, left)
                };
                if let Some(&other) = edge_owner.get(&edge) {
                    add_portal_edges(
                        &mut adjacency,
                        &centroids,
                        navmesh.vertices[edge.0 as usize],
                        navmesh.vertices[edge.1 as usize],
                        triangle_index,
                        other,
                    );
                } else {
                    edge_owner.insert(edge, triangle_index);
                }
            }
        }

        let mut has_off_mesh_links = false;
        for link in &resource.links {
            if !link.layers.intersects(layers) {
                continue;
            }
            let Some(from) =
                nearest_triangle_point(navmesh, link.start, link.snap_distance, layers, &[])
            else {
                continue;
            };
            let Some(to) =
                nearest_triangle_point(navmesh, link.end, link.snap_distance, layers, &[])
            else {
                continue;
            };
            if from.triangle == to.triangle {
                continue;
            }
            add_off_mesh_edge(
                &mut adjacency,
                &centroids,
                from.triangle,
                to.triangle,
                from.point,
                to.point,
                link.cost,
            );
            if link.bidirectional {
                add_off_mesh_edge(
                    &mut adjacency,
                    &centroids,
                    to.triangle,
                    from.triangle,
                    to.point,
                    from.point,
                    link.cost,
                );
            }
            has_off_mesh_links = true;
        }

        Self {
            adjacency,
            centroids,
            areas: resource.triangle_areas.clone(),
            has_off_mesh_links,
        }
    }
}

fn add_portal_edges(
    adjacency: &mut [Vec<SearchEdge>],
    centroids: &[Vector3],
    a: Vector3,
    b: Vector3,
    first: usize,
    second: usize,
) {
    for (from, to) in [(first, second), (second, first)] {
        let (left, right) = orient_portal(a, b, centroids[from], centroids[to]);
        adjacency[from].push(SearchEdge {
            to,
            transition: Transition::Portal { left, right },
            base_cost: centroids[from].distance_to(centroids[to]),
        });
    }
}

fn add_off_mesh_edge(
    adjacency: &mut [Vec<SearchEdge>],
    centroids: &[Vector3],
    from: usize,
    to: usize,
    start: Vector3,
    end: Vector3,
    cost: f32,
) {
    adjacency[from].push(SearchEdge {
        to,
        transition: Transition::OffMesh { start, end },
        base_cost: centroids[from].distance_to(start)
            + start.distance_to(end) * cost
            + end.distance_to(centroids[to]),
    });
}

fn orient_portal(a: Vector3, b: Vector3, from: Vector3, to: Vector3) -> (Vector3, Vector3) {
    let direction = (to.x - from.x, to.z - from.z);
    let side_a = direction.0 * (a.z - from.z) - direction.1 * (a.x - from.x);
    let side_b = direction.0 * (b.z - from.z) - direction.1 * (b.x - from.x);
    if side_a >= side_b { (a, b) } else { (b, a) }
}

fn astar(
    graph: &SearchGraph,
    start: usize,
    end: usize,
    area_costs: &[f32; 32],
    blocked: &[bool],
    use_off_mesh_links: bool,
    obstacles: &[NavMeshObstacle3D],
) -> Option<Vec<Transition>> {
    let mut open = BinaryHeap::new();
    let mut closed = vec![false; graph.adjacency.len()];
    let mut came_from = vec![None; graph.adjacency.len()];
    let mut g_score = vec![f32::INFINITY; graph.adjacency.len()];
    let min_area_cost = area_costs.iter().copied().fold(1.0, f32::min);
    let use_heuristic = !(use_off_mesh_links && graph.has_off_mesh_links);
    g_score[start] = 0.0;
    open.push(OpenEntry {
        triangle: start,
        estimated_cost: heuristic(graph, start, end, min_area_cost, use_heuristic),
    });

    while let Some(OpenEntry {
        triangle: current, ..
    }) = open.pop()
    {
        if closed[current] {
            continue;
        }
        if current == end {
            return Some(reconstruct(came_from, current));
        }
        closed[current] = true;
        for edge in &graph.adjacency[current] {
            if closed[edge.to] || blocked.get(edge.to).copied().unwrap_or(false) {
                continue;
            }
            if matches!(edge.transition, Transition::OffMesh { .. })
                && (!use_off_mesh_links || transition_hits_obstacle(edge.transition, obstacles))
            {
                continue;
            }
            let area = graph.areas[edge.to] as usize - 1;
            let tentative = g_score[current] + edge.base_cost * area_costs[area];
            if tentative < g_score[edge.to] {
                came_from[edge.to] = Some((current, edge.transition));
                g_score[edge.to] = tentative;
                open.push(OpenEntry {
                    triangle: edge.to,
                    estimated_cost: tentative
                        + heuristic(graph, edge.to, end, min_area_cost, use_heuristic),
                });
            }
        }
    }
    None
}

fn heuristic(
    graph: &SearchGraph,
    from: usize,
    to: usize,
    min_area_cost: f32,
    enabled: bool,
) -> f32 {
    if enabled {
        graph.centroids[from].distance_to(graph.centroids[to]) * min_area_cost
    } else {
        0.0
    }
}

fn reconstruct(came_from: Vec<Option<(usize, Transition)>>, mut current: usize) -> Vec<Transition> {
    let mut transitions = Vec::new();
    while let Some((previous, transition)) = came_from[current] {
        transitions.push(transition);
        current = previous;
    }
    transitions.reverse();
    transitions
}

fn corridor_points(
    start: Vector3,
    end: Vector3,
    transitions: &[Transition],
    funnel: bool,
) -> Vec<Vector3> {
    let mut points = vec![start];
    let mut segment_start = start;
    let mut portals = Vec::new();

    for transition in transitions {
        match *transition {
            Transition::Portal { left, right } if funnel => portals.push((left, right)),
            Transition::Portal { left, right } => points.push((left + right) * 0.5),
            Transition::OffMesh { start, end } => {
                if funnel {
                    append_without_first(&mut points, string_pull(segment_start, start, &portals));
                    portals.clear();
                } else {
                    points.push(start);
                }
                points.push(end);
                segment_start = end;
            }
        }
    }
    if funnel {
        append_without_first(&mut points, string_pull(segment_start, end, &portals));
    } else {
        points.push(end);
    }
    points
}

fn append_without_first(output: &mut Vec<Vector3>, input: Vec<Vector3>) {
    output.extend(input.into_iter().skip(1));
}

fn string_pull(start: Vector3, end: Vector3, portals: &[(Vector3, Vector3)]) -> Vec<Vector3> {
    if portals.is_empty() {
        return vec![start, end];
    }
    let mut all = Vec::with_capacity(portals.len() + 2);
    all.push((start, start));
    all.extend_from_slice(portals);
    all.push((end, end));

    let mut output = vec![start];
    let mut apex = start;
    let mut left = start;
    let mut right = start;
    let mut left_index = 0;
    let mut right_index = 0;
    let mut index = 1;

    while index < all.len() {
        let (next_left, next_right) = all[index];
        if tri_area2_xz(apex, right, next_right) <= 0.0 {
            if points_equal_xz(apex, right) || tri_area2_xz(apex, left, next_right) > 0.0 {
                right = next_right;
                right_index = index;
            } else {
                output.push(left);
                apex = left;
                let apex_index = left_index;
                left = apex;
                right = apex;
                left_index = apex_index;
                right_index = apex_index;
                index = apex_index + 1;
                continue;
            }
        }
        if tri_area2_xz(apex, left, next_left) >= 0.0 {
            if points_equal_xz(apex, left) || tri_area2_xz(apex, right, next_left) < 0.0 {
                left = next_left;
                left_index = index;
            } else {
                output.push(right);
                apex = right;
                let apex_index = right_index;
                left = apex;
                right = apex;
                left_index = apex_index;
                right_index = apex_index;
                index = apex_index + 1;
                continue;
            }
        }
        index += 1;
    }
    output.push(end);
    dedup_points(&mut output);
    output
}

fn tri_area2_xz(a: Vector3, b: Vector3, c: Vector3) -> f32 {
    (c.x - a.x) * (b.z - a.z) - (b.x - a.x) * (c.z - a.z)
}

fn points_equal_xz(a: Vector3, b: Vector3) -> bool {
    distance2_xz(a, b) <= POINT_EPSILON * POINT_EPSILON
}

fn blocked_triangles(navmesh: &NavMesh3D, obstacles: &[NavMeshObstacle3D]) -> Vec<bool> {
    (0..navmesh.triangles.len())
        .map(|triangle| {
            let [a, b, c] = triangle_points(navmesh, triangle);
            obstacles
                .iter()
                .any(|obstacle| triangle_hits_obstacle(a, b, c, *obstacle))
        })
        .collect()
}

fn triangle_hits_obstacle(a: Vector3, b: Vector3, c: Vector3, obstacle: NavMeshObstacle3D) -> bool {
    match obstacle {
        NavMeshObstacle3D::Circle { center, radius } => {
            distance2_xz(center, closest_point_on_triangle_xz(center, a, b, c)) <= radius * radius
        }
        NavMeshObstacle3D::Aabb { min, max } => {
            let triangle_min_x = a.x.min(b.x).min(c.x);
            let triangle_max_x = a.x.max(b.x).max(c.x);
            let triangle_min_z = a.z.min(b.z).min(c.z);
            let triangle_max_z = a.z.max(b.z).max(c.z);
            triangle_max_x >= min.x
                && triangle_min_x <= max.x
                && triangle_max_z >= min.z
                && triangle_min_z <= max.z
        }
    }
}

fn transition_hits_obstacle(transition: Transition, obstacles: &[NavMeshObstacle3D]) -> bool {
    let Transition::OffMesh { start, end } = transition else {
        return false;
    };
    obstacles.iter().any(|obstacle| match *obstacle {
        NavMeshObstacle3D::Circle { center, radius } => {
            distance2_xz(center, closest_point_on_segment_xz(center, start, end)) <= radius * radius
        }
        NavMeshObstacle3D::Aabb { min, max } => segment_hits_aabb_xz(start, end, min, max),
    })
}

fn segment_hits_aabb_xz(start: Vector3, end: Vector3, min: Vector3, max: Vector3) -> bool {
    let direction = (end.x - start.x, end.z - start.z);
    let mut low: f32 = 0.0;
    let mut high: f32 = 1.0;
    for (origin, delta, lower, upper) in [
        (start.x, direction.0, min.x, max.x),
        (start.z, direction.1, min.z, max.z),
    ] {
        if delta.abs() <= f32::EPSILON {
            if origin < lower || origin > upper {
                return false;
            }
        } else {
            let first = (lower - origin) / delta;
            let second = (upper - origin) / delta;
            low = low.max(first.min(second));
            high = high.min(first.max(second));
            if low > high {
                return false;
            }
        }
    }
    true
}

fn centroid(navmesh: &NavMesh3D, triangle: usize) -> Vector3 {
    let [a, b, c] = triangle_points(navmesh, triangle);
    Vector3::new(
        ((f64::from(a.x) + f64::from(b.x) + f64::from(c.x)) / 3.0) as f32,
        ((f64::from(a.y) + f64::from(b.y) + f64::from(c.y)) / 3.0) as f32,
        ((f64::from(a.z) + f64::from(b.z) + f64::from(c.z)) / 3.0) as f32,
    )
}

fn triangle_points(navmesh: &NavMesh3D, triangle: usize) -> [Vector3; 3] {
    navmesh.triangles[triangle]
        .vertices
        .map(|vertex| navmesh.vertices[vertex as usize])
}

fn dedup_points(points: &mut Vec<Vector3>) {
    points.dedup_by(|a, b| a.distance_to(*b) <= POINT_EPSILON);
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_resource_api::sub_apis::{NavMeshLink3D, NavMeshTriangle3D};

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
    fn funnel_pulls_straight_corridor_to_two_points() {
        let nav = strip_navmesh(4);
        let path = find_path_3d(
            &nav,
            Vector3::new(0.1, 0.0, 0.5),
            Vector3::new(3.9, 0.0, 0.5),
            NavMeshPathOptions::default(),
        );
        assert_eq!(path.status, NavMeshPathStatus::Complete);
        assert_eq!(path.points.len(), 2);
    }

    #[test]
    fn funnel_keeps_required_corner() {
        let nav = NavMesh3D {
            vertices: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(1.0, 0.0, 1.0),
                Vector3::new(2.0, 0.0, 1.0),
                Vector3::new(1.0, 0.0, 2.0),
                Vector3::new(2.0, 0.0, 2.0),
            ],
            triangles: vec![
                tri([0, 1, 2], 1),
                tri([1, 3, 2], 1),
                tri([1, 4, 3], 1),
                tri([3, 4, 5], 1),
                tri([4, 6, 5], 1),
            ],
        };
        let path = find_path_3d(
            &nav,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(1.9, 0.0, 1.9),
            NavMeshPathOptions::default(),
        );
        assert_eq!(path.status, NavMeshPathStatus::Complete);
        assert!(path.points.len() >= 3);
    }

    #[test]
    fn area_cost_chooses_longer_low_cost_route() {
        let (resource, start, end) = two_route_resource();
        let graph = SearchGraph::new(&resource, BitMask::ALL);
        let path = find_path_query_3d_prepared(
            &resource,
            &graph,
            start,
            end,
            NavMeshQueryOptions {
                path: NavMeshPathOptions {
                    simplify: false,
                    ..Default::default()
                },
                area_costs: vec![NavMeshAreaCost {
                    area: 2,
                    multiplier: 20.0,
                }],
                ..Default::default()
            },
        );
        assert_eq!(path.status, NavMeshPathStatus::Complete);
        assert!(
            path.points.iter().any(|point| point.z > 1.0),
            "{:?}",
            path.points
        );
    }

    #[test]
    fn off_mesh_link_connects_islands_and_honors_direction() {
        let mut resource = disconnected_resource();
        resource.links.push(NavMeshLink3D {
            start: Vector3::new(0.2, 0.0, 0.2),
            end: Vector3::new(4.2, 0.0, 0.2),
            bidirectional: false,
            layers: BitMask::ALL,
            cost: 1.0,
            snap_distance: 0.2,
        });
        let graph = SearchGraph::new(&resource, BitMask::ALL);
        let forward = find_path_3d_prepared(
            &resource,
            &graph,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(4.1, 0.0, 0.1),
            NavMeshPathOptions::default(),
        );
        let backward = find_path_3d_prepared(
            &resource,
            &graph,
            Vector3::new(4.1, 0.0, 0.1),
            Vector3::new(0.1, 0.0, 0.1),
            NavMeshPathOptions::default(),
        );
        let no_links = find_path_query_3d_prepared(
            &resource,
            &graph,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(4.1, 0.0, 0.1),
            NavMeshQueryOptions {
                use_off_mesh_links: false,
                ..Default::default()
            },
        );
        let blocked_link = find_path_query_3d_prepared(
            &resource,
            &graph,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(4.1, 0.0, 0.1),
            NavMeshQueryOptions {
                obstacles: vec![NavMeshObstacle3D::Circle {
                    center: Vector3::new(2.0, 0.0, 0.2),
                    radius: 0.2,
                }],
                ..Default::default()
            },
        );
        assert_eq!(forward.status, NavMeshPathStatus::Complete);
        assert_eq!(forward.points.len(), 4);
        assert_eq!(backward.status, NavMeshPathStatus::Failed);
        assert_eq!(no_links.status, NavMeshPathStatus::Failed);
        assert_eq!(blocked_link.status, NavMeshPathStatus::Failed);
    }

    #[test]
    fn query_obstacle_blocks_corridor_without_carve() {
        let nav = strip_navmesh(2);
        let resource = NavMeshResource3D::from_mesh(nav);
        let graph = SearchGraph::new(&resource, BitMask::ALL);
        let path = find_path_query_3d_prepared(
            &resource,
            &graph,
            Vector3::new(0.1, 0.0, 0.2),
            Vector3::new(1.9, 0.0, 0.8),
            NavMeshQueryOptions {
                obstacles: vec![NavMeshObstacle3D::Circle {
                    center: Vector3::new(1.0, 0.0, 0.5),
                    radius: 0.4,
                }],
                ..Default::default()
            },
        );
        assert_eq!(path.status, NavMeshPathStatus::Failed);
    }

    #[test]
    fn disconnected_returns_failed() {
        let resource = disconnected_resource();
        let graph = SearchGraph::new(&resource, BitMask::ALL);
        let path = find_path_3d_prepared(
            &resource,
            &graph,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(4.1, 0.0, 0.1),
            NavMeshPathOptions::default(),
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

    #[test]
    fn invalid_direct_data_never_panics() {
        let invalid = NavMesh3D {
            vertices: vec![Vector3::new(0.0, 0.0, 0.0)],
            triangles: vec![tri([0, 1, 2], 1)],
        };
        let result = std::panic::catch_unwind(|| {
            find_path_3d(
                &invalid,
                Vector3::ZERO,
                Vector3::new(1.0, 0.0, 1.0),
                NavMeshPathOptions::default(),
            )
        });
        assert_eq!(
            result.expect("test or bench setup must succeed").status,
            NavMeshPathStatus::Failed
        );
        assert_eq!(
            project_point_3d(&invalid, Vector3::ZERO, 1.0, BitMask::ALL),
            None
        );
    }

    #[test]
    fn large_corridor_keeps_portals_when_simplify_off() {
        const CELL_COUNT: usize = 2_000;
        let nav = strip_navmesh(CELL_COUNT);
        let path = find_path_3d(
            &nav,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(CELL_COUNT as f32 - 0.1, 0.0, 0.9),
            NavMeshPathOptions {
                max_points: u32::MAX,
                simplify: false,
                ..Default::default()
            },
        );
        assert_eq!(path.status, NavMeshPathStatus::Complete);
        assert_eq!(path.points.len(), CELL_COUNT * 2 + 1);
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

    fn strip_navmesh(cell_count: usize) -> NavMesh3D {
        let mut vertices = Vec::with_capacity((cell_count + 1) * 2);
        for x in 0..=cell_count {
            vertices.push(Vector3::new(x as f32, 0.0, 0.0));
            vertices.push(Vector3::new(x as f32, 0.0, 1.0));
        }
        let mut triangles = Vec::with_capacity(cell_count * 2);
        for cell in 0..cell_count as u32 {
            let bottom = cell * 2;
            let top = bottom + 1;
            let next_bottom = bottom + 2;
            let next_top = bottom + 3;
            triangles.push(tri([bottom, next_bottom, top], 1));
            triangles.push(tri([next_bottom, next_top, top], 1));
        }
        NavMesh3D {
            vertices,
            triangles,
        }
    }

    fn disconnected_resource() -> NavMeshResource3D {
        NavMeshResource3D::from_mesh(NavMesh3D {
            vertices: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(4.0, 0.0, 0.0),
                Vector3::new(5.0, 0.0, 0.0),
                Vector3::new(4.0, 0.0, 1.0),
            ],
            triangles: vec![tri([0, 1, 2], 1), tri([3, 4, 5], 1)],
        })
    }

    fn two_route_resource() -> (NavMeshResource3D, Vector3, Vector3) {
        let mesh = NavMesh3D {
            vertices: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(2.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(1.0, 0.0, 1.0),
                Vector3::new(2.0, 0.0, 1.0),
                Vector3::new(0.0, 0.0, 2.0),
                Vector3::new(1.0, 0.0, 2.0),
                Vector3::new(2.0, 0.0, 2.0),
            ],
            triangles: vec![
                tri([0, 1, 3], 1),
                tri([1, 4, 3], 1),
                tri([1, 2, 4], 1),
                tri([2, 5, 4], 1),
                tri([3, 4, 6], 1),
                tri([4, 7, 6], 1),
                tri([4, 5, 7], 1),
                tri([5, 8, 7], 1),
            ],
        };
        let mut resource = NavMeshResource3D::from_mesh(mesh);
        resource.triangle_areas[2] = 2;
        resource.triangle_areas[3] = 2;
        (
            resource,
            Vector3::new(0.1, 0.0, 0.1),
            Vector3::new(1.9, 0.0, 0.2),
        )
    }

    fn tri(vertices: [u32; 3], layer: u8) -> NavMeshTriangle3D {
        NavMeshTriangle3D {
            vertices,
            layers: BitMask::layer(layer),
        }
    }
}
