//! A simple test world: flat ground at y = 0, box walls, and target dummies.

use crate::events::{Ability, Event, HitKind};
use crate::math::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn contains_inflated(&self, p: Vec3, r: f32) -> bool {
        p.x >= self.min.x - r
            && p.x <= self.max.x + r
            && p.y >= self.min.y - r
            && p.y <= self.max.y + r
            && p.z >= self.min.z - r
            && p.z <= self.max.z + r
    }
}

/// A target was punched and will take wall damage if it hits a wall before `window` runs out.
#[derive(Clone, Copy, Debug)]
pub struct PunchedState {
    pub window: f32,
    pub wall_damage: f32,
}

#[derive(Clone, Debug)]
pub struct Target {
    pub id: u32,
    /// Feet position.
    pub pos: Vec3,
    pub vel: Vec3,
    pub radius: f32,
    pub height: f32,
    pub health: f32,
    pub max_health: f32,
    /// Flat knockback multiplier: 1.0 for most heroes, 0.6 for tanks (Reinhardt: 21 vs 35 m/s).
    /// The upward part isn't scaled. [measured 2026-11-02]
    pub knockback_scale: f32,
    /// Seconds left in the knocked state (flat speed barely fades; normal braking after).
    pub knock_timer: f32,
    pub grounded: bool,
    pub punched: Option<PunchedState>,
}

impl Target {
    pub fn new(id: u32, pos: Vec3) -> Self {
        Self {
            id,
            pos,
            vel: Vec3::ZERO,
            radius: 0.4,
            height: 1.8,
            health: 250.0,
            max_health: 250.0,
            knockback_scale: 1.0,
            knock_timer: 0.0,
            grounded: pos.y <= 0.0,
            punched: None,
        }
    }

    pub fn center(&self) -> Vec3 {
        self.pos + Vec3::UP * (self.height * 0.5)
    }
}

#[derive(Clone, Debug)]
pub struct World {
    pub walls: Vec<Aabb>,
    pub targets: Vec<Target>,
    pub gravity: f32,
    // Knockback, measured 2026-11-02 from dummies in logs/hit-01.txt:
    /// How long a knocked target stays in the knocked state, s (20 ticks; counted from
    /// positions, since some log lines were garbled).
    pub knock_time: f32,
    /// Flat speed fades by this fraction per second while knocked (35 -> 34.24 over 0.29 s).
    pub knock_fade: f32,
    /// After that, normal ground braking, like Doomfist's: extra braking above `overspeed` and
    /// constant friction (matched tick for tick at 15, 21 and 35 m/s). `max_ground_speed` is a
    /// safety cap only; no cap showed up in the logs.
    pub max_ground_speed: f32,
    pub overspeed: f32,
    pub overspeed_decel: f32,
    pub ground_friction: f32,
}

pub fn blocked(walls: &[Aabb], p: Vec3, r: f32) -> bool {
    walls.iter().any(|w| w.contains_inflated(p, r))
}

impl World {
    pub fn flat(gravity: f32) -> Self {
        Self {
            walls: Vec::new(),
            targets: Vec::new(),
            gravity,
            knock_time: 0.32,
            knock_fade: 0.114,
            max_ground_speed: 1000.0,
            overspeed: 6.2,
            overspeed_decel: 11.0,
            ground_friction: 82.5,
        }
    }

    pub fn target(&self, id: u32) -> Option<&Target> {
        self.targets.iter().find(|t| t.id == id)
    }

    /// Moves a sphere from `a` to `b`. Returns how far (0..1) it got before touching a wall.
    pub fn sweep(&self, a: Vec3, b: Vec3, r: f32) -> Option<f32> {
        let n = (((b - a).length() / 0.05).ceil() as usize).max(1);
        for i in 1..=n {
            if blocked(&self.walls, a.lerp(b, i as f32 / n as f32), r) {
                return Some((i - 1) as f32 / n as f32);
            }
        }
        None
    }

    /// Moves targets one tick. Punched targets that reach a wall while still sliding take
    /// wall-slam damage.
    pub fn step(&mut self, dt: f32, events: &mut Vec<Event>) {
        let g = self.gravity;
        let (knock_fade, cap, over, over_decel, friction) =
            (self.knock_fade, self.max_ground_speed, self.overspeed, self.overspeed_decel, self.ground_friction);
        let walls = &self.walls;
        for t in self.targets.iter_mut() {
            let mut h = t.vel.horizontal();
            let mut speed = h.length();
            if t.knock_timer > 0.0 {
                t.knock_timer -= dt;
                speed *= 1.0 - knock_fade * dt;
            } else if t.grounded && speed > 0.0 {
                speed = speed.min(cap);
                if speed > over {
                    speed -= over_decel * dt * (speed - over);
                }
                speed = (speed - friction * dt).max(0.0);
            }
            h = h.normalize_or_zero() * speed;
            t.vel.x = h.x;
            t.vel.z = h.z;
            if !t.grounded {
                t.vel.y -= g * dt;
            }

            let next = t.pos + t.vel * dt;
            if blocked(walls, next + Vec3::UP * (t.height * 0.5), t.radius) {
                if let Some(p) = t.punched.take() {
                    t.health -= p.wall_damage;
                    events.push(Event::DamageDealt {
                        ability: Ability::RocketPunch,
                        target: t.id,
                        amount: p.wall_damage,
                        kind: HitKind::WallSlam,
                    });
                }
                t.vel.x = 0.0;
                t.vel.z = 0.0;
                t.knock_timer = 0.0;
                t.pos.y = next.y.max(0.0);
            } else {
                t.pos = next;
            }

            if t.pos.y <= 0.0 {
                t.pos.y = 0.0;
                t.vel.y = t.vel.y.max(0.0);
                t.grounded = t.vel.y <= 0.0;
            } else {
                t.grounded = false;
            }

            // Still a wall slam while sliding; over once it stops (or the window runs out).
            if let Some(p) = &mut t.punched {
                p.window -= dt;
                if p.window <= 0.0 || (t.knock_timer <= 0.0 && t.vel.horizontal().length() < 0.05) {
                    t.punched = None;
                }
            }
        }
    }
}
