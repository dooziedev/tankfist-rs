//! Hand Cannon against the automatic gun tests (logs/gun-01.txt).

use doomfist::events::{Ability, Event};
use doomfist::world::Target;
use doomfist::{Input, Sim, Tunables, Vec3};

fn sim() -> Sim {
    Sim::new(Tunables::default(), Vec3::ZERO)
}

/// Holding fire for 3 s from a full clip, then letting go (suite 40): seconds after the first
/// shot at which ammo changes, as (time, ammo).
fn hold_fire_timeline() -> Vec<(f32, u32)> {
    let mut s = sim();
    let fire = Input { primary: true, ..Input::facing(Vec3::FORWARD) };
    let idle = Input::facing(Vec3::FORWARD);
    let mut out = Vec::new();
    let mut last = s.doom.gun.ammo;
    let mut shots = Vec::new();
    for tick in 0..300 {
        let ev = s.step(if tick < 188 { &fire } else { &idle });
        let t = tick as f32 * 0.016;
        if ev.iter().any(|e| matches!(e, Event::Shot)) {
            shots.push(t);
        }
        if s.doom.gun.ammo != last {
            last = s.doom.gun.ammo;
            out.push((t, last));
        }
    }
    println!("shots at {shots:?}");
    out
}

#[test]
fn hold_fire_and_refill_match_the_log() {
    let tl = hold_fire_timeline();
    println!("ammo changes {tl:?}");
    // Logged (relative to the first shot): 4 shots 0.32-0.336 s apart, then while still holding
    // fire on empty, a shot each time a shell returns (1.82 s and 2.56 s); after letting go at
    // ~2.99 s the clip refills 1 shell at ~3.20 s then one every 0.40 s.
    let empty_at = tl.iter().find(|x| x.1 == 0).unwrap().0;
    assert!((empty_at - 1.008).abs() < 0.04, "clip empty at {empty_at}");
    let full_again = tl.iter().rev().find(|x| x.1 == 4).unwrap().0;
    let first_back = tl.iter().rev().find(|x| x.1 == 1).unwrap().0;
    assert!((full_again - first_back - 1.2).abs() < 0.04, "3 more shells take {}", full_again - first_back);
    assert!((first_back - 3.20).abs() < 0.15, "first shell back at {first_back}");
}

#[test]
fn one_shot_damage_by_distance_matches_the_log() {
    // Logged total for one shot at an Ana (aimed at her chest): 3 m 55, 6 m 60-65, 10 m 45,
    // 15 m 35, 20 m 19, 30 m 6.
    for (d, real) in [(3.0, 55.0), (6.0, 62.5), (10.0, 45.0), (15.0, 35.0), (20.0, 19.2), (30.0, 6.2)] {
        let mut s = sim();
        s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, d)));
        let aim = (Vec3::new(0.0, 1.0, d) - Vec3::new(0.0, s.doom.tun.gun.muzzle_height, 0.0)).normalize_or_zero();
        let mut total = 0.0;
        let ev = s.step(&Input { primary: true, ..Input::facing(aim) });
        let mut all = ev;
        all.extend(s.run(&Input::facing(aim), 1.0));
        for e in &all {
            if let Event::DamageDealt { ability: Ability::HandCannon, amount, .. } = e {
                total += amount;
            }
        }
        println!("{d} m: {total:.1} (log {real})");
        assert!((total - real).abs() <= real * 0.35 + 3.0, "{d} m: {total} vs log {real}");
    }
}
