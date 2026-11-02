//! Bean Doomfist: a playable test map driven by the Rust sim at Overwatch's 62.5 Hz tick.
//!
//!   cargo run --release --bin bean
//!
//! Controls: mouse look, WASD move, Space jump, Ctrl crouch, Right mouse = Rocket Punch (hold to
//! charge), Shift = Seismic Slam, E = Power Block. V third person, T slow motion, R reset,
//! H respawn dummies, Esc free/capture mouse.
//!
//! F starts/stops a recording (like Interact in the Workshop logger). Each recording is saved
//! to logs/bean-<time>.txt in the same format as the Overwatch logger, plus
//! logs/bean-<time>-ow.txt: a Workshop script that replays your inputs on a dummy Doomfist in
//! Overwatch. Compare the two with `cargo run --bin compare -- <bean log> <overwatch log>`.

mod anim;
mod gltf_load;
mod mannequin;

use anim::{AnimMap, Model};
use doomfist::abilities::slam::SlamPhase;
use doomfist::doomfist::Action;
use doomfist::events::Event;
use doomfist::world::{Aabb, Target};
use doomfist::log::{parse_log, split_takes};
use doomfist::playback::{ticks_from_take, workshop_script, MAX_TICKS};
use doomfist::record::log_rows;
use doomfist::{Input, Sim, Tunables, Vec3 as DVec3};
use macroquad::prelude::*;
use std::collections::VecDeque;

fn conf() -> Conf {
    Conf {
        window_title: "Doomfist Bean".to_owned(),
        window_width: 1280,
        window_height: 720,
        sample_count: 4,
        ..Default::default()
    }
}

fn v(d: DVec3) -> Vec3 {
    vec3(d.x, d.y, d.z)
}

const DUMMY_SPOTS: [(f32, f32); 6] = [(0.0, 12.0), (4.0, 18.0), (-6.0, 22.0), (10.0, 6.0), (-12.0, 8.0), (0.0, 26.0)];

fn spawn_dummies(sim: &mut Sim) {
    sim.world.targets = DUMMY_SPOTS
        .iter()
        .enumerate()
        .map(|(i, &(x, z))| Target::new(i as u32 + 1, DVec3::new(x, 0.0, z)))
        .collect();
}

fn new_sim() -> Sim {
    let mut sim = Sim::new(Tunables::default(), DVec3::ZERO);
    let wall = |x0: f32, z0: f32, x1: f32, z1: f32| Aabb::new(DVec3::new(x0, 0.0, z0), DVec3::new(x1, 5.0, z1));
    sim.world.walls = vec![
        // Arena edges.
        wall(-40.0, 40.0, 40.0, 41.0),
        wall(-40.0, -41.0, 40.0, -40.0),
        wall(40.0, -40.0, 41.0, 40.0),
        wall(-41.0, -40.0, -40.0, 40.0),
        // A wall right behind the first dummies, for wall slams.
        wall(-8.0, 29.0, 8.0, 30.0),
        // Pillars.
        wall(14.0, 14.0, 16.0, 16.0),
        wall(-18.0, 16.0, -16.0, 18.0),
        wall(20.0, -10.0, 22.0, -8.0),
        // Corner.
        wall(-30.0, -20.0, -29.0, -5.0),
        wall(-30.0, -21.0, -15.0, -20.0),
    ];
    spawn_dummies(&mut sim);
    sim
}

/// Writes the recording and its Overwatch playback script. Returns a message for the HUD.
fn save_recording(rows: &[String]) -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let name = format!("bean-{secs}");
    let _ = std::fs::create_dir_all("logs");
    let log_path = format!("logs/{name}.txt");
    let text = rows.join("\n");
    if let Err(e) = std::fs::write(&log_path, &text) {
        return format!("couldn't save {log_path}: {e}");
    }
    let Some(take) = split_takes(parse_log(&text)).into_iter().last() else {
        return format!("saved {log_path} (empty)");
    };
    let ticks = ticks_from_take(&take);
    let ow_path = format!("logs/{name}-ow.txt");
    let _ = std::fs::write(&ow_path, workshop_script(&ticks, &name));
    let cut = if ticks.len() > MAX_TICKS { format!(" (Overwatch script cut to {:.0} s)", MAX_TICKS as f32 / 62.5) } else { String::new() };
    format!("saved {log_path} + {ow_path}{cut}")
}

/// Doomfist's model: the user's own exported file from `assets/doomfist/` if there is one
/// (with `assets/doomfist/animations.txt` naming its clips), otherwise the built-in mannequin.
fn load_character() -> (Model, AnimMap, String) {
    let dir = std::path::Path::new("assets/doomfist");
    let map = std::fs::read_to_string(dir.join("animations.txt")).map(|t| AnimMap::parse(&t)).unwrap_or_default();
    let file = std::fs::read_dir(dir).ok().and_then(|it| {
        it.filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("glb") || x.eq_ignore_ascii_case("gltf")))
    });
    if let Some(path) = file {
        match gltf_load::load(&path) {
            Ok(mut m) => {
                let s = map.scale.unwrap_or(1.0);
                let yaw = map.yaw_offset_deg.unwrap_or(0.0).to_radians();
                m.fix = Mat4::from_rotation_y(yaw) * Mat4::from_scale(Vec3::splat(s)) * m.fix;
                let msg = format!("model: {} ({} bones, {} clips)", path.display(), m.bones.len(), m.clips.len());
                return (m, map, msg);
            }
            Err(e) => return (mannequin::build(), AnimMap::default(), format!("model: built-in mannequin ({e})")),
        }
    }
    (mannequin::build(), AnimMap::default(), "model: built-in mannequin (no file in assets/doomfist)".into())
}

/// Which animation state Doomfist is in, and how fast to play it.
fn anim_state(d: &doomfist::Doomfist) -> (&'static str, f32) {
    match &d.action {
        Action::Block(_) => ("block", 1.0),
        Action::PunchCharge { .. } => ("punch_charge", 1.0),
        Action::Punch(_) => ("punch", 1.0),
        Action::Slam(s) if matches!(s.phase, SlamPhase::Air { .. }) => ("slam_leap", 1.0),
        Action::Slam(_) => ("slam_land", 1.0),
        Action::Idle if !d.grounded => ("jump", 1.0),
        Action::Idle => {
            let speed = d.vel.horizontal().length();
            if speed > 0.3 { ("walk", speed / 5.5) } else { ("idle", 1.0) }
        }
    }
}

fn looping(state: &str) -> bool {
    matches!(state, "idle" | "walk")
}

fn draw_model(model: &Model, map: &AnimMap, player: &anim::Player, feet: Vec3, facing: Vec3, tint: Color) {
    let pose = player.pose(model, map, looping);
    let joints = model.joint_matrices(&pose);
    let yaw = facing.x.atan2(facing.z);
    let world = Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), feet);
    for mesh in model.build_meshes(&joints, world, tint) {
        draw_mesh(&mesh);
    }
}

#[allow(dead_code)]
fn bean(feet: Vec3, color: Color) {
    let r = 0.4;
    draw_cylinder(feet + vec3(0.0, r, 0.0), r, r, 1.8 - 2.0 * r, None, color);
    draw_sphere(feet + vec3(0.0, r, 0.0), r, None, color);
    draw_sphere(feet + vec3(0.0, 1.8 - r, 0.0), r, None, color);
}

fn action_name(a: &Action) -> String {
    match a {
        Action::Idle => "idle".into(),
        Action::PunchCharge { t, held } => {
            format!("charging punch {:.2}s{}", t, if held.is_some() { " (released, waiting)" } else { "" })
        }
        Action::Punch(s) => format!("PUNCH tick {}/{}{}", s.ticks_done, s.full_ticks + 1, if s.empowered { " EMPOWERED" } else { "" }),
        Action::Slam(s) => format!("SLAM {:?} {:.2}s", s.phase, s.t),
        Action::Block(s) => format!("POWER BLOCK {:.2}s, blocked {:.0}", s.t, s.blocked),
    }
}

fn event_text(e: &Event) -> Option<String> {
    Some(match e {
        Event::ActionStarted(k) => format!("start {k:?}"),
        Event::ActionEnded(k, r) => format!("end {k:?} ({r:?})"),
        Event::Jumped => "jump".into(),
        Event::DamageDealt { target, amount, kind, .. } => format!("hit dummy {target}: {amount:.0} {kind:?}"),
        Event::OverhealthGained { amount, total } => format!("+{amount:.0} temp HP ({total:.0})"),
        Event::Empowered => "EMPOWERED".into(),
        Event::EmpowerConsumed => "empower used".into(),
        Event::EmpowerExpired => "empower expired".into(),
        Event::DamageTaken { taken, blocked, .. } => format!("took {taken:.0} (blocked {blocked:.0})"),
        Event::Knockback { .. } => return None,
    })
}

#[macroquad::main(conf)]
async fn main() {
    let mut sim = new_sim();
    let (model, anim_map, model_msg) = load_character();
    let dummy_model = mannequin::build();
    let mut player = anim::Player::new();
    let mut dummy_player = anim::Player::new();
    // Screenshot mode for checking models without playing: BEAN_SHOT=out.png [BEAN_POSE=state]
    // renders from in front of Doomfist, saves the frame and exits.
    let shot = std::env::var("BEAN_SHOT").ok();
    let shot_pose = std::env::var("BEAN_POSE").ok();
    let mut frame_no = 0u32;
    let dt = sim.doom.tun.dt();
    let (mut yaw, mut pitch) = (0.0f32, 0.0f32);
    let mut grabbed = true;
    set_cursor_grab(true);
    show_mouse(false);
    let mut last_mouse = mouse_position();
    let mut acc = 0.0f32;
    let mut third_person = false;
    let speeds = [1.0f32, 0.25, 0.1];
    let mut speed_i = 0;
    let mut feed: VecDeque<(f32, String)> = VecDeque::new();
    // Recording: logger rows while on; `tick` counts sim ticks for the 16 ms timestamps.
    let mut recording: Option<Vec<String>> = None;
    let mut rec_msg = String::from("F: record (open ground only: the Overwatch map has no walls here)");
    let mut tick: u64 = 0;
    let mut trail: VecDeque<Vec3> = VecDeque::new();
    let mut top_speed = 0.0f32;
    // Presses shorter than a tick still count: latch them until the next sim step.
    let (mut jump_latch, mut slam_latch, mut block_latch, mut punch_latch) = (false, false, false, false);

    loop {
        // ---- input ----
        if is_key_pressed(KeyCode::Escape) {
            grabbed = !grabbed;
            set_cursor_grab(grabbed);
            show_mouse(!grabbed);
        }
        let m = mouse_position();
        let (dx, dy) = (m.0 - last_mouse.0, m.1 - last_mouse.1);
        last_mouse = m;
        if grabbed {
            let sens = 0.0025;
            yaw -= dx * sens;
            // While dashing the camera can only look within the cone around the dash.
            if let Action::Punch(s) = &sim.doom.action {
                let dash_yaw = s.dir.x.atan2(s.dir.z);
                let cone = sim.doom.tun.punch.camera_cone_deg.to_radians();
                let off = (yaw - dash_yaw + 3.0 * std::f32::consts::PI).rem_euclid(2.0 * std::f32::consts::PI) - std::f32::consts::PI;
                yaw = dash_yaw + off.clamp(-cone, cone);
            }
            pitch = (pitch - dy * sens).clamp(-1.55, 1.55);
        }
        if is_key_pressed(KeyCode::V) {
            third_person = !third_person;
        }
        if is_key_pressed(KeyCode::T) {
            speed_i = (speed_i + 1) % speeds.len();
        }
        if is_key_pressed(KeyCode::R) {
            sim = new_sim();
            trail.clear();
            top_speed = 0.0;
        }
        if is_key_pressed(KeyCode::F) {
            if recording.is_some() {
                let mut rows = recording.take().unwrap();
                rows.push(format!("R;{};1;False", tick * 16));
                rec_msg = save_recording(&rows);
            } else {
                recording = Some(vec![format!("R;{};1;True", tick * 16)]);
                rec_msg = "recording... (F to stop)".into();
            }
        }
        if is_key_pressed(KeyCode::H) {
            spawn_dummies(&mut sim);
        }
        jump_latch |= is_key_pressed(KeyCode::Space);
        slam_latch |= is_key_pressed(KeyCode::LeftShift);
        block_latch |= is_key_pressed(KeyCode::E);
        punch_latch |= is_mouse_button_pressed(MouseButton::Right);

        let facing = DVec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos());
        let axis = |pos: KeyCode, neg: KeyCode| (is_key_down(pos) as i32 - is_key_down(neg) as i32) as f32;

        // ---- fixed-rate sim ----
        acc += get_frame_time().min(0.1) * speeds[speed_i];
        let mut steps = 0;
        while acc >= dt && steps < 10 {
            let input = Input {
                facing,
                move_forward: axis(KeyCode::W, KeyCode::S),
                move_right: axis(KeyCode::D, KeyCode::A),
                jump: is_key_down(KeyCode::Space) || jump_latch,
                crouch: is_key_down(KeyCode::LeftControl),
                secondary: is_mouse_button_down(MouseButton::Right) || punch_latch,
                ability1: is_key_down(KeyCode::LeftShift) || slam_latch,
                ability2: is_key_down(KeyCode::E) || block_latch,
            };
            (jump_latch, slam_latch, block_latch, punch_latch) = (false, false, false, false);
            tick += 1;
            let events = sim.step(&input);
            if let Some(rows) = &mut recording {
                rows.extend(log_rows(tick * 16, &sim.doom, &input));
            }
            for e in events {
                if let Some(t) = event_text(&e) {
                    feed.push_front((sim.doom.time, t));
                }
            }
            feed.truncate(8);
            trail.push_back(v(sim.doom.pos) + vec3(0.0, 0.05, 0.0));
            if trail.len() > 250 {
                trail.pop_front();
            }
            top_speed = top_speed.max(sim.doom.vel.horizontal().length());
            acc -= dt;
            steps += 1;
        }

        // ---- animation ----
        let frame_dt = get_frame_time().min(0.1) * speeds[speed_i];
        let (state, rate) = anim_state(&sim.doom);
        let state: &str = shot_pose.as_deref().unwrap_or(state);
        player.update(state, frame_dt, rate);
        dummy_player.update("idle", frame_dt, 1.0);

        // ---- 3D ----
        clear_background(Color::from_rgba(150, 190, 230, 255));
        let d = &sim.doom;
        let look = v(facing);
        let eye = if shot.is_some() {
            v(d.pos) + vec3(look.x, 0.0, look.z).normalize_or_zero() * 3.6 + vec3(0.0, 1.3, 0.0)
        } else if third_person {
            v(d.pos) + vec3(0.0, 2.4, 0.0) - vec3(look.x, 0.0, look.z).normalize_or_zero() * 6.0
        } else {
            v(d.pos) + vec3(0.0, 1.6, 0.0)
        };
        let target = if shot.is_some() { v(d.pos) + vec3(0.0, 1.1, 0.0) } else { eye + look };
        set_camera(&Camera3D {
            position: eye,
            target,
            up: vec3(0.0, 1.0, 0.0),
            fovy: 70f32.to_radians(),
            ..Default::default()
        });

        draw_cube(vec3(0.0, -0.05, 0.0), vec3(82.0, 0.1, 82.0), None, Color::from_rgba(110, 120, 110, 255));
        draw_grid(40, 2.0, DARKGRAY, Color::from_rgba(95, 105, 95, 255));
        for w in &sim.world.walls {
            let (c, s) = (v((w.min + w.max) * 0.5), v(w.max - w.min));
            draw_cube(c, s, None, Color::from_rgba(200, 190, 170, 255));
            draw_cube_wires(c, s, Color::from_rgba(90, 80, 70, 255));
        }
        for t in &sim.world.targets {
            let col = if t.health <= 0.0 {
                DARKGRAY
            } else if t.punched.is_some() {
                ORANGE
            } else {
                Color::from_rgba(200, 60, 60, 255)
            };
            let to_doom = (sim.doom.pos - t.pos).horizontal().normalize_or_zero();
            let mut tint = col;
            tint.r = (tint.r + 0.5).min(1.0);
            tint.g = (tint.g + 0.5).min(1.0);
            tint.b = (tint.b + 0.5).min(1.0);
            draw_model(&dummy_model, &AnimMap::default(), &dummy_player, v(t.pos), v(to_doom), tint);
            let frac = (t.health / t.max_health).clamp(0.0, 1.0);
            draw_cube(v(t.pos) + vec3(0.0, 2.2, 0.0), vec3(1.2 * frac, 0.12, 0.12), None, GREEN);
        }
        for w in trail.iter().zip(trail.iter().skip(1)) {
            draw_line_3d(*w.0, *w.1, Color::from_rgba(255, 220, 60, 200));
        }
        if third_person || shot.is_some() {
            let tint = if d.empowered { Color::new(1.0, 0.75, 0.45, 1.0) } else { WHITE };
            draw_model(&model, &anim_map, &player, v(d.pos), look, tint);
        }
        if matches!(d.action, Action::Block(_)) {
            let h = vec3(look.x, 0.0, look.z).normalize_or_zero();
            draw_cube(v(d.pos) + vec3(0.0, 1.0, 0.0) + h * 0.8, vec3(1.2, 1.6, 1.2) * 0.6, None, Color::from_rgba(80, 160, 255, 120));
        }

        // ---- HUD ----
        set_default_camera();
        let (w, h) = (screen_width(), screen_height());
        draw_line(w / 2.0 - 8.0, h / 2.0, w / 2.0 + 8.0, h / 2.0, 2.0, WHITE);
        draw_line(w / 2.0, h / 2.0 - 8.0, w / 2.0, h / 2.0 + 8.0, 2.0, WHITE);

        let flat = d.vel.horizontal().length();
        let c = &d.cooldowns;
        let hud = [
            format!("speed {flat:6.2} m/s  vertical {:+6.2}  top {top_speed:.1}", d.vel.y),
            format!("{}  {}", action_name(&d.action), if d.grounded { "grounded" } else { "airborne" }),
            format!("HP {:.0} + {:.0} temp   {}", d.health, d.overhealth, if d.empowered { "EMPOWERED PUNCH" } else { "" }),
            format!("cooldowns  punch {:.1}  slam {:.1}  block {:.1}", c.punch, c.slam, c.block),
            format!("pos ({:.1}, {:.1}, {:.1})  sim speed x{}", d.pos.x, d.pos.y, d.pos.z, speeds[speed_i]),
        ];
        for (i, line) in hud.iter().enumerate() {
            draw_text(line, 16.0, 30.0 + i as f32 * 24.0, 24.0, BLACK);
        }
        if let Action::PunchCharge { t, .. } = d.action {
            let p = d.tun.punch;
            let frac = ((t - p.charge_start) / (p.charge_full - p.charge_start)).clamp(0.0, 1.0);
            draw_rectangle(w / 2.0 - 100.0, h / 2.0 + 40.0, 200.0, 10.0, Color::from_rgba(0, 0, 0, 120));
            draw_rectangle(w / 2.0 - 100.0, h / 2.0 + 40.0, 200.0 * frac, 10.0, ORANGE);
        }
        let rec_color = if recording.is_some() { RED } else { Color::from_rgba(30, 30, 30, 255) };
        draw_text(&rec_msg, 16.0, 30.0 + 5.0 * 24.0, 22.0, rec_color);
        draw_text(&format!("{model_msg}  anim: {}", player.state), 16.0, 30.0 + 6.0 * 24.0, 18.0, Color::from_rgba(30, 30, 30, 255));
        for (i, (t, text)) in feed.iter().enumerate() {
            draw_text(&format!("{t:7.2}  {text}"), 16.0, h - 220.0 + i as f32 * 22.0, 20.0, Color::from_rgba(30, 30, 30, 255));
        }
        draw_text(
            "WASD move  Space jump  RMB punch (hold)  Shift slam  E block  F record  V 3rd person  T slow-mo  R reset  H dummies  Esc mouse",
            16.0,
            h - 16.0,
            18.0,
            Color::from_rgba(30, 30, 30, 255),
        );

        if let Some(path) = &shot {
            frame_no += 1;
            if frame_no == 40 {
                get_screen_data().export_png(path);
                break;
            }
        }
        next_frame().await
    }
}
