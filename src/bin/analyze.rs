//! Summarises a Workshop log, one take at a time.
//!
//!   cargo run --bin analyze -- <log.txt> [--take N] [--replay] [--csv out.csv]
//!
//! For each take it lists every ability use (start, length, distance moved, peak speed, hits,
//! temporary health gained). `--replay` also re-runs the take through the sim and reports how
//! far the sim drifts from the real path; `--csv` writes the real vs sim path per tick.

use doomfist::log::{read_log_file, split_takes, Row, Take};
use doomfist::replay::replay_take;
use doomfist::{Tunables, Vec3};
use std::fmt::Write as _;
use std::path::PathBuf;

struct Args {
    log: PathBuf,
    take: Option<i64>,
    replay: bool,
    csv: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let mut log = None;
    let (mut take, mut replay, mut csv) = (None, false, None);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--take" => take = Some(it.next().and_then(|v| v.parse().ok()).ok_or("--take needs a number")?),
            "--replay" => replay = true,
            "--csv" => csv = Some(PathBuf::from(it.next().ok_or("--csv needs a path")?)),
            _ if log.is_none() => log = Some(PathBuf::from(a)),
            _ => return Err(format!("unexpected argument: {a}")),
        }
    }
    Ok(Args {
        log: log.ok_or("usage: analyze <log.txt> [--take N] [--replay] [--csv out.csv]")?,
        take,
        replay,
        csv,
    })
}

/// Stretches of time where `on` is true for State/Input rows.
fn segments(take: &Take, on: impl Fn(&Row) -> Option<bool>) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut start: Option<f64> = None;
    let mut last_t = 0.0;
    for r in &take.rows {
        let Some(v) = on(r) else { continue };
        let t = r.t();
        match (v, start) {
            (true, None) => start = Some(t),
            (false, Some(s)) => {
                out.push((s, t));
                start = None;
            }
            _ => {}
        }
        last_t = t;
    }
    if let Some(s) = start {
        out.push((s, last_t));
    }
    out
}

fn frame_at(take: &Take, t: f64) -> Option<(Vec3, Vec3)> {
    take.frames()
        .min_by(|a, b| (a.0 - t).abs().total_cmp(&(b.0 - t).abs()))
        .map(|(_, p, v)| (p, v))
}

fn describe(take: &Take, label: &str, (t0, t1): (f64, f64), origin: f64) -> String {
    let mut s = String::new();
    let (Some((p0, _)), Some((p1, _))) = (frame_at(take, t0), frame_at(take, t1)) else {
        return s;
    };
    let in_seg = take.frames().filter(|(t, _, _)| *t >= t0 && *t <= t1);
    let (mut peak_h, mut peak_y) = (0.0f32, p0.y);
    for (_, p, v) in in_seg {
        peak_h = peak_h.max(v.horizontal().length());
        peak_y = peak_y.max(p.y);
    }
    let d = p1 - p0;
    let _ = write!(
        s,
        "  {label:<13} at +{:.3}s  for {:.3}s  moved {:.2} m flat, {:+.2} m up (peak {:+.2})  peak flat speed {:.2} m/s",
        t0 - origin,
        t1 - t0,
        d.horizontal().length(),
        d.y,
        peak_y - p0.y,
        peak_h
    );

    // Hits and temporary health in the segment plus a short tail (wall slams land after).
    let tail = t1 + 0.75;
    let hits: Vec<String> = take
        .rows
        .iter()
        .filter_map(|r| match r {
            Row::Dealt { t, ability, damage, victim_slot } if *t >= t0 && *t <= tail => Some(format!(
                "{:.1} dmg (ability {ability}, slot {victim_slot}, +{:.3}s)",
                damage,
                t - t0
            )),
            _ => None,
        })
        .collect();
    if !hits.is_empty() {
        let _ = write!(s, "\n      hits: {}", hits.join(", "));
    }
    let over = take
        .rows
        .iter()
        .filter_map(|r| match r {
            Row::State { t, health, max_health, .. } if *t >= t0 && *t <= tail => Some(health - max_health),
            _ => None,
        })
        .fold(0.0f32, f32::max);
    if over > 0.0 {
        let _ = write!(s, "\n      health above max (temp health): {over:.1}");
    }
    s
}

fn summarize(take: &Take) {
    let origin = take.rows.first().map(|r| r.t()).unwrap_or(0.0);
    println!("Take {}: {:.2}s, {} frames", take.id, take.duration(), take.frames().count());
    if let Some(Row::State { max_health, .. }) = take.rows.iter().find(|r| matches!(r, Row::State { .. })) {
        println!("  max health: {max_health:.1}");
    }

    type Pick = fn(&Row) -> Option<bool>;
    let kinds: [(&str, Pick); 6] = [
        ("punch held", |r| match r { Row::Input { secondary, .. } => Some(*secondary), _ => None }),
        ("punch firing", |r| match r { Row::State { firing_secondary, .. } => Some(*firing_secondary), _ => None }),
        ("slam", |r| match r { Row::State { using_ability1, .. } => Some(*using_ability1), _ => None }),
        ("power block", |r| match r { Row::State { using_ability2, .. } => Some(*using_ability2), _ => None }),
        ("airborne", |r| match r { Row::State { grounded, .. } => Some(!*grounded), _ => None }),
        ("jump held", |r| match r { Row::Input { jump, .. } => Some(*jump), _ => None }),
    ];
    let mut lines: Vec<(f64, String)> = Vec::new();
    for (label, pick) in kinds {
        for seg in segments(take, pick) {
            let text = describe(take, label, seg, origin);
            if !text.is_empty() {
                lines.push((seg.0, text));
            }
        }
    }
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, l) in lines {
        println!("{l}");
    }

    dash_turns(take);

    let taken: Vec<String> = take
        .rows
        .iter()
        .filter_map(|r| match r {
            Row::Took { t, damage, attacker_slot } => {
                Some(format!("{damage:.1} from slot {attacker_slot} at +{:.3}s", t - origin))
            }
            _ => None,
        })
        .collect();
    if !taken.is_empty() {
        println!("  damage taken: {}", taken.join(", "));
    }
}

/// One line per automatic slam test: what was tested and what the leap did.
fn slam_table(takes: &[&Take]) {
    let mut lines = Vec::new();
    for take in takes {
        let Some(&Row::Test { suite, pitch_deg, mode, .. }) =
            take.rows.iter().find(|r| matches!(r, Row::Test { .. }))
        else {
            continue;
        };
        let pressed = |code: i64| {
            take.rows.iter().find_map(|r| match r {
                Row::Pressed { t, code: c } if *c == code => Some(*t),
                _ => None,
            })
        };
        let Some(t_slam) = pressed(1) else { continue };
        let block_at = pressed(2).map(|t| t - t_slam);
        let grounded: Vec<(f64, bool)> = take
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::State { t, grounded, .. } => Some((*t, *grounded)),
                _ => None,
            })
            .collect();
        let frames: Vec<(f64, Vec3, Vec3)> = take.frames().collect();
        let Some(&(_, start, v0)) = frames.iter().rev().find(|f| f.0 <= t_slam) else { continue };

        // Launch: first tick after the press where velocity jumps.
        let launch = frames
            .iter()
            .find(|f| f.0 > t_slam && (f.2 - v0).length() > 2.0)
            .copied();
        let Some((t_launch, _, v_launch)) = launch else {
            lines.push(format!(
                "suite {suite} pitch {pitch_deg:>6.1} mode {mode} block {:>6}  no launch seen",
                block_at.map_or("-".into(), |b| format!("{b:+.3}"))
            ));
            continue;
        };
        // Landing: first grounded tick after launch.
        // Landing: first grounded tick after launch, or first tick back at floor height while
        // falling (a bunny hop may never show as grounded).
        let by_flag = grounded.iter().find(|(t, g)| *t > t_launch + 0.05 && *g).map(|(t, _)| *t);
        let by_height = frames
            .windows(2)
            .find(|w| w[1].0 > t_launch + 0.05 && w[1].1.y <= start.y + 0.02 && w[0].2.y < 0.0)
            .map(|w| w[1].0);
        let t_land = match (by_flag, by_height) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let in_air = frames.iter().filter(|f| f.0 >= t_launch && t_land.is_none_or(|l| f.0 <= l));
        let peak = in_air.clone().map(|f| f.1.y).fold(start.y, f32::max) - start.y;
        let end = t_land
            .and_then(|l| frames.iter().find(|f| f.0 >= l))
            .map(|f| f.1)
            .unwrap_or(frames.last().unwrap().1);
        let facing = take.rows.iter().find_map(|r| match r {
            Row::Input { t, facing, .. } if *t >= t_slam => Some(*facing),
            _ => None,
        });
        let real_pitch = facing.map(|f| f.y.clamp(-1.0, 1.0).asin().to_degrees()).unwrap_or(f32::NAN);
        // A hop after touchdown: first tick within 1 s of landing moving up faster than 2 m/s.
        let hop = t_land.and_then(|l| {
            let before = frames.iter().rev().find(|f| f.0 < l).map(|f| f.2);
            frames
                .iter()
                .find(|f| f.0 >= l && f.0 <= l + 1.0 && f.2.y > 2.0)
                .map(|f| format!("  hop +{:.3}s after landing: {:.2} flat ({:.2} before) {:+.2} up", f.0 - l, f.2.horizontal().length(), before.map_or(f32::NAN, |b| b.horizontal().length()), f.2.y))
        });
        lines.push(format!(
            "suite {suite} pitch {pitch_deg:>6.1} (logged {real_pitch:>6.1}) mode {mode} block {:>6}  delay {:.3}s  launch flat {:>6.2} up {:>6.2} m/s  peak {:>5.2} m  air {:>5.3}s  dist {:>6.2} m flat, {:+.2} m up",
            block_at.map_or("-".into(), |b| format!("{b:+.3}")),
            t_launch - t_slam,
            v_launch.horizontal().length(),
            v_launch.y,
            peak,
            t_land.map_or(f64::NAN, |l| l - t_launch),
            (end - start).horizontal().length(),
            end.y - start.y,
        ) + &hop.unwrap_or_default());
    }
    if !lines.is_empty() {
        println!("Slam tests ({}):", lines.len());
        for l in lines {
            println!("  {l}");
        }
        println!();
    }
}

/// Signed flat angle from `a` to `b`, degrees.
fn flat_angle(a: Vec3, b: Vec3) -> f32 {
    let (a, b) = (a.horizontal(), b.horizontal());
    (a.x * b.z - a.z * b.x).atan2(a.dot(b)).to_degrees()
}

/// One line per automatic punch test: charge, punch dash, jump boost, slam launch.
fn punch_table(takes: &[&Take]) {
    let mut lines = Vec::new();
    for take in takes {
        let Some(&Row::PunchTest { suite, charge, pitch_deg, jump_at, slam_at, turn_deg, no_jump, .. }) =
            take.rows.iter().find(|r| matches!(r, Row::PunchTest { .. }))
        else {
            continue;
        };
        let pressed = |code: i64| {
            take.rows.iter().find_map(|r| match r {
                Row::Pressed { t, code: c } if *c == code => Some(*t),
                _ => None,
            })
        };
        let frames: Vec<(f64, Vec3, Vec3)> = take.frames().collect();
        let (Some(t_hold), Some(t_rel)) = (pressed(4), pressed(5)) else { continue };
        let Some(&(_, start, _)) = frames.iter().rev().find(|f| f.0 <= t_hold) else { continue };
        let after = |t: f64, ticks: usize| frames.iter().filter(|f| f.0 > t).nth(ticks.saturating_sub(1)).copied();

        // The dash: the run of ticks after release moving faster than 8 m/s.
        let dash: Vec<_> = frames
            .iter()
            .skip_while(|f| f.0 <= t_rel || f.2.horizontal().length() <= 8.0)
            .take_while(|f| f.2.horizontal().length() > 8.0)
            .copied()
            .collect();
        let mut text = format!(
            "s{suite} charge {:.3}s (asked {charge:.2}) pitch {pitch_deg:>5.0}",
            t_rel - t_hold
        );
        if let (Some(first), Some(last)) = (dash.first(), dash.last()) {
            let peak = dash.iter().map(|f| f.2.horizontal().length()).fold(0.0f32, f32::max);
            text += &format!(
                " | dash starts +{:.3}s, {:.3}s long, peak {:>5.2} m/s, {:>5.2} m flat, {:+.2} m up",
                first.0 - t_rel,
                last.0 - first.0 + 0.016,
                peak,
                (last.1 - start).horizontal().length(),
                last.1.y - start.y
            );
        } else {
            text += " | no dash seen";
        }
        let punch_dir = dash.first().map(|f| f.2).unwrap_or(Vec3::FORWARD);

        if let Some(t_jump) = pressed(3) {
            if let Some((_, _, v)) = after(t_jump, 2) {
                text += &format!(
                    " | jump at +{:.3}s (asked {jump_at:.2}){}: {:>5.2} m/s flat, {:+.2} up, {:+.0} deg off punch",
                    t_jump - t_rel,
                    if turn_deg != 0.0 { format!(" turn {turn_deg:+.0}") } else { String::new() },
                    v.horizontal().length(),
                    v.y,
                    flat_angle(punch_dir, v)
                );
            }
        } else if no_jump && turn_deg != 0.0 {
            if let Some((_, _, v)) = pressed(6).and_then(|t| after(t, 2)) {
                text += &format!(" | turn {turn_deg:+.0} no jump: {:>5.2} m/s, {:+.0} deg off punch", v.horizontal().length(), flat_angle(punch_dir, v));
            }
        }
        if let Some(t_slam) = pressed(1) {
            let v_before = frames.iter().rev().find(|f| f.0 <= t_slam).map(|f| f.2).unwrap_or(Vec3::ZERO);
            if let Some((tl, _, v)) = frames.iter().find(|f| f.0 > t_slam && (f.2 - v_before).length() > 2.0).copied() {
                text += &format!(
                    " | slam (asked +{slam_at:.2}) launches +{:.3}s: {:>5.2} flat, {:+.2} up",
                    tl - t_slam,
                    v.horizontal().length(),
                    v.y
                );
            } else {
                text += " | slam: no launch seen";
            }
        }
        if let Some(&(_, end, _)) = frames.last() {
            let peak = frames.iter().map(|f| f.1.y).fold(start.y, f32::max) - start.y;
            text += &format!(" | total {:>5.2} m flat, peak {:.2} m", (end - start).horizontal().length(), peak);
        }
        lines.push(text);
    }
    if !lines.is_empty() {
        println!("Punch tests ({}):", lines.len());
        for l in lines {
            println!("  {l}");
        }
        println!();
    }
}

/// Rocket Punch dashes (flat speed above 40 m/s) and how much they turned compared to the camera,
/// plus the jump-cancel boost direction if one follows. For manual takes with mouse turning.
fn dash_turns(take: &Take) {
    let yaw = |v: Vec3| v.x.atan2(v.z).to_degrees();
    let wrap = |d: f32| (d + 540.0).rem_euclid(360.0) - 180.0;
    let facing: std::collections::BTreeMap<i64, Vec3> = take
        .rows
        .iter()
        .filter_map(|r| match r {
            Row::Input { t, facing, .. } => Some(((t * 1000.0).round() as i64, *facing)),
            _ => None,
        })
        .collect();
    let frames: Vec<(f64, Vec3, Vec3)> = take.frames().collect();
    let origin = frames.first().map(|f| f.0).unwrap_or(0.0);
    let mut i = 0;
    while i < frames.len() {
        if frames[i].2.horizontal().length() <= 40.0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < frames.len() && frames[i].2.horizontal().length() > 40.0 {
            i += 1;
        }
        let dash = &frames[start..i];
        let face = |t: f64| facing.get(&((t * 1000.0).round() as i64)).copied();
        let (mut cam_turn, mut dash_turn, mut max_rate) = (0.0f32, 0.0f32, 0.0f32);
        for w in dash.windows(2) {
            let d = wrap(yaw(w[1].2) - yaw(w[0].2));
            dash_turn += d;
            max_rate = max_rate.max(d.abs() / (w[1].0 - w[0].0) as f32);
            if let (Some(a), Some(b)) = (face(w[0].0), face(w[1].0)) {
                cam_turn += wrap(yaw(b) - yaw(a));
            }
        }
        let mut line = format!(
            "  dash at +{:.3}s, {} ticks: camera turned {:+.1} deg, dash turned {:+.1} deg, fastest dash turn {:.0} deg/s",
            dash[0].0 - origin,
            dash.len(),
            cam_turn,
            dash_turn,
            max_rate
        );
        if let Some(next) = frames.get(i) {
            let v = next.2;
            if v.y > 4.0 && v.horizontal().length() > 20.0 {
                let last = dash.last().unwrap().2;
                let off_cam = face(next.0).map_or(f32::NAN, |f| wrap(yaw(v) - yaw(f)));
                line += &format!(
                    "
      jump boost {:.2} m/s flat, {:+.1} deg from last dash direction, {:+.1} deg from camera",
                    v.horizontal().length(),
                    wrap(yaw(v) - yaw(last)),
                    off_cam
                );
            }
        }
        println!("{line}");
    }
}

/// One block per automatic hit / Power Block / empowered punch test (workshop/auto_hit_tests.txt).
fn hit_table(takes: &[&Take]) {
    let mut any = false;
    for take in takes {
        let Some(&Row::HitTest { suite, a, b, c, .. }) = take.rows.iter().find(|r| matches!(r, Row::HitTest { .. })) else {
            continue;
        };
        if !any {
            println!("Hit tests:");
            any = true;
        }
        let t0 = take.rows.first().map(|r| r.t()).unwrap_or(0.0);
        let what = match suite {
            20 => format!("punch impact: charge {a:.2}s into slot {b:.0} at 6 m"),
            21 => format!("wall slam: charge {a:.2}s, target {b:.0} m from the wall (wall {c:.1} m away)"),
            22 => format!("slam hit area: grid {a:.0}"),
            23 => format!("Power Block: {c:.0} raw damage from {a:.0} deg, blocking {}", b > 0.5),
            24 => format!("empower threshold: {a:.0} raw frontal damage blocked, then full punch"),
            25 => format!("empowered punch: mode {a:.0} (0 open, 1 target+neighbour, 2 wall, 3 after {c:.0}s), charge {b:.2}s"),
            40 => "Hand Cannon: hold fire 3 s from full, Ana at 6 m".into(),
            41 => format!("Hand Cannon: two single shots {a:.2} s apart"),
            42 => "Hand Cannon: empty the clip, then 6 s to refill".into(),
            43 => format!("Hand Cannon: {}, then press Reload", if a < 1.5 { "empty the clip" } else { "fire one shot" }),
            44 => format!("Hand Cannon: one shot at an Ana {a:.0} m away"),
            30 => format!("Power Block arc: {c:.0} raw damage from {a:.0} deg"),
            31 => format!("empower threshold: one frontal hit of {a:.2} raw ({:.2} prevented if 75%), then full punch", a * 0.75),
            32 => format!("slam hit area edges: grid {a:.0}"),
            33 => format!("wall slam range: charge {a:.2}s, target {b:.0} m from the wall (wall {c:.1} m away)"),
            34 => format!("empowered splash: second target {a:.1} m to the side of the main one"),
            _ => format!("suite {suite}"),
        };
        println!("  take {} (suite {suite}): {what}", take.id);
        let rel = |t: f64| t - t0;
        let presses: Vec<String> = take
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Pressed { t, code } => Some(format!("{code}@{:.3}", rel(*t))),
                _ => None,
            })
            .collect();
        println!("    script events (code@s): {}", presses.join(" "));
        for r in &take.rows {
            match r {
                Row::Dealt { t, ability, damage, victim_slot } => {
                    println!("    dealt {damage:7.2} with ability {ability} to slot {victim_slot} at +{:.3}s", rel(*t))
                }
                Row::Took { t, damage, attacker_slot } => {
                    println!("    took  {damage:7.2} from slot {attacker_slot} at +{:.3}s", rel(*t))
                }
                _ => {}
            }
        }
        // Hand Cannon: every ammo change, and damage grouped by the tick it landed on.
        let mut last_ammo = None;
        let mut changes = Vec::new();
        for r in &take.rows {
            if let Row::Ammo { t, ammo, max_ammo, reloading, .. } = r {
                if last_ammo != Some(*ammo) {
                    changes.push(format!("{ammo:.0}/{max_ammo:.0}{}@{:.3}", if *reloading { "R" } else { "" }, rel(*t)));
                    last_ammo = Some(*ammo);
                }
            }
        }
        if !changes.is_empty() {
            println!("    ammo: {}", changes.join(" "));
        }
        if (40..=44).contains(&suite) {
            let mut volleys: std::collections::BTreeMap<i64, (usize, f32)> = Default::default();
            for r in &take.rows {
                if let Row::Dealt { t, damage, .. } = r {
                    let e = volleys.entry((rel(*t) * 1000.0).round() as i64).or_default();
                    e.0 += 1;
                    e.1 += damage;
                }
            }
            for (ms, (n, total)) in volleys {
                println!("    at +{:.3}s: {n} damage events, {total:.2} total", ms as f64 / 1000.0);
            }
        }
        // Each dummy: start position, top knockback speed, how far it moved.
        let mut slots: std::collections::BTreeMap<i64, (Vec3, Vec3, f32, f32)> = Default::default();
        for r in &take.rows {
            if let Row::Other { slot, pos, vel, .. } = r {
                let e = slots.entry(*slot).or_insert((*pos, *pos, 0.0, 0.0));
                e.1 = *pos;
                e.2 = e.2.max(vel.horizontal().length());
                e.3 = e.3.max(vel.y);
            }
        }
        for (slot, (p0, p1, flat, up)) in &slots {
            if *flat > 0.5 || (*p1 - *p0).length() > 0.2 {
                println!(
                    "    slot {slot}: knocked to {flat:.2} m/s flat, {up:+.2} up; moved {:.2} m flat",
                    (*p1 - *p0).horizontal().length()
                );
            }
        }
        // The bot: dash, block time, health above max.
        let fast: Vec<f32> = take.frames().map(|f| f.2.horizontal().length()).filter(|v| *v > 8.0).collect();
        if !fast.is_empty() {
            println!("    bot: {} ticks over 8 m/s, peak {:.2} m/s", fast.len(), fast.iter().cloned().fold(0.0f32, f32::max));
        }
        let block_ticks = take.rows.iter().filter(|r| matches!(r, Row::State { using_ability2: true, .. })).count();
        if block_ticks > 0 {
            println!("    bot: Power Block active {:.3}s", block_ticks as f32 * 0.016);
        }
        let over = take
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::State { health, max_health, .. } => Some(health - max_health),
                _ => None,
            })
            .fold(0.0f32, f32::max);
        if over > 0.0 {
            println!("    bot: up to {over:.1} health above max (temporary health)");
        }
    }
    if any {
        println!();
    }
}

fn replay(take: &Take, csv: Option<&PathBuf>) -> std::io::Result<()> {
    let ticks = replay_take(take, &Tunables::default());
    if ticks.is_empty() {
        println!("  replay: no paired F/I rows");
        return Ok(());
    }
    let max = ticks.iter().map(|t| t.error()).fold(0.0f32, f32::max);
    let rms = (ticks.iter().map(|t| t.error().powi(2)).sum::<f32>() / ticks.len() as f32).sqrt();
    let end = ticks.last().unwrap().error();
    println!("  replay vs sim: max error {max:.3} m, rms {rms:.3} m, final {end:.3} m");
    if let Some(path) = csv {
        let mut out = String::from("t,real_x,real_y,real_z,sim_x,sim_y,sim_z,real_vx,real_vy,real_vz,sim_vx,sim_vy,sim_vz,error\n");
        for t in &ticks {
            let (a, b, c, d) = (t.real_pos, t.sim_pos, t.real_vel, t.sim_vel);
            let _ = writeln!(
                out,
                "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4}",
                t.t, a.x, a.y, a.z, b.x, b.y, b.z, c.x, c.y, c.z, d.x, d.y, d.z, t.error()
            );
        }
        std::fs::write(path, out)?;
        println!("  wrote {}", path.display());
    }
    Ok(())
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let text = match read_log_file(&args.log) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("can't read {}: {e}", args.log.display());
            std::process::exit(1);
        }
    };
    let takes = split_takes(doomfist::log::parse_log(&text));
    if takes.is_empty() {
        eprintln!("no logger rows found in {}", args.log.display());
        std::process::exit(1);
    }
    let selected: Vec<&Take> = takes.iter().filter(|t| args.take.is_none_or(|n| n == t.id)).collect();
    slam_table(&selected);
    punch_table(&selected);
    hit_table(&selected);
    for take in selected {
        summarize(take);
        if args.replay {
            if let Err(e) = replay(take, args.csv.as_ref()) {
                eprintln!("  replay failed: {e}");
            }
        }
        println!();
    }
}
