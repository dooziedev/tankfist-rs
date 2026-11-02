use crate::math::Vec3;

/// What the player is doing on one tick. Buttons are "held" states; presses are found by
/// comparing with the previous tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// Aim direction (unit vector).
    pub facing: Vec3,
    /// Movement keys: forward/back and right/left, each -1..1.
    pub move_forward: f32,
    pub move_right: f32,
    pub jump: bool,
    pub crouch: bool,
    /// Rocket Punch.
    pub secondary: bool,
    /// Seismic Slam.
    pub ability1: bool,
    /// Power Block.
    pub ability2: bool,
    /// Quick melee.
    pub melee: bool,
    /// Hand Cannon.
    pub primary: bool,
}

impl Input {
    pub fn facing(facing: Vec3) -> Self {
        Self { facing: facing.normalize_or_zero(), ..Default::default() }
    }
}
