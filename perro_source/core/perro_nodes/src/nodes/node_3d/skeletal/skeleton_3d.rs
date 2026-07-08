use crate::node_3d::Node3D;
use perro_structs::{Matrix4, Transform3D};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, Default)]
pub struct Bone3D {
    pub name: Cow<'static, str>,
    pub parent: i32,
    pub rest: Transform3D,
    pub pose: Transform3D,
    pub inv_bind: Transform3D,
}

impl Bone3D {
    pub const fn new() -> Self {
        Self {
            name: Cow::Borrowed("Bone"),
            parent: -1,
            rest: Transform3D::IDENTITY,
            pose: Transform3D::IDENTITY,
            inv_bind: Transform3D::IDENTITY,
        }
    }
}

impl Deref for Skeleton3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Skeleton3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct Skeleton3D {
    pub base: Node3D,
    pub bones: Vec<Bone3D>,
    /// Derived hot lane: precomputed inverse-bind matrix per bone, index-aligned
    /// with `bones`. The bind pose is import-time constant (never animated), so
    /// only structural changes to `bones` invalidate it — call
    /// [`Skeleton3D::refresh_inv_bind_cache`] after a bone-set load/reassignment.
    /// Empty by default; the skinning-palette builder falls back to computing
    /// `inv_bind.to_mat4()` inline whenever the lane length does not match.
    inv_bind_mats: Vec<Matrix4>,
}

impl Skeleton3D {
    #[deprecated(note = "use Skeleton3D::default()")]
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuild the derived inverse-bind matrix lane from `bones`. Cheap (one
    /// TRS→matrix conversion per bone) and only needed after the bone set
    /// changes, not on pose updates.
    pub fn refresh_inv_bind_cache(&mut self) {
        self.inv_bind_mats.clear();
        self.inv_bind_mats
            .extend(self.bones.iter().map(|bone| bone.inv_bind.to_matrix4()));
    }

    /// Precomputed inverse-bind matrices. Index-aligned with `bones` when
    /// `len() == bones.len()`; otherwise absent/stale and callers must fall
    /// back to `bone.inv_bind`.
    #[inline]
    pub fn inv_bind_mats(&self) -> &[Matrix4] {
        &self.inv_bind_mats
    }

    pub fn bone_name(&self, index: usize) -> Option<&str> {
        self.bones.get(index).map(|bone| bone.name.as_ref())
    }

    pub fn bone_index<S: AsRef<str>>(&self, name: S) -> Option<usize> {
        let name = name.as_ref();
        self.bones
            .iter()
            .position(|bone| bone.name.as_ref() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_structs::{Quaternion, Vector3};

    #[test]
    fn refresh_inv_bind_cache_matches_inline_conversion() {
        let mut skeleton = Skeleton3D::default();
        // Non-identity inv_bind so a stale/identity lane would be caught.
        let inv_bind = Transform3D::new(
            Vector3::new(1.0, -2.0, 3.0),
            Quaternion::new(0.0, 0.0, 0.382_683_43, 0.923_879_5),
            Vector3::new(2.0, 0.5, 1.0),
        );
        skeleton.bones = vec![Bone3D {
            inv_bind,
            ..Bone3D::new()
        }];
        assert!(skeleton.inv_bind_mats().is_empty(), "lane starts empty");

        skeleton.refresh_inv_bind_cache();
        assert_eq!(skeleton.inv_bind_mats().len(), skeleton.bones.len());
        // Cached matrix must equal the inline TRS→matrix conversion the palette
        // builder falls back to.
        assert_eq!(skeleton.inv_bind_mats()[0].0, inv_bind.to_mat4());
    }
}
