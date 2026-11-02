//! Replays the automatic punch tests in logs/punch-01.txt through the sim and checks Doomfist
//! follows the real path tick for tick.
//!
//! The test bot's buttons come from the E rows (1 slam, 3 jump, 4 start holding punch,
//! 5 release punch). Presses (1, 3) act one tick after their row, like `slam_replay.rs`; holds and
//! releases (4, 5) act on their own tick. Facing comes from the I rows.
//!
//!   cargo test --test punch_replay -- --nocapture   (prints one line per take)

use doomfist::log::{parse_log, read_log_file, split_takes, Row, Take};
use doomfist::replay::input_from_row;
use doomfist::{Input, Sim, Tunables, Vec3};
use std::collections::BTreeMap;
use std::path::Path;

const LOG: &str = "logs/punch-01.txt";

fn ms(t: f64) -> i64 {
    (t * 1000.0).round() as i64
}

struct Label {
    suite: i64,
    charge: f32,
    jump_at: f32,
    slam_at: f32,
    turn: f32,
}

/// Returns (label, per-tick (t, real pos, sim pos)).
fn replay(take: &Take, tun: &Tunables) -> Option<(Label, Vec<(i64, Vec3, Vec3)>)> {
    let mut label = None;
    let mut presses = Vec::new();
    let mut frames = BTreeMap::new();
    let mut inputs = BTreeMap::new();
    for r in &take.rows {
        match r {
            Row::PunchTest { suite, charge, jump_at, slam_at, turn_deg, .. } => {
                label = Some(Label { suite: *suite, charge: *charge, jump_at: *jump_at, slam_at: *slam_at, turn: *turn_deg })
            }
            // Press Button lands one tick after its E row, except the slam, whose own one-tick
            // delay is inside the sim.
            Row::Pressed { t, code } => presses.push((ms(*t) + if *code == 1 { 0 } else { 16 }, *code)),
            Row::Frame { t, pos, vel } => {
                frames.insert(ms(*t), (*pos, *vel));
            }
            Row::Input { t, .. } => {
                if let Some(i) = input_from_row(r) {
                    inputs.insert(ms(*t), i);
                }
            }
            _ => {}
        }
    }
    let label = label?;
    // Start/Stop Holding act on the tick they're logged (only Press Button lands a tick later):
    // the real dash starts one tick after the logged release.
    // The bot jumps with a one-tick Press Button. Overwatch then cancels the dash on that same
    // tick with no hop, unlike a held jump from a player (press tick hops, next tick cancels;
    // see logs/punch-turn-01.txt), which is what the sim models. So these takes are only
    // compared up to the jump.
    let jump_tick = presses.iter().find(|p| p.1 == 3).map(|p| p.0);
    let hold = presses.iter().find(|p| p.1 == 4)?.0 - 16;
    let release = presses.iter().find(|p| p.1 == 5)?.0 - 16;

    let (&t0, &(pos0, vel0)) = frames.iter().next()?;
    let t_end = *frames.keys().last()?;
    let mut sim = Sim::new(tun.clone(), pos0);
    sim.doom.vel = vel0;
    sim.doom.grounded = pos0.y <= 0.05 && vel0.y.abs() < 0.01;
    let mut input: Input = inputs.get(&t0).copied().unwrap_or_default();
    sim.doom.prev_input = input;

    let mut out = Vec::new();
    let mut t = t0 + 16;
    while t <= t_end {
        if let Some(i) = inputs.get(&t) {
            input = *i;
        }
        let mut step = input;
        step.secondary = t >= hold && t < release;
        for &(pt, code) in &presses {
            if pt == t {
                match code {
                    1 => step.ability1 = true,
                    3 => step.jump = true,
                    _ => {}
                }
            }
        }
        sim.step(&step);
        if let Some(&(real, _)) = frames.get(&t) {
            if jump_tick.is_none_or(|j| t < j) {
                out.push((t, real, sim.doom.pos));
            }
        }
        t += 16;
    }
    Some((label, out))
}

#[test]
fn punches_match_the_log() {
    let Ok(text) = read_log_file(Path::new(LOG)) else {
        eprintln!("{LOG} missing; skipped");
        return;
    };
    let tun = Tunables::default();
    let mut worst = (0.0f32, 0usize);
    let mut checked = 0;
    for (n, take) in split_takes(parse_log(&text)).iter().enumerate() {
        let Some((l, ticks)) = replay(take, &tun) else { continue };
        if std::env::var("PUNCH_DUMP").ok().and_then(|v| v.parse::<usize>().ok()) == Some(n + 1) {
            let mut prev: Option<(Vec3, Vec3)> = None;
            for (t, r, sm) in &ticks {
                let v = prev.map(|(pr, ps)| ((*r - pr) * 62.5, (*sm - ps) * 62.5));
                if let Some((rv, sv)) = v {
                    println!("  t={t} real=({:.3},{:.3},{:.3}) sim=({:.3},{:.3},{:.3}) real_flat={:.3} sim_flat={:.3} real_vy={:+.3} sim_vy={:+.3} err={:.4}",
                        r.x, r.y, r.z, sm.x, sm.y, sm.z, rv.horizontal().length(), sv.horizontal().length(), rv.y, sv.y, r.distance(*sm));
                }
                prev = Some((*r, *sm));
            }
        }
        let max = ticks.iter().map(|(_, r, s)| r.distance(*s)).fold(0.0f32, f32::max);
        let end = ticks.last().map(|(_, r, s)| r.distance(*s)).unwrap_or(0.0);
        println!(
            "take {:>2} suite {} charge {:.2} jump {:>5.2} slam {:>5.2} turn {:>5.0}: max {:.4} m, end {:.4} m",
            n + 1, l.suite, l.charge, l.jump_at, l.slam_at, l.turn, max, end
        );
        if max > worst.0 {
            worst = (max, n + 1);
        }
        checked += 1;
    }
    println!("checked {checked} takes, worst {:.4} m (take {})", worst.0, worst.1);
    assert!(checked > 0);
    assert!(worst.0 < 0.05, "worst replay error {} m in take {}", worst.0, worst.1);
}
