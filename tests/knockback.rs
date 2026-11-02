//! A knocked dummy's slide against the hit tests (logs/hit-01.txt): distance travelled after a
//! punch at minimum charge (15 m/s), Reinhardt at full charge (21 m/s), and full charge (35 m/s).

use doomfist::events::Event;
use doomfist::world::{Target, World};
use doomfist::Vec3;

fn slide(speed: f32) -> f32 {
    let mut w = World::flat(17.5);
    let mut t = Target::new(1, Vec3::ZERO);
    t.vel = Vec3::new(0.0, 1.0, speed);
    t.grounded = false;
    t.knock_timer = w.knock_time;
    w.targets.push(t);
    let mut ev: Vec<Event> = Vec::new();
    for _ in 0..200 {
        w.step(0.016, &mut ev);
    }
    w.targets[0].pos.z
}

#[test]
fn knockback_slide_distances_match_the_log() {
    // Logged: dummy moved 5.50 m (15 m/s), 7.81 m (21 m/s, Reinhardt), 13.23 m (35 m/s).
    for (speed, real) in [(15.0, 5.50), (21.0, 7.81), (35.0, 13.23)] {
        let d = slide(speed);
        println!("{speed} m/s: slid {d:.2} m (log {real})");
        // Within ~1%; the remainder is likely a tick of timing on the hit itself.
        assert!((d - real).abs() < 0.2, "{speed} m/s slid {d} m, log {real}");
    }
}

/// Punches a dummy standing `gap` m in front of a wall; true if it got wall-slammed.
fn wall_slam(charge_s: f32, gap: f32) -> bool {
    use doomfist::events::HitKind;
    use doomfist::world::Aabb;
    use doomfist::{Input, Sim, Tunables};
    let mut s = Sim::new(Tunables::default(), Vec3::ZERO);
    s.world.targets.push(Target::new(1, Vec3::new(0.0, 0.0, 6.0)));
    let wall_z = 6.0 + gap + 0.4; // dummy radius 0.4
    s.world.walls.push(Aabb::new(Vec3::new(-5.0, 0.0, wall_z), Vec3::new(5.0, 5.0, wall_z + 1.0)));
    let ahead = Input::facing(Vec3::FORWARD);
    let ticks = ((charge_s * 62.5).round() as usize).max(1);
    let mut ev = Vec::new();
    for _ in 0..ticks {
        ev.extend(s.step(&Input { secondary: true, ..ahead }));
    }
    for _ in 0..150 {
        ev.extend(s.step(&ahead));
    }
    ev.iter().any(|e| matches!(e, Event::DamageDealt { kind: HitKind::WallSlam, .. }))
}

#[test]
fn wall_slams_happen_where_the_log_says() {
    // logs/hit-01 and hit-02: full charge slammed at 12 m from the wall, not at 16 m;
    // minimum charge slammed at 3 m, not at 8 m.
    assert!(wall_slam(1.0, 12.0));
    assert!(!wall_slam(1.0, 16.0));
    assert!(wall_slam(0.0, 3.0));
    assert!(!wall_slam(0.0, 8.0));
}
