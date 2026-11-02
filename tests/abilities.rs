//! These check that the abilities and their interactions behave as designed, using whatever
//! numbers are in `Tunables`. They read values from the tunables, so they keep passing as
//! guesses are replaced with measured numbers.

use doomfist::events::{Ability, EndReason, Event, HitKind};
use doomfist::world::{Aabb, Target};
use doomfist::{ActionKind, Input, Sim, Tunables, Vec3};

fn sim() -> Sim {
    Sim::new(Tunables::default(), Vec3::ZERO)
}

fn look(dir: Vec3) -> Input {
    Input::facing(dir)
}

/// Holds secondary for `charge` seconds, then releases and waits `after` seconds.
fn punch(s: &mut Sim, charge: f32, after: f32) -> Vec<Event> {
    // Always hold for at least one tick so the press registers.
    let charge = charge.max(s.doom.tun.dt());
    let mut ev = s.run(&Input { secondary: true, ..look(Vec3::FORWARD) }, charge);
    ev.extend(s.run(&look(Vec3::FORWARD), after));
    ev
}

fn has(ev: &[Event], f: impl Fn(&Event) -> bool) -> bool {
    ev.iter().any(f)
}

// ---------- Movement ----------

#[test]
fn walks_at_walk_speed() {
    let mut s = sim();
    s.run(&Input { move_forward: 1.0, ..look(Vec3::FORWARD) }, 1.0);
    let expected = s.doom.tun.movement.walk_speed;
    assert!((s.doom.pos.z - expected).abs() < 0.05, "moved {}", s.doom.pos.z);
}

#[test]
fn jump_goes_up_and_comes_back_down() {
    let mut s = sim();
    let ev = s.step(&Input { jump: true, ..look(Vec3::FORWARD) });
    assert!(has(&ev, |e| matches!(e, Event::Jumped)));
    s.run(&look(Vec3::FORWARD), 2.0);
    assert!(s.doom.grounded && s.doom.pos.y == 0.0);
}

#[test]
fn walls_stop_movement() {
    let mut s = sim();
    s.world.walls.push(Aabb::new(Vec3::new(-5.0, 0.0, 2.0), Vec3::new(5.0, 5.0, 3.0)));
    s.run(&Input { move_forward: 1.0, ..look(Vec3::FORWARD) }, 2.0);
    assert!(s.doom.pos.z < 2.0);
}

// ---------- Rocket Punch ----------

#[test]
fn uncharged_punch_travels_min_distance() {
    let mut s = sim();
    punch(&mut s, 0.0, 2.0);
    let p = s.doom.tun.punch;
    let dt = s.doom.tun.dt();
    // Partial first tick + the minimum full-speed ticks (6.03 m in the log), then the exit tick.
    let dash = p.speed * dt * (p.first_tick_fraction + p.min_dash_ticks as f32);
    let expected = dash + p.exit_speed * dt;
    let slide = 0.25; // friction slows the exit speed to a stop over a few more cm
    let went = s.doom.pos.z;
    assert!(went >= expected - 0.01 && went <= expected + slide, "went {went}, dash {expected}");
}

#[test]
fn full_charge_goes_further_than_no_charge() {
    let mut a = sim();
    punch(&mut a, 0.0, 2.0);
    let mut b = sim();
    punch(&mut b, 1.5, 2.0);
    assert!(b.doom.pos.z > a.doom.pos.z + 5.0);
}

#[test]
fn punch_hits_target_knocks_back_and_starts_cooldown() {
    let mut s = sim();
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 8.0)));
    let ev = punch(&mut s, 0.5, 0.2);
    assert!(has(&ev, |e| matches!(e, Event::DamageDealt { kind: HitKind::Impact, target: 1, .. })));
    assert!(has(&ev, |e| matches!(e, Event::ActionEnded(ActionKind::Punch, EndReason::HitTarget))));
    assert!(s.world.target(1).unwrap().pos.z > 8.0, "target was pushed");
    assert!(s.doom.cooldowns.punch > 0.0);
}

#[test]
fn punched_into_wall_takes_wall_damage() {
    let mut s = sim();
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 8.0)));
    s.world.walls.push(Aabb::new(Vec3::new(-5.0, 0.0, 10.0), Vec3::new(5.0, 5.0, 11.0)));
    let ev = punch(&mut s, 1.0, 1.0);
    assert!(has(&ev, |e| matches!(e, Event::DamageDealt { kind: HitKind::WallSlam, target: 1, .. })));
}

#[test]
fn no_wall_damage_if_wall_is_too_far() {
    let mut s = sim();
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 8.0)));
    s.world.walls.push(Aabb::new(Vec3::new(-5.0, 0.0, 60.0), Vec3::new(5.0, 5.0, 61.0)));
    let ev = punch(&mut s, 1.0, 3.0);
    assert!(!has(&ev, |e| matches!(e, Event::DamageDealt { kind: HitKind::WallSlam, .. })));
}

// ---------- Passive ----------

#[test]
fn ability_hits_give_temp_health_up_to_cap_then_it_drains() {
    let mut s = sim();
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 8.0)));
    let ev = punch(&mut s, 0.5, 0.5);
    let p = s.doom.tun.passive;
    assert!(has(&ev, |e| matches!(e, Event::OverhealthGained { .. })));
    assert!(s.doom.overhealth > 0.0 && s.doom.overhealth <= p.cap);

    let before = s.doom.overhealth;
    s.run(&look(Vec3::FORWARD), p.decay_delay + 1.0);
    assert!(s.doom.overhealth < before, "temp health drains");
}

#[test]
fn temp_health_absorbs_damage_first() {
    let mut s = sim();
    s.doom.overhealth = 50.0;
    s.doom.take_damage(30.0, Vec3::new(0.0, 0.0, 10.0));
    assert_eq!(s.doom.overhealth, 20.0);
    assert_eq!(s.doom.health, s.doom.tun.max_health);
}

// ---------- Power Block ----------

#[test]
fn block_reduces_frontal_damage_only() {
    let mut s = sim();
    s.step(&Input { ability2: true, ..look(Vec3::FORWARD) });
    let max = s.doom.tun.max_health;
    let red = s.doom.tun.block.reduction;

    s.doom.take_damage(100.0, Vec3::new(0.0, 0.0, 10.0)); // in front
    assert!((max - s.doom.health - 100.0 * (1.0 - red)).abs() < 1e-3);

    let hp = s.doom.health;
    s.doom.take_damage(100.0, Vec3::new(0.0, 0.0, -10.0)); // behind
    assert!((hp - s.doom.health - 100.0).abs() < 1e-3);
}

#[test]
fn blocking_enough_empowers_and_punch_uses_it() {
    let mut s = sim();
    s.step(&Input { ability2: true, ..look(Vec3::FORWARD) });
    let need = s.doom.tun.block.empower_threshold / s.doom.tun.block.reduction;
    s.doom.take_damage(need + 1.0, Vec3::new(0.0, 0.0, 10.0));
    let ev = s.run(&look(Vec3::FORWARD), s.doom.tun.block.duration + 0.1);
    assert!(has(&ev, |e| matches!(e, Event::Empowered)));
    assert!(s.doom.empowered);

    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 8.0)));
    // Half charge: a minimum-charge punch only goes 6 m (measured), short of the target.
    let ev = punch(&mut s, 0.5, 0.5);
    assert!(has(&ev, |e| matches!(e, Event::EmpowerConsumed)));
    // Empowered impact = the normal impact for this charge x the empowered multiplier.
    let p = s.doom.tun.punch;
    let held = (0.5f32 * 62.5).round() * s.doom.tun.dt();
    let power = doomfist::abilities::punch::charge_fraction(held, p.damage_charge_start, p.damage_charge_full);
    let emp = (p.min_impact_damage + (p.max_impact_damage - p.min_impact_damage) * power) * p.empowered_damage_mult;
    assert!(has(&ev, |e| matches!(e, Event::DamageDealt { amount, kind: HitKind::Impact, .. } if (*amount - emp).abs() < 0.01)));
    assert!(!s.doom.empowered);
}

#[test]
fn empowered_punch_also_hits_nearby_enemies() {
    let mut s = sim();
    s.doom.empowered = true;
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 8.0)));
    // 1.6 m from the main target, like the second dummy in the hit tests.
    s.world.targets.push(Target::new(2, Vec3::new(1.5, 0.0, 8.5)));
    let ev = punch(&mut s, 0.5, 0.3);
    let hit_ids: Vec<u32> = ev
        .iter()
        .filter_map(|e| match e {
            Event::DamageDealt { target, kind: HitKind::Impact, .. } => Some(*target),
            _ => None,
        })
        .collect();
    assert!(hit_ids.contains(&1) && hit_ids.contains(&2));
}

// ---------- Seismic Slam ----------

#[test]
fn slam_leaps_lands_and_damages_target_in_front() {
    let mut s = sim();
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 16.0))); // level slam lands ~14 m out
    let mut ev = s.step(&Input { ability1: true, ..look(Vec3::FORWARD) });
    ev.extend(s.run(&look(Vec3::FORWARD), 2.0));
    assert!(has(&ev, |e| matches!(e, Event::ActionEnded(ActionKind::Slam, EndReason::Completed))));
    assert!(has(&ev, |e| matches!(e, Event::DamageDealt { ability: Ability::SeismicSlam, .. })));
    assert!(s.doom.pos.z > 3.0, "leapt forward");
    assert!(s.doom.cooldowns.slam > 0.0);
}

#[test]
fn slam_misses_target_behind() {
    let mut s = sim();
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, -3.0)));
    let mut ev = s.step(&Input { ability1: true, ..look(Vec3::FORWARD) });
    ev.extend(s.run(&look(Vec3::FORWARD), 2.0));
    assert!(!has(&ev, |e| matches!(e, Event::DamageDealt { .. })));
}

// ---------- Interactions (cancel rules) ----------

#[test]
fn allowed_cancel_punch_out_of_block() {
    let mut s = sim();
    s.step(&Input { ability2: true, ..look(Vec3::FORWARD) });
    let ev = s.step(&Input { secondary: true, ..look(Vec3::FORWARD) });
    assert!(has(&ev, |e| matches!(e, Event::ActionEnded(ActionKind::Block, EndReason::Cancelled))));
    assert_eq!(s.doom.current(), Some(ActionKind::PunchCharge));
    assert!(s.doom.cooldowns.block > 0.0, "cancelled block still goes on cooldown");
}

#[test]
fn disallowed_cancel_is_ignored() {
    let mut s = sim();
    s.doom.tun.interactions.allowed_cancels.clear();
    s.step(&Input { ability2: true, ..look(Vec3::FORWARD) });
    s.step(&Input { secondary: true, ..look(Vec3::FORWARD) });
    assert_eq!(s.doom.current(), Some(ActionKind::Block));
}

#[test]
fn slam_hit_can_empower_when_enabled() {
    let mut s = sim();
    s.doom.tun.interactions.slam_hit_empowers = true;
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 16.0)));
    s.step(&Input { ability1: true, ..look(Vec3::FORWARD) });
    s.run(&look(Vec3::FORWARD), 2.0);
    assert!(s.doom.empowered);
}

// ---------- Mouse turning bends the punch; slam bunny hop ----------

#[test]
fn jump_boost_goes_toward_the_camera_within_the_cone() {
    let mut s = sim();
    s.run(&Input { secondary: true, ..look(Vec3::FORWARD) }, 1.0);
    s.run(&look(Vec3::FORWARD), 0.2);
    // Look 90° to the side (past the 60° cone) and hold jump.
    let side = look(Vec3::new(1.0, 0.0, 0.0));
    s.step(&side);
    s.step(&Input { jump: true, ..side }); // press tick: small hop, dash goes on
    s.step(&Input { jump: true, ..side }); // held: boost
    let v = s.doom.vel.horizontal();
    let angle = v.x.atan2(v.z).to_degrees();
    // 6 along the dash + 24.947 at the 60° cone edge -> 49.5° at 28.4 m/s (log: 49.56°, 28.50).
    assert!((angle - 49.5).abs() < 0.5, "boost angle {angle}");
    assert!((v.length() - 28.45).abs() < 0.2, "boost speed {}", v.length());
}

#[test]
fn slam_with_jump_held_bunny_hops_on_landing() {
    let mut hold = sim();
    hold.step(&Input { ability1: true, ..look(Vec3::FORWARD) });
    let ev = hold.run(&Input { jump: true, ..look(Vec3::FORWARD) }, 1.5);
    assert!(has(&ev, |e| matches!(e, Event::Jumped)), "bunny hop on touchdown");

    let mut plain = sim();
    plain.step(&Input { ability1: true, ..look(Vec3::FORWARD) });
    plain.run(&look(Vec3::FORWARD), 1.5);
    assert!(hold.doom.pos.z > plain.doom.pos.z + 1.0, "bunny hop carries further than the slide");
}

#[test]
fn slam_can_be_cancelled_by_pressing_it_again_after_half_a_second() {
    let mut s = sim();
    let slam = Input { ability1: true, ..look(Vec3::FORWARD) };
    s.step(&slam);
    s.step(&look(Vec3::FORWARD)); // launches (one tick after the press)
    // Too early: pressing again does nothing.
    s.run(&look(Vec3::FORWARD), 0.2);
    s.step(&slam);
    assert_eq!(s.doom.current(), Some(ActionKind::Slam));
    // After 0.5 s: cancels, keeping the leap's speed.
    s.run(&look(Vec3::FORWARD), 0.35);
    let speed = s.doom.vel.horizontal().length();
    let ev = s.step(&slam);
    assert!(has(&ev, |e| matches!(e, Event::ActionEnded(ActionKind::Slam, EndReason::Cancelled))));
    assert_eq!(s.doom.current(), None);
    assert!(s.doom.vel.horizontal().length() > speed * 0.9, "keeps its speed");
}

// ---------- Quick melee, normal punch splash ----------

#[test]
fn melee_hits_in_front_only_and_gives_no_temp_health() {
    let mut s = sim();
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 1.5)));
    s.world.targets.push(Target::new(2, Vec3::new(0.0, 0.0, -1.5)));
    let ev = s.step(&Input { melee: true, ..look(Vec3::FORWARD) });
    let hits: Vec<u32> = ev.iter().filter_map(|e| match e { Event::DamageDealt { target, .. } => Some(*target), _ => None }).collect();
    assert_eq!(hits, vec![1]);
    assert_eq!(s.doom.overhealth, 0.0, "melee doesn't feed the passive");
    // Cooldown: pressing again right away does nothing.
    s.step(&look(Vec3::FORWARD));
    let ev = s.step(&Input { melee: true, ..look(Vec3::FORWARD) });
    assert!(!has(&ev, |e| matches!(e, Event::DamageDealt { .. })));
}

#[test]
fn normal_punch_also_hits_a_nearby_enemy() {
    let mut s = sim();
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 8.0)));
    s.world.targets.push(Target::new(2, Vec3::new(2.0, 0.0, 8.0)));
    let ev = punch(&mut s, 0.5, 0.5);
    let hit: Vec<u32> = ev.iter().filter_map(|e| match e { Event::DamageDealt { target, kind: HitKind::Impact, .. } => Some(*target), _ => None }).collect();
    assert!(hit.contains(&1) && hit.contains(&2), "{hit:?}");
}
