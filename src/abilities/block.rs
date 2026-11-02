//! Power Block: cuts frontal damage. Blocking enough damage empowers the next Rocket Punch.

use crate::abilities::movement::MoveMode;
use crate::doomfist::{Action, ActionKind, Doomfist, Pressed};
use crate::events::{EndReason, Event};
use crate::math::Vec3;

#[derive(Clone, Copy, Debug, Default)]
pub struct BlockState {
    pub t: f32,
    /// Damage prevented so far during this block.
    pub blocked: f32,
}

impl Doomfist {
    pub(crate) fn block_input(&mut self, pressed: &Pressed) {
        // Pressing it again while blocking ends it early (a player's take: tapped on, tapped off
        // 0.61 s later, 7 s cooldown from then).
        if pressed.ability2 {
            if let Action::Block(s) = self.action {
                self.action = Action::Idle;
                self.block_finish(s, EndReason::Cancelled);
                return;
            }
        }
        if !pressed.ability2 || self.cooldowns.block > 0.0 || !self.can_start(ActionKind::Block) {
            return;
        }
        self.interrupt_current();
        self.action = Action::Block(BlockState::default());
        self.emit(Event::ActionStarted(ActionKind::Block));
    }

    pub(crate) fn block_update(&mut self, mut s: BlockState, dt: f32) -> (Action, MoveMode) {
        let b = self.tun.block;
        s.t += dt;
        if s.t >= b.duration {
            self.block_finish(s, EndReason::Completed);
            return (Action::Idle, MoveMode::normal());
        }
        (Action::Block(s), MoveMode::Walk { speed_mult: b.move_mult, can_jump: b.can_jump })
    }

    pub(crate) fn block_finish(&mut self, s: BlockState, reason: EndReason) {
        self.cooldowns.block = self.tun.block.cooldown;
        if s.blocked >= self.tun.block.empower_threshold {
            self.empower();
        }
        self.emit(Event::ActionEnded(ActionKind::Block, reason));
    }

    /// Doomfist gets hit for `raw` damage by something at `from`.
    /// Power Block reduces it if the attacker is in front; temporary health absorbs damage first.
    pub fn take_damage(&mut self, raw: f32, from: Vec3) {
        let b = self.tun.block;
        let mut taken = raw;
        let mut blocked = 0.0;
        let mut reached_threshold = false;

        if let Action::Block(s) = &mut self.action {
            let to_attacker = (from - self.pos).horizontal().normalize_or_zero();
            let fwd = self.facing.horizontal().normalize_or_zero();
            let angle = fwd.dot(to_attacker).clamp(-1.0, 1.0).acos().to_degrees();
            if to_attacker == Vec3::ZERO || angle <= b.frontal_half_angle_deg {
                taken = raw * (1.0 - b.reduction);
                blocked = raw - taken;
                s.blocked += blocked;
                reached_threshold = s.blocked >= b.empower_threshold;
            }
        }
        if reached_threshold && self.tun.interactions.block_empowers_immediately {
            self.empower();
        }

        let from_over = taken.min(self.overhealth);
        self.overhealth -= from_over;
        self.health = (self.health - (taken - from_over)).max(0.0);
        self.emit(Event::DamageTaken { raw, taken, blocked });
    }
}
