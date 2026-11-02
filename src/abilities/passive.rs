//! Passive: The Best Defense. Hitting enemies with abilities gives temporary health.

use crate::doomfist::Doomfist;
use crate::events::Event;

impl Doomfist {
    /// Drains temporary health once it hasn't been topped up for a while.
    pub(crate) fn passive_tick(&mut self, dt: f32) {
        if self.overhealth <= 0.0 {
            return;
        }
        let p = self.tun.passive;
        self.overhealth_idle += dt;
        if self.overhealth_idle >= p.decay_delay {
            self.overhealth = (self.overhealth - p.decay_rate * dt).max(0.0);
        }
    }

    /// Turns this tick's ability hits into temporary health. Called after targets move, so
    /// wall-slam hits count as well.
    pub fn passive_on_events(&mut self, events: &mut Vec<Event>) {
        let p = self.tun.passive;
        let mut gained = 0.0;
        for e in events.iter() {
            // Only abilities feed the passive, not quick melee.
            if let Event::DamageDealt { amount, ability, .. } = e {
                if *ability == crate::events::Ability::Melee {
                    continue;
                }
                gained += p.per_hit + amount * p.damage_fraction;
            }
        }
        if gained <= 0.0 {
            return;
        }
        let before = self.overhealth;
        self.overhealth = (self.overhealth + gained).min(p.cap);
        self.overhealth_idle = 0.0;
        events.push(Event::OverhealthGained { amount: self.overhealth - before, total: self.overhealth });
    }

    /// Health plus temporary health (what the Workshop's `Health()` reports).
    pub fn total_health(&self) -> f32 {
        self.health + self.overhealth
    }
}
