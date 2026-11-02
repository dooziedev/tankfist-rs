//! Hand Cannon: a 4-shot shotgun that refills on its own. Measured 2026-11-02 from
//! logs/gun-01.txt (automatic gun tests):
//!   * 11 pellets x 5 damage (headshots x2), one shot every 0.336 s while held;
//!   * no manual reload: the first shell comes back 0.72 s after the last shot, then one every
//!     0.40 s; holding fire on an empty clip fires each shell as it comes back;
//!   * pellets fly at ~78 m/s; damage is full to 15 m and falls in a straight line to 30% at 30 m.
//! Its hits don't feed the passive (it's not an ability).

use crate::doomfist::Doomfist;
use crate::events::Event;
use crate::input::Input;
use crate::math::Vec3;
use crate::world::{Projectile, World};

/// Directions of the pellets: one in the middle and a ring around it, `spread_deg` out.
pub fn pellet_dirs(facing: Vec3, pellets: u32, spread_deg: f32) -> Vec<Vec3> {
    let f = facing.normalize_or_zero();
    let side = {
        let s = f.cross(Vec3::UP);
        if s.length() < 1e-4 { Vec3::LEFT } else { s.normalize_or_zero() }
    };
    let up = side.cross(f).normalize_or_zero();
    let mut out = vec![f];
    let ring = pellets.saturating_sub(1).max(1);
    let r = spread_deg.to_radians().tan();
    for i in 0..pellets.saturating_sub(1) {
        let a = i as f32 / ring as f32 * std::f32::consts::TAU;
        out.push((f + side * (r * a.cos()) + up * (r * a.sin())).normalize_or_zero());
    }
    out
}

impl Doomfist {
    pub(crate) fn gun_update(&mut self, input: &Input, world: &mut World, dt: f32) {
        let g = self.tun.gun;
        self.gun.cooldown = (self.gun.cooldown - dt).max(0.0);
        self.gun.since_shot += dt;

        // Refill: one shell at a time, starting after the delay.
        if self.gun.ammo < g.clip {
            self.gun.refill -= dt;
            if self.gun.refill <= 0.0 {
                self.gun.ammo += 1;
                self.gun.refill = g.refill_interval;
            }
        }

        if input.primary && self.gun.ammo > 0 && self.gun.cooldown <= 0.0 && self.current().is_none() {
            self.gun.ammo -= 1;
            self.gun.cooldown = g.fire_interval;
            self.gun.since_shot = 0.0;
            self.gun.refill = g.refill_delay;
            let muzzle = self.pos + Vec3::UP * g.muzzle_height;
            for dir in pellet_dirs(self.facing, g.pellets, g.spread_deg) {
                world.projectiles.push(Projectile {
                    pos: muzzle,
                    vel: dir * g.pellet_speed,
                    travelled: 0.0,
                    damage: g.pellet_damage,
                    head_mult: g.headshot_mult,
                    falloff_start: g.falloff_start,
                    falloff_end: g.falloff_end,
                    falloff_min: g.falloff_min,
                    max_range: g.max_range,
                });
            }
            self.emit(Event::Shot);
        }
    }

    /// The clip is refilling (for the reload animation).
    pub fn gun_refilling(&self) -> bool {
        self.gun.ammo < self.tun.gun.clip && self.gun.since_shot >= self.tun.gun.refill_delay
    }
}
