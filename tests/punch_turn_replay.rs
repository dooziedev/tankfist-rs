//! Replays the manual turn-punch takes (logs/punch-turn-01.txt: a player charging, punching,
//! swinging the camera and holding jump) through the sim from the logged inputs. This is the
//! ground truth for the jump-cancel rule (6 m/s along the dash + 24.947 m/s toward the camera,
//! camera limited to ±60° of the dash).
//!
//! Air steering isn't fully modelled yet, so the sim is pinned to the real position and velocity
//! every tick except inside "punch windows": from 2 ticks before each dash to 25 ticks after it
//! ends. Only those windows are scored.
//!
//!   cargo test --test punch_turn_replay -- --nocapture

use doomfist::log::{parse_log, read_log_file, split_takes, Row};
use doomfist::replay::input_from_row;
use doomfist::{Sim, Tunables, Vec3};
use std::collections::BTreeMap;
use std::path::Path;

fn ms(t: f64) -> i64 {
    (t * 1000.0).round() as i64
}

#[test]
fn manual_turn_punches_match_the_log() {
    let Ok(text) = read_log_file(Path::new("logs/punch-turn-01.txt")) else {
        eprintln!("log missing; skipped");
        return;
    };
    let mut worst = 0.0f32;
    let mut windows = 0;
    for take in split_takes(parse_log(&text)) {
        let mut frames = BTreeMap::new();
        let mut inputs = BTreeMap::new();
        let mut grounded = BTreeMap::new();
        for r in &take.rows {
            match r {
                Row::Frame { t, pos, vel } => {
                    frames.insert(ms(*t), (*pos, *vel));
                }
                Row::Input { t, .. } => {
                    inputs.insert(ms(*t), input_from_row(r).unwrap());
                }
                Row::State { t, grounded: g, .. } => {
                    grounded.insert(ms(*t), *g);
                }
                _ => {}
            }
        }
        let ts: Vec<i64> = frames.keys().copied().filter(|t| inputs.contains_key(t)).collect();
        // Punch windows by tick index.
        let fast: Vec<bool> = ts.iter().map(|t| frames[t].1.horizontal().length() > 40.0).collect();
        let mut free = vec![false; ts.len()];
        let mut i = 0;
        while i < ts.len() {
            if fast[i] {
                let start = i.saturating_sub(2);
                while i < ts.len() && fast[i] {
                    i += 1;
                }
                for f in free.iter_mut().take((i + 25).min(ts.len())).skip(start) {
                    *f = true;
                }
                windows += 1;
            }
            i += 1;
        }

        let (p0, v0) = frames[&ts[0]];
        let mut sim = Sim::new(Tunables::default(), p0);
        sim.doom.vel = v0;
        sim.doom.prev_input = inputs[&ts[0]];
        let mut max = 0.0f32;
        let mut max_at = 0;
        for k in 1..ts.len() {
            sim.step(&inputs[&ts[k]]);
            let (real, vel) = frames[&ts[k]];
            if free[k] {
                let err = real.distance(sim.doom.pos);
                if std::env::var("DUMP").ok().and_then(|v| v.parse::<i64>().ok()) == Some(take.id) {
                    let sv = sim.doom.vel;
                    let i = inputs[&ts[k]];
                    println!("  +{:.3} err {err:.3} real v ({:.2},{:.2},{:.2}) sim v ({:.2},{:.2},{:.2}) jump {} sec {} act {:?}", (ts[k]-ts[0]) as f64/1000.0, vel.x, vel.y, vel.z, sv.x, sv.y, sv.z, i.jump, i.secondary, sim.doom.current());
                }
                if err > max {
                    max = err;
                    max_at = ts[k] - ts[0];
                }
            } else {
                sim.doom.pos = real;
                sim.doom.vel = vel;
                sim.doom.grounded = *grounded.get(&ts[k]).unwrap_or(&(real.y <= 0.0));
            }
        }
        let _ = Vec3::ZERO;
        println!("take {}: worst in punch windows {max:.4} m at +{:.3}s", take.id, max_at as f64 / 1000.0);
        worst = worst.max(max);
    }
    println!("{windows} punch windows, worst {worst:.4} m");
    assert!(windows > 0);
    // What remains is air steering 25 ticks after the boost.
    assert!(worst < 0.07, "worst {worst} m");
}
