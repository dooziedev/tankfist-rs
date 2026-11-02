//! Replays the automatic slam tests in logs/slam-01.txt through the sim and checks the leap
//! follows the real path tick for tick.
//!
//! The test bot presses buttons with the Workshop's Press Button, which never shows in the I row,
//! so presses come from the E rows (1 slam, 2 block, 3 jump), applied on the next tick (the log
//! shows the slam's launch velocity one tick after its E row). A few log lines are corrupted
//! ("1****"); missing ticks are still stepped so the sim stays in sync.
//!
//!   cargo test --test slam_replay -- --nocapture          (prints a table per take)
//!   SLAM_CSV=logs/slam-01.csv cargo test --test slam_replay  (also writes real vs sim per tick)

use doomfist::log::{parse_log, read_log_file, split_takes, Row, Take};
use doomfist::replay::input_from_row;
use doomfist::{Input, Sim, Tunables, Vec3};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

const LOG: &str = "logs/slam-01.txt";

struct TestInfo {
    suite: i64,
    pitch: f32,
    mode: i64,
    presses: Vec<(i64, i64)>, // (tick ms, code)
}

fn info(take: &Take) -> Option<TestInfo> {
    let mut out = None;
    let mut presses = Vec::new();
    for r in &take.rows {
        match *r {
            Row::Test { suite, pitch_deg, mode, .. } => {
                out = Some((suite, pitch_deg, mode));
            }
            Row::Pressed { t, code } => presses.push((ms(t), code)),
            _ => {}
        }
    }
    let (suite, pitch, mode) = out?;
    Some(TestInfo { suite, pitch, mode, presses })
}

fn ms(t: f64) -> i64 {
    (t * 1000.0).round() as i64
}

struct Tick {
    t: i64,
    real: Option<(Vec3, Vec3)>,
    sim_pos: Vec3,
    #[allow(dead_code)]
    sim_vel: Vec3,
}

/// Steps the sim once per 16 ms from the first frame to the last, pressing E-row buttons.
fn replay(take: &Take, tun: &Tunables) -> Vec<Tick> {
    let ti = info(take).expect("take has an X row");
    let mut frames = BTreeMap::new();
    let mut inputs = BTreeMap::new();
    for r in &take.rows {
        match r {
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
    let (&t0, &(pos0, vel0)) = frames.iter().next().expect("frames");
    let t_end = *frames.keys().last().unwrap();
    let mut sim = Sim::new(tun.clone(), pos0);
    sim.doom.vel = vel0;
    sim.doom.grounded = pos0.y <= 0.05 && vel0.y.abs() < 0.01;
    let mut input: Input = inputs.get(&t0).copied().unwrap_or_default();
    sim.doom.prev_input = input;

    let mut out = vec![Tick { t: t0, real: Some((pos0, vel0)), sim_pos: pos0, sim_vel: vel0 }];
    let mut t = t0 + 16;
    while t <= t_end {
        if let Some(i) = inputs.get(&t) {
            input = *i;
        }
        let mut step_input = input;
        for &(pt, code) in &ti.presses {
            // Press Button lands one tick after its E row; the slam then adds its own one-tick
            // delay inside the sim, so the slam press goes in on the E row's tick.
            let at = if code == 1 { pt } else { pt + 16 };
            if at == t {
                match code {
                    1 => step_input.ability1 = true,
                    2 => step_input.ability2 = true,
                    3 => step_input.jump = true,
                    _ => {}
                }
            }
        }
        sim.step(&step_input);
        out.push(Tick { t, real: frames.get(&t).copied(), sim_pos: sim.doom.pos, sim_vel: sim.doom.vel });
        t += 16;
    }
    out
}

fn load() -> Option<Vec<Take>> {
    let text = read_log_file(Path::new(LOG)).ok()?;
    Some(split_takes(parse_log(&text)))
}

#[test]
fn slam_leaps_match_the_log() {
    let Some(takes) = load() else {
        eprintln!("{LOG} missing; skipped");
        return;
    };
    let tun = Tunables::default();
    let mut csv = String::from("take,suite,pitch,mode,t,real_x,real_y,real_z,sim_x,sim_y,sim_z,err\n");
    let mut worst = 0.0f32;
    let mut checked = 0;
    for (n, take) in takes.iter().enumerate() {
        let Some(ti) = info(take) else { continue };
        let ticks = replay(take, &tun);
        let slam_t = ti.presses.iter().find(|p| p.1 == 1).map(|p| p.0).unwrap_or(0);
        let block_t = ti.presses.iter().find(|p| p.1 == 2).map(|p| p.0);
        // Compare until the block press (Power Block's own air physics is not the slam's job),
        // or the whole take. Block-first takes (mode 3) need a Block -> Slam cancel rule.
        let until = match block_t {
            Some(b) if b < slam_t => {
                println!("take {:>2} suite {} mode {} pitch {:>6.1}: block before slam, skipped", n + 1, ti.suite, ti.mode, ti.pitch);
                continue;
            }
            // The press acts on the next tick, so stop at the press tick.
            Some(b) => b,
            None => i64::MAX,
        };
        let (mut max, mut sum, mut cnt, mut max_t) = (0.0f32, 0.0f32, 0, 0);
        for tk in &ticks {
            let Some((real, _)) = tk.real else { continue };
            let err = real.distance(tk.sim_pos);
            let _ = writeln!(
                csv,
                "{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4}",
                n + 1, ti.suite, ti.pitch, ti.mode, tk.t as f64 / 1000.0,
                real.x, real.y, real.z, tk.sim_pos.x, tk.sim_pos.y, tk.sim_pos.z, err
            );
            if tk.t > until {
                continue;
            }
            if err > max {
                max = err;
                max_t = tk.t - slam_t;
            }
            sum += err * err;
            cnt += 1;
        }
        let rms = (sum / cnt.max(1) as f32).sqrt();
        println!(
            "take {:>2} suite {} mode {} pitch {:>6.1}: max {:.4} m (at slam{:+} ms), rms {:.4} m{}",
            n + 1, ti.suite, ti.mode, ti.pitch, max, max_t, rms,
            if block_t.is_some() { "  (until block)" } else { "" }
        );
        // Looking almost straight up/down, the logged facing (3 decimals) has ~0.017 of horizontal
        // part, so the leap's compass direction can't be recovered from the log; only check those
        // loosely. The real launch heading there matches the bot's level heading exactly.
        if ti.pitch.abs() > 85.0 {
            assert!(max < 0.5, "take {} near-vertical error {max} m", n + 1);
        } else {
            worst = worst.max(max);
        }
        checked += 1;
    }
    if let Ok(path) = std::env::var("SLAM_CSV") {
        std::fs::write(&path, csv).expect("write csv");
    }
    println!("checked {checked} takes, worst error {worst:.4} m");
    assert!(checked > 0);
    assert!(worst < 0.05, "worst replay error {worst} m");
}
