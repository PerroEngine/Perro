use super::*;
use smallvec::SmallVec;

#[derive(Clone, Copy)]
struct Bounds {
    min: [f64; 2],
    max: [f64; 2],
}

impl Bounds {
    fn triangle(mesh: &NavMesh3D, index: usize) -> Self {
        let points = triangle_points(mesh, index);
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for point in points {
            for (axis, value) in [point.x, point.z].into_iter().enumerate() {
                min[axis] = min[axis].min(f64::from(value));
                max[axis] = max[axis].max(f64::from(value));
            }
        }
        for axis in 0..2 {
            // The projection accepts slightly negative barycentric weights.
            // Include that tolerance and float reconstruction roundoff in the
            // bounds, otherwise a point just beyond an edge could be culled.
            let extent = max[axis] - min[axis];
            let margin = 4.0 * f64::from(POINT_EPSILON) * extent
                + 32.0
                    * f64::from(f32::EPSILON)
                    * (min[axis].abs().max(max[axis].abs()) + extent + 1.0);
            min[axis] -= margin;
            max[axis] += margin;
        }
        Self { min, max }
    }

    fn union(self, other: Self) -> Self {
        Self {
            min: [self.min[0].min(other.min[0]), self.min[1].min(other.min[1])],
            max: [self.max[0].max(other.max[0]), self.max[1].max(other.max[1])],
        }
    }

    fn distance2(self, point: Vector3) -> f64 {
        [point.x, point.z]
            .into_iter()
            .enumerate()
            .map(|(axis, value)| {
                let value = f64::from(value);
                let delta = (self.min[axis] - value)
                    .max(value - self.max[axis])
                    .max(0.0);
                delta * delta
            })
            .sum()
    }
}

struct Node {
    bounds: Bounds,
    start: usize,
    count: usize,
    children: Option<[usize; 2]>,
}

#[derive(Default)]
pub(super) struct TriangleIndex {
    nodes: Vec<Node>,
    triangles: Vec<usize>,
}

impl TriangleIndex {
    pub(super) fn new(mesh: &NavMesh3D) -> Self {
        let mut index = Self {
            nodes: Vec::new(),
            triangles: (0..mesh.triangles.len()).collect(),
        };
        let bounds: Vec<_> = (0..mesh.triangles.len())
            .map(|i| Bounds::triangle(mesh, i))
            .collect();
        if !bounds.is_empty() {
            index.build(&bounds, 0, bounds.len());
        }
        index
    }

    fn build(&mut self, bounds: &[Bounds], start: usize, count: usize) -> usize {
        let merged = self.triangles[start..start + count]
            .iter()
            .copied()
            .map(|triangle| bounds[triangle])
            .reduce(Bounds::union)
            .expect("nonempty BVH range");
        let node = self.nodes.len();
        self.nodes.push(Node {
            bounds: merged,
            start,
            count,
            children: None,
        });
        if count > 8 {
            let axis = usize::from(merged.max[1] - merged.min[1] > merged.max[0] - merged.min[0]);
            let half = count / 2;
            self.triangles[start..start + count].select_nth_unstable_by(half, |&a, &b| {
                (bounds[a].min[axis] + bounds[a].max[axis])
                    .total_cmp(&(bounds[b].min[axis] + bounds[b].max[axis]))
                    .then(a.cmp(&b))
            });
            let left = self.build(bounds, start, half);
            let right = self.build(bounds, start + half, count - half);
            self.nodes[node].children = Some([left, right]);
        }
        node
    }

    pub(super) fn nearest(
        &self,
        mesh: &NavMesh3D,
        point: Vector3,
        max_distance: f32,
        layers: BitMask,
        blocked: &[bool],
    ) -> Option<ProjectedPoint> {
        if self.nodes.is_empty() {
            return None;
        }
        let max_distance = max_distance.max(0.0);
        let mut limit = max_distance * max_distance;
        let mut best: Option<ProjectedPoint> = None;
        let mut stack = SmallVec::<[usize; 64]>::new();
        stack.push(0);
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index];
            // The exact metric below is f32; allow its final rounding at the
            // bound so equal-distance candidates remain eligible.
            if node.bounds.distance2(point)
                > f64::from(limit) * (1.0 + 8.0 * f64::from(f32::EPSILON))
            {
                continue;
            }
            if let Some([left, right]) = node.children {
                if self.nodes[left].bounds.distance2(point)
                    <= self.nodes[right].bounds.distance2(point)
                {
                    stack.push(right);
                    stack.push(left);
                } else {
                    stack.push(left);
                    stack.push(right);
                }
                continue;
            }
            for &triangle in &self.triangles[node.start..node.start + node.count] {
                if blocked.get(triangle).copied().unwrap_or(false)
                    || !mesh.triangles[triangle].layers.intersects(layers)
                {
                    continue;
                }
                let [a, b, c] = triangle_points(mesh, triangle);
                let projected = closest_point_on_triangle_xz(point, a, b, c);
                let distance2 = distance2_xz(point, projected);
                if distance2 <= limit
                    && best.is_none_or(|current| {
                        distance2 < current.distance2
                            || (distance2 == current.distance2 && triangle < current.triangle)
                    })
                {
                    best = Some(ProjectedPoint {
                        point: projected,
                        triangle,
                        distance2,
                    });
                    limit = distance2;
                }
            }
        }
        best
    }
}
