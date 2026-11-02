//! Lines up a bean recording with Overwatch's playback of the same inputs and shows where the
//! two diverge.
//!
//!   cargo run --bin compare -- <bean log> <overwatch log> [--bean-take N] [--ow-take N] [--csv out.csv]
//!   cargo run --bin compare -- sim <overwatch log> [--ow-take N] [--csv out.csv]
//!
//! `sim` replays an Overwatch take's own inputs (e.g. you playing) through the Rust Doomfist and
//! compares that with what really happened.
//!
//! Default takes: the last one in each file. Overwatch's path is moved and rotated so both start
//! at the same spot facing the same way, and shifted by up to ±4 ticks to line up best.

use doomfist::log::{parse_log, read_log_file, split_takes, Row, Take};
use doomfist::replay::replay_take;
use doomfist::{Tunables, Vec3};
use std::fmt::Write as _;
use std::path::PathBuf;

struct Tick {
    pos: Vec3,
    vel: Vec3,
    grounded: bool,
    keys: String,
}

fn ticks(take: &Take) -> (Vec<Tick>, f32) {
    let ms = |t: f64| (t * 1000.0).round() as i64;
    let mut grounded = std::collections::HashMap::new();
    let mut keys = std::collections::HashMap::new();
    let mut yaw0 = None;
    for r in &take.rows {
        match r {
            Row::State { t, grounded: g, .. } => {
                grounded.insert(ms(*t), *g);
            }
            Row::Input { t, facing, secondary, ability1, ability2, jump, .. } => {
                yaw0.get_or_insert(facing.x.atan2(facing.z));
                let k: String = [(*secondary, 'P'), (*ability1, 'S'), (*ability2, 'B'), (*jump, 'J')]
                    .iter()
                    .map(|&(on, c)| if on { c } else { '.' })
                    .collect();
                keys.insert(ms(*t), k);
            }
            _ => {}
        }
    }
    let out = take
        .frames()
        .map(|(t, pos, vel)| Tick {
            pos,
            vel,
            grounded: *grounded.get(&ms(t)).unwrap_or(&false),
            keys: keys.get(&ms(t)).cloned().unwrap_or_default(),
        })
        .collect();
    (out, yaw0.unwrap_or(0.0))
}

/// The Rust sim fed this take's inputs, next to what really happened (same start, same frame).
fn sim_vs_real(take: &Take) -> ((Vec<Tick>, f32), (Vec<Tick>, f32)) {
    let (real, yaw) = ticks(take);
    let keys: std::collections::HashMap<i64, String> = real
        .iter()
        .zip(take.frames())
        .map(|(t, f)| ((f.0 * 1000.0).round() as i64, t.keys.clone()))
        .collect();
    let rep = replay_take(take, &Tunables::default());
    let k = |t: f64| keys.get(&((t * 1000.0).round() as i64)).cloned().unwrap_or_default();
    let sim = rep
        .iter()
        .map(|r| Tick { pos: r.sim_pos, vel: r.sim_vel, grounded: r.sim_pos.y <= 1e-3 && r.sim_vel.y.abs() < 1e-3, keys: k(r.t) })
        .collect();
    let real = rep
        .iter()
        .map(|r| Tick { pos: r.real_pos, vel: r.real_vel, grounded: r.real_pos.y <= 1e-3 && r.real_vel.y.abs() < 1e-3, keys: k(r.t) })
        .collect();
    ((sim, yaw), (real, yaw))
}

/// Rotates around the vertical by `a` radians (same sense as yaw = atan2(x, z)).
fn rot(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c + v.z * s, v.y, -v.x * s + v.z * c)
}

fn pick(path: &PathBuf, take: Option<i64>) -> Take {
    let text = read_log_file(path).unwrap_or_else(|e| {
        eprintln!("can't read {}: {e}", path.display());
        std::process::exit(1);
    });
    let takes = split_takes(parse_log(&text));
    let t = match take {
        Some(n) => takes.into_iter().find(|t| t.id == n),
        None => takes.into_iter().last(),
    };
    t.unwrap_or_else(|| {
        eprintln!("no matching take in {}", path.display());
        std::process::exit(1);
    })
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut files = Vec::new();
    let (mut bt, mut ot, mut csv) = (None, None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--bean-take" => bt = args.next().and_then(|v| v.parse().ok()),
            "--ow-take" => ot = args.next().and_then(|v| v.parse().ok()),
            "--csv" => csv = args.next().map(PathBuf::from),
            _ => files.push(PathBuf::from(a)),
        }
    }
    if files.len() != 2 {
        eprintln!("usage: compare <bean log> <overwatch log> [--bean-take N] [--ow-take N] [--csv out.csv]
       compare sim <overwatch log> [--ow-take N] [--csv out.csv]");
        std::process::exit(2);
    }
    let sim_mode = files[0].as_os_str() == "sim";
    let ((bean, byaw), (ow, oyaw)) = if sim_mode {
        sim_vs_real(&pick(&files[1], ot))
    } else {
        (ticks(&pick(&files[0], bt)), ticks(&pick(&files[1], ot)))
    };
    if bean.is_empty() || ow.is_empty() {
        eprintln!("no frames to compare");
        std::process::exit(1);
    }

    // Move Overwatch's path into bean's frame: same start point, same starting yaw.
    let turn = byaw - oyaw;
    let (b0, o0) = (bean[0].pos, ow[0].pos);
    let ow: Vec<Tick> = ow
        .into_iter()
        .map(|t| Tick { pos: b0 + rot(t.pos - o0, turn), vel: rot(t.vel, turn), ..t })
        .collect();

    // Best tick shift (Overwatch index = bean index + shift) over the first 2 seconds.
    let err_at = |shift: i64, n: usize| -> f32 {
        let mut sum = 0.0;
        let mut cnt = 0;
        for i in 0..n.min(bean.len()) {
            let j = i as i64 + shift;
            if j >= 0 && (j as usize) < ow.len() {
                sum += (bean[i].pos - ow[j as usize].pos).length();
                cnt += 1;
            }
        }
        if cnt == 0 { f32::MAX } else { sum / cnt as f32 }
    };
    // Sim mode steps the sim on the logged ticks themselves, so no shift is needed.
    let shift = if sim_mode { 0 } else { (-4..=4).min_by(|&a, &b| err_at(a, 125).total_cmp(&err_at(b, 125))).unwrap() };
    let pairs: Vec<(usize, usize)> = (0..bean.len())
        .filter_map(|i| {
            let j = i as i64 + shift;
            (j >= 0 && (j as usize) < ow.len()).then_some((i, j as usize))
        })
        .collect();
    println!(
        "Rust {} ticks, Overwatch {} ticks, shift {shift:+} ticks, Overwatch rotated {:+.1} deg",
        bean.len(),
        ow.len(),
        turn.to_degrees()
    );

    let err = |&(i, j): &(usize, usize)| (bean[i].pos - ow[j].pos).length();
    let max = pairs.iter().map(err).fold(0.0f32, f32::max);
    println!("max position difference {max:.3} m, at end {:.3} m", pairs.last().map(err).unwrap_or(0.0));
    for lim in [0.05f32, 0.25, 1.0] {
        if let Some(&(i, j)) = pairs.iter().find(|p| err(p) > lim) {
            let (b, o) = (&bean[i], &ow[j]);
            println!(
                "first over {lim} m at +{:.3}s  keys {}  Rust speed {:.2}/{:+.2} {}  Overwatch {:.2}/{:+.2} {}",
                i as f32 * 0.016,
                b.keys,
                b.vel.horizontal().length(),
                b.vel.y,
                if b.grounded { "ground" } else { "air" },
                o.vel.horizontal().length(),
                o.vel.y,
                if o.grounded { "ground" } else { "air" },
            );
        }
    }
    println!("\n   time  keys   diff    Rust flat/up        Overwatch flat/up");
    for &(i, j) in pairs.iter().step_by(16) {
        let (b, o) = (&bean[i], &ow[j]);
        println!(
            "{:7.2}  {}  {:5.2}  {:6.2} {:+6.2} {}   {:6.2} {:+6.2} {}",
            i as f32 * 0.016,
            b.keys,
            err(&(i, j)),
            b.vel.horizontal().length(),
            b.vel.y,
            if b.grounded { "G" } else { " " },
            o.vel.horizontal().length(),
            o.vel.y,
            if o.grounded { "G" } else { " " },
        );
    }

    if let Some(path) = csv {
        let mut out = String::from("t,keys,bean_x,bean_y,bean_z,ow_x,ow_y,ow_z,bean_vx,bean_vy,bean_vz,ow_vx,ow_vy,ow_vz,diff\n");
        for &(i, j) in &pairs {
            let (b, o) = (&bean[i], &ow[j]);
            let _ = writeln!(
                out,
                "{:.3},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4}",
                i as f32 * 0.016, b.keys, b.pos.x, b.pos.y, b.pos.z, o.pos.x, o.pos.y, o.pos.z,
                b.vel.x, b.vel.y, b.vel.z, o.vel.x, o.vel.y, o.vel.z, err(&(i, j))
            );
        }
        std::fs::write(&path, out).expect("write csv");
        println!("wrote {}", path.display());
    }
}
