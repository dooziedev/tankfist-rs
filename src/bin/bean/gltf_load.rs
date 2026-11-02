//! Loads a skinned, animated model from a glTF 2.0 file (.glb or .gltf) that the user exported
//! from their own copy of the game. Nothing here ships any asset: files live in the git-ignored
//! `assets/` folder and never leave the PC.
//!
//! Uses the first skin in the file and every animation in it. Materials contribute their base
//! colour (textures aren't drawn yet).

use crate::anim::{Bone, Channel, Clip, Keys, Model, SkinMesh, Xf};
use gltf::animation::util::ReadOutputs;
use macroquad::prelude::*;
use std::path::Path;

fn xf(node: &gltf::Node) -> Xf {
    let (t, r, s) = node.transform().decomposed();
    Xf { t: Vec3::from(t), r: Quat::from_array(r), s: Vec3::from(s) }
}

pub fn load(path: &Path) -> Result<Model, String> {
    let (doc, buffers, _images) = gltf::import(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let skin = doc.skins().next().ok_or("the file has no skin (skeleton)")?;
    let joints: Vec<gltf::Node> = skin.joints().collect();
    let joint_index = |node_index: usize| joints.iter().position(|j| j.index() == node_index);

    // Parents: walk every node's children.
    let mut parent_of = vec![None; doc.nodes().count()];
    for n in doc.nodes() {
        for c in n.children() {
            parent_of[c.index()] = Some(n.index());
        }
    }
    let reader = skin.reader(|b| Some(&buffers[b.index()]));
    let inverse_binds: Vec<Mat4> = reader
        .read_inverse_bind_matrices()
        .map(|it| it.map(|m| Mat4::from_cols_array_2d(&m)).collect())
        .unwrap_or_else(|| vec![Mat4::IDENTITY; joints.len()]);

    // Bones are the skin's joints; a joint's parent is the nearest ancestor that is also a joint.
    // Ancestors outside the skin (e.g. an armature node with a scale) are folded into `fix`.
    let mut bones = Vec::new();
    for (i, j) in joints.iter().enumerate() {
        let mut p = parent_of[j.index()];
        let mut parent = None;
        while let Some(pi) = p {
            if let Some(ji) = joint_index(pi) {
                parent = Some(ji);
                break;
            }
            p = parent_of[pi];
        }
        bones.push(Bone {
            name: j.name().unwrap_or("bone").to_string(),
            parent,
            rest: xf(j),
            inverse_bind: inverse_binds.get(i).copied().unwrap_or(Mat4::IDENTITY),
        });
    }
    // Bones must come after their parents for the global pass.
    for (i, b) in bones.iter().enumerate() {
        if b.parent.is_some_and(|p| p >= i) {
            return Err("skin joints aren't ordered parents-first; re-export with a standard exporter".into());
        }
    }
    // Transform of the root joint's non-joint ancestors.
    let mut fix = Mat4::IDENTITY;
    if let Some(root) = joints.first() {
        let mut p = parent_of[root.index()];
        while let Some(pi) = p {
            let node = doc.nodes().nth(pi).unwrap();
            fix = xf(&node).mat() * fix;
            p = parent_of[pi];
        }
    }

    // Meshes attached to the skin.
    let mut meshes = Vec::new();
    for node in doc.nodes().filter(|n| n.skin().is_some_and(|s| s.index() == skin.index())) {
        let Some(mesh) = node.mesh() else { continue };
        for prim in mesh.primitives() {
            let r = prim.reader(|b| Some(&buffers[b.index()]));
            let Some(pos) = r.read_positions() else { continue };
            let positions: Vec<Vec3> = pos.map(Vec3::from).collect();
            let n = positions.len();
            let normals = r.read_normals().map(|it| it.map(Vec3::from).collect()).unwrap_or(vec![Vec3::Y; n]);
            let joints_ = r.read_joints(0).map(|it| it.into_u16().collect()).unwrap_or(vec![[0; 4]; n]);
            let weights = r.read_weights(0).map(|it| it.into_f32().collect()).unwrap_or(vec![[1.0, 0.0, 0.0, 0.0]; n]);
            let base = prim.material().pbr_metallic_roughness().base_color_factor();
            let color = Color::new(base[0], base[1], base[2], base[3]);
            let indices = r.read_indices().map(|it| it.into_u32().collect()).unwrap_or((0..n as u32).collect());
            meshes.push(SkinMesh { positions, normals, joints: joints_, weights, colors: vec![color; n], indices });
        }
    }
    if meshes.is_empty() {
        return Err("no skinned mesh found".into());
    }

    // Animations: channels that target joints.
    let mut clips = Vec::new();
    for (ai, anim) in doc.animations().enumerate() {
        let mut channels = Vec::new();
        let mut duration: f32 = 0.0;
        for ch in anim.channels() {
            let Some(bone) = joint_index(ch.target().node().index()) else { continue };
            let r = ch.reader(|b| Some(&buffers[b.index()]));
            let Some(times) = r.read_inputs() else { continue };
            let times: Vec<f32> = times.collect();
            duration = duration.max(times.last().copied().unwrap_or(0.0));
            let keys = match r.read_outputs() {
                Some(ReadOutputs::Translations(v)) => Keys::Translation(v.map(Vec3::from).collect()),
                Some(ReadOutputs::Rotations(v)) => Keys::Rotation(v.into_f32().map(Quat::from_array).collect()),
                Some(ReadOutputs::Scales(v)) => Keys::Scale(v.map(Vec3::from).collect()),
                _ => continue,
            };
            // Cubic-spline data stores in-tangent, value, out-tangent: keep just the values.
            let keys = if ch.sampler().interpolation() == gltf::animation::Interpolation::CubicSpline {
                match keys {
                    Keys::Translation(v) => Keys::Translation(v.chunks(3).map(|c| c[1]).collect()),
                    Keys::Rotation(v) => Keys::Rotation(v.chunks(3).map(|c| c[1]).collect()),
                    Keys::Scale(v) => Keys::Scale(v.chunks(3).map(|c| c[1]).collect()),
                }
            } else {
                keys
            };
            channels.push(Channel { bone, times, keys });
        }
        let name = anim.name().map(str::to_string).unwrap_or_else(|| format!("animation_{ai}"));
        clips.push(Clip { name, duration, channels });
    }
    Ok(Model { bones, meshes, clips, fix })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(data: &[u8]) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for c in data.chunks(3) {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            for i in 0..4 {
                if i <= c.len() {
                    out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    /// A two-bone arm: one triangle on bone 1, which an animation rotates 90° about Z.
    #[test]
    fn loads_a_skinned_animated_gltf() {
        let mut buf: Vec<u8> = Vec::new();
        let f = |buf: &mut Vec<u8>, v: &[f32]| v.iter().for_each(|x| buf.extend_from_slice(&x.to_le_bytes()));
        f(&mut buf, &[0.0, 1.0, 0.0, 0.1, 1.0, 0.0, 0.0, 2.0, 0.0]); // positions: 36 bytes at 0
        for _ in 0..3 {
            for j in [1u16, 0, 0, 0] {
                buf.extend_from_slice(&j.to_le_bytes());
            }
        } // joints: 24 bytes at 36
        for _ in 0..3 {
            f(&mut buf, &[1.0, 0.0, 0.0, 0.0]);
        } // weights: 48 bytes at 60
        let ib0 = Mat4::IDENTITY;
        let ib1 = Mat4::from_translation(vec3(0.0, -1.0, 0.0));
        f(&mut buf, &ib0.to_cols_array());
        f(&mut buf, &ib1.to_cols_array()); // inverse binds: 128 bytes at 108
        f(&mut buf, &[0.0, 1.0]); // times: 8 bytes at 236
        let q = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        f(&mut buf, &[0.0, 0.0, 0.0, 1.0, q.x, q.y, q.z, q.w]); // rotations: 32 bytes at 244
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0,2]}}],
            "nodes":[{{"name":"root","children":[1]}},{{"name":"arm","translation":[0,1,0]}},{{"name":"mesh","mesh":0,"skin":0}}],
            "skins":[{{"joints":[0,1],"inverseBindMatrices":3}}],
            "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"JOINTS_0":1,"WEIGHTS_0":2}}}}]}}],
            "animations":[{{"name":"swing","channels":[{{"sampler":0,"target":{{"node":1,"path":"rotation"}}}}],"samplers":[{{"input":4,"output":5}}]}}],
            "buffers":[{{"byteLength":{len},"uri":"data:application/octet-stream;base64,{data}"}}],
            "bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":36}},{{"buffer":0,"byteOffset":36,"byteLength":24}},{{"buffer":0,"byteOffset":60,"byteLength":48}},{{"buffer":0,"byteOffset":108,"byteLength":128}},{{"buffer":0,"byteOffset":236,"byteLength":8}},{{"buffer":0,"byteOffset":244,"byteLength":32}}],
            "accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,1,0],"max":[0.1,2,0]}},{{"bufferView":1,"componentType":5123,"count":3,"type":"VEC4"}},{{"bufferView":2,"componentType":5126,"count":3,"type":"VEC4"}},{{"bufferView":3,"componentType":5126,"count":2,"type":"MAT4"}},{{"bufferView":4,"componentType":5126,"count":2,"type":"SCALAR","min":[0],"max":[1]}},{{"bufferView":5,"componentType":5126,"count":2,"type":"VEC4"}}]}}"#,
            len = buf.len(),
            data = b64(&buf)
        );
        let dir = std::env::temp_dir().join("doomfist-gltf-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("arm.gltf");
        std::fs::write(&path, json).unwrap();

        let model = load(&path).expect("loads");
        assert_eq!(model.bones.len(), 2);
        assert_eq!(model.bones[1].parent, Some(0));
        assert_eq!(model.clips.len(), 1);
        assert_eq!(model.clips[0].name, "swing");
        // At rest the tip vertex (0, 2, 0) stays put; after the swing it points along -X.
        let tip = |t: f32| {
            let pose = model.sample(Some(0), t);
            let j = model.joint_matrices(&pose);
            j[1].transform_point3(vec3(0.0, 2.0, 0.0))
        };
        assert!((tip(0.0) - vec3(0.0, 2.0, 0.0)).length() < 1e-4);
        assert!((tip(1.0) - vec3(-1.0, 1.0, 0.0)).length() < 1e-4, "{}", tip(1.0));
    }
}
