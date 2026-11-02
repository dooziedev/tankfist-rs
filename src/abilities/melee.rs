//! Quick melee: a short swing in front. Independent of the other abilities' action slot (it
//! only needs Doomfist to be free); its hits don't feed the passive. Measured from
//! logs/gun-01.txt: 40 damage, a light 1 m/s nudge, ready again after 1 s, and a press up to
//! ~0.1-0.3 s early is held until then.

use crate::doomfist::{Doomfist, Pressed};
use crate::events::{Ability, Event, HitKind};
use crate::math::Vec3;
use crate::world::World;

impl Doomfist {
    pub(crate) fn melee_input(&mut self, pressed: &Pressed, world: &mut World) {
        let m = self.tun.melee;
        if pressed.melee && self.cooldowns.melee <= m.buffer {
            self.melee_queued = true;
        }
        if !self.melee_queued || self.cooldowns.melee > 0.0 || self.current().is_some() {
            return;
        }
        self.melee_queued = false;
        self.cooldowns.melee = m.cooldown;
        self.melee_anim = 0.0;
        let fwd = self.facing.horizontal().normalize_or_zero();
        let center = self.pos + Vec3::UP * self.tun.movement.body_center_height;
        let knock_time = world.knock_time;
        for t in world.targets.iter_mut() {
            if t.health <= 0.0 {
                continue;
            }
            let to = (t.center() - center).horizontal();
            let d = to.length();
            if d > m.range + t.radius {
                continue;
            }
            let angle = if d > 1e-3 { fwd.dot(to * (1.0 / d)).clamp(-1.0, 1.0).acos().to_degrees() } else { 0.0 };
            if angle > m.half_angle_deg {
                continue;
            }
            t.health -= m.damage;
            if m.knockback > 0.0 {
                t.vel += fwd * (m.knockback * t.knockback_scale);
                t.knock_timer = knock_time;
            }
            let id = t.id;
            self.emit(Event::DamageDealt { ability: Ability::Melee, target: id, amount: m.damage, kind: HitKind::Melee });
        }
    }
}
