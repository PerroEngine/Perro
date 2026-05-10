use perro_ids::NodeID;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IKTargetParams {
    pub skeleton: NodeID,
    pub bone_index: i32,
    pub chain_length: u32,
    pub iterations: u32,
    pub tolerance: f32,
    pub weight: f32,
    pub match_rotation: bool,
    pub solver: IKTargetSolver,
}

impl Default for IKTargetParams {
    fn default() -> Self {
        Self::new()
    }
}

impl IKTargetParams {
    pub const fn new() -> Self {
        Self {
            skeleton: NodeID::nil(),
            bone_index: -1,
            chain_length: 2,
            iterations: 8,
            tolerance: 0.01,
            weight: 1.0,
            match_rotation: true,
            solver: IKTargetSolver::FABRIK,
        }
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum IKTargetSolver {
    #[default]
    FABRIK,
    CCD,
}
