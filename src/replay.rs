//! Feeds a logged take's inputs into the sim and compares where Doomfist ends up each tick.
//! This is how the guesses in `tunables.rs` get checked: if the paths line up, the math is right.
//! Targets are not recreated, so use takes without hits for movement checks.

use crate::input::Input;
use crate::log::{Row, Take};
use crate::math::Vec3;
use crate::sim::Sim;
use crate::tunables::Tunables;

#[derive(Clone, Copy, Debug)]
pub struct ReplayTick {
    pub t: f64,
    pub real_pos: Vec3,
    pub real_vel: Vec3,
    pub sim_pos: Vec3,
    pub sim_vel: Vec3,
}

impl ReplayTick {
    pub fn error(&self) -> f32 {
        self.real_pos.distance(self.sim_pos)
    }
}

/// Converts a logged input row. Workshop throttle is +X = left, +Z = forward.
pub fn input_from_row(row: &Row) -> Option<Input> {
    match *row {
        Row::Input { facing, throttle_x, throttle_z, secondary, ability1, ability2, jump, crouch, .. } => {
            Some(Input {
                facing: facing.normalize_or_zero(),
                move_forward: throttle_z,
                move_right: -throttle_x,
                jump,
                crouch,
                secondary,
                ability1,
                ability2,
                melee: false,
            })
        }
        _ => None,
    }
}

pub fn replay_take(take: &Take, tun: &Tunables) -> Vec<ReplayTick> {
    // Pair each frame with the input logged on the same tick.
    let mut ticks: Vec<(f64, Vec3, Vec3, Input)> = Vec::new();
    let mut last_frame: Option<(f64, Vec3, Vec3)> = None;
    for row in &take.rows {
        match row {
            Row::Frame { t, pos, vel } => last_frame = Some((*t, *pos, *vel)),
            Row::Input { t, .. } => {
                if let (Some((ft, pos, vel)), Some(input)) = (last_frame, input_from_row(row)) {
                    if (ft - t).abs() < 1e-6 {
                        ticks.push((*t, pos, vel, input));
                    }
                }
            }
            _ => {}
        }
    }
    let Some(&(t0, pos0, vel0, input0)) = ticks.first() else {
        return Vec::new();
    };

    let mut sim = Sim::new(tun.clone(), pos0);
    sim.doom.vel = vel0;
    sim.doom.grounded = pos0.y <= 0.05 && vel0.y.abs() < 0.01;
    sim.doom.prev_input = input0;

    let mut out = vec![ReplayTick { t: t0, real_pos: pos0, real_vel: vel0, sim_pos: pos0, sim_vel: vel0 }];
    for &(t, real_pos, real_vel, input) in ticks.iter().skip(1) {
        sim.step(&input);
        out.push(ReplayTick { t, real_pos, real_vel, sim_pos: sim.doom.pos, sim_vel: sim.doom.vel });
    }
    out
}
