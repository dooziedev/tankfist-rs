//! Rocket Punch: hold to charge, release to dash. Measured 2026-11-02 (logs/punch-01.txt):
//!   * the dash can't start before 0.208 s of hold; an early release waits for it;
//!   * charge counts from 0.144 s to 0.992 s of hold and sets the number of full-speed ticks:
//!     floor(8 + 20 * charge); holding longer adds nothing and never auto-fires;
//!   * dash: one partial tick (0.56 of a tick), then that many ticks at exactly 44 m/s, always
//!     level on flat ground, then one tick at the 6 m/s exit speed and normal movement after;
//!   * jumping once the dash has run ~0.1 s cancels it into a boosted jump (30.95 m/s flat,
//!     6.075 m/s up), which then fades in the air (see movement's `air_excess_decay`);
//!   * a jump before that only hops: +0.051 m, held one tick, then falls back under gravity
//!     while the dash continues;
//!   * a slam during the dash ends it with the 6 m/s exit speed, which the slam adds to;
//!   * the 4 s cooldown starts 0.368 s after the dash ends.
//!
//! Hitting an enemy (not measured yet) knocks them back; if they hit a wall soon after, they
//! take wall-slam damage. An empowered punch (from Power Block) hits harder and also hits enemies
//! near the main target.

use crate::abilities::movement::MoveMode;
use crate::doomfist::{Action, ActionKind, Doomfist, Pressed};
use crate::events::{Ability, EndReason, Event, HitKind};
use crate::input::Input;
use crate::math::{lerp, point_segment_distance, Vec3};
use crate::world::{PunchedState, World};

#[derive(Clone, Copy, Debug)]
pub struct PunchState {
    /// Seconds since launch.
    pub t: f32,
    /// 0..1 charge at release (dash length).
    pub charge: f32,
    /// 0..1 charge for damage and knockback (its own hold window).
    pub power: f32,
    pub dir: Vec3,
    pub speed: f32,
    /// Dash ticks done so far (the first one is the partial tick).
    pub ticks_done: u32,
    /// Full-speed ticks after the partial one.
    pub full_ticks: u32,
    pub empowered: bool,
    /// Early-jump hop: ticks since it started (None = no hop), and its falling speed.
    pub hop_ticks: Option<u32>,
    pub hop_vy: f32,
    /// Launched in the air: the dash falls, starting from 0 vertical speed on the first tick.
    pub airborne: bool,
    pub air_vy: f32,
}

/// Limits how far up or down the punch can go.
pub fn clamp_pitch(facing: Vec3, max_deg: f32) -> Vec3 {
    let h = facing.horizontal().normalize_or_zero();
    if h == Vec3::ZERO {
        return Vec3::FORWARD;
    }
    let pitch = facing.normalize_or_zero().y.clamp(-1.0, 1.0).asin();
    let m = max_deg.to_radians();
    let p = pitch.clamp(-m, m);
    h * p.cos() + Vec3::UP * p.sin()
}

/// Flat direction of `facing`, limited to within `cone_deg` of the flat direction `dir`.
/// While dashing, the camera can only look this far away from the dash (measured: ±60°).
pub fn clamp_to_cone(facing: Vec3, dir: Vec3, cone_deg: f32) -> Vec3 {
    let f = facing.horizontal().normalize_or_zero();
    if f == Vec3::ZERO {
        return dir;
    }
    let angle = (dir.z * f.x - dir.x * f.z).atan2(dir.dot(f)); // signed, positive = toward +x
    let a = angle.clamp(-cone_deg.to_radians(), cone_deg.to_radians());
    let (sin, cos) = a.sin_cos();
    // Rotate `dir` by `a` (positive turns toward +x when dir is +z).
    Vec3::new(dir.x * cos + dir.z * sin, 0.0, -dir.x * sin + dir.z * cos)
}

/// 0..1 charge for a hold of `held` seconds.
pub fn charge_fraction(held: f32, start: f32, full: f32) -> f32 {
    ((held - start) / (full - start)).clamp(0.0, 1.0)
}

/// First target the fist touches while moving from `a` to `b`, and how far along (0..1).
fn first_target_hit(world: &World, a: Vec3, b: Vec3, r: f32) -> Option<(usize, f32)> {
    let n = (((b - a).length() / 0.05).ceil() as usize).max(1);
    for i in 0..=n {
        let f = i as f32 / n as f32;
        let p = a.lerp(b, f);
        for (idx, t) in world.targets.iter().enumerate() {
            if t.health <= 0.0 {
                continue;
            }
            let (d, _) = point_segment_distance(p, t.pos, t.pos + Vec3::UP * t.height);
            if d <= t.radius + r {
                return Some((idx, f));
            }
        }
    }
    None
}

impl Doomfist {
    pub(crate) fn punch_input(&mut self, input: &Input, pressed: &Pressed) {
        // Holding the button through the cooldown starts charging the moment it's ready
        // (log: held from 0.37 s before ready, charge began on the tick it hit 0). Cutting into
        // another ability still needs a fresh press.
        let wants = pressed.secondary || (input.secondary && self.action.kind().is_none());
        if !wants || self.cooldowns.punch > 0.0 || !self.can_start(ActionKind::PunchCharge) {
            return;
        }
        self.interrupt_current();
        self.action = Action::PunchCharge { t: 0.0, held: None };
        self.emit(Event::ActionStarted(ActionKind::PunchCharge));
    }

    pub(crate) fn punch_charge_update(
        &mut self,
        t: f32,
        mut held: Option<f32>,
        input: &Input,
        dt: f32,
    ) -> (Action, MoveMode) {
        let p = self.tun.punch;
        // `t` is how long the button has been held before this tick.
        if held.is_none() {
            let auto_fire = p.max_hold_at_full.is_some_and(|h| t >= p.charge_full + h);
            if !input.secondary || auto_fire {
                held = Some(t);
            }
        }
        if let Some(h) = held {
            if t + 1e-4 >= p.min_launch_time {
                self.emit(Event::ActionEnded(ActionKind::PunchCharge, EndReason::Completed));
                let s = self.punch_launch(h);
                self.emit(Event::ActionStarted(ActionKind::Punch));
                // The release tick still moves normally (a player's take: kept falling and
                // steering in the air on the release tick; the dash starts next tick).
                return (Action::Punch(s), MoveMode::Walk { speed_mult: p.charge_move_mult, can_jump: false });
            }
        }
        (
            Action::PunchCharge { t: t + dt, held },
            MoveMode::Walk { speed_mult: p.charge_move_mult, can_jump: p.can_jump_while_charging },
        )
    }

    pub(crate) fn punch_charge_cancelled(&mut self) {
        self.cooldowns.punch = self.tun.punch.cancel_cooldown;
        self.emit(Event::ActionEnded(ActionKind::PunchCharge, EndReason::Cancelled));
    }

    fn punch_launch(&mut self, held: f32) -> PunchState {
        let p = self.tun.punch;
        let charge = charge_fraction(held, p.charge_start, p.charge_full);
        let empowered = self.empowered;
        if empowered {
            self.empowered = false;
            self.emit(Event::EmpowerConsumed);
        }
        let (lo, hi) = if empowered {
            (p.empowered_min_ticks, p.empowered_max_ticks)
        } else {
            (p.min_dash_ticks, p.max_dash_ticks)
        };
        let full_ticks = (lo as f32 + (hi - lo) as f32 * charge + 1e-4).floor() as u32;
        let speed = if empowered { p.speed * p.empowered_speed_mult } else { p.speed };
        PunchState {
            t: 0.0,
            charge,
            power: charge_fraction(held, p.damage_charge_start, p.damage_charge_full),
            dir: clamp_pitch(self.facing, p.max_pitch_deg),
            speed,
            ticks_done: 0,
            full_ticks,
            empowered,
            hop_ticks: None,
            hop_vy: 0.0,
            airborne: !self.grounded,
            air_vy: 0.0,
        }
    }

    pub(crate) fn punch_update(
        &mut self,
        mut s: PunchState,
        input: &Input,
        world: &mut World,
        dt: f32,
    ) -> (Action, MoveMode) {
        let p = self.tun.punch;
        let m = self.tun.movement;
        s.t += dt;

        // Jump cancel (measured from manual takes, logs/punch-turn-01.txt): with jump held once the
        // window is open (and not on the press tick, which only hops), the dash ends with its
        // 6 m/s exit speed plus a boost toward last tick's camera, clamped to the 60° cone.
        let jump_pressed = input.jump && !self.prev_input.jump;
        if input.jump && !jump_pressed && s.ticks_done > 0 && s.t - dt >= p.jump_cancel_after - 1e-4 {
            let flat = s.dir.horizontal().normalize_or_zero();
            let cam = clamp_to_cone(self.prev_input.facing, flat, p.camera_cone_deg);
            // Upward speed depends on last tick's vertical speed (hop, sinking hop, or a falling
            // airborne dash): 6 player cancels fit up = 4.96 + 0.139 x that, all within 0.012.
            let up = p.jump_cancel_up + p.jump_cancel_up_keep * self.vel.y;
            self.vel = flat * p.exit_speed + cam * p.jump_cancel_boost + Vec3::UP * up;
            self.pos += self.vel * dt;
            self.grounded = false;
            self.emit(Event::Jumped);
            self.punch_finish(EndReason::Cancelled);
            // A jump cancel starts the cooldown at once, without the end-of-dash delay.
            self.cooldowns.punch = p.cooldown;
            return (Action::Idle, MoveMode::Locked);
        }

        // A jump before the cancel window only gives a small hop: the dash goes on, Doomfist
        // pops up `early_jump_hop`, holds one tick, then falls back under normal gravity.
        let mut lift_y = 0.0;
        match s.hop_ticks {
            // Only on the ground: a jump pressed during an airborne dash does nothing.
            None if jump_pressed && !s.airborne => {
                s.hop_ticks = Some(0);
                lift_y = p.early_jump_hop;
            }
            Some(k) => {
                if k >= 1 {
                    s.hop_vy -= m.gravity * dt;
                    lift_y = s.hop_vy * dt;
                }
                s.hop_ticks = Some(k + 1);
            }
            None => {}
        }

        // Past the last full-speed tick: one tick at the exit speed, then it's over.
        if s.ticks_done > s.full_ticks {
            self.vel = s.dir * p.exit_speed;
            self.pos += self.vel * dt;
            self.punch_finish(EndReason::Completed);
            return (Action::Idle, MoveMode::Locked);
        }

        // A dash started in the air falls under normal gravity from 0 vertical speed.
        if s.airborne {
            if s.ticks_done > 0 {
                s.air_vy -= m.gravity * dt;
            }
            lift_y += s.air_vy * dt;
        }

        let frac = if s.ticks_done == 0 { p.first_tick_fraction } else { 1.0 };
        // The first (partial) tick also keeps the speed you had while charging
        // (log: walking 2.75 m/s -> first tick 27.39 = 24.64 + 2.75).
        let carry = if s.ticks_done == 0 { self.vel.horizontal() } else { Vec3::ZERO };
        let dash_vel = s.dir * (s.speed * frac) + carry;
        let from = self.pos;
        let mut to = from + dash_vel * dt + Vec3::UP * lift_y;
        to.y = to.y.max(0.0); // slides along the ground when aimed down
        let lift = Vec3::UP * m.body_center_height;

        if let Some((idx, f)) = first_target_hit(world, from + lift, to + lift, p.hit_radius) {
            self.pos = from.lerp(to, f);
            self.punch_hit(&s, world, idx);
            self.vel = s.dir * p.exit_speed;
            self.punch_finish(EndReason::HitTarget);
            return (Action::Idle, MoveMode::Locked);
        }
        if let Some(f) = world.sweep(from + lift, to + lift, m.collision_radius) {
            self.pos = from.lerp(to, f);
            self.vel = Vec3::ZERO;
            self.punch_finish(EndReason::HitWall);
            return (Action::Idle, MoveMode::Locked);
        }

        self.pos = to;
        self.grounded = self.pos.y <= 0.0;
        if self.grounded && s.airborne {
            s.airborne = false;
            s.air_vy = 0.0;
            lift_y = 0.0;
        }
        self.vel = dash_vel + Vec3::UP * (lift_y / dt);
        s.ticks_done += 1;
        (Action::Punch(s), MoveMode::Locked)
    }

    fn punch_hit(&mut self, s: &PunchState, world: &mut World, idx: usize) {
        let knock_time = world.knock_time;
        let p = self.tun.punch;
        let mut impact = lerp(p.min_impact_damage, p.max_impact_damage, s.power);
        let mut wall = lerp(p.min_wall_damage, p.max_wall_damage, s.power);
        let mut kb = lerp(p.min_knockback, p.max_knockback, s.power);
        if s.empowered {
            impact *= p.empowered_damage_mult;
            wall *= p.empowered_damage_mult;
            kb *= p.empowered_knockback_mult;
        }

        let main = world.targets[idx].pos;
        let mut hit = vec![idx];
        let splash = if s.empowered { p.empowered_splash_radius } else { p.splash_radius };
        if splash > 0.0 {
            for (i, t) in world.targets.iter().enumerate() {
                if i != idx && t.health > 0.0 && t.pos.distance(main) <= splash {
                    hit.push(i);
                }
            }
        }

        let push = s.dir.horizontal().normalize_or_zero();
        for i in hit {
            let t = &mut world.targets[i];
            t.health -= impact;
            t.vel = push * (kb * t.knockback_scale) + Vec3::UP * p.knockback_up;
            t.knock_timer = knock_time;
            if t.vel.y > 0.0 {
                t.grounded = false;
            }
            t.punched = Some(PunchedState { window: p.wall_slam_window, wall_damage: wall });
            let (id, velocity) = (t.id, t.vel);
            self.emit(Event::DamageDealt {
                ability: Ability::RocketPunch,
                target: id,
                amount: impact,
                kind: HitKind::Impact,
            });
            self.emit(Event::Knockback { target: id, velocity });
        }
    }

    /// Ends the dash. The cooldown is set to include the delay before it visibly starts.
    pub(crate) fn punch_finish(&mut self, reason: EndReason) {
        let p = self.tun.punch;
        self.cooldowns.punch = p.cooldown + p.cooldown_delay;
        self.emit(Event::ActionEnded(ActionKind::Punch, reason));
    }
}
