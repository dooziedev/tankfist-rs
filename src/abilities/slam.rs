//! Seismic Slam: leap forward on a fixed-speed arc, damage enemies in front on touchdown,
//! then a short slide on the ground before the ability ends and goes on cooldown.
//!
//! Measured 2026-11-02 from logs/slam-01.txt (automatic tests: 19 aim pitches, running, mid-air,
//! and Power Block at different times). The leap is its own physics, not normal movement:
//!   * launch: 30 m/s along the aim with a small upward lift, elevation clamped to 20°..70°;
//!     existing horizontal speed is kept and added, vertical speed is replaced (same from the air);
//!   * air: double gravity, horizontal speed decays toward ~5.2 m/s, falling speed has quadratic
//!     drag (terminal ~30 m/s); forward input changes nothing;
//!   * touchdown: one tick below the floor, then a snap tick with no horizontal change;
//!   * slide: normal ground movement plus an extra slow-down above 6.2 m/s, for 0.304 s after
//!     touchdown, then the slam ends and the cooldown starts.

use crate::abilities::movement::{wish_dir, MoveMode};
use crate::doomfist::{Action, ActionKind, Doomfist, Pressed};
use crate::events::{Ability, EndReason, Event, HitKind};
use crate::input::Input;
use crate::math::Vec3;
use crate::world::{blocked, World};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SlamPhase {
    /// In the air. `ticks` counts finished air ticks (0 = the launch tick).
    Air { ticks: u32 },
    /// Touched down below the floor last tick; this tick snaps to the floor.
    Touchdown,
    /// On the ground sliding; the slam still counts as active.
    Slide,
}

#[derive(Clone, Copy, Debug)]
pub struct SlamState {
    /// Seconds since the slam started.
    pub t: f32,
    pub start: Vec3,
    /// Flat direction of the leap.
    pub dir: Vec3,
    pub from_air: bool,
    pub phase: SlamPhase,
    /// Seconds since touchdown (the tick Doomfist went below the floor).
    pub since_touchdown: f32,
}

impl Doomfist {
    pub(crate) fn slam_input(&mut self, pressed: &Pressed) {
        // Pressing slam again during the leap, once it has run `recast_cancel_after`, cancels it:
        // the leap ends and normal movement takes over with the speed you have.
        if let Action::Slam(s) = self.action {
            if pressed.ability1 {
                self.slam_queued = false;
                if s.t >= self.tun.slam.recast_cancel_after && matches!(s.phase, SlamPhase::Air { .. }) {
                    self.action = Action::Idle;
                    self.slam_finish(EndReason::Cancelled);
                }
                return;
            }
        }
        // The slam launches one tick after the press (a player's take shows the button down on
        // tick N and the launch on N+1; jump, by contrast, acts on the press tick).
        let go = std::mem::replace(&mut self.slam_queued, pressed.ability1);
        if !go || self.cooldowns.slam > 0.0 || !self.can_start(ActionKind::Slam) {
            return;
        }
        self.interrupt_current();
        let sl = self.tun.slam;
        // Aimed with the camera from the press tick, not the launch tick (a player's take while
        // moving the mouse: press-tick pitch 36.44° gives the logged 19.61 m/s up exactly).
        let mut facing = self.prev_input.facing.normalize_or_zero();
        if facing == Vec3::ZERO {
            facing = self.facing;
        }
        let mut dir = facing.horizontal().normalize_or_zero();
        if dir == Vec3::ZERO {
            dir = Vec3::FORWARD;
        }
        // Aim plus a small lift, then clamp the elevation.
        let aim = facing + Vec3::UP * sl.aim_lift;
        let elevation = aim
            .y
            .atan2(aim.horizontal().length())
            .clamp(sl.min_elevation_deg.to_radians(), sl.max_elevation_deg.to_radians());
        let mut kept = if sl.keep_horizontal_momentum { self.vel.horizontal() } else { Vec3::ZERO };
        // The kept speed gets one tick of the leap's horizontal decay on the launch tick.
        // Measured 2026-11-02 (logs/punch-01, slam right after a jump-cancelled punch):
        // 30.95 m/s carried in -> 30.55 kept (58.74 - 28.19 launch).
        let ks = kept.length();
        if ks > sl.air_decay_floor {
            let dt = self.tun.dt();
            kept = kept * ((ks - sl.air_decay_rate * dt * (ks - sl.air_decay_floor)) / ks);
        }
        self.vel = kept + dir * (sl.launch_speed * elevation.cos()) + Vec3::UP * (sl.launch_speed * elevation.sin());
        let from_air = !self.grounded;
        self.grounded = false;
        self.action = Action::Slam(SlamState {
            t: 0.0,
            start: self.pos,
            dir,
            from_air,
            phase: SlamPhase::Air { ticks: 0 },
            since_touchdown: 0.0,
        });
        self.emit(Event::ActionStarted(ActionKind::Slam));
    }

    /// Turns the flat velocity toward the keys by a sideways push of `push` m/s (never past the
    /// keys' direction), keeping its speed. Keys pointing back against the motion do nothing.
    fn steer_sideways(&mut self, input: &Input, push: f32) {
        let m = self.tun.movement;
        let (wish, _) = wish_dir(input, self.facing, m.backward_mult);
        let h = self.vel.horizontal();
        let speed = h.length();
        if wish == Vec3::ZERO || speed < 1e-3 {
            return;
        }
        let along = h * (1.0 / speed);
        let side = Vec3::UP.cross(along);
        let (w_along, w_side) = (wish.dot(along), wish.dot(side));
        if w_along < 0.0 || w_side.abs() < 1e-6 {
            return;
        }
        let max_push = speed * w_side.abs() / w_along.max(1e-4);
        let p = push.min(max_push) * w_side.signum();
        let v = (along * speed + side * p).normalize_or_zero() * speed;
        self.vel.x = v.x;
        self.vel.z = v.z;
    }

    /// The first slide tick with jump held: time for a bunny hop.
    fn bhop_now(s: &SlamState, enabled: bool, input: &Input, dt: f32) -> bool {
        enabled && input.jump && s.phase == SlamPhase::Slide && s.since_touchdown < dt * 1.5
    }

    pub(crate) fn slam_update(&mut self, mut s: SlamState, input: &Input, dt: f32) -> (Action, MoveMode) {
        let sl = self.tun.slam;
        s.t += dt;
        if Self::bhop_now(&s, sl.bhop_on_held_jump, input, dt) {
            // `slam_after_move` does the hop itself.
            return (Action::Slam(s), MoveMode::Locked);
        }
        let mode = match s.phase {
            // Moved by `slam_after_move`, which has the world for wall checks.
            SlamPhase::Air { .. } | SlamPhase::Touchdown => MoveMode::Locked,
            // The extra slow-down above 6.2 m/s is general ground movement (movement.rs).
            SlamPhase::Slide => MoveMode::Walk { speed_mult: 1.0, can_jump: sl.slide_can_jump },
        };
        (Action::Slam(s), mode)
    }

    /// Runs after movement: moves Doomfist through the leap, lands, and ends the slam.
    pub(crate) fn slam_after_move(&mut self, world: &mut World, input: &Input) {
        let Action::Slam(mut s) = self.action else {
            return;
        };
        let sl = self.tun.slam;
        let dt = self.tun.dt();
        match s.phase {
            SlamPhase::Air { ticks } => {
                if ticks > 0 {
                    self.slam_air_forces(dt, input);
                }
                self.slam_integrate(world, dt);
                s.phase = SlamPhase::Air { ticks: ticks + 1 };
                if self.pos.y <= 0.0 && self.vel.y < 0.0 {
                    // Stays below the floor this tick, like a normal landing.
                    s.phase = SlamPhase::Touchdown;
                    self.action = Action::Slam(s);
                    self.slam_land(world);
                    return;
                }
                if s.t >= sl.max_duration {
                    self.action = Action::Idle;
                    self.slam_finish(EndReason::TimedOut);
                    return;
                }
            }
            SlamPhase::Touchdown => {
                // Snap to the floor; speed is untouched this tick, but keys turn it with a
                // sideways push of one tick of ground acceleration (a player's take: 3.08 m/s
                // sideways, 19.083 m/s before and after).
                self.pos.y = 0.0;
                self.vel.y = 0.0;
                self.grounded = true;
                self.steer_sideways(input, self.tun.movement.ground_accel * dt);
                self.slam_integrate(world, dt);
                s.since_touchdown += dt;
                s.phase = SlamPhase::Slide;
            }
            SlamPhase::Slide if Self::bhop_now(&s, sl.bhop_on_held_jump, input, dt) => {
                // Bunny hop: jump held through touchdown jumps on the first slide tick (the tick
                // after the snap), with air rules for the horizontal speed, and skips the slide.
                // Measured from a player's take: dip, snap (speed unchanged), then jump 5.72 up
                // with 19.08 -> 18.66 flat.
                self.vel.y = self.tun.movement.jump_speed - self.tun.movement.gravity * dt;
                self.grounded = false;
                let (wish, dir_mult) = wish_dir(input, self.facing, self.tun.movement.backward_mult);
                self.air_accelerate(wish, self.tun.movement.walk_speed * dir_mult, self.tun.movement.air_accel, dt);
                self.slam_integrate(world, dt);
                self.emit(Event::Jumped);
                self.action = Action::Idle;
                self.slam_finish(EndReason::Completed);
                return;
            }
            SlamPhase::Slide => s.since_touchdown += dt,
        }
        if s.phase == SlamPhase::Slide && s.since_touchdown >= sl.slide_time - 1e-4 {
            self.action = Action::Idle;
            self.slam_finish(EndReason::Completed);
        } else {
            self.action = Action::Slam(s);
        }
    }

    /// One air tick: gravity, falling drag, horizontal decay, and steering toward the keys.
    fn slam_air_forces(&mut self, dt: f32, input: &Input) {
        let sl = self.tun.slam;
        let g = self.tun.movement.gravity * sl.gravity_mult;
        let vy = self.vel.y;
        let fall_drag = if vy < 0.0 { sl.fall_drag * vy * vy } else { 0.0 };
        self.vel.y = vy + (fall_drag - g) * dt;

        let h = self.vel.horizontal();
        let speed = h.length();
        if speed > sl.air_decay_floor {
            let next = speed - sl.air_decay_rate * dt * (speed - sl.air_decay_floor);
            self.vel.x = h.x * next / speed;
            self.vel.z = h.z * next / speed;
        }

        // Steering: keys pointing partly sideways push the leap toward them by `air_accel` per
        // second without changing speed (a player's take: the leap followed the camera with W
        // held, at ~0.5°/tick when turning fast). Keys straight along the leap do nothing.
        let push = self.tun.movement.air_accel * sl.air_control * dt;
        self.steer_sideways(input, push);
    }

    /// Moves one axis at a time so walls stop only the axis that hits them.
    fn slam_integrate(&mut self, world: &World, dt: f32) {
        let m = self.tun.movement;
        let lift = Vec3::UP * m.body_center_height;
        let step = self.vel * dt;
        for axis in 0..3 {
            let mut next = self.pos;
            match axis {
                0 => next.x += step.x,
                1 => next.y += step.y,
                _ => next.z += step.z,
            }
            if blocked(&world.walls, next + lift, m.collision_radius) {
                match axis {
                    0 => self.vel.x = 0.0,
                    1 => self.vel.y = 0.0,
                    _ => self.vel.z = 0.0,
                }
            } else {
                self.pos = next;
            }
        }
    }

    /// Damage on touchdown. The slam keeps running (slide) after this.
    fn slam_land(&mut self, world: &mut World) {
        let sl = self.tun.slam;
        // Ahead = the way the leap was moving (or facing, if it had stopped).
        let mut dir = self.vel.horizontal().normalize_or_zero();
        if dir == Vec3::ZERO {
            dir = self.facing.horizontal().normalize_or_zero();
        }
        let side = Vec3::UP.cross(dir);
        let mut any_hit = false;
        for t in world.targets.iter_mut() {
            if t.health <= 0.0 {
                continue;
            }
            let offset = t.pos - self.pos;
            let (ahead, across) = (offset.dot(dir), offset.dot(side).abs());
            let half_width = ahead * sl.hit_cone_half_angle_deg.to_radians().tan();
            if ahead < sl.hit_ahead_min || ahead > sl.hit_ahead_max || across > half_width || offset.y.abs() > sl.vertical_reach {
                continue;
            }
            t.health -= sl.damage;
            let n = offset.horizontal().normalize_or_zero();
            t.vel = n * sl.knockback + Vec3::UP * sl.knockback_up;
            if t.vel.y > 0.0 {
                t.grounded = false;
            }
            any_hit = true;
            let (id, velocity) = (t.id, t.vel);
            self.emit(Event::DamageDealt {
                ability: Ability::SeismicSlam,
                target: id,
                amount: sl.damage,
                kind: HitKind::Slam,
            });
            if velocity != Vec3::ZERO {
                self.emit(Event::Knockback { target: id, velocity });
            }
        }
        if any_hit && self.tun.interactions.slam_hit_empowers {
            self.empower();
        }
    }

    pub(crate) fn slam_finish(&mut self, reason: EndReason) {
        self.cooldowns.slam = self.tun.slam.cooldown;
        self.emit(Event::ActionEnded(ActionKind::Slam, reason));
    }
}
