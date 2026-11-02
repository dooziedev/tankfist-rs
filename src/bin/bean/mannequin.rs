//! Built-in stand-in model: a test-dummy mannequin (white segmented body, round head with eyes,
//! blue right limbs, red left limbs) with simple keyframed clips for every game state. Used when
//! no model has been put in `assets/doomfist/`. Entirely generated here, so it's safe to publish.
//!
//! Axes: +Y up, +Z forward, so the character's right side is -X.

use crate::anim::{Bone, Channel, Clip, Keys, Model, SkinMesh, Xf};
use macroquad::prelude::*;

const WHITE: Color = Color::new(0.88, 0.88, 0.86, 1.0);
const SEAM: Color = Color::new(0.74, 0.74, 0.72, 1.0);
const BLUE: Color = Color::new(0.22, 0.40, 0.95, 1.0);
const RED: Color = Color::new(0.92, 0.25, 0.20, 1.0);
const DARK: Color = Color::new(0.08, 0.08, 0.10, 1.0);

// Bone indices.
const ROOT: usize = 0;
const HIPS: usize = 1;
const SPINE: usize = 2;
const CHEST: usize = 3;
const NECK: usize = 4;
const HEAD: usize = 5;
const UARM_R: usize = 6;
const FARM_R: usize = 7;
const HAND_R: usize = 8;
const UARM_L: usize = 9;
const FARM_L: usize = 10;
const HAND_L: usize = 11;
const THIGH_R: usize = 12;
const SHIN_R: usize = 13;
const FOOT_R: usize = 14;
const THIGH_L: usize = 15;
const SHIN_L: usize = 16;
const FOOT_L: usize = 17;

fn skeleton() -> Vec<Bone> {
    let defs: [(&str, Option<usize>, Vec3); 18] = [
        ("root", None, Vec3::ZERO),
        ("hips", Some(ROOT), vec3(0.0, 1.0, 0.0)),
        ("spine", Some(HIPS), vec3(0.0, 0.16, 0.0)),
        ("chest", Some(SPINE), vec3(0.0, 0.26, 0.0)),
        ("neck", Some(CHEST), vec3(0.0, 0.33, 0.0)),
        ("head", Some(NECK), vec3(0.0, 0.10, 0.0)),
        ("upper_arm.R", Some(CHEST), vec3(-0.33, 0.23, 0.0)),
        ("forearm.R", Some(UARM_R), vec3(0.0, -0.33, 0.0)),
        ("hand.R", Some(FARM_R), vec3(0.0, -0.30, 0.0)),
        ("upper_arm.L", Some(CHEST), vec3(0.33, 0.23, 0.0)),
        ("forearm.L", Some(UARM_L), vec3(0.0, -0.33, 0.0)),
        ("hand.L", Some(FARM_L), vec3(0.0, -0.30, 0.0)),
        ("thigh.R", Some(HIPS), vec3(-0.13, -0.04, 0.0)),
        ("shin.R", Some(THIGH_R), vec3(0.0, -0.45, 0.0)),
        ("foot.R", Some(SHIN_R), vec3(0.0, -0.46, 0.0)),
        ("thigh.L", Some(HIPS), vec3(0.13, -0.04, 0.0)),
        ("shin.L", Some(THIGH_L), vec3(0.0, -0.45, 0.0)),
        ("foot.L", Some(SHIN_L), vec3(0.0, -0.46, 0.0)),
    ];
    let mut bones: Vec<Bone> = Vec::new();
    let mut globals: Vec<Mat4> = Vec::new();
    for (name, parent, t) in defs {
        let rest = Xf { t, ..Xf::IDENTITY };
        let g = parent.map(|p| globals[p]).unwrap_or(Mat4::IDENTITY) * rest.mat();
        globals.push(g);
        bones.push(Bone { name: name.into(), parent, rest, inverse_bind: g.inverse() });
    }
    bones
}

/// Adds an ellipsoid (in the bone's rest space at `center`) rigidly bound to `bone`.
fn ellipsoid(m: &mut SkinMesh, bones: &[Bone], bone: usize, center: Vec3, radii: Vec3, color: Color) {
    let to_model = bones[bone].inverse_bind.inverse();
    let (rings, segs) = (10u32, 16u32);
    let base = m.positions.len() as u32;
    for i in 0..=rings {
        let v = i as f32 / rings as f32 * std::f32::consts::PI;
        for j in 0..=segs {
            let u = j as f32 / segs as f32 * std::f32::consts::TAU;
            let n = vec3(v.sin() * u.cos(), v.cos(), v.sin() * u.sin());
            let p = center + n * radii;
            m.positions.push(to_model.transform_point3(p));
            m.normals.push(to_model.transform_vector3((n / radii).normalize()));
            m.joints.push([bone as u16, 0, 0, 0]);
            m.weights.push([1.0, 0.0, 0.0, 0.0]);
            m.colors.push(color);
        }
    }
    for i in 0..rings {
        for j in 0..segs {
            let a = base + i * (segs + 1) + j;
            let b = a + segs + 1;
            m.indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
}

/// Adds a box rigidly bound to `bone`.
fn cuboid(m: &mut SkinMesh, bones: &[Bone], bone: usize, center: Vec3, half: Vec3, color: Color) {
    let to_model = bones[bone].inverse_bind.inverse();
    let faces = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (-Vec3::X, Vec3::Y, -Vec3::Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (-Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::Y, Vec3::X),
    ];
    for (n, a, b) in faces {
        let base = m.positions.len() as u32;
        for (sa, sb) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let p = center + (n + a * sa + b * sb) * half;
            m.positions.push(to_model.transform_point3(p));
            m.normals.push(to_model.transform_vector3(n));
            m.joints.push([bone as u16, 0, 0, 0]);
            m.weights.push([1.0, 0.0, 0.0, 0.0]);
            m.colors.push(color);
        }
        m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

fn body(bones: &[Bone]) -> SkinMesh {
    let mut m = SkinMesh::default();
    // Segmented torso: hip band, three belly bands, chest.
    ellipsoid(&mut m, bones, HIPS, vec3(0.0, 0.02, 0.0), vec3(0.24, 0.12, 0.17), WHITE);
    for (i, y) in [0.0, 0.09, 0.18].iter().enumerate() {
        let col = if i % 2 == 0 { WHITE } else { SEAM };
        ellipsoid(&mut m, bones, SPINE, vec3(0.0, *y, 0.0), vec3(0.235 - i as f32 * 0.005, 0.07, 0.165), col);
    }
    ellipsoid(&mut m, bones, CHEST, vec3(0.0, 0.14, 0.0), vec3(0.29, 0.21, 0.19), WHITE);
    ellipsoid(&mut m, bones, NECK, vec3(0.0, 0.0, 0.0), vec3(0.07, 0.07, 0.07), SEAM);
    // Head with two eyes and brows.
    ellipsoid(&mut m, bones, HEAD, vec3(0.0, 0.15, 0.0), vec3(0.13, 0.17, 0.14), WHITE);
    for side in [-1.0, 1.0] {
        ellipsoid(&mut m, bones, HEAD, vec3(0.045 * side, 0.17, 0.125), vec3(0.017, 0.022, 0.012), DARK);
        cuboid(&mut m, bones, HEAD, vec3(0.05 * side, 0.215, 0.125), vec3(0.028, 0.005, 0.006), DARK);
    }
    // Limbs: right side blue, left side red.
    for (uarm, farm, hand, thigh, shin, foot, col) in [
        (UARM_R, FARM_R, HAND_R, THIGH_R, SHIN_R, FOOT_R, BLUE),
        (UARM_L, FARM_L, HAND_L, THIGH_L, SHIN_L, FOOT_L, RED),
    ] {
        ellipsoid(&mut m, bones, uarm, vec3(0.0, 0.0, 0.0), vec3(0.085, 0.085, 0.085), col);
        ellipsoid(&mut m, bones, uarm, vec3(0.0, -0.17, 0.0), vec3(0.07, 0.17, 0.07), col);
        ellipsoid(&mut m, bones, farm, vec3(0.0, -0.15, 0.0), vec3(0.065, 0.16, 0.065), col);
        cuboid(&mut m, bones, hand, vec3(0.0, -0.06, 0.0), vec3(0.055, 0.065, 0.05), col);
        ellipsoid(&mut m, bones, thigh, vec3(0.0, -0.22, 0.0), vec3(0.095, 0.25, 0.095), col);
        ellipsoid(&mut m, bones, shin, vec3(0.0, -0.22, 0.0), vec3(0.08, 0.24, 0.08), col);
        cuboid(&mut m, bones, foot, vec3(0.0, -0.04, 0.05), vec3(0.075, 0.045, 0.13), col);
    }
    m
}

/// A rotation from Euler angles in degrees (X then Y then Z). Negative X swings a limb forward.
fn rot(x: f32, y: f32, z: f32) -> Quat {
    Quat::from_euler(EulerRot::XYZ, x.to_radians(), y.to_radians(), z.to_radians())
}

struct ClipBuilder {
    name: &'static str,
    duration: f32,
    channels: Vec<Channel>,
}

impl ClipBuilder {
    fn new(name: &'static str, duration: f32) -> Self {
        ClipBuilder { name, duration, channels: Vec::new() }
    }

    /// Rotation keys for one bone: (time, x°, y°, z°).
    fn rot(mut self, bone: usize, keys: &[(f32, f32, f32, f32)]) -> Self {
        self.channels.push(Channel {
            bone,
            times: keys.iter().map(|k| k.0).collect(),
            keys: Keys::Rotation(keys.iter().map(|k| rot(k.1, k.2, k.3)).collect()),
        });
        self
    }

    /// Translation keys for one bone, added to its rest offset.
    fn mov(mut self, bone: usize, rest: Vec3, keys: &[(f32, f32, f32, f32)]) -> Self {
        self.channels.push(Channel {
            bone,
            times: keys.iter().map(|k| k.0).collect(),
            keys: Keys::Translation(keys.iter().map(|k| rest + vec3(k.1, k.2, k.3)).collect()),
        });
        self
    }

    fn done(self) -> Clip {
        Clip { name: self.name.into(), duration: self.duration, channels: self.channels }
    }
}

fn clips(bones: &[Bone]) -> Vec<Clip> {
    let hips = bones[HIPS].rest.t;
    // Arms hang slightly out from the body at rest.
    let arms_down = |c: ClipBuilder| c.rot(UARM_R, &[(0.0, 0.0, 0.0, -8.0)]).rot(UARM_L, &[(0.0, 0.0, 0.0, 8.0)]);
    vec![
        arms_down(ClipBuilder::new("idle", 2.0))
            .rot(CHEST, &[(0.0, 0.0, 0.0, 0.0), (1.0, -3.0, 0.0, 0.0), (2.0, 0.0, 0.0, 0.0)])
            .rot(FARM_R, &[(0.0, -15.0, 0.0, 0.0)])
            .rot(FARM_L, &[(0.0, -15.0, 0.0, 0.0)])
            .done(),
        ClipBuilder::new("walk", 0.8)
            .rot(THIGH_R, &[(0.0, -28.0, 0.0, 0.0), (0.4, 24.0, 0.0, 0.0), (0.8, -28.0, 0.0, 0.0)])
            .rot(THIGH_L, &[(0.0, 24.0, 0.0, 0.0), (0.4, -28.0, 0.0, 0.0), (0.8, 24.0, 0.0, 0.0)])
            .rot(SHIN_R, &[(0.0, 10.0, 0.0, 0.0), (0.2, 5.0, 0.0, 0.0), (0.4, 45.0, 0.0, 0.0), (0.6, 30.0, 0.0, 0.0), (0.8, 10.0, 0.0, 0.0)])
            .rot(SHIN_L, &[(0.0, 45.0, 0.0, 0.0), (0.2, 30.0, 0.0, 0.0), (0.4, 10.0, 0.0, 0.0), (0.6, 5.0, 0.0, 0.0), (0.8, 45.0, 0.0, 0.0)])
            .rot(UARM_R, &[(0.0, 22.0, 0.0, -8.0), (0.4, -22.0, 0.0, -8.0), (0.8, 22.0, 0.0, -8.0)])
            .rot(UARM_L, &[(0.0, -22.0, 0.0, 8.0), (0.4, 22.0, 0.0, 8.0), (0.8, -22.0, 0.0, 8.0)])
            .rot(FARM_R, &[(0.0, -20.0, 0.0, 0.0)])
            .rot(FARM_L, &[(0.0, -20.0, 0.0, 0.0)])
            .mov(HIPS, hips, &[(0.0, 0.0, 0.0, 0.0), (0.2, 0.0, 0.03, 0.0), (0.4, 0.0, 0.0, 0.0), (0.6, 0.0, 0.03, 0.0), (0.8, 0.0, 0.0, 0.0)])
            .done(),
        ClipBuilder::new("jump", 0.3)
            .rot(THIGH_R, &[(0.0, -10.0, 0.0, 0.0), (0.3, -35.0, 0.0, 0.0)])
            .rot(SHIN_R, &[(0.0, 15.0, 0.0, 0.0), (0.3, 60.0, 0.0, 0.0)])
            .rot(THIGH_L, &[(0.0, 5.0, 0.0, 0.0), (0.3, -10.0, 0.0, 0.0)])
            .rot(SHIN_L, &[(0.0, 10.0, 0.0, 0.0), (0.3, 30.0, 0.0, 0.0)])
            .rot(UARM_R, &[(0.0, -20.0, 0.0, -30.0)])
            .rot(UARM_L, &[(0.0, -20.0, 0.0, 30.0)])
            .done(),
        // Rocket Punch: the right (blue) gauntlet arm.
        ClipBuilder::new("punch_charge", 0.6)
            .rot(CHEST, &[(0.0, 0.0, 0.0, 0.0), (0.6, 5.0, -30.0, 0.0)])
            .rot(UARM_R, &[(0.0, 0.0, 0.0, -8.0), (0.6, 45.0, 0.0, -35.0)])
            .rot(FARM_R, &[(0.0, -20.0, 0.0, 0.0), (0.6, -110.0, 0.0, 0.0)])
            .rot(UARM_L, &[(0.0, -50.0, 0.0, 10.0)])
            .rot(FARM_L, &[(0.0, -70.0, 0.0, 0.0)])
            .rot(THIGH_R, &[(0.0, 15.0, 0.0, 0.0)])
            .rot(THIGH_L, &[(0.0, -25.0, 0.0, 0.0)])
            .rot(SHIN_L, &[(0.0, 25.0, 0.0, 0.0)])
            .done(),
        ClipBuilder::new("punch", 0.15)
            .rot(CHEST, &[(0.0, 5.0, -30.0, 0.0), (0.15, 10.0, 25.0, 0.0)])
            .rot(UARM_R, &[(0.0, 45.0, 0.0, -35.0), (0.15, -95.0, 0.0, -5.0)])
            .rot(FARM_R, &[(0.0, -110.0, 0.0, 0.0), (0.15, -5.0, 0.0, 0.0)])
            .rot(UARM_L, &[(0.0, 30.0, 0.0, 15.0)])
            .rot(FARM_L, &[(0.0, -60.0, 0.0, 0.0)])
            .rot(THIGH_R, &[(0.0, 30.0, 0.0, 0.0)])
            .rot(THIGH_L, &[(0.0, -40.0, 0.0, 0.0)])
            .rot(SHIN_L, &[(0.0, 35.0, 0.0, 0.0)])
            .done(),
        ClipBuilder::new("slam_leap", 0.3)
            .rot(UARM_R, &[(0.0, 0.0, 0.0, -8.0), (0.3, -165.0, 0.0, -15.0)])
            .rot(UARM_L, &[(0.0, 0.0, 0.0, 8.0), (0.3, -165.0, 0.0, 15.0)])
            .rot(FARM_R, &[(0.0, -30.0, 0.0, 0.0)])
            .rot(FARM_L, &[(0.0, -30.0, 0.0, 0.0)])
            .rot(THIGH_R, &[(0.0, -45.0, 0.0, 0.0)])
            .rot(SHIN_R, &[(0.0, 70.0, 0.0, 0.0)])
            .rot(THIGH_L, &[(0.0, -20.0, 0.0, 0.0)])
            .rot(SHIN_L, &[(0.0, 40.0, 0.0, 0.0)])
            .done(),
        ClipBuilder::new("slam_land", 0.3)
            .mov(HIPS, hips, &[(0.0, 0.0, -0.25, 0.0), (0.3, 0.0, -0.12, 0.0)])
            .rot(CHEST, &[(0.0, 35.0, 0.0, 0.0), (0.3, 20.0, 0.0, 0.0)])
            .rot(UARM_R, &[(0.0, -60.0, 0.0, -10.0)])
            .rot(UARM_L, &[(0.0, -60.0, 0.0, 10.0)])
            .rot(FARM_R, &[(0.0, -10.0, 0.0, 0.0)])
            .rot(FARM_L, &[(0.0, -10.0, 0.0, 0.0)])
            .rot(THIGH_R, &[(0.0, -70.0, 0.0, 0.0)])
            .rot(SHIN_R, &[(0.0, 90.0, 0.0, 0.0)])
            .rot(THIGH_L, &[(0.0, -40.0, 0.0, 0.0)])
            .rot(SHIN_L, &[(0.0, 70.0, 0.0, 0.0)])
            .done(),
        // Power Block: forearms crossed in front of the chest.
        ClipBuilder::new("block", 0.15)
            .rot(UARM_R, &[(0.0, 0.0, 0.0, -8.0), (0.15, -75.0, -25.0, -10.0)])
            .rot(UARM_L, &[(0.0, 0.0, 0.0, 8.0), (0.15, -75.0, 25.0, 10.0)])
            .rot(FARM_R, &[(0.0, -20.0, 0.0, 0.0), (0.15, -40.0, 0.0, 70.0)])
            .rot(FARM_L, &[(0.0, -20.0, 0.0, 0.0), (0.15, -40.0, 0.0, -70.0)])
            .rot(THIGH_R, &[(0.0, -10.0, 0.0, -5.0)])
            .rot(THIGH_L, &[(0.0, 10.0, 0.0, 5.0)])
            .done(),
        // Quick melee: a short backhand swing with the gauntlet (right) arm.
        ClipBuilder::new("melee", 0.3)
            .rot(CHEST, &[(0.0, 0.0, -20.0, 0.0), (0.12, 0.0, 25.0, 0.0), (0.3, 0.0, 0.0, 0.0)])
            .rot(UARM_R, &[(0.0, -80.0, 30.0, -40.0), (0.12, -85.0, -40.0, -10.0), (0.3, -50.0, 0.0, -10.0)])
            .rot(FARM_R, &[(0.0, -60.0, 0.0, 0.0), (0.12, -15.0, 0.0, 0.0), (0.3, -40.0, 0.0, 0.0)])
            .rot(UARM_L, &[(0.0, -40.0, 0.0, 10.0)])
            .rot(FARM_L, &[(0.0, -60.0, 0.0, 0.0)])
            .done(),
        // First-person ready pose: gauntlet (right) raised in front, cannon hand (left) forward.
        ClipBuilder::new("fp_idle", 2.0)
            .rot(UARM_R, &[(0.0, -55.0, 0.0, 15.0), (1.0, -58.0, 0.0, 15.0), (2.0, -55.0, 0.0, 15.0)])
            .rot(FARM_R, &[(0.0, -70.0, 0.0, 0.0)])
            .rot(UARM_L, &[(0.0, -65.0, 0.0, -12.0), (1.0, -62.0, 0.0, -12.0), (2.0, -65.0, 0.0, -12.0)])
            .rot(FARM_L, &[(0.0, -25.0, 0.0, 0.0)])
            .done(),
        // Hand Cannon: fired from the left (red) hand.
        ClipBuilder::new("fire", 0.25)
            .rot(UARM_R, &[(0.0, 0.0, 0.0, -8.0)])
            .rot(UARM_L, &[(0.0, -85.0, 0.0, 5.0), (0.05, -100.0, 0.0, 5.0), (0.25, -85.0, 0.0, 5.0)])
            .rot(FARM_L, &[(0.0, -5.0, 0.0, 0.0), (0.05, -25.0, 0.0, 0.0), (0.25, -5.0, 0.0, 0.0)])
            .done(),
        ClipBuilder::new("reload", 0.6)
            .rot(UARM_R, &[(0.0, 0.0, 0.0, -8.0)])
            .rot(UARM_L, &[(0.0, -40.0, 0.0, 5.0), (0.3, -55.0, 20.0, 5.0), (0.6, -40.0, 0.0, 5.0)])
            .rot(FARM_L, &[(0.0, -60.0, 0.0, 0.0), (0.3, -100.0, 0.0, 0.0), (0.6, -60.0, 0.0, 0.0)])
            .done(),
    ]
}

pub fn build() -> Model {
    let bones = skeleton();
    let mesh = body(&bones);
    let clips = clips(&bones);
    Model { bones, meshes: vec![mesh], clips, fix: Mat4::IDENTITY }
}
