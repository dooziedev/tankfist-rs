//! Doomfist's mechanics rebuilt in Rust.
//!
//! * `abilities/`   one file per ability (movement, passive, block, punch, slam)
//! * `tunables.rs`  every number, tagged guess/measured
//! * `log.rs`       reads logs from `workshop/doomfist_logger.txt`
//! * `replay.rs`    re-runs a logged take through the sim and compares positions

pub mod abilities;
pub mod doomfist;
pub mod events;
pub mod input;
pub mod log;
pub mod math;
pub mod playback;
pub mod record;
pub mod replay;
pub mod sim;
pub mod tunables;
pub mod world;

pub use doomfist::{ActionKind, Doomfist};
pub use events::Event;
pub use input::Input;
pub use math::Vec3;
pub use sim::Sim;
pub use tunables::Tunables;
