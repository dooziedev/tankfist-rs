//! A long free-play take (logs/compare-01.txt take 1: punches, jump cancels, slam steering,
//! bunny hop, Power Block tap on/off, an airborne punch) replayed through the sim from the
//! logged inputs, with no re-syncing. Guards everything fitted from it.
//!
//!   cargo run --bin compare -- sim logs/compare-01.txt --ow-take 1   (full report)

use doomfist::log::{parse_log, read_log_file, split_takes};
use doomfist::replay::replay_take;
use doomfist::Tunables;
use std::path::Path;

#[test]
fn free_play_take_matches_for_twelve_seconds() {
    let Ok(text) = read_log_file(Path::new("logs/compare-01.txt")) else {
        eprintln!("log missing; skipped");
        return;
    };
    let take = split_takes(parse_log(&text)).into_iter().find(|t| t.id == 1).unwrap();
    let ticks = replay_take(&take, &Tunables::default());
    let t0 = ticks[0].t;
    let worst = ticks.iter().filter(|t| t.t - t0 < 12.0).map(|t| t.error()).fold(0.0f32, f32::max);
    println!("worst over the first 12 s: {worst:.3} m");
    assert!(worst < 0.25, "worst {worst} m");
}

#[test]
fn long_air_steering_take_matches_for_thirty_seconds() {
    // logs/air-cancel-01.txt take 8: 51 s of jumps, strafing in the air, punches, airborne
    // punch cancels and landings with jump pressed on the landing tick.
    let Ok(text) = read_log_file(Path::new("logs/air-cancel-01.txt")) else {
        eprintln!("log missing; skipped");
        return;
    };
    let Some(take) = split_takes(parse_log(&text)).into_iter().find(|t| t.id == 8) else { return };
    let ticks = replay_take(&take, &Tunables::default());
    let t0 = ticks[0].t;
    let worst = ticks.iter().filter(|t| t.t - t0 < 30.0).map(|t| t.error()).fold(0.0f32, f32::max);
    println!("worst over the first 30 s: {worst:.3} m");
    assert!(worst < 0.25, "worst {worst} m");
}
