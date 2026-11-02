//! Writes the sim's state in the Overwatch Workshop logger's format, so recordings from the
//! bean demo can be read by every tool here and compared with Overwatch logs.

use crate::doomfist::{Action, Doomfist};
use crate::input::Input;

/// Logger rows for one tick, in the Workshop logger's format (numbers x1000, ';' separated).
pub fn log_rows(t_ms: u64, d: &Doomfist, input: &Input) -> [String; 3] {
    let k = |v: f32| (v * 1000.0).round() as i64;
    let b = |v: bool| if v { "True" } else { "False" };
    let (p, v, f) = (d.pos, d.vel, input.facing);
    let c = &d.cooldowns;
    [
        format!("F;{t_ms};{};{};{};{};{};{}", k(p.x), k(p.y), k(p.z), k(v.x), k(v.y), k(v.z)),
        format!(
            "I;{t_ms};{};{};{};{};{};{};{};{};{};{}",
            k(f.x), k(f.y), k(f.z), k(-input.move_right), k(input.move_forward),
            b(input.secondary), b(input.ability1), b(input.ability2), b(input.jump), b(input.crouch)
        ),
        format!(
            "S;{t_ms};{};{};{};{};{};{};{};{};{}",
            k(d.total_health()), k(d.tun.max_health), b(d.grounded),
            b(matches!(d.action, Action::Slam(_))), b(matches!(d.action, Action::Block(_))),
            b(matches!(d.action, Action::PunchCharge { .. } | Action::Punch(_))),
            k(c.punch), k(c.slam), k(c.block)
        ),
    ]
}


/// Logger rows for the dummies this tick: `P;t;slot;pos;vel;health` (the slot is the dummy's id).
pub fn world_rows(t_ms: u64, world: &crate::world::World) -> Vec<String> {
    let k = |v: f32| (v * 1000.0).round() as i64;
    world
        .targets
        .iter()
        .map(|t| {
            let (p, v) = (t.pos, t.vel);
            format!("P;{t_ms};{};{};{};{};{};{};{};{}", t.id, k(p.x), k(p.y), k(p.z), k(v.x), k(v.y), k(v.z), k(t.health))
        })
        .collect()
}

/// Logger rows for this tick's damage (`D;t;ability;damage;slot`) and knockback (`K;t;slot`).
/// Ability numbers follow the Workshop logger: 0 Hand Cannon, 1 Rocket Punch, 2 Seismic Slam,
/// 5 quick melee.
pub fn event_rows(t_ms: u64, events: &[crate::events::Event]) -> Vec<String> {
    use crate::events::{Ability, Event};
    let mut out = Vec::new();
    for e in events {
        match e {
            Event::DamageDealt { ability, target, amount, .. } => {
                let a = match ability {
                    Ability::HandCannon => 0,
                    Ability::RocketPunch => 1,
                    Ability::SeismicSlam => 2,
                    Ability::Melee => 5,
                };
                out.push(format!("D;{t_ms};{a};{};{target}", (amount * 1000.0).round() as i64));
            }
            Event::Knockback { target, .. } => out.push(format!("K;{t_ms};{target}")),
            _ => {}
        }
    }
    out
}
