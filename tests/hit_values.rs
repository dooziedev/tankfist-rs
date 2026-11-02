//! Checks the punch damage formulas against every logged hit in logs/hit-01.txt (automatic hit
//! tests): impact and wall-slam damage for each charge, and the empowered multiplier.

use doomfist::abilities::punch::charge_fraction;
use doomfist::log::{parse_log, read_log_file, split_takes, Row};
use doomfist::Tunables;
use std::path::Path;

#[test]
fn punch_damage_matches_the_log() {
    let Ok(text) = read_log_file(Path::new("logs/hit-01.txt")) else {
        eprintln!("log missing; skipped");
        return;
    };
    let p = Tunables::default().punch;
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let mut checked = 0;
    for take in split_takes(parse_log(&text)) {
        let Some(&Row::HitTest { suite, b, .. }) = take.rows.iter().find(|r| matches!(r, Row::HitTest { .. })) else { continue };
        let press = |code: i64| take.rows.iter().find_map(|r| match r { Row::Pressed { t, code: c } if *c == code => Some(*t), _ => None });
        let (Some(hold), Some(release)) = (press(4), press(5)) else { continue };
        let held = (release - hold) as f32;
        let power = charge_fraction(held, p.damage_charge_start, p.damage_charge_full);
        let dealt: Vec<(i64, f32)> = take.rows.iter().filter_map(|r| match r { Row::Dealt { damage, victim_slot, .. } => Some((*victim_slot, *damage)), _ => None }).collect();
        let empowered = suite == 25 || (suite == 24 && dealt.first().is_some_and(|d| d.1 > 80.0));
        let mult = if empowered { p.empowered_damage_mult } else { 1.0 };
        let impact = lerp(p.min_impact_damage, p.max_impact_damage, power) * mult;
        let wall = lerp(p.min_wall_damage, p.max_wall_damage, power) * mult;
        // Reinhardt (slot 4) has armor, which isn't modelled here.
        let Some(&(slot, first)) = dealt.first() else { continue };
        if slot == 4 {
            continue;
        }
        assert!((first - impact).abs() < 0.01, "take {} impact {first} vs {impact} (held {held})", take.id);
        if suite == 21 || (suite == 25 && b > 0.5 && dealt.len() == 2 && dealt[1].0 == slot) {
            if let Some(&(_, w)) = dealt.get(1) {
                assert!((w - wall).abs() < 0.01, "take {} wall {w} vs {wall}", take.id);
            }
        }
        checked += 1;
    }
    println!("checked {checked} punch hits");
    assert!(checked >= 15);
}
