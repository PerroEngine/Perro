#[derive(Clone, Copy)]
pub struct Frustum {
    pub planes: [glam::Vec4; 6], // each plane: (a, b, c, d)
}

impl Frustum {
    pub fn from_matrix(vp: &glam::Mat4) -> Self {
        let m = *vp;

        let mut planes = [
            m.row(3) + m.row(0), // Left
            m.row(3) - m.row(0), // Right
            m.row(3) + m.row(1), // Bottom
            m.row(3) - m.row(1), // Top
            m.row(3) + m.row(2), // Near
            m.row(3) - m.row(2), // Far
        ];

        // Normalize planes: divide by length of normal (xyz components)
        for plane in &mut planes {
            let n = plane.truncate(); // take x,y,z
            *plane /= n.length();
        }

        Self { planes }
    }

    pub fn contains_sphere(&self, center: glam::Vec3, radius: f32) -> bool {
        const CULL_BIAS: f32 = 0.8;
        for plane in &self.planes {
            let d = plane.x * center.x + plane.y * center.y + plane.z * center.z + plane.w;
            if d < -radius * CULL_BIAS {
                return false;
            }
        }
        true
    }
}
