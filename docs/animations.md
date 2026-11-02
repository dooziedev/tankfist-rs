# Models and animations

The bean demo draws Doomfist with a skinned, animated model. By default it uses a **built-in
stand-in mannequin** that's generated in code (`src/bin/bean/mannequin.rs`), so the project
runs and can be published with no game files at all.

You can swap in a model you export **from your own copy of the game**. Those files stay on your
PC: everything under `assets/` is git-ignored, and a pre-commit hook (`scripts/check-no-assets.sh`,
enabled with `git config core.hooksPath scripts/hooks`) refuses to commit models, animations,
textures or audio anywhere in the repo.

## Bringing your own model

1. Export Doomfist (mesh + skeleton + animations) to **glTF 2.0** (`.glb` or `.gltf`) with the tool
   of your choice.
2. Put it in `assets/doomfist/` (create the folder). The first `.glb`/`.gltf` found is used.
3. Add `assets/doomfist/animations.txt` naming which clip plays for each game state (see below).
4. Run `cargo run --release --bin bean`. The HUD shows which model loaded, or why it fell back to
   the mannequin.

What's read from the file: the first skin (skeleton), every mesh bound to it (up to 4 bone
weights per vertex), every animation, and each material's base colour. Textures aren't drawn yet.

## animations.txt

```text
# game state = clip name in your file
idle         = <clip name>
walk         = <clip name>
jump         = <clip name>
punch_charge = <clip name>
punch        = <clip name>
slam_leap    = <clip name>
slam_land    = <clip name>
block        = <clip name>
melee        = <clip name>
fire         = <clip name>
reload       = <clip name>

fp_idle      = <clip name>   # first-person ready pose (optional)

# optional fixes for how the file was exported
scale = 1.0            # e.g. 0.01 if the file is in centimetres
yaw_offset_deg = 0     # e.g. 180 if the model faces backwards

# first person (optional): which bones make up the arms, and the bone they hang from.
# Without these, bones named like arm/hand/finger/shoulder/clavicle are used, hanging
# from a chest or upper-spine bone.
first_person_bones = clavicle_l, clavicle_r
first_person_anchor = spine_03
```

A state with no line uses a clip with the same name as the state, if the file has one. The
model should stand on the origin with +Y up, facing +Z; use `scale` and `yaw_offset_deg` to fix
other conventions.

## Game states

| State | When it plays | Loops |
|---|---|---|
| `idle` | on the ground, not moving | yes |
| `walk` | on the ground, moving (speed scales the playback rate; 1.0 at 5.5 m/s) | yes |
| `jump` | in the air, no ability | no |
| `punch_charge` | holding Rocket Punch | no (holds last frame) |
| `punch` | Rocket Punch dash | no |
| `slam_leap` | Seismic Slam, in the air | no |
| `slam_land` | Seismic Slam, touchdown and slide | no |
| `block` | Power Block | no |
| `melee` | quick melee (0.35 s) | no |
| `fire` | Hand Cannon shot (once the gun is in) | no |
| `reload` | Hand Cannon reload / refill (once the gun is in) | no |
| `fp_idle` | first person, used instead of `idle`/`walk`/`jump` if the model has it | yes |

## First person

In first person only the arm bones are drawn, hung from the anchor bone just under the camera
so they follow your aim up and down. Listed bones include everything below them (fingers).

States switch with a 0.12 s crossfade.

## Checking a model without playing

```bash
BEAN_SHOT=shot.png BEAN_POSE=block cargo run --release --bin bean
```

renders Doomfist from the front holding the given state, saves `shot.png` and exits. Add
`BEAN_VIEW=fp` to capture the first-person view instead. Screenshots
are git-ignored outside `docs/`.
