//! Reads the Workshop Inspector log written by `workshop/doomfist_logger.txt`.
//!
//! Each line is one row, fields split by ';'. Numbers are logged x1000 as whole numbers so no
//! precision is lost. Lines may start with a "[hh:mm:ss] " timestamp, which is skipped.
//!
//!   R;t;take;recording
//!   F;t;px;py;pz;vx;vy;vz                              Doomfist position, velocity
//!   I;t;fx;fy;fz;throttle_x;throttle_z;sec;a1;a2;jump;crouch
//!   S;t;health;max_health;grounded;using_a1;using_a2;firing_secondary;cd_sec;cd_a1;cd_a2
//!   P;t;slot;px;py;pz;vx;vy;vz;health                  any other player (dummies)
//!   D;t;ability;damage;victim_slot                     ability: 0 primary, 1 punch, 2 slam, 3 block, 4 ult, 5 melee, -1 other
//!   T;t;damage;attacker_slot
//!   K;t;victim_slot
//!   X;t;suite;pitch_deg;delay_s;mode                   automatic test label (pitch/delay x1000)
//!                                                      mode: 0 standing, 1 running, 2 mid-air, 3 block first
//!   Y;t;suite;charge_s;pitch_deg;jump_at;slam_at;turn_deg;no_jump   automatic punch test label (x1000; -1 = not used)
//!   E;t;code                                           automatic test pressed a button:
//!                                                      1 slam, 2 block, 3 jump, 4 punch charge start, 5 punch release, 6 turn,
//!                                                      7 start holding jump, 8 stop holding jump, 9 bot seen on the ground,
//!                                                      11 script dealt damage to the bot, 12 bot's Power Block ended,
//!                                                      13 start holding fire, 14 stop holding fire, 15 one shot, 16 reload
//!   A;t;ammo;max_ammo;firing;reloading;fire_held        Hand Cannon ammo, every tick (whole numbers)
//!   Z;t;suite;a;b;c                                    hit-test label (a, b, c x1000; meaning depends on the suite,
//!                                                      see workshop/auto_hit_tests.txt)

use crate::math::Vec3;

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Take { t: f64, take: i64, recording: bool },
    Frame { t: f64, pos: Vec3, vel: Vec3 },
    Input {
        t: f64,
        facing: Vec3,
        throttle_x: f32,
        throttle_z: f32,
        secondary: bool,
        ability1: bool,
        ability2: bool,
        jump: bool,
        crouch: bool,
    },
    State {
        t: f64,
        health: f32,
        max_health: f32,
        grounded: bool,
        using_ability1: bool,
        using_ability2: bool,
        firing_secondary: bool,
        cd_secondary: f32,
        cd_ability1: f32,
        cd_ability2: f32,
    },
    Other { t: f64, slot: i64, pos: Vec3, vel: Vec3, health: f32 },
    Dealt { t: f64, ability: i64, damage: f32, victim_slot: i64 },
    Took { t: f64, damage: f32, attacker_slot: i64 },
    Knockback { t: f64, victim_slot: i64 },
    /// Which automatic test this take is.
    Test { t: f64, suite: i64, pitch_deg: f32, delay: f32, mode: i64 },
    /// Which automatic punch test this take is. Times are seconds; -1 means not used.
    /// `jump_at` is after the punch release; `slam_at` is after the jump (or after release if no jump).
    PunchTest {
        t: f64,
        suite: i64,
        charge: f32,
        pitch_deg: f32,
        jump_at: f32,
        slam_at: f32,
        turn_deg: f32,
        no_jump: bool,
    },
    /// Hand Cannon ammo and state on one tick.
    Ammo { t: f64, ammo: f32, max_ammo: f32, firing: bool, reloading: bool, fire_held: bool },
    /// Which automatic hit / block / empower test this take is (see workshop/auto_hit_tests.txt).
    HitTest { t: f64, suite: i64, a: f32, b: f32, c: f32 },
    /// The automatic test pressed a button (1 slam, 2 block, 3 jump, 4 charge, 5 release, 6 turn).
    Pressed { t: f64, code: i64 },
}

impl Row {
    pub fn t(&self) -> f64 {
        match self {
            Row::Take { t, .. }
            | Row::Frame { t, .. }
            | Row::Input { t, .. }
            | Row::State { t, .. }
            | Row::Other { t, .. }
            | Row::Dealt { t, .. }
            | Row::Took { t, .. }
            | Row::Knockback { t, .. }
            | Row::Test { t, .. }
            | Row::PunchTest { t, .. }
            | Row::HitTest { t, .. }
            | Row::Ammo { t, .. }
            | Row::Pressed { t, .. } => *t,
        }
    }
}

/// A raw number as logged (thousands separators removed).
fn raw(s: &str) -> Option<f64> {
    s.trim().replace(',', "").parse::<f64>().ok()
}

/// A number that was logged x1000.
fn milli(s: &str) -> Option<f32> {
    raw(s).map(|v| (v / 1000.0) as f32)
}

fn int(s: &str) -> i64 {
    raw(s).map(|v| v as i64).unwrap_or(-1)
}

fn boolean(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn vec3(f: &[&str]) -> Option<Vec3> {
    Some(Vec3::new(milli(f[0])?, milli(f[1])?, milli(f[2])?))
}

pub fn parse_line(line: &str) -> Option<Row> {
    let mut s = line.trim().trim_start_matches('\u{feff}');
    if s.starts_with('[') {
        s = &s[s.find(']')? + 1..];
    }
    let f: Vec<&str> = s.trim().split(';').collect();
    let t = || raw(f.get(1)?).map(|v| v / 1000.0);
    let need = |n: usize| if f.len() >= n { Some(()) } else { None };

    Some(match f[0] {
        "R" => {
            need(4)?;
            Row::Take { t: t()?, take: int(f[2]), recording: boolean(f[3])? }
        }
        "F" => {
            need(8)?;
            Row::Frame { t: t()?, pos: vec3(&f[2..5])?, vel: vec3(&f[5..8])? }
        }
        "I" => {
            need(12)?;
            Row::Input {
                t: t()?,
                facing: vec3(&f[2..5])?,
                throttle_x: milli(f[5])?,
                throttle_z: milli(f[6])?,
                secondary: boolean(f[7])?,
                ability1: boolean(f[8])?,
                ability2: boolean(f[9])?,
                jump: boolean(f[10])?,
                crouch: boolean(f[11])?,
            }
        }
        "S" => {
            need(11)?;
            Row::State {
                t: t()?,
                health: milli(f[2])?,
                max_health: milli(f[3])?,
                grounded: boolean(f[4])?,
                using_ability1: boolean(f[5])?,
                using_ability2: boolean(f[6])?,
                firing_secondary: boolean(f[7])?,
                cd_secondary: milli(f[8])?,
                cd_ability1: milli(f[9])?,
                cd_ability2: milli(f[10])?,
            }
        }
        "P" => {
            need(10)?;
            Row::Other {
                t: t()?,
                slot: int(f[2]),
                pos: vec3(&f[3..6])?,
                vel: vec3(&f[6..9])?,
                health: milli(f[9])?,
            }
        }
        "D" => {
            need(5)?;
            Row::Dealt { t: t()?, ability: int(f[2]), damage: milli(f[3])?, victim_slot: int(f[4]) }
        }
        "T" => {
            need(4)?;
            Row::Took { t: t()?, damage: milli(f[2])?, attacker_slot: int(f[3]) }
        }
        "K" => {
            need(3)?;
            Row::Knockback { t: t()?, victim_slot: int(f[2]) }
        }
        "X" => {
            need(6)?;
            Row::Test {
                t: t()?,
                suite: int(f[2]),
                pitch_deg: milli(f[3])?,
                delay: milli(f[4])?,
                mode: int(f[5]),
            }
        }
        "Y" => {
            need(9)?;
            Row::PunchTest {
                t: t()?,
                suite: int(f[2]),
                charge: milli(f[3])?,
                pitch_deg: milli(f[4])?,
                jump_at: milli(f[5])?,
                slam_at: milli(f[6])?,
                turn_deg: milli(f[7])?,
                no_jump: boolean(f[8])?,
            }
        }
        "A" => {
            need(7)?;
            Row::Ammo {
                t: t()?,
                ammo: raw(f[2])? as f32,
                max_ammo: raw(f[3])? as f32,
                firing: boolean(f[4])?,
                reloading: boolean(f[5])?,
                fire_held: boolean(f[6])?,
            }
        }
        "Z" => {
            need(6)?;
            Row::HitTest { t: t()?, suite: int(f[2]), a: milli(f[3])?, b: milli(f[4])?, c: milli(f[5])? }
        }
        "E" => {
            need(3)?;
            Row::Pressed { t: t()?, code: int(f[2]) }
        }
        _ => return None,
    })
}

pub fn parse_log(text: &str) -> Vec<Row> {
    text.lines().filter_map(parse_line).collect()
}

/// Reads a log file, handling UTF-8 or UTF-16 text.
pub fn read_log_file(path: &std::path::Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> =
            bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return Ok(String::from_utf16_lossy(&units));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// One recording, from pressing Interact to pressing it again.
#[derive(Clone, Debug, Default)]
pub struct Take {
    pub id: i64,
    pub rows: Vec<Row>,
}

impl Take {
    pub fn frames(&self) -> impl Iterator<Item = (f64, Vec3, Vec3)> + '_ {
        self.rows.iter().filter_map(|r| match r {
            Row::Frame { t, pos, vel } => Some((*t, *pos, *vel)),
            _ => None,
        })
    }

    pub fn duration(&self) -> f64 {
        let ts = self.rows.iter().map(Row::t);
        let lo = ts.clone().fold(f64::INFINITY, f64::min);
        let hi = ts.fold(f64::NEG_INFINITY, f64::max);
        if lo.is_finite() { hi - lo } else { 0.0 }
    }
}

/// Splits rows into takes. If the log has no take markers, everything is one take.
pub fn split_takes(rows: Vec<Row>) -> Vec<Take> {
    let mut takes = Vec::new();
    let mut cur: Option<Take> = None;
    let mut saw_marker = false;
    for row in rows {
        match row {
            Row::Take { take, recording, .. } => {
                saw_marker = true;
                if let Some(t) = cur.take() {
                    takes.push(t);
                }
                if recording {
                    cur = Some(Take { id: take, rows: Vec::new() });
                }
            }
            r => {
                if let Some(t) = &mut cur {
                    t.rows.push(r);
                } else if !saw_marker {
                    cur = Some(Take { id: 0, rows: vec![r] });
                }
            }
        }
    }
    if let Some(t) = cur {
        takes.push(t);
    }
    takes.retain(|t| !t.rows.is_empty());
    takes
}
