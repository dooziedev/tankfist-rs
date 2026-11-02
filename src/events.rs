use crate::doomfist::ActionKind;
use crate::math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ability {
    RocketPunch,
    SeismicSlam,
    Melee,
    HandCannon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    Melee,
    Pellet,
    Headshot,
    Impact,
    WallSlam,
    Slam,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    Completed,
    Cancelled,
    HitTarget,
    HitWall,
    TimedOut,
}

/// Everything that happens in the sim is reported as an event, so abilities can react to
/// each other (e.g. the passive listens for damage) and tests can check exact behaviour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    ActionStarted(ActionKind),
    ActionEnded(ActionKind, EndReason),
    Jumped,
    /// The Hand Cannon fired a shot.
    Shot,
    DamageDealt { ability: Ability, target: u32, amount: f32, kind: HitKind },
    Knockback { target: u32, velocity: Vec3 },
    DamageTaken { raw: f32, taken: f32, blocked: f32 },
    OverhealthGained { amount: f32, total: f32 },
    Empowered,
    EmpowerConsumed,
    EmpowerExpired,
}
