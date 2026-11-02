//! General movement: running, jumping, air steering, gravity, wall collision.
//! Other abilities change movement by returning a `MoveMode` for the tick.

use crate::doomfist::{Doomfist, Pressed};
use crate::events::Event;
use crate::input::Input;
use crate::math::Vec3;
use crate::world::{blocked, World};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MoveMode {
    /// Normal control, with a speed multiplier (charging punch / blocking slow you down).
    Walk { speed_mult: f32, can_jump: bool },
    /// Flying through the air on a fixed arc (Seismic Slam).
    Ballistic { gravity_mult: f32, air_control: f32 },
    /// The ability moved Doomfist itself this tick (Rocket Punch).
    Locked,
}

impl MoveMode {
    pub fn normal() -> Self {
        MoveMode::Walk { speed_mult: 1.0, can_jump: true }
    }
}

/// Where the movement keys point, flattened to the ground (unit length, or zero).
/// Also returns the top-speed multiplier for that direction (backward is slower).
pub fn wish_dir(input: &Input, facing: Vec3, backward_mult: f32) -> (Vec3, f32) {
    let fwd = facing.horizontal().normalize_or_zero();
    let right = fwd.cross(Vec3::UP);
    let dir = (fwd * input.move_forward + right * input.move_right).normalize_or_zero();
    let back = (-dir.dot(fwd)).max(0.0); // 1 = straight back, 0 = sideways or forward
    (dir, 1.0 + (backward_mult - 1.0) * back)
}

/// Moves `v` toward `target` by at most `step`.
fn move_toward(v: Vec3, target: Vec3, step: f32) -> Vec3 {
    let d = target - v;
    let len = d.length();
    if len <= step || len < 1e-6 {
        target
    } else {
        v + d * (step / len)
    }
}

impl Doomfist {
    pub(crate) fn movement_step(
        &mut self,
        input: &Input,
        pressed: &Pressed,
        mode: MoveMode,
        world: &World,
        dt: f32,
    ) {
        let m = self.tun.movement;
        let (wish, dir_mult) = wish_dir(input, self.facing, m.backward_mult);

        if mode == MoveMode::Locked {
            return;
        }
        // Landing: the log shows Doomfist dips just below the floor for one tick and only
        // counts as grounded on the next one, so a jump pressed on the dip tick does nothing.
        let mut just_landed = false;
        if !self.grounded && self.pos.y <= 0.0 && self.vel.y <= 0.0 {
            self.pos.y = 0.0;
            self.vel.y = 0.0;
            self.grounded = true;
            just_landed = true;
        }

        match mode {
            MoveMode::Locked => unreachable!(),
            MoveMode::Walk { speed_mult, can_jump } => {
                let top = m.walk_speed * speed_mult * dir_mult;
                // No jump on the landing tick itself (two player takes: jump pressed exactly as
                // they landed did nothing, and holding it didn't jump later either).
                if self.grounded && can_jump && pressed.jump && !just_landed {
                    // The jump tick already uses air rules: no ground friction (log: 4.70 -> 4.67
                    // m/s jumping just after a punch, instead of -1.32).
                    self.vel.y = m.jump_speed;
                    self.grounded = false;
                    self.emit(Event::Jumped);
                    self.air_accelerate(wish, top, m.air_accel, dt);
                } else if self.grounded {
                    self.ground_accelerate(wish, top, dt);
                } else {
                    self.air_accelerate(wish, top, m.air_accel, dt);
                }
                if !self.grounded {
                    // Gravity first, then position uses the new velocity (matches the log).
                    self.vel.y -= m.gravity * dt;
                }
            }
            MoveMode::Ballistic { gravity_mult, air_control } => {
                if air_control > 0.0 {
                    let top = m.walk_speed * dir_mult;
                    self.air_accelerate(wish, top, m.air_accel * air_control, dt);
                }
                self.vel.y -= m.gravity * gravity_mult * dt;
            }
        }

        self.integrate(world, dt);
    }

    /// Ground: constant friction, then acceleration toward the input, capped at top speed.
    /// Measured: 0 -> 3.08 -> 4.84 -> 5.50 when starting, -1.32 per tick when stopping.
    fn ground_accelerate(&mut self, wish: Vec3, top: f32, dt: f32) {
        let m = self.tun.movement;
        let mut h = self.vel.horizontal();
        let mut speed = h.length();
        if speed > m.ground_overspeed {
            let slower = speed - m.ground_overspeed_decel * dt * (speed - m.ground_overspeed);
            h = h * (slower / speed);
            speed = slower;
        }
        let after_friction = (speed - m.ground_friction * dt).max(0.0);
        if speed > 0.0 {
            h = h * (after_friction / speed);
        }
        h += wish * (m.ground_accel * dt);
        // Acceleration can't push past top speed, but faster momentum (e.g. after a punch)
        // only bleeds off through friction.
        let cap = top.max(after_friction);
        if h.length() > cap {
            h = h.normalize_or_zero() * cap;
        }
        self.vel.x = h.x;
        self.vel.z = h.z;
    }

    /// Air control, measured 2026-11-02 (logs/movement-01, movement-02, punch-turn-01):
    ///   * no keys: speed fades by air_drag + air_excess_decay x (speed - walk_speed);
    ///   * keys pointing back against the motion: velocity steps `accel` toward keys x top speed;
    ///   * at or below top speed: steps `accel` toward keys x top speed, without losing speed;
    ///   * above top speed (boosted): in the velocity's own frame the speed fades as with no
    ///     keys, and a sideways push of `accel` turns it toward the keys (never past them).
    /// One-tick check over 2761 logged air ticks (tests/air_one_tick.rs): all but 26 within
    /// 0.1 m/s; the rest are keys held sideways just under top speed, where real speed creeps up
    /// more slowly than this, plus one wall hit.
    pub(crate) fn air_accelerate(&mut self, wish: Vec3, top: f32, accel: f32, dt: f32) {
        let m = self.tun.movement;
        let h = self.vel.horizontal();
        let speed = h.length();
        // The excess is measured from the current top speed (blocking lowers it: the log fades
        // toward ~3.6 m/s while blocking in the air).
        let fade = (m.air_drag + m.air_excess_decay * (speed - top).max(0.0)) * dt;
        let step = accel * dt;
        let next = if wish == Vec3::ZERO {
            move_toward(h, Vec3::ZERO, fade)
        } else if speed < 1e-4 || h.dot(wish) < 0.0 {
            move_toward(h, wish * top, step)
        } else if speed <= top {
            // At or below top speed: step toward keys x top speed without losing speed.
            let n = move_toward(h, wish * top, step);
            if n.length() < speed { n * (speed / n.length()) } else { n }
        } else {
            let along = h * (1.0 / speed);
            let side = Vec3::UP.cross(along); // perpendicular, flat
            let w_side = wish.dot(side);
            let new_speed = if speed < top { (speed + step).min(top) } else { (speed - fade).max(top.min(speed)) };
            // Push sideways toward the keys, but not past their direction.
            let max_push = new_speed * (w_side.abs() / wish.dot(along).max(1e-4));
            let push = step.min(max_push) * w_side.signum();
            let v = along * new_speed + side * push;
            // The push turns the velocity without adding speed above the cap.
            let cap = if speed < top { new_speed } else { new_speed.max(top) };
            if v.length() > cap { v * (cap / v.length()) } else { v }
        };
        self.vel.x = next.x;
        self.vel.z = next.z;
    }

    /// Moves one axis at a time so walls stop only the axis that hits them.
    fn integrate(&mut self, world: &World, dt: f32) {
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

        if self.pos.y > 0.0 {
            self.grounded = false;
        } else if self.grounded {
            self.pos.y = 0.0;
            self.vel.y = 0.0;
        }
        // Falling below the floor: stay there this tick, snap and land at the start of the next.
    }
}
