//! Scripted showcase scenes for the README GIFs: `BEAN_DEMO=<scene> BEAN_DEMO_OUT=<dir>` plays
//! fixed inputs at a fixed rate, saves every frame as a PNG and exits. `scripts/make_gifs.py`
//! turns the frames into GIFs.

use doomfist::{Input, Vec3};

/// One stretch of input: held for `ticks` sim ticks while the aim turns linearly to
/// (`yaw`, `pitch`) degrees.
#[derive(Clone, Copy, Default)]
pub struct Step {
    pub ticks: u32,
    pub yaw: f32,
    pub pitch: f32,
    pub fwd: f32,
    pub right: f32,
    pub jump: bool,
    pub punch: bool,
    pub slam: bool,
    pub block: bool,
    pub fire: bool,
    pub melee: bool,
}

/// Where the demo camera sits.
#[derive(Clone, Copy)]
pub enum Cam {
    FirstPerson,
    /// Behind and above Doomfist, looking where he looks.
    Chase,
    /// A fixed spot, turning to follow Doomfist.
    Fixed(Vec3),
}

pub struct Scene {
    pub start: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub cam: Cam,
    pub steps: Vec<Step>,
}

/// Degrees to face from `a` towards `b` on the ground (0 = +z).
fn yaw_to(a: (f32, f32), b: (f32, f32)) -> f32 {
    (b.0 - a.0).atan2(b.1 - a.1).to_degrees()
}

fn wait(ticks: u32, yaw: f32, pitch: f32) -> Step {
    Step { ticks, yaw, pitch, ..Default::default() }
}

pub fn scene(name: &str) -> Option<Scene> {
    let s = match name {
        // Full-charge Rocket Punch into a dummy standing near a wall: impact, then wall slam.
        "punch" => {
            let (from, to) = ((3.0, 12.0), (0.0, 26.0));
            let y = yaw_to(from, to);
            Scene {
                start: Vec3::new(from.0, 0.0, from.1),
                yaw: y,
                pitch: -2.0,
                cam: Cam::Fixed(Vec3::new(8.0, 3.2, 24.0)),
                steps: vec![
                    wait(20, y, -2.0),
                    Step { ticks: 66, punch: true, ..wait(0, y, -2.0) },
                    wait(110, y, -2.0),
                ],
            }
        }
        // Seismic Slam with the landing preview, catching two dummies in the cone.
        "slam" => {
            let from = (-4.0, -2.0);
            let y = yaw_to(from, (3.0, 10.0));
            Scene {
                start: Vec3::new(from.0, 0.0, from.1),
                yaw: y - 25.0,
                pitch: -20.0,
                cam: Cam::Fixed(Vec3::new(-8.0, 3.8, -5.0)),
                steps: vec![
                    wait(60, y, -24.0),
                    Step { ticks: 2, slam: true, ..wait(0, y, -24.0) },
                    wait(100, y, -24.0),
                ],
            }
        }
        // Movement tech: charged punch, jump cancel while turning, then a slam with jump held
        // through the landing for a bunny hop.
        "tech" => {
            let y0 = 90.0;
            Scene {
                start: Vec3::new(-30.0, 0.0, -33.0),
                yaw: y0,
                pitch: 0.0,
                cam: Cam::Chase,
                steps: vec![
                    wait(10, y0, 0.0),
                    Step { ticks: 40, punch: true, ..wait(0, y0, 0.0) },
                    wait(12, y0, 0.0),
                    Step { ticks: 14, jump: true, fwd: 1.0, ..wait(0, y0 - 20.0, 5.0) },
                    Step { ticks: 20, fwd: 1.0, ..wait(0, y0 - 20.0, 0.0) },
                    Step { ticks: 2, slam: true, fwd: 1.0, ..wait(0, y0 - 20.0, -10.0) },
                    Step { ticks: 40, fwd: 1.0, ..wait(0, y0 - 15.0, -10.0) },
                    Step { ticks: 40, jump: true, fwd: 1.0, ..wait(0, y0 - 10.0, -10.0) },
                    Step { ticks: 40, fwd: 1.0, ..wait(0, y0 - 10.0, -10.0) },
                ],
            }
        }
        // First person: Hand Cannon at a dummy, then quick melee up close.
        "gun" => {
            let from = (0.0, 4.0);
            let y = yaw_to(from, (0.0, 12.0));
            Scene {
                start: Vec3::new(from.0, 0.0, from.1),
                yaw: y,
                pitch: -1.0,
                cam: Cam::FirstPerson,
                steps: vec![
                    wait(15, y, 0.0),
                    Step { ticks: 90, fire: true, ..wait(0, y, 0.0) },
                    Step { ticks: 60, fwd: 1.0, ..wait(0, y, -2.0) },
                    Step { ticks: 2, melee: true, ..wait(0, y, -2.0) },
                    wait(60, y, -2.0),
                ],
            }
        }
        _ => return None,
    };
    Some(s)
}

impl Scene {
    pub fn len(&self) -> u32 {
        self.steps.iter().map(|s| s.ticks).sum()
    }

    /// Aim (yaw, pitch in degrees) and input at sim tick `tick`.
    pub fn at(&self, tick: u32) -> (f32, f32, Input) {
        let (mut yaw, mut pitch) = (self.yaw, self.pitch);
        let mut t = tick;
        let mut step = *self.steps.last().unwrap();
        for s in &self.steps {
            if t < s.ticks {
                let f = (t + 1) as f32 / s.ticks as f32;
                yaw += (s.yaw - yaw) * f;
                pitch += (s.pitch - pitch) * f;
                step = *s;
                break;
            }
            t -= s.ticks;
            yaw = s.yaw;
            pitch = s.pitch;
        }
        let (yr, pr) = (yaw.to_radians(), pitch.to_radians());
        let input = Input {
            facing: Vec3::new(yr.sin() * pr.cos(), pr.sin(), yr.cos() * pr.cos()),
            move_forward: step.fwd,
            move_right: step.right,
            jump: step.jump,
            crouch: false,
            secondary: step.punch,
            ability1: step.slam,
            ability2: step.block,
            melee: step.melee,
            primary: step.fire,
        };
        (yaw, pitch, input)
    }
}
