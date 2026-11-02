//! Doomfist's shared state and the per-tick order in which abilities run.
//!
//! Each ability lives in its own file under `abilities/` and adds methods to `Doomfist`.
//! They interact through this shared state:
//!   * only one action (punch charge, punch, slam, block) runs at a time; whether a new one can
//!     cut in is decided by `Interactions::allowed_cancels`
//!   * Power Block sets `empowered`, Rocket Punch consumes it
//!   * every ability hit is an event the passive turns into temporary health
//!   * each action tells movement how Doomfist moves this tick (`MoveMode`)

use crate::abilities::block::BlockState;
use crate::abilities::movement::MoveMode;
use crate::abilities::punch::PunchState;
use crate::abilities::slam::SlamState;
use crate::events::{EndReason, Event};
use crate::input::Input;
use crate::math::Vec3;
use crate::tunables::Tunables;
use crate::world::World;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionKind {
    PunchCharge,
    Punch,
    Slam,
    Block,
}

#[derive(Clone, Debug)]
pub enum Action {
    Idle,
    /// `t` = seconds held so far; `held` = how long the button was actually held, once released
    /// (the dash may still be waiting for the minimum launch time).
    PunchCharge { t: f32, held: Option<f32> },
    Punch(PunchState),
    Slam(SlamState),
    Block(BlockState),
}

impl Action {
    pub fn kind(&self) -> Option<ActionKind> {
        match self {
            Action::Idle => None,
            Action::PunchCharge { .. } => Some(ActionKind::PunchCharge),
            Action::Punch(_) => Some(ActionKind::Punch),
            Action::Slam(_) => Some(ActionKind::Slam),
            Action::Block(_) => Some(ActionKind::Block),
        }
    }
}

/// Hand Cannon state.
#[derive(Clone, Copy, Debug)]
pub struct GunState {
    pub ammo: u32,
    /// Seconds until the next shot is allowed.
    pub cooldown: f32,
    /// Seconds until the next shell comes back (while not full).
    pub refill: f32,
    pub since_shot: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Cooldowns {
    pub punch: f32,
    pub slam: f32,
    pub block: f32,
    pub melee: f32,
}

/// Buttons that went down this tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pressed {
    pub melee: bool,
    pub jump: bool,
    pub secondary: bool,
    pub ability1: bool,
    pub ability2: bool,
}

impl Pressed {
    pub fn new(prev: &Input, now: &Input) -> Self {
        Self {
            melee: now.melee && !prev.melee,
            jump: now.jump && !prev.jump,
            secondary: now.secondary && !prev.secondary,
            ability1: now.ability1 && !prev.ability1,
            ability2: now.ability2 && !prev.ability2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Doomfist {
    pub tun: Tunables,
    pub time: f32,
    /// Feet position.
    pub pos: Vec3,
    pub vel: Vec3,
    pub facing: Vec3,
    pub grounded: bool,
    pub health: f32,
    /// Temporary health from the passive.
    pub overhealth: f32,
    /// Seconds since overhealth was last gained.
    pub overhealth_idle: f32,
    pub cooldowns: Cooldowns,
    pub empowered: bool,
    pub empowered_timer: f32,
    pub gun: GunState,
    /// Seconds since the last quick melee (for animation); large when none recently.
    pub melee_anim: f32,
    /// A melee press waiting for the cooldown to end (pressed within the buffer window).
    pub melee_queued: bool,
    /// Slam was pressed last tick; Overwatch launches it one tick after the press.
    pub slam_queued: bool,
    pub action: Action,
    pub prev_input: Input,
    pending: Vec<Event>,
}

impl Doomfist {
    pub fn new(pos: Vec3, tun: Tunables) -> Self {
        let clip = tun.gun.clip;
        Self {
            health: tun.max_health,
            tun,
            time: 0.0,
            pos,
            vel: Vec3::ZERO,
            facing: Vec3::FORWARD,
            grounded: pos.y <= 0.0,
            overhealth: 0.0,
            overhealth_idle: 0.0,
            cooldowns: Cooldowns::default(),
            empowered: false,
            empowered_timer: 0.0,
            slam_queued: false,
            gun: GunState { ammo: clip, cooldown: 0.0, refill: 0.0, since_shot: 99.0 },
            melee_anim: 99.0,
            melee_queued: false,
            action: Action::Idle,
            prev_input: Input::default(),
            pending: Vec::new(),
        }
    }

    pub fn current(&self) -> Option<ActionKind> {
        self.action.kind()
    }

    pub(crate) fn emit(&mut self, e: Event) {
        self.pending.push(e);
    }

    /// Can `new` start right now, given what's already running?
    pub(crate) fn can_start(&self, new: ActionKind) -> bool {
        match self.action.kind() {
            None => true,
            Some(cur) => self.tun.interactions.allowed_cancels.contains(&(cur, new)),
        }
    }

    /// Stops the current action because another one is replacing it.
    pub(crate) fn interrupt_current(&mut self) {
        match std::mem::replace(&mut self.action, Action::Idle) {
            Action::Idle => {}
            Action::PunchCharge { .. } => self.punch_charge_cancelled(),
            Action::Punch(s) => {
                // A cut-short dash leaves the normal exit speed behind (a slam adds to it).
                self.vel = s.dir * self.tun.punch.exit_speed;
                self.punch_finish(EndReason::Cancelled);
            }
            Action::Slam(_) => self.slam_finish(EndReason::Cancelled),
            Action::Block(s) => self.block_finish(s, EndReason::Cancelled),
        }
    }

    pub(crate) fn empower(&mut self) {
        if !self.empowered {
            self.empowered = true;
            self.empowered_timer = 0.0;
            self.emit(Event::Empowered);
        }
    }

    /// Runs one server tick. Returns what happened (including damage taken since last tick).
    /// The passive is applied by `Sim::step`, after targets have moved, so wall slams count too.
    pub fn tick(&mut self, input: &Input, world: &mut World) -> Vec<Event> {
        let dt = self.tun.dt();
        self.time += dt;
        let f = input.facing.normalize_or_zero();
        if f != Vec3::ZERO {
            self.facing = f;
        }

        self.tick_timers(dt);
        self.passive_tick(dt);

        // Start abilities. If several are pressed on one tick, later ones win only if allowed
        // to cancel earlier ones.
        let pressed = Pressed::new(&self.prev_input, input);
        self.block_input(&pressed);
        self.slam_input(&pressed);
        self.punch_input(input, &pressed);
        self.melee_input(&pressed, world);
        self.gun_update(input, world, dt);

        let mode = self.update_action(input, world, dt);
        self.movement_step(input, &pressed, mode, world, dt);
        self.slam_after_move(world, input);

        self.prev_input = *input;
        std::mem::take(&mut self.pending)
    }

    fn tick_timers(&mut self, dt: f32) {
        let c = &mut self.cooldowns;
        c.punch = (c.punch - dt).max(0.0);
        c.slam = (c.slam - dt).max(0.0);
        c.block = (c.block - dt).max(0.0);
        c.melee = (c.melee - dt).max(0.0);
        self.melee_anim += dt;

        if self.empowered {
            self.empowered_timer += dt;
            if let Some(limit) = self.tun.interactions.empowered_duration {
                if self.empowered_timer >= limit {
                    self.empowered = false;
                    self.emit(Event::EmpowerExpired);
                }
            }
        }
    }

    fn update_action(&mut self, input: &Input, world: &mut World, dt: f32) -> MoveMode {
        let (next, mode) = match std::mem::replace(&mut self.action, Action::Idle) {
            Action::Idle => (Action::Idle, MoveMode::normal()),
            Action::PunchCharge { t, held } => self.punch_charge_update(t, held, input, dt),
            Action::Punch(s) => self.punch_update(s, input, world, dt),
            Action::Slam(s) => self.slam_update(s, input, dt),
            Action::Block(s) => self.block_update(s, dt),
        };
        self.action = next;
        mode
    }
}
