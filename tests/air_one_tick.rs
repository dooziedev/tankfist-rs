//! One-tick check of air movement: for every airborne tick in the movement logs, put the sim in
//! the real state, step once with the logged input, and compare the new horizontal velocity.
//! Pinning the state each tick isolates the air-control rule from drift.
//!
//!   cargo test --test air_one_tick -- --nocapture

use doomfist::log::{parse_log, read_log_file, split_takes, Row};
use doomfist::replay::input_from_row;
use doomfist::{Sim, Tunables};
use std::collections::BTreeMap;
use std::path::Path;

fn ms(t: f64) -> i64 {
    (t * 1000.0).round() as i64
}

#[test]
fn air_control_one_tick() {
    let mut all = Vec::new();
    for file in ["logs/movement-01.txt", "logs/movement-02.txt", "logs/punch-turn-01.txt"] {
        let Ok(text) = read_log_file(Path::new(file)) else { continue };
        for take in split_takes(parse_log(&text)) {
            let mut f = BTreeMap::new();
            let mut inp = BTreeMap::new();
            let mut g = BTreeMap::new();
            for r in &take.rows {
                match r {
                    Row::Frame { t, pos, vel } => {
                        f.insert(ms(*t), (*pos, *vel));
                    }
                    Row::Input { t, .. } => {
                        inp.insert(ms(*t), input_from_row(r).unwrap());
                    }
                    Row::State { t, grounded, .. } => {
                        g.insert(ms(*t), *grounded);
                    }
                    _ => {}
                }
            }
            for (&t, &(pos, vel)) in &f {
                let n = t + 16;
                let (Some(&(_, v1)), Some(i0), Some(i1)) = (f.get(&n), inp.get(&t), inp.get(&n)) else { continue };
                // Airborne on both ticks, not punching (flat speed under 40), and not landing.
                if g.get(&t) != Some(&false) || g.get(&n) != Some(&false) || vel.horizontal().length() > 40.0 || i1.secondary || i1.ability1 {
                    continue;
                }
                let mut sim = Sim::new(Tunables::default(), pos);
                sim.doom.vel = vel;
                sim.doom.grounded = false;
                sim.doom.prev_input = *i0;
                sim.step(i1);
                let err = (sim.doom.vel.horizontal() - v1.horizontal()).length();
                let (fl, _) = (i1.facing, 0);
                let wish = { let fw = fl.horizontal().normalize_or_zero(); let rt = fw.cross(doomfist::Vec3::UP); (fw * i1.move_forward + rt * i1.move_right).normalize_or_zero() };
                let c = if vel.horizontal().length() > 0.01 { vel.horizontal().normalize_or_zero().dot(wish) } else { 0.0 };
                all.push((err, file, n, vel.horizontal().length(), c, i1.move_right));
            }
        }
    }
    if all.is_empty() {
        eprintln!("skipped: the movement logs aren't in logs/");
        return;
    }
    all.sort_by(|a, b| b.0.total_cmp(&a.0));
    for lim in [0.01f32, 0.03, 0.1, 0.25] {
        println!("  ticks over {lim} m/s: {}", all.iter().filter(|a| a.0 > lim).count());
    }
    let rms = (all.iter().map(|a| a.0 * a.0).sum::<f32>() / all.len() as f32).sqrt();
    println!("{} air ticks, rms velocity error {rms:.4} m/s", all.len());
    for a in all.iter().take(25) {
        println!("  {:.3} m/s off in {} at {} ms (speed {:.2}, cos(vel,keys) {:+.2})", a.0, a.1, a.2, a.3, a.4);
    }
}
