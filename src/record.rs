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

