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

/// A Hand Cannon pellet in flight.
#[derive(Clone, Copy, Debug)]
pub struct Projectile {
    pub pos: Vec3,
    pub vel: Vec3,
    pub travelled: f32,
    pub damage: f32,
    pub head_mult: f32,
    pub falloff_start: f32,
    pub falloff_end: f32,
    pub falloff_min: f32,
    pub max_range: f32,
}

impl Projectile {
    /// Damage multiplier after travelling `d` m.
    pub fn falloff(&self, d: f32) -> f32 {
        if d <= self.falloff_start {
            1.0
        } else if d >= self.falloff_end {
            self.falloff_min
        } else {
            1.0 + (self.falloff_min - 1.0) * (d - self.falloff_start) / (self.falloff_end - self.falloff_start)
        }
    }
}

/// Height of the head zone at the top of a target, m (headshots). [guess]
pub const HEAD_ZONE: f32 = 0.3;

#[derive(Clone, Debug)]
pub struct World {
    pub walls: Vec<Aabb>,
    pub targets: Vec<Target>,
    pub projectiles: Vec<Projectile>,
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
            projectiles: Vec::new(),
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

    /// Moves pellets one tick: each hits the first live target it passes through (a head-zone
    /// hit counts double), or stops at a wall or its range.
    fn step_projectiles(&mut self, dt: f32, events: &mut Vec<Event>) {
        let mut keep = Vec::with_capacity(self.projectiles.len());
        for mut p in std::mem::take(&mut self.projectiles) {
            let from = p.pos;
            let to = from + p.vel * dt;
            let step = (to - from).length();
            let mut hit: Option<(usize, f32, Vec3)> = None;
            let n = ((step / 0.1).ceil() as usize).max(1);
            'search: for k in 1..=n {
                let f = k as f32 / n as f32;
                let q = from.lerp(to, f);
                for (i, t) in self.targets.iter().enumerate() {
                    if t.health <= 0.0 || q.y < t.pos.y || q.y > t.pos.y + t.height {
                        continue;
                    }
                    if (q - t.pos).horizontal().length() <= t.radius + 0.05 {
                        hit = Some((i, f, q));
                        break 'search;
                    }
                }
            }
            if let Some((i, f, q)) = hit {
                let t = &mut self.targets[i];
                let head = q.y > t.pos.y + t.height - HEAD_ZONE;
                let amount = p.damage * p.falloff(p.travelled + step * f) * if head { p.head_mult } else { 1.0 };
                t.health -= amount;
                events.push(Event::DamageDealt {
                    ability: Ability::HandCannon,
                    target: t.id,
                    amount,
                    kind: if head { HitKind::Headshot } else { HitKind::Pellet },
                });
                continue;
            }
            if self.sweep(from, to, 0.05).is_some() || to.y < 0.0 {
                continue;
            }
            p.pos = to;
            p.travelled += step;
            if p.travelled < p.max_range {
                keep.push(p);
            }
        }
        self.projectiles = keep;
    }

    /// Moves targets one tick. Punched targets that reach a wall while still sliding take
    /// wall-slam damage.
    pub fn step(&mut self, dt: f32, events: &mut Vec<Event>) {
        self.step_projectiles(dt, events);
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
