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
mod keys;
mod mannequin;

use anim::Model;
use keys::Act;
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

#[derive(Clone, Copy)]
enum FxKind {
    Shockwave,
    Burst,
    Number,
}

impl FxKind {
    fn life(&self) -> f32 {
        match self {
            FxKind::Shockwave => 0.5,
            FxKind::Burst => 0.2,
            FxKind::Number => 0.8,
        }
    }
}

struct Fx {
    kind: FxKind,
    pos: Vec3,
    dir: Vec3,
    t: f32,
    value: f32,
}

impl Fx {
    /// Slam: the hit cone spreading out at Overwatch's ~42 m/s; bursts: a fading puff.
    fn draw_3d(&self, near: f32, far: f32, half_angle_deg: f32) {
        let fade = 1.0 - self.t / self.kind.life();
        match self.kind {
            FxKind::Shockwave => {
                let r = (near + 42.0 * self.t).min(far);
                let side = vec3(self.dir.z, 0.0, -self.dir.x);
                let ground = self.pos + vec3(0.0, 0.05, 0.0);
                let col = Color::new(1.0, 0.75, 0.25, fade);
                let a = half_angle_deg.to_radians();
                let point = |dist: f32, ang: f32| ground + (self.dir * ang.cos() + side * ang.sin()) * dist;
                let steps = 16;
                for i in 0..steps {
                    let a0 = -a + 2.0 * a * i as f32 / steps as f32;
                    let a1 = -a + 2.0 * a * (i + 1) as f32 / steps as f32;
                    draw_line_3d(point(r, a0), point(r, a1), col);
                    draw_line_3d(point(r * 0.85, a0), point(r * 0.85, a1), col);
                }
                draw_line_3d(point(near, -a), point(r, -a), col);
                draw_line_3d(point(near, a), point(r, a), col);
            }
            FxKind::Burst => {
                draw_sphere_wires(self.pos, 0.3 + self.t * 3.0, None, Color::new(1.0, 0.9, 0.4, fade));
            }
            FxKind::Number => {}
        }
    }
}

/// The controls line at the bottom of the screen, from the current keybinds.
fn help_line(k: &keys::Keybinds) -> String {
    let l = |a| k.label(a);
    format!(
        "{}{}{}{} move  {} jump  {} Hand Cannon  {} punch (hold)  {} slam  {} block  {} melee  {} record  {} 3rd person  {} slow-mo  {} reset  {} dummies  {} {} sens  {} mouse   (keys: {SETTINGS_FILE})",
        l(Act::Forward), l(Act::Left), l(Act::Back), l(Act::Right), l(Act::Jump), l(Act::HandCannon), l(Act::RocketPunch),
        l(Act::SeismicSlam), l(Act::PowerBlock), l(Act::Melee), l(Act::Record), l(Act::ThirdPerson), l(Act::SlowMo),
        l(Act::Reset), l(Act::Dummies), l(Act::SensDown), l(Act::SensUp), l(Act::ReleaseMouse),
    )
}

/// Player settings, read from `bean_settings.txt` (`key = value` lines). Overwatch's units:
/// sensitivity turns 0.0066° per raw mouse count per point; FOV is horizontal degrees.
struct Settings {
    sensitivity: f32,
    fov_h: f32,
    keys: keys::Keybinds,
    /// Problems found while reading the file, shown on screen at start.
    warnings: Vec<String>,
}

const SETTINGS_FILE: &str = "bean_settings.txt";

impl Settings {
    fn load() -> Settings {
        let mut s = Settings { sensitivity: 5.0, fov_h: 103.0, keys: Default::default(), warnings: Vec::new() };
        let text = std::fs::read_to_string(SETTINGS_FILE).unwrap_or_default();
        let mut seen_binds = 0;
        for line in text.lines() {
            let Some((k, v)) = line.split('#').next().unwrap_or("").split_once('=') else { continue };
            let k = k.trim();
            match k {
                "sensitivity" | "fov" => match v.trim().parse::<f32>() {
                    Ok(v) if k == "sensitivity" => s.sensitivity = v,
                    Ok(v) => s.fov_h = v.clamp(60.0, 120.0),
                    Err(_) => s.warnings.push(format!("settings: `{k}` needs a number")),
                },
                _ => match s.keys.set(k, v) {
                    Some(bad) => {
                        seen_binds += 1;
                        for b in bad {
                            s.warnings.push(format!("settings: unknown key `{b}` for {k}"));
                        }
                    }
                    None => s.warnings.push(format!("settings: unknown setting `{k}`")),
                },
            }
        }
        // Write the file out (with every keybind listed) when it's new or missing some.
        if seen_binds < keys::ACTS.len() && s.warnings.is_empty() {
            s.save();
        }
        s
    }

    fn save(&self) {
        let _ = std::fs::write(
            SETTINGS_FILE,
            format!("# Overwatch units: sensitivity (default 5), fov = horizontal degrees (default 103)
sensitivity = {}
fov = {}
{}", self.sensitivity, self.fov_h, self.keys.to_lines()),
        );
    }

    /// Radians of turn per raw mouse count.
    fn rad_per_count(&self) -> f32 {
        (self.sensitivity * 0.0066).to_radians()
    }

    /// Vertical field of view for the current window shape.
    fn fovy(&self) -> f32 {
        let aspect = screen_width() / screen_height().max(1.0);
        2.0 * ((self.fov_h.to_radians() * 0.5).tan() / aspect).atan()
    }
}

/// Which animation state Doomfist is in, and how fast to play it.
fn anim_state(d: &doomfist::Doomfist) -> (&'static str, f32) {
    match &d.action {
        Action::Block(_) => ("block", 1.0),
        Action::PunchCharge { .. } => ("punch_charge", 1.0),
        Action::Punch(_) => ("punch", 1.0),
        Action::Slam(s) if matches!(s.phase, SlamPhase::Air { .. }) => ("slam_leap", 1.0),
        Action::Slam(_) => ("slam_land", 1.0),
        Action::Idle if d.melee_anim < 0.35 => ("melee", 1.0),
        Action::Idle if !d.grounded => ("jump", 1.0),
        Action::Idle => {
            let speed = d.vel.horizontal().length();
            if speed > 0.3 { ("walk", speed / 5.5) } else { ("idle", 1.0) }
        }
    }
}

fn looping(state: &str) -> bool {
    matches!(state, "idle" | "walk" | "fp_idle")
}

fn draw_model(model: &Model, player: &anim::Player, feet: Vec3, facing: Vec3, tint: Color) {
    let pose = player.pose(model, looping);
    let joints = model.joint_matrices(&pose);
    let yaw = facing.x.atan2(facing.z);
    let world = Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), feet);
    for mesh in model.build_meshes(&joints, world, tint, None) {
        draw_mesh(&mesh);
    }
}

/// Where the arms' anchor bone sits relative to the eye in first person (x right is -X,
/// y up, z forward), m.
const VIEWMODEL_OFFSET: Vec3 = vec3(0.0, -0.46, 0.0);

/// First-person arms: only the arm bones, hung from the anchor bone under the camera so they
/// follow the aim up and down.
fn draw_viewmodel(model: &Model, player: &anim::Player, eye: Vec3, yaw: f32, pitch: f32, tint: Color) {
    let pose = player.pose(model, looping);
    let globals = model.globals(&pose);
    let anchor = model.anchor_bone();
    let keep = model.arm_bones();
    let joints = model.joint_matrices(&pose);
    let cam = Mat4::from_rotation_translation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch), eye);
    let world = cam * Mat4::from_translation(VIEWMODEL_OFFSET) * (model.fix * globals[anchor]).inverse();
    for mesh in model.build_meshes(&joints, world, tint, Some(&keep)) {
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
        Event::Knockback { .. } | Event::Shot => return None,
    })
}

#[macroquad::main(conf)]
async fn main() {
    let mut sim = new_sim();
    let model = mannequin::build();
    let dummy_model = mannequin::build();
    let mut player = anim::Player::new();
    let mut dummy_player = anim::Player::new();
    // Screenshot mode for checking models without playing: BEAN_SHOT=out.png [BEAN_POSE=state]
    // renders from in front of Doomfist, saves the frame and exits.
    let shot = std::env::var("BEAN_SHOT").ok();
    let shot_pose = std::env::var("BEAN_POSE").ok();
    // BEAN_VIEW=fp keeps the first-person view in screenshots.
    let shot_fp = std::env::var("BEAN_VIEW").is_ok_and(|v| v == "fp");
    let mut frame_no = 0u32;
    let dt = sim.doom.tun.dt();
    let (mut yaw, mut pitch) = (0.0f32, 0.0f32);
    let mut settings = Settings::load();
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
    let mut melee_latch = false;
    let mut fire_latch = false;
    let mut since_shot = 99.0f32;
    // Hit effects: slam shockwaves, impact bursts, hit flashes and floating damage numbers.
    let mut fx: Vec<Fx> = Vec::new();
    let mut flash: std::collections::HashMap<u32, f32> = Default::default();
    let mut prev_state = String::new();
    for w in std::mem::take(&mut settings.warnings) {
        feed.push_front((0.0, w));
    }

    loop {
        // ---- input ----
        settings.keys.frame();
        let kb = settings.keys.clone();
        if kb.pressed(Act::ReleaseMouse) {
            grabbed = !grabbed;
            set_cursor_grab(grabbed);
            show_mouse(!grabbed);
        }
        let m = mouse_position();
        let (dx, dy) = (m.0 - last_mouse.0, m.1 - last_mouse.1);
        last_mouse = m;
        if grabbed {
            let sens = settings.rad_per_count();
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
        // [ and ] change sensitivity in Overwatch steps of 0.25 and save it.
        for (key, step) in [(Act::SensDown, -0.25), (Act::SensUp, 0.25)] {
            if kb.pressed(key) {
                settings.sensitivity = (settings.sensitivity + step).clamp(0.25, 100.0);
                settings.save();
                feed.push_front((sim.doom.time, format!("sensitivity {:.2}", settings.sensitivity)));
            }
        }
        if kb.pressed(Act::ThirdPerson) {
            third_person = !third_person;
        }
        if kb.pressed(Act::SlowMo) {
            speed_i = (speed_i + 1) % speeds.len();
        }
        if kb.pressed(Act::Reset) {
            sim = new_sim();
            trail.clear();
            top_speed = 0.0;
        }
        if kb.pressed(Act::Record) {
            if recording.is_some() {
                let mut rows = recording.take().unwrap();
                rows.push(format!("R;{};1;False", tick * 16));
                rec_msg = save_recording(&rows);
            } else {
                recording = Some(vec![format!("R;{};1;True", tick * 16)]);
                rec_msg = "recording... (F to stop)".into();
            }
        }
        if kb.pressed(Act::Dummies) {
            spawn_dummies(&mut sim);
        }
        jump_latch |= kb.pressed(Act::Jump);
        slam_latch |= kb.pressed(Act::SeismicSlam);
        block_latch |= kb.pressed(Act::PowerBlock);
        punch_latch |= kb.pressed(Act::RocketPunch);
        melee_latch |= kb.pressed(Act::Melee);
        fire_latch |= kb.pressed(Act::HandCannon);

        let facing = DVec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos());

        // ---- fixed-rate sim ----
        acc += get_frame_time().min(0.1) * speeds[speed_i];
        let mut steps = 0;
        while acc >= dt && steps < 10 {
            let input = Input {
                facing,
                move_forward: kb.axis(Act::Forward, Act::Back),
                move_right: kb.axis(Act::Right, Act::Left),
                jump: kb.down(Act::Jump) || jump_latch,
                crouch: kb.down(Act::Crouch),
                secondary: kb.down(Act::RocketPunch) || punch_latch,
                ability1: kb.down(Act::SeismicSlam) || slam_latch,
                ability2: kb.down(Act::PowerBlock) || block_latch,
                melee: kb.down(Act::Melee) || melee_latch,
                primary: kb.down(Act::HandCannon) || fire_latch,
            };
            (jump_latch, slam_latch, block_latch, punch_latch) = (false, false, false, false);
            melee_latch = false;
            fire_latch = false;
            tick += 1;
            let events = sim.step(&input);
            if let Some(rows) = &mut recording {
                rows.extend(log_rows(tick * 16, &sim.doom, &input));
            }
            for e in events {
                if matches!(e, Event::Shot) {
                    since_shot = 0.0;
                }
                if let Event::DamageDealt { target, amount, .. } = e {
                    if let Some(t) = sim.world.target(target) {
                        fx.push(Fx { kind: FxKind::Burst, pos: v(t.center()), dir: Vec3::ZERO, t: 0.0, value: 0.0 });
                        fx.push(Fx { kind: FxKind::Number, pos: v(t.pos) + vec3(0.0, 2.4, 0.0), dir: Vec3::ZERO, t: 0.0, value: amount });
                        flash.insert(target, 0.25);
                    }
                }
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
        since_shot += frame_dt;
        let (mut state, rate) = anim_state(&sim.doom);
        if matches!(state, "idle" | "walk" | "jump") {
            if since_shot < 0.25 {
                state = "fire";
            } else if sim.doom.gun_refilling() {
                state = "reload";
            }
        }
        if state == "slam_land" && prev_state == "slam_leap" {
            let mut dir = sim.doom.vel.horizontal().normalize_or_zero();
            if dir == doomfist::Vec3::ZERO {
                dir = facing.horizontal().normalize_or_zero();
            }
            fx.push(Fx { kind: FxKind::Shockwave, pos: v(sim.doom.pos), dir: v(dir), t: 0.0, value: 0.0 });
        }
        prev_state = state.to_string();
        for f in fx.iter_mut() {
            f.t += frame_dt;
        }
        fx.retain(|f| f.t < f.kind.life());
        flash.retain(|_, t| {
            *t -= frame_dt;
            *t > 0.0
        });
        let state: &str = shot_pose.as_deref().unwrap_or(state);
        // First person: idle/walk/jump use the model's first-person ready pose if it has one.
        let first_person = !third_person && (shot.is_none() || shot_fp);
        let state = if first_person && matches!(state, "idle" | "walk" | "jump") && model.clip("fp_idle").is_some() {
            "fp_idle"
        } else {
            state
        };
        player.update(state, frame_dt, rate);
        dummy_player.update("idle", frame_dt, 1.0);

        // ---- 3D ----
        clear_background(Color::from_rgba(150, 190, 230, 255));
        let d = &sim.doom;
        let look = v(facing);
        let eye = if shot.is_some() && !shot_fp {
            v(d.pos) + vec3(look.x, 0.0, look.z).normalize_or_zero() * 3.6 + vec3(0.0, 1.3, 0.0)
        } else if third_person {
            v(d.pos) + vec3(0.0, 2.4, 0.0) - vec3(look.x, 0.0, look.z).normalize_or_zero() * 6.0
        } else {
            v(d.pos) + vec3(0.0, 1.6, 0.0)
        };
        let target = if shot.is_some() && !shot_fp { v(d.pos) + vec3(0.0, 1.1, 0.0) } else { eye + look };
        let camera = Camera3D {
            position: eye,
            target,
            up: vec3(0.0, 1.0, 0.0),
            fovy: settings.fovy(),
            ..Default::default()
        };
        set_camera(&camera);
        let view_proj = camera.matrix();

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
            if flash.contains_key(&t.id) {
                tint = Color::new(1.6, 1.5, 1.0, 1.0);
            }
            draw_model(&dummy_model, &dummy_player, v(t.pos), v(to_doom), tint);
            let frac = (t.health / t.max_health).clamp(0.0, 1.0);
            draw_cube(v(t.pos) + vec3(0.0, 2.2, 0.0), vec3(1.2 * frac, 0.12, 0.12), None, GREEN);
        }
        for w in trail.iter().zip(trail.iter().skip(1)) {
            draw_line_3d(*w.0, *w.1, Color::from_rgba(255, 220, 60, 200));
        }
        for p in &sim.world.projectiles {
            draw_line_3d(v(p.pos), v(p.pos - p.vel.normalize_or_zero() * 0.6), Color::new(1.0, 0.95, 0.6, 1.0));
        }
        let slam = sim.doom.tun.slam;
        for f in &fx {
            f.draw_3d(slam.hit_ahead_min, slam.hit_ahead_max, slam.hit_cone_half_angle_deg);
        }
        let tint = if d.empowered { Color::new(1.0, 0.75, 0.45, 1.0) } else { WHITE };
        if first_person {
            draw_viewmodel(&model, &player, eye, yaw, pitch, tint);
        } else {
            draw_model(&model, &player, v(d.pos), look, tint);
        }
        if matches!(d.action, Action::Block(_)) {
            let h = vec3(look.x, 0.0, look.z).normalize_or_zero();
            draw_cube(v(d.pos) + vec3(0.0, 1.0, 0.0) + h * 0.8, vec3(1.2, 1.6, 1.2) * 0.6, None, Color::from_rgba(80, 160, 255, 120));
        }

        // ---- HUD ----
        set_default_camera();
        let (w, h) = (screen_width(), screen_height());
        for f in fx.iter().filter(|f| matches!(f.kind, FxKind::Number)) {
            let p = f.pos + vec3(0.0, f.t * 1.2, 0.0);
            let clip = view_proj * p.extend(1.0);
            if clip.w > 0.1 {
                let (sx, sy) = ((clip.x / clip.w + 1.0) * 0.5 * w, (1.0 - clip.y / clip.w) * 0.5 * h);
                let alpha = (1.0 - f.t / f.kind.life()).clamp(0.0, 1.0);
                let text = format!("{:.0}", f.value);
                draw_text(&text, sx - 8.0, sy + 2.0, 34.0, Color::new(0.0, 0.0, 0.0, alpha * 0.6));
                draw_text(&text, sx - 10.0, sy, 34.0, Color::new(1.0, 0.85, 0.2, alpha));
            }
        }
        draw_line(w / 2.0 - 8.0, h / 2.0, w / 2.0 + 8.0, h / 2.0, 2.0, WHITE);
        draw_line(w / 2.0, h / 2.0 - 8.0, w / 2.0, h / 2.0 + 8.0, 2.0, WHITE);

        let flat = d.vel.horizontal().length();
        let c = &d.cooldowns;
        let hud = [
            format!("speed {flat:6.2} m/s  vertical {:+6.2}  top {top_speed:.1}", d.vel.y),
            format!("{}  {}", action_name(&d.action), if d.grounded { "grounded" } else { "airborne" }),
            format!("HP {:.0} + {:.0} temp   {}", d.health, d.overhealth, if d.empowered { "EMPOWERED PUNCH" } else { "" }),
            format!(
                "cooldowns  punch {:.1}  slam {:.1}  block {:.1}  melee {:.1}   Hand Cannon {}/{}",
                c.punch, c.slam, c.block, c.melee, d.gun.ammo, d.tun.gun.clip
            ),
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
        draw_text(&format!("anim: {}", player.state), 16.0, 30.0 + 6.0 * 24.0, 18.0, Color::from_rgba(30, 30, 30, 255));
        for (i, (t, text)) in feed.iter().enumerate() {
            draw_text(&format!("{t:7.2}  {text}"), 16.0, h - 220.0 + i as f32 * 22.0, 20.0, Color::from_rgba(30, 30, 30, 255));
        }
        draw_text(
            &help_line(&settings.keys),
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
