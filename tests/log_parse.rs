use doomfist::log::{parse_line, parse_log, split_takes, Row};
use doomfist::replay::replay_take;
use doomfist::{Tunables, Vec3};

#[test]
fn parses_each_row_type() {
    let f = parse_line("[12:00:01] F;1000;1500;0;-2000;5500;0;0").unwrap();
    assert_eq!(f, Row::Frame { t: 1.0, pos: Vec3::new(1.5, 0.0, -2.0), vel: Vec3::new(5.5, 0.0, 0.0) });

    let i = parse_line("I;1000;0;0;1000;-1000;1000;True;False;False;False;False").unwrap();
    assert!(matches!(i, Row::Input { secondary: true, throttle_x, .. } if throttle_x == -1.0));

    let s = parse_line("S;1000;555000;525000;True;False;False;True;0;3500;0").unwrap();
    assert!(matches!(s, Row::State { health, cd_ability1, firing_secondary: true, .. } if health == 555.0 && cd_ability1 == 3.5));

    assert!(matches!(parse_line("D;1000;1;30000;6"), Some(Row::Dealt { ability: 1, damage, victim_slot: 6, .. }) if damage == 30.0));
    assert!(matches!(parse_line("K;1000;6"), Some(Row::Knockback { victim_slot: 6, .. })));
    assert!(parse_line("random chat line").is_none());
}

#[test]
fn handles_thousands_separators() {
    let s = parse_line("S;12,345,678;525,000;525,000;True;False;False;False;0;0;0").unwrap();
    assert!(matches!(s, Row::State { health, .. } if health == 525.0));
}

#[test]
fn splits_takes_and_replays_a_straight_walk() {
    // Doomfist walks forward at 5.5 m/s for 1 second, logged every 16 ms.
    let mut text = String::from("R;0;1;True\n");
    for i in 0..=62 {
        let t = i * 16;
        let z = (5.5 * t as f64).round() as i64; // metres*1000 = 5.5 * ms
        text += &format!("F;{t};0;0;{z};0;0;5500\n");
        text += &format!("I;{t};0;0;1000;0;1000;False;False;False;False;False\n");
    }
    text += "R;2000;1;False\n";

    let takes = split_takes(parse_log(&text));
    assert_eq!(takes.len(), 1);
    let ticks = replay_take(&takes[0], &Tunables::default());
    assert_eq!(ticks.len(), 63);
    let worst = ticks.iter().map(|t| t.error()).fold(0.0f32, f32::max);
    assert!(worst < 0.05, "sim drifted {worst} m from a straight 5.5 m/s walk");
}

#[test]
fn parses_automatic_test_rows() {
    assert!(matches!(
        parse_line("[00:01:00] X;5000;2;-30000;150;0"),
        Some(Row::Test { suite: 2, pitch_deg, delay, mode: 0, .. }) if pitch_deg == -30.0 && delay == 0.15
    ));
    assert!(matches!(parse_line("E;5200;1"), Some(Row::Pressed { code: 1, .. })));
}

#[test]
fn parses_punch_test_rows() {
    let r = parse_line("Y;9000;4;1000;0;200;-1000;-45000;0").unwrap();
    assert!(matches!(r, Row::PunchTest { suite: 4, charge, jump_at, slam_at, turn_deg, no_jump: false, .. }
        if charge == 1.0 && jump_at == 0.2 && slam_at == -1.0 && turn_deg == -45.0));
}

#[test]
fn sim_recording_round_trips_and_makes_a_playback_script() {
    use doomfist::playback::{ticks_from_take, workshop_script};
    use doomfist::record::log_rows;
    use doomfist::{Input, Sim};
    // Charge a punch, release, then hold jump mid-dash with the camera turned 40°.
    let mut sim = Sim::new(Tunables::default(), Vec3::ZERO);
    let ahead = Input::facing(Vec3::FORWARD);
    let turned = Input::facing(Vec3::new(0.643, 0.0, 0.766));
    let mut rows = vec!["R;0;1;True".to_string()];
    let script: Vec<Input> = (0..40)
        .map(|_| Input { secondary: true, ..ahead })
        .chain((0..10).map(|_| ahead))
        .chain((0..30).map(|_| Input { jump: true, ..turned }))
        .collect();
    for (k, input) in script.iter().enumerate() {
        sim.step(input);
        rows.extend(log_rows((k as u64 + 1) * 16, &sim.doom, input));
    }
    rows.push("R;9999;1;False".into());
    let takes = split_takes(parse_log(&rows.join("\n")));
    assert_eq!(takes.len(), 1);
    let frames: Vec<_> = takes[0].frames().collect();
    assert_eq!(frames.len(), script.len());
    assert!((frames.last().unwrap().1 - sim.doom.pos).length() < 0.002, "positions survive the round trip");

    let ticks = ticks_from_take(&takes[0]);
    assert_eq!(ticks.len(), script.len());
    assert_eq!(ticks[0].buttons & 1, 1, "punch held at the start");
    assert!((ticks.last().unwrap().yaw_deg - 40.0).abs() < 0.2, "yaw relative to start");
    let ow = workshop_script(&ticks, "test");
    assert!(ow.contains("Global.Events = Array(") && !ow.contains("@ARRAYS@"));
}

#[test]
fn compressed_playback_stays_within_tolerance_of_real_inputs() {
    use doomfist::playback::{compress, embedded_numbers, expand, key_state, ticks_from_take, AIM_TOLERANCE_DEG};
    let Ok(text) = doomfist::log::read_log_file(std::path::Path::new("logs/punch-turn-01.txt")) else { return };
    for take in split_takes(parse_log(&text)) {
        let ticks = ticks_from_take(&take);
        let c = compress(&ticks);
        let back = expand(&c);
        assert_eq!(back.len(), ticks.len());
        for (a, b) in ticks.iter().zip(&back) {
            assert_eq!(key_state(a), key_state(b), "keys are exact");
            assert!((a.yaw_deg - b.yaw_deg).abs() <= AIM_TOLERANCE_DEG + 0.01);
            assert!((a.pitch_deg - b.pitch_deg).abs() <= AIM_TOLERANCE_DEG + 0.01);
        }
        println!("take {}: {} ticks -> {} numbers ({} key changes, {} aim keyframes)", take.id, ticks.len(), embedded_numbers(&c), c.events.len(), c.key_ticks.len());
    }
}

#[test]
fn parses_hit_test_rows() {
    let r = parse_line("[00:01:00] Z;5000;23;90000;1000;100000").unwrap();
    assert!(matches!(r, Row::HitTest { suite: 23, a, b, c, .. } if a == 90.0 && b == 1.0 && c == 100.0));
}

#[test]
fn parses_ammo_rows() {
    let r = parse_line("A;5000;3;4;True;False;True").unwrap();
    assert!(matches!(r, Row::Ammo { ammo, max_ammo, firing: true, reloading: false, fire_held: true, .. } if ammo == 3.0 && max_ammo == 4.0));
}
