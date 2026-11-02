//! Turns a logged take's inputs into an Overwatch Workshop script that replays them on a dummy
//! Doomfist (see `workshop/playback_template.txt`). The bot's run is logged in the same format,
//! so `compare` can line the two up.
//!
//! Caveat: the Workshop turns the bot with Set Facing, which Overwatch doesn't treat as mouse
//! turning, so punch bends from mouse movement won't happen in the playback.

use crate::log::{Row, Take};

const TEMPLATE: &str = include_str!("../workshop/playback_template.txt");

/// Rough Workshop element budget for the input arrays (the Workshop's total limit is 32768).
pub const MAX_TICKS: usize = 4000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackTick {
    /// Yaw relative to the first tick, degrees, unwrapped (Overwatch's horizontal angle).
    pub yaw_deg: f32,
    /// Overwatch vertical angle: positive looks down.
    pub pitch_deg: f32,
    /// Overwatch throttle: +X left, +Z forward.
    pub thr_x: f32,
    pub thr_z: f32,
    /// 1 punch, 2 slam, 4 block, 8 jump, 16 crouch.
    pub buttons: u8,
}

/// The inputs of a take, one per logged tick.
pub fn ticks_from_take(take: &Take) -> Vec<PlaybackTick> {
    let mut out = Vec::new();
    let mut last_raw: Option<f32> = None;
    let mut yaw = 0.0f32;
    for r in &take.rows {
        let Row::Input { facing, throttle_x, throttle_z, secondary, ability1, ability2, jump, crouch, .. } = *r else {
            continue;
        };
        let raw = facing.x.atan2(facing.z).to_degrees();
        if let Some(prev) = last_raw {
            yaw += (raw - prev + 540.0).rem_euclid(360.0) - 180.0;
        }
        last_raw = Some(raw);
        let buttons = secondary as u8 | (ability1 as u8) << 1 | (ability2 as u8) << 2 | (jump as u8) << 3 | (crouch as u8) << 4;
        out.push(PlaybackTick {
            yaw_deg: yaw,
            pitch_deg: -facing.normalize_or_zero().y.clamp(-1.0, 1.0).asin().to_degrees(),
            thr_x: throttle_x,
            thr_z: throttle_z,
            buttons,
        });
    }
    out
}

fn num(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.to_string() }
}

/// Keys and buttons as one small number: buttons + 32 x (throttle X + 1) + 128 x (throttle Z + 1).
pub fn key_state(t: &PlaybackTick) -> i64 {
    let axis = |v: f32| (v.round().clamp(-1.0, 1.0) as i64) + 1;
    t.buttons as i64 + 32 * axis(t.thr_x) + 128 * axis(t.thr_z)
}

/// What gets embedded in the Workshop script.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Compressed {
    /// Key changes: tick x 1000 + `key_state`, only when it changes (always at tick 0).
    pub events: Vec<i64>,
    /// Aim keyframes; the bot slides linearly between them.
    pub key_ticks: Vec<usize>,
    pub key_yaws: Vec<f32>,
    pub key_pitches: Vec<f32>,
    pub len: usize,
}

/// Aim keyframes are added until every tick is within this many degrees of the real aim.
pub const AIM_TOLERANCE_DEG: f32 = 0.25;

pub fn compress(ticks: &[PlaybackTick]) -> Compressed {
    let mut c = Compressed { len: ticks.len(), ..Default::default() };
    let mut last = None;
    for (i, t) in ticks.iter().enumerate() {
        let k = key_state(t);
        if last != Some(k) {
            c.events.push(i as i64 * 1000 + k);
            last = Some(k);
        }
    }
    if ticks.is_empty() {
        return c;
    }
    // Simplify the aim path (Ramer-Douglas-Peucker on yaw and pitch against time).
    let mut keep = vec![false; ticks.len()];
    keep[0] = true;
    keep[ticks.len() - 1] = true;
    let mut stack = vec![(0usize, ticks.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (mut worst, mut at) = (0.0f32, a);
        for i in a + 1..b {
            let f = (i - a) as f32 / (b - a) as f32;
            let dy = (ticks[a].yaw_deg + (ticks[b].yaw_deg - ticks[a].yaw_deg) * f - ticks[i].yaw_deg).abs();
            let dp = (ticks[a].pitch_deg + (ticks[b].pitch_deg - ticks[a].pitch_deg) * f - ticks[i].pitch_deg).abs();
            if dy.max(dp) > worst {
                worst = dy.max(dp);
                at = i;
            }
        }
        if worst > AIM_TOLERANCE_DEG {
            keep[at] = true;
            stack.push((a, at));
            stack.push((at, b));
        }
    }
    for (i, t) in ticks.iter().enumerate().filter(|(i, _)| keep[*i]) {
        c.key_ticks.push(i);
        c.key_yaws.push((t.yaw_deg * 100.0).round() / 100.0);
        c.key_pitches.push((t.pitch_deg * 100.0).round() / 100.0);
    }
    c
}

/// Decodes `Compressed` tick by tick the same way the Workshop rules do.
pub fn expand(c: &Compressed) -> Vec<PlaybackTick> {
    let (mut ev, mut ki, mut state) = (0usize, 0usize, 0i64);
    (0..c.len)
        .map(|tick| {
            if ev < c.events.len() && c.events[ev] / 1000 <= tick as i64 {
                state = c.events[ev] % 1000;
                ev += 1;
            }
            if ki + 1 < c.key_ticks.len() && c.key_ticks[ki + 1] <= tick {
                ki += 1;
            }
            let (yaw, pitch) = if ki + 1 < c.key_ticks.len() {
                let f = (tick - c.key_ticks[ki]) as f32 / (c.key_ticks[ki + 1] - c.key_ticks[ki]) as f32;
                (
                    c.key_yaws[ki] + (c.key_yaws[ki + 1] - c.key_yaws[ki]) * f,
                    c.key_pitches[ki] + (c.key_pitches[ki + 1] - c.key_pitches[ki]) * f,
                )
            } else {
                (c.key_yaws[ki], c.key_pitches[ki])
            };
            PlaybackTick {
                yaw_deg: yaw,
                pitch_deg: pitch,
                thr_x: ((state / 32) % 4 - 1) as f32,
                thr_z: (state / 128 - 1) as f32,
                buttons: (state % 32) as u8,
            }
        })
        .collect()
}

/// Writes `Global.<name> = Array(...)`, split into appended chunks to keep each line short.
fn array(name: &str, values: impl Iterator<Item = String>) -> String {
    let values: Vec<String> = values.collect();
    let mut out = String::new();
    for (i, chunk) in values.chunks(200).enumerate() {
        let list = chunk.join(", ");
        if i == 0 {
            out += &format!("\t\tGlobal.{name} = Array({list});\n");
        } else {
            out += &format!("\t\tModify Global Variable({name}, Append To Array, Array({list}));\n");
        }
    }
    if values.is_empty() {
        out += &format!("\t\tGlobal.{name} = Array(0);\n");
    }
    out
}

/// How many numbers the script embeds (a rough measure of its size).
pub fn embedded_numbers(c: &Compressed) -> usize {
    c.events.len() + 3 * c.key_ticks.len()
}

/// The full Workshop script that replays `ticks`.
pub fn workshop_script(ticks: &[PlaybackTick], label: &str) -> String {
    let ticks = &ticks[..ticks.len().min(MAX_TICKS)];
    let c = compress(ticks);
    let arrays = [
        array("Events", c.events.iter().map(|e| e.to_string())),
        array("KeyTicks", c.key_ticks.iter().map(|t| t.to_string())),
        array("KeyYaws", c.key_yaws.iter().map(|&v| num(v))),
        array("KeyPitches", c.key_pitches.iter().map(|&v| num(v))),
        format!("\t\tGlobal.TickCount = {};\n", c.len),
    ]
    .concat();
    let label: String = label.chars().filter(|c| c.is_ascii_alphanumeric() || " -_.".contains(*c)).collect();
    TEMPLATE
        .replace("\r\n", "\n")
        .replace("@ARRAYS@\n", &arrays)
        .replace("@LABEL@", &label)
        .replace("@TICKS@", &ticks.len().to_string())
}
