//! Every number that defines Doomfist.
//!
//! All values start as guesses. Each field is tagged:
//!   [guess]            not measured yet
//!   [measured <date>]  confirmed from a Workshop log (note the take/log file next to it)
//! Change the tag when you confirm a value, so it's always clear what is trusted.

use crate::doomfist::ActionKind;

#[derive(Clone, Debug)]
pub struct Tunables {
    /// Server tick rate. Overwatch servers run at 62.5 Hz (16 ms). [guess]
    pub tick_rate: f32,
    /// Base health. [confirmed by user 2026-11-02] (the test lobby reported 375; check its health settings)
    pub max_health: f32,
    pub movement: MovementTun,
    pub passive: PassiveTun,
    pub block: BlockTun,
    pub punch: PunchTun,
    pub slam: SlamTun,
    pub melee: MeleeTun,
    pub gun: GunTun,
    pub interactions: Interactions,
}

impl Tunables {
    pub fn dt(&self) -> f32 {
        1.0 / self.tick_rate
    }
}

impl Default for Tunables {
    fn default() -> Self {
        Self {
            tick_rate: 62.5,
            max_health: 525.0,
            movement: MovementTun::default(),
            passive: PassiveTun::default(),
            block: BlockTun::default(),
            punch: PunchTun::default(),
            slam: SlamTun::default(),
            melee: MeleeTun::default(),
            gun: GunTun::default(),
            interactions: Interactions::default(),
        }
    }
}

// Movement measured 2026-11-02 from logs/movement-01.txt (walk 4 directions, standing and moving jumps).
#[derive(Clone, Copy, Debug)]
pub struct MovementTun {
    /// Top run speed forward and sideways, m/s. [measured 2026-11-02]
    pub walk_speed: f32,
    /// Top speed multiplier when the input points backward (4.95 / 5.5). [measured 2026-11-02]
    /// Diagonal-backward blends between 1.0 and this. [guess for diagonals]
    pub backward_mult: f32,
    /// Ground acceleration toward the input direction, m/s². [measured 2026-11-02]
    pub ground_accel: f32,
    /// Ground friction: a constant slow-down applied every grounded tick, m/s². [measured 2026-11-02]
    pub ground_friction: f32,
    /// Downward acceleration, m/s². [measured 2026-11-02]
    pub gravity: f32,
    /// Upward speed at the start of a jump, m/s. [measured 2026-11-02]
    pub jump_speed: f32,
    /// In the air, velocity moves toward the input direction at this rate, m/s². [measured 2026-11-02]
    pub air_accel: f32,
    /// On the ground above this speed, extra slow-down of `ground_overspeed_decel` x (speed - this)
    /// per second, before normal friction. Found in the slam slide and again landing from a
    /// jump-cancelled punch. [measured 2026-11-02]
    pub ground_overspeed: f32,
    pub ground_overspeed_decel: f32,
    /// In the air with no input, constant horizontal slow-down, m/s². [measured 2026-11-02]
    pub air_drag: f32,
    /// In the air, speed above `walk_speed` also fades at this rate (per second, times the
    /// excess). Fitted on a jump-cancelled punch. [measured 2026-11-02]
    pub air_excess_decay: f32,
    /// Body radius for wall collision, m. [guess]
    pub collision_radius: f32,
    /// Height of the body's center above the feet, m. Punch hits are traced from here. [guess]
    pub body_center_height: f32,
}

impl Default for MovementTun {
    fn default() -> Self {
        Self {
            walk_speed: 5.5,
            backward_mult: 0.9,
            ground_accel: 192.5,
            ground_friction: 82.5,
            gravity: 17.5,
            jump_speed: 6.0,
            air_accel: 18.75,
            ground_overspeed: 6.2,
            ground_overspeed_decel: 11.0,
            air_drag: 0.54,
            air_excess_decay: 1.925,
            collision_radius: 0.45,
            body_center_height: 1.0,
        }
    }
}

/// Passive: The Best Defense. Ability hits give temporary health.
#[derive(Clone, Copy, Debug)]
pub struct PassiveTun {
    /// Temporary health per enemy hit by an ability. [from the user 2026-11-02]
    /// (Whether a punch's impact and its wall slam count as two hits: to check in the hit tests.)
    pub per_hit: f32,
    /// Temporary health per enemy hit by the ultimate (ultimate not built yet). [from the user]
    pub ult_per_hit: f32,
    /// Extra temporary health as a fraction of damage dealt (0: it's flat per hit). [from the user]
    pub damage_fraction: f32,
    /// Most temporary health you can hold (525 + 200 = 725 max). [from the user]
    pub cap: f32,
    /// Seconds after the last gain before it starts draining; a new hit restarts it.
    /// [from the user 2026-11-02]
    pub decay_delay: f32,
    /// Drain rate once draining, HP/s. [measured 2026-11-02: 1.15 every 0.384 s = 3.0/s, applied
    /// smoothly here]
    pub decay_rate: f32,
}

impl Default for PassiveTun {
    fn default() -> Self {
        Self { per_hit: 40.0, ult_per_hit: 75.0, damage_fraction: 0.0, cap: 200.0, decay_delay: 3.0, decay_rate: 3.0 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BlockTun {
    /// How long Power Block lasts, s. [measured 2026-11-02: active 2.496 s in every test]
    pub duration: f32,
    /// Cooldown after it ends, s. [measured 2026-11-02: 6.99 one tick after a block ended]
    pub cooldown: f32,
    /// Fraction of frontal damage removed (100 in, 25 taken). [measured 2026-11-02]
    pub reduction: f32,
    /// Half-angle of the protected front arc, degrees. [measured 2026-11-02 (hit-02): blocked
    /// at 50°, not at 55°; 52.5 is the middle]
    pub frontal_half_angle_deg: f32,
    /// Damage that must be prevented (not raw) to empower the next punch. [measured 2026-11-02
    /// (hit-02): 98 prevented didn't empower, 100.00 did]
    pub empower_threshold: f32,
    /// Movement speed multiplier while blocking (log: walking settles at 3.575 = 0.65 x 5.5).
    /// [measured 2026-11-02]
    pub move_mult: f32,
    pub can_jump: bool,
}

impl Default for BlockTun {
    fn default() -> Self {
        Self {
            duration: 2.496,
            cooldown: 7.0,
            reduction: 0.75,
            frontal_half_angle_deg: 52.5,
            empower_threshold: 100.0,
            move_mult: 0.65,
            can_jump: true,
        }
    }
}

// Rocket Punch measured 2026-11-02 from logs/punch-01.txt (68 automatic tests: charge sweep,
// pitch, jump at different times, punch -> jump -> slam, turning).
#[derive(Clone, Copy, Debug)]
pub struct PunchTun {
    /// Cooldown once it starts, s. [measured 2026-11-02]
    pub cooldown: f32,
    /// The cooldown starts this long after the dash runs out, s; a jump cancel starts it at once.
    /// [measured 2026-11-02]
    pub cooldown_delay: f32,
    /// Cooldown if the charge is cancelled, s. [guess]
    pub cancel_cooldown: f32,
    /// The dash can't start until the button has been held this long (an early release waits), s.
    /// [measured 2026-11-02]
    pub min_launch_time: f32,
    /// Hold time where charge starts counting, and where it's full, s. [measured 2026-11-02]
    pub charge_start: f32,
    pub charge_full: f32,
    /// Full-speed dash ticks at zero / full charge: ticks = floor(min + (max - min) * charge).
    /// [measured 2026-11-02]
    pub min_dash_ticks: u32,
    pub max_dash_ticks: u32,
    /// Dash speed, m/s. [measured 2026-11-02]
    pub speed: f32,
    /// The first dash tick moves this fraction of a full tick (logged speed 24.64). [measured 2026-11-02]
    pub first_tick_fraction: f32,
    /// Speed left when the dash ends (also what a slam cancel keeps), m/s. [measured 2026-11-02]
    pub exit_speed: f32,
    /// Seconds you can hold at full charge before it fires on its own (None = forever).
    /// [measured 2026-11-02: no auto-fire after 1.6 s]
    pub max_hold_at_full: Option<f32>,
    /// Movement speed multiplier while charging (log: 2.75 m/s). [measured 2026-11-02]
    pub charge_move_mult: f32,
    pub can_jump_while_charging: bool,
    /// While dashing the dash itself never bends, but the camera can look up to this far either
    /// side of it (hits exactly ±59.99° in the log). [measured 2026-11-02, logs/punch-turn-01.txt]
    pub camera_cone_deg: f32,
    /// How far up/down the dash can go. [measured 2026-11-02 on flat ground: always level]
    pub max_pitch_deg: f32,
    /// Holding jump during the dash cancels it into a boosted jump once the dash has run this
    /// long (dash tick 7), s. [measured 2026-11-02: jump held from tick 2 fired on tick 7]
    pub jump_cancel_after: f32,
    /// Boosted jump = exit_speed along the dash + this much toward the camera (last tick's,
    /// clamped to the cone), m/s. Straight ahead that's 30.95 m/s; fits every manual take to
    /// ~0.1°. [measured 2026-11-02, logs/punch-turn-01.txt]
    pub jump_cancel_boost: f32,
    /// Upward speed on the cancel tick = jump_cancel_up + jump_cancel_up_keep x last tick's
    /// vertical speed. Fits 6 player cancels (hop +3.20 -> 5.41, sinking -0.56 -> 4.88, airborne
    /// dashes -1.40 -> 4.76 twice, -5.81 -> 4.16) within 0.012 m/s. [measured 2026-11-02]
    /// (Bots that press and cancel on the same tick log 5.795 instead; see tests/punch_replay.rs.)
    pub jump_cancel_up: f32,
    pub jump_cancel_up_keep: f32,
    /// A jump before `jump_cancel_after` only lifts Doomfist this much on the jump tick; he holds
    /// it one tick, then falls back under normal gravity while the dash goes on, m.
    /// [measured 2026-11-02: same in all 6 early-jump takes]
    pub early_jump_hop: f32,
    /// Extra radius of the fist when checking for hits, m. [guess]
    pub hit_radius: f32,
    // Hits measured 2026-11-02 from logs/hit-01.txt (automatic hit tests, Ana and Reinhardt).
    /// Damage and knockback scale with this charge, counted from `damage_charge_start` to
    /// `damage_charge_full` seconds of hold (different from the dash-length charge window).
    /// [measured: 0.256 s -> 28.5, 0.512 -> 44.5, 0.752 -> 59.5, 1.008 -> 75]
    pub damage_charge_start: f32,
    pub damage_charge_full: f32,
    /// Impact damage at zero / full charge. [measured 2026-11-02]
    pub min_impact_damage: f32,
    pub max_impact_damage: f32,
    /// Wall-slam damage at zero / full charge. [measured 2026-11-02]
    pub min_wall_damage: f32,
    pub max_wall_damage: f32,
    /// Knockback speed given to the target at zero / full charge, m/s. [measured 2026-11-02;
    /// Reinhardt took 0.6x: set `Target::knockback_scale` for tanks]
    pub min_knockback: f32,
    pub max_knockback: f32,
    /// Upward knockback added to the target, m/s. [measured 2026-11-02]
    pub knockback_up: f32,
    /// Upper limit on how long a wall hit still counts as a wall slam, s. In the logs any wall
    /// reached while still sliding from the punch counts; the slide itself (World's knockback
    /// rules) decides the range: 13.23 m at full charge, 5.5 m at minimum. [measured 2026-11-02]
    pub wall_slam_window: f32,
    // --- Empowered punch, measured 2026-11-02 ---
    /// Speed and damage multipliers (66 m/s; 112.5 impact and 60 wall at full charge).
    pub empowered_speed_mult: f32,
    pub empowered_damage_mult: f32,
    /// Knockback multiplier (35 m/s, same as normal).
    pub empowered_knockback_mult: f32,
    /// Full-speed dash ticks at zero / full charge (normal: 8 / 28).
    pub empowered_min_ticks: u32,
    pub empowered_max_ticks: u32,
    /// A normal punch also hits other enemies within this distance of the one it connects with,
    /// m. [measured 2026-11-02 (gun-01 suite 45): 2 m away hit, 3 m not]
    pub splash_radius: f32,
    /// Other enemies within this distance of the main target are also hit, m. [measured
    /// 2026-11-02 (hit-02): 3 m away hit, 4 m not; 3.5 is the middle]
    pub empowered_splash_radius: f32,
}

impl Default for PunchTun {
    fn default() -> Self {
        Self {
            cooldown: 4.0,
            cooldown_delay: 0.368,
            cancel_cooldown: 1.0,
            min_launch_time: 0.208,
            charge_start: 0.144,
            charge_full: 0.992,
            min_dash_ticks: 8,
            max_dash_ticks: 28,
            speed: 44.0,
            first_tick_fraction: 0.56,
            exit_speed: 6.0,
            max_hold_at_full: None,
            charge_move_mult: 0.5,
            can_jump_while_charging: true,
            camera_cone_deg: 60.0,
            max_pitch_deg: 0.0,
            jump_cancel_after: 0.096,
            jump_cancel_boost: 24.947,
            jump_cancel_up: 4.96,
            jump_cancel_up_keep: 0.139,
            early_jump_hop: 0.051,
            hit_radius: 0.5,
            damage_charge_start: 0.2,
            damage_charge_full: 1.0,
            min_impact_damage: 25.0,
            max_impact_damage: 75.0,
            min_wall_damage: 10.0,
            max_wall_damage: 40.0,
            min_knockback: 15.0,
            max_knockback: 35.0,
            knockback_up: 1.0,
            wall_slam_window: 2.0,
            empowered_speed_mult: 1.5,
            empowered_damage_mult: 1.5,
            empowered_knockback_mult: 1.0,
            empowered_min_ticks: 8,
            empowered_max_ticks: 26,
            splash_radius: 2.5,
            empowered_splash_radius: 3.5,
        }
    }
}

#[derive(Clone, Copy, Debug)]
// Slam leap measured 2026-11-02 from logs/slam-01.txt (58 automatic takes, see slam.rs).
pub struct SlamTun {
    /// Starts when the slam ends (after the slide). Log shows 6.496 one tick later. [measured 2026-11-02]
    /// Pressing slam again this long into the leap cancels it, s. [from the user 2026-11-02;
    /// what you keep afterwards (speed, cooldown) is a guess: full speed, normal cooldown]
    pub recast_cancel_after: f32,
    pub cooldown: f32,
    /// Launch speed along the leap direction, m/s. Same on the ground and in the air. [measured 2026-11-02]
    pub launch_speed: f32,
    /// Leap direction = aim + UP * this, before the elevation clamp. [measured 2026-11-02]
    pub aim_lift: f32,
    /// Leap elevation is clamped to this range, degrees (aim below ~15° gives 20°). [measured 2026-11-02]
    pub min_elevation_deg: f32,
    pub max_elevation_deg: f32,
    /// Horizontal speed before the slam is kept and added (vertical is replaced).
    /// [measured 2026-11-02 for running straight forward; sideways momentum untested]
    pub keep_horizontal_momentum: bool,
    /// Gravity multiplier during the leap (35 m/s²). [measured 2026-11-02]
    pub gravity_mult: f32,
    /// Air: horizontal speed moves toward `air_decay_floor` at this rate, 1/s. [measured 2026-11-02]
    pub air_decay_rate: f32,
    pub air_decay_floor: f32,
    /// Air, falling only: upward drag = this × vy², 1/m (terminal speed ~30 m/s). [measured 2026-11-02]
    /// Steering during the leap, as a fraction of normal air acceleration: keys pointing partly
    /// sideways turn the leap toward them without changing its speed. [measured 2026-11-02 from a
    /// player's take: 1.0 fits the turn rate; the bot never steered sideways]
    pub air_control: f32,
    pub fall_drag: f32,
    /// Slam stays active this long after touchdown (19 ticks), sliding on the ground. [measured 2026-11-02]
    pub slide_time: f32,
    /// Slide: above this speed, extra slow-down of `slide_overspeed_decel` × (speed − this) per second.
    /// Now applied by general ground movement (`MovementTun::ground_overspeed`); kept for reference.
    /// on top of normal ground friction. Input still steers. [measured 2026-11-02]
    /// (May be general ground behaviour, not slam-only: untested outside slam.)
    pub slide_overspeed: f32,
    pub slide_overspeed_decel: f32,
    /// Holding jump through touchdown bunny hops: jump at once, keep horizontal speed, no slide.
    /// [described by the user 2026-11-02; exact jump speed and speed kept are guesses]
    pub bhop_on_held_jump: bool,
    /// Can jump during the slide. [guess]
    pub slide_can_jump: bool,
    /// Leap ends without damage after this long in the air (never seen: longest leap 1.7 s). [guess]
    pub max_duration: f32,
    /// Damage per enemy hit. [measured 2026-11-02]
    pub damage: f32,
    /// Hit area on landing: a cone ahead of the landing point, from `hit_ahead_min` to
    /// `hit_ahead_max` m ahead, `hit_cone_half_angle_deg` either side. [measured 2026-11-02
    /// (hit-01, hit-02): 0.5 m ahead missed, 1 m hit; hits out to 14 m (farther untested, 15 is a
    /// guess); 3 m aside at 4 m ahead (37°) hit, 3 m aside at 3 m ahead (45°) missed. In Overwatch
    /// the hits also land later the farther out they are (~42 m/s wave); here they're instant]
    pub hit_ahead_min: f32,
    pub hit_ahead_max: f32,
    pub hit_cone_half_angle_deg: f32,
    pub vertical_reach: f32,
    /// Knockback on landing, m/s. [measured 2026-11-02: hit dummies didn't move]
    pub knockback: f32,
    pub knockback_up: f32,
}

impl Default for SlamTun {
    fn default() -> Self {
        Self {
            recast_cancel_after: 0.5,
            cooldown: 6.5,
            launch_speed: 30.0,
            aim_lift: 0.1,
            min_elevation_deg: 20.0,
            max_elevation_deg: 70.0,
            keep_horizontal_momentum: true,
            gravity_mult: 2.0,
            air_decay_rate: 0.9626,
            air_decay_floor: 5.217,
            air_control: 1.0,
            fall_drag: 0.03889,
            slide_time: 0.304,
            slide_overspeed: 6.2,
            slide_overspeed_decel: 11.0,
            bhop_on_held_jump: true,
            slide_can_jump: false,
            max_duration: 4.0,
            damage: 60.0,
            hit_ahead_min: 0.75,
            hit_ahead_max: 15.0,
            hit_cone_half_angle_deg: 41.0,
            vertical_reach: 2.5,
            knockback: 0.0,
            knockback_up: 0.0,
        }
    }
}

/// Quick melee, measured 2026-11-02 (logs/gun-01.txt, suites 46-47).
#[derive(Clone, Copy, Debug)]
pub struct MeleeTun {
    /// [measured]
    pub damage: f32,
    /// Reach from Doomfist's center to the target's edge, m. [partly measured: a dummy 2.5 m
    /// away was hit, 3.5 m not]
    pub range: f32,
    /// Half-angle of the swing, degrees. [partly measured: 30° hit, 60° not]
    pub half_angle_deg: f32,
    /// Seconds before the next melee. [measured: ready again 1.0 s after a hit]
    pub cooldown: f32,
    /// A press this early still goes through when the cooldown ends, s. [partly measured: 0.1 s
    /// early went through, 0.3 s early didn't]
    pub buffer: f32,
    /// Flat knockback on the target, m/s (with the normal knocked state). [measured: 1 m/s,
    /// target slid 0.30 m]
    pub knockback: f32,
}

impl Default for MeleeTun {
    fn default() -> Self {
        Self { damage: 40.0, range: 2.6, half_angle_deg: 45.0, cooldown: 1.0, buffer: 0.2, knockback: 1.0 }
    }
}

/// Hand Cannon, measured 2026-11-02 (logs/gun-01.txt, suites 40-44).
#[derive(Clone, Copy, Debug)]
pub struct GunTun {
    /// Shots in a full clip. [measured]
    pub clip: u32,
    /// Seconds between shots while holding fire. [measured: 0.32-0.336]
    pub fire_interval: f32,
    /// The first shell comes back this long after the last shot, then one every
    /// `refill_interval`, s. The reload button does nothing. [measured]
    pub refill_delay: f32,
    pub refill_interval: f32,
    /// Pellets per shot, damage each, headshot multiplier. [measured: 11 x 5, 2x]
    pub pellets: u32,
    pub pellet_damage: f32,
    pub headshot_mult: f32,
    /// Pellet speed, m/s. [measured: ~78 from hit times at 3-30 m]
    pub pellet_speed: f32,
    /// Damage is full up to `falloff_start` m, then falls in a straight line to `falloff_min`
    /// of full at `falloff_end` m. [measured: 77% at 20 m, 31% at 30 m]
    pub falloff_start: f32,
    pub falloff_end: f32,
    pub falloff_min: f32,
    /// Pellets disappear after this far, m. [guess]
    pub max_range: f32,
    /// Ring of pellets around the centre one, this many degrees out. [partly measured: fits how
    /// many of 11 hit an Ana at 10, 15 and 20 m (9, 5, 5 vs logged 8, 6, 5)]
    pub spread_deg: f32,
    /// Pellets start this high above the feet, m. [guess]
    pub muzzle_height: f32,
}

impl Default for GunTun {
    fn default() -> Self {
        Self {
            clip: 4,
            fire_interval: 0.336,
            refill_delay: 0.72,
            refill_interval: 0.40,
            pellets: 11,
            pellet_damage: 5.0,
            headshot_mult: 2.0,
            pellet_speed: 78.0,
            falloff_start: 15.0,
            falloff_end: 30.0,
            falloff_min: 0.3,
            max_range: 60.0,
            spread_deg: 2.6,
            muzzle_height: 1.5,
        }
    }
}

/// How the abilities affect each other. Every entry is a guess until a combo test confirms it.
#[derive(Clone, Debug)]
pub struct Interactions {
    /// (current action, new action) pairs where the new one may cut the current one short.
    /// Starting an ability when nothing is active is always allowed (if off cooldown).
    pub allowed_cancels: Vec<(ActionKind, ActionKind)>,
    /// Does a Seismic Slam that hits someone empower the next punch? [guess]
    pub slam_hit_empowers: bool,
    /// Does Power Block empower as soon as the threshold is reached (vs. when the block ends)? [guess]
    pub block_empowers_immediately: bool,
    /// Seconds the empowered punch stays ready (None = until used). [guess]
    pub empowered_duration: Option<f32>,
}

impl Default for Interactions {
    fn default() -> Self {
        use ActionKind::*;
        Self {
            allowed_cancels: vec![
                (Block, PunchCharge),  // [guess] punch out of Power Block
                (PunchCharge, Slam),   // [guess] slam cancels a punch charge
                (Slam, PunchCharge),   // [guess] punch out of slam mid-air
                (Punch, Slam),         // [measured 2026-11-02] slam cuts the dash, keeping exit_speed
            ],
            slam_hit_empowers: false,
            block_empowers_immediately: false,
            empowered_duration: None,
        }
    }
}
