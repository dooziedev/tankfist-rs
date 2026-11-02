//! Skeletal animation for bean: skeleton, clips, sampling, crossfades and CPU skinning.
//!
//! The model is the built-in mannequin (`mannequin.rs`); each game state plays the clip of the
//! same name.

use macroquad::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub t: Vec3,
    pub r: Quat,
    pub s: Vec3,
}

impl Xf {
    pub const IDENTITY: Xf = Xf { t: Vec3::ZERO, r: Quat::IDENTITY, s: Vec3::ONE };

    pub fn mat(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.s, self.r, self.t)
    }

    pub fn lerp(&self, o: &Xf, f: f32) -> Xf {
        Xf { t: self.t.lerp(o.t, f), r: self.r.slerp(o.r, f), s: self.s.lerp(o.s, f) }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub rest: Xf,
    pub inverse_bind: Mat4,
}

#[derive(Clone, Debug)]
pub enum Keys {
    Translation(Vec<Vec3>),
    Rotation(Vec<Quat>),
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub bone: usize,
    pub times: Vec<f32>,
    pub keys: Keys,
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    pub duration: f32,
    pub channels: Vec<Channel>,
}

/// One skinned mesh: up to 4 bone influences per vertex.
#[derive(Clone, Debug, Default)]
pub struct SkinMesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub colors: Vec<Color>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub bones: Vec<Bone>,
    pub meshes: Vec<SkinMesh>,
    pub clips: Vec<Clip>,
    /// Applied to the whole model.
    pub fix: Mat4,
}

impl Model {
    pub fn clip(&self, name: &str) -> Option<usize> {
        self.clips.iter().position(|c| c.name == name)
    }

    /// Local bone transforms for `clip` at time `time` (rest pose where a bone isn't animated).
    pub fn sample(&self, clip: Option<usize>, time: f32) -> Vec<Xf> {
        let mut pose: Vec<Xf> = self.bones.iter().map(|b| b.rest).collect();
        let Some(clip) = clip.map(|i| &self.clips[i]) else { return pose };
        for ch in &clip.channels {
            let n = ch.times.len();
            if n == 0 {
                continue;
            }
            let (i, f) = match ch.times.iter().position(|&k| k > time) {
                None => (n - 1, 0.0),
                Some(0) => (0, 0.0),
                Some(j) => {
                    let (a, b) = (ch.times[j - 1], ch.times[j]);
                    (j - 1, if b > a { (time - a) / (b - a) } else { 0.0 })
                }
            };
            let j = (i + 1).min(n - 1);
            let x = &mut pose[ch.bone];
            match &ch.keys {
                Keys::Translation(v) => x.t = v[i].lerp(v[j], f),
                Keys::Rotation(v) => x.r = v[i].slerp(v[j], f).normalize(),
            }
        }
        pose
    }

    /// Each bone's transform in model space for a local pose.
    pub fn globals(&self, pose: &[Xf]) -> Vec<Mat4> {
        let mut global = vec![Mat4::IDENTITY; self.bones.len()];
        for (i, b) in self.bones.iter().enumerate() {
            let local = pose[i].mat();
            global[i] = match b.parent {
                Some(p) => global[p] * local,
                None => local,
            };
        }
        global
    }

    /// Joint matrices (global x inverse bind) for a local pose.
    pub fn joint_matrices(&self, pose: &[Xf]) -> Vec<Mat4> {
        self.globals(pose).iter().zip(&self.bones).map(|(g, b)| *g * b.inverse_bind).collect()
    }

    /// Bones that make up the first-person arms (named like arm, hand, ...), plus everything
    /// below them.
    pub fn arm_bones(&self) -> Vec<bool> {
        const WORDS: [&str; 12] =
            ["arm", "hand", "finger", "thumb", "index", "middle", "ring", "pinky", "clavicle", "shoulder", "elbow", "wrist"];
        let mut keep = vec![false; self.bones.len()];
        for (i, b) in self.bones.iter().enumerate() {
            let n = b.name.to_lowercase();
            let picked = WORDS.iter().any(|w| n.contains(w));
            keep[i] = picked || b.parent.is_some_and(|p| keep[p]);
        }
        keep
    }

    /// The bone the first-person arms hang from: a chest/upper-spine bone.
    pub fn anchor_bone(&self) -> usize {
        for word in ["chest", "spine2", "spine1", "spine"] {
            if let Some(i) = self.bones.iter().position(|b| b.name.to_lowercase().contains(word)) {
                return i;
            }
        }
        0
    }

    /// Skins every mesh with simple lighting and returns macroquad meshes placed by `world`.
    /// With `keep`, only triangles whose vertices are mostly driven by kept bones are drawn.
    pub fn build_meshes(&self, joints: &[Mat4], world: Mat4, tint: Color, keep: Option<&[bool]>) -> Vec<Mesh> {
        let light = vec3(0.4, 1.0, 0.3).normalize();
        let world = world * self.fix;
        let mut out = Vec::new();
        for m in &self.meshes {
            let mut verts = Vec::with_capacity(m.positions.len());
            for v in 0..m.positions.len() {
                let mut skin = Mat4::ZERO;
                let mut total = 0.0;
                for k in 0..4 {
                    let w = m.weights[v][k];
                    if w > 0.0 {
                        skin += joints[m.joints[v][k] as usize] * w;
                        total += w;
                    }
                }
                if total <= 0.0 {
                    skin = Mat4::IDENTITY;
                }
                let full = world * skin;
                let p = full.transform_point3(m.positions[v]);
                let n = full.transform_vector3(m.normals[v]).normalize_or_zero();
                let shade = 0.45 + 0.55 * n.dot(light).max(0.0);
                let c = m.colors[v];
                let col = Color::new(c.r * tint.r * shade, c.g * tint.g * shade, c.b * tint.b * shade, c.a * tint.a);
                verts.push(Vertex { position: p, uv: Vec2::ZERO, color: col.into(), normal: Vec4::ZERO });
            }
            // macroquad draws at most ~5000 indices / 10000 vertices per call (16-bit indices):
            // split meshes into chunks of 1600 triangles.
            let main_bone = |v: u32| {
                let (j, w) = (m.joints[v as usize], m.weights[v as usize]);
                let k = (0..4).max_by(|&a, &b| w[a].total_cmp(&w[b])).unwrap_or(0);
                j[k] as usize
            };
            let indices: Vec<u32> = match keep {
                Some(keep) => m
                    .indices
                    .chunks(3)
                    .filter(|t| t.iter().all(|&v| keep.get(main_bone(v)).copied().unwrap_or(false)))
                    .flatten()
                    .copied()
                    .collect(),
                None => m.indices.clone(),
            };
            for tris in indices.chunks(3 * 1600) {
                let mut map = HashMap::new();
                let mut chunk_verts = Vec::new();
                let mut idx = Vec::with_capacity(tris.len());
                for &i in tris {
                    let next = map.len() as u16;
                    let local = *map.entry(i).or_insert_with(|| {
                        chunk_verts.push(verts[i as usize]);
                        next
                    });
                    idx.push(local);
                }
                out.push(Mesh { vertices: chunk_verts, indices: idx, texture: None });
            }
        }
        out
    }
}

/// Plays one state at a time with a short crossfade between states.
pub struct Player {
    pub state: String,
    pub time: f32,
    prev: Option<(String, f32)>,
    fade: f32,
}

pub const FADE_TIME: f32 = 0.12;

impl Player {
    pub fn new() -> Self {
        Player { state: "idle".into(), time: 0.0, prev: None, fade: 1.0 }
    }

    /// Advances time; switching state starts a crossfade (and restarts one-shot clips).
    pub fn update(&mut self, state: &str, dt: f32, rate: f32) {
        if state != self.state {
            self.prev = Some((std::mem::replace(&mut self.state, state.to_string()), self.time));
            self.time = 0.0;
            self.fade = 0.0;
        }
        self.time += dt * rate;
        self.fade = (self.fade + dt / FADE_TIME).min(1.0);
        if let Some(p) = &mut self.prev {
            p.1 += dt;
        }
        if self.fade >= 1.0 {
            self.prev = None;
        }
    }

    pub fn pose(&self, model: &Model, looping: impl Fn(&str) -> bool) -> Vec<Xf> {
        let at = |state: &str, t: f32| {
            let clip = model.clip(state);
            let d = clip.map(|c| model.clips[c].duration).unwrap_or(1.0).max(1e-3);
            let t = if looping(state) { t.rem_euclid(d) } else { t.min(d) };
            model.sample(clip, t)
        };
        let now = at(&self.state, self.time);
        match &self.prev {
            Some((s, t)) => {
                let before = at(s, *t);
                before.iter().zip(&now).map(|(a, b)| a.lerp(b, self.fade)).collect()
            }
            None => now,
        }
    }
}
