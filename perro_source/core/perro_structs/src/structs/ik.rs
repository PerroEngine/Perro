#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum IKTargetSolver {
    #[default]
    FABRIK,
    CCD,
}
