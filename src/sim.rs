//! Runs Doomfist and the world together, tick by tick.

use crate::doomfist::Doomfist;
use crate::events::Event;
use crate::input::Input;
use crate::math::Vec3;
use crate::tunables::Tunables;
use crate::world::World;

#[derive(Clone, Debug)]
pub struct Sim {
    pub doom: Doomfist,
    pub world: World,
    /// Every event so far, with the time it happened.
    pub history: Vec<(f32, Event)>,
}

impl Sim {
    pub fn new(tun: Tunables, start: Vec3) -> Self {
        let world = World::flat(tun.movement.gravity);
        Self { doom: Doomfist::new(start, tun), world, history: Vec::new() }
    }

    pub fn step(&mut self, input: &Input) -> Vec<Event> {
        let dt = self.doom.tun.dt();
        let mut events = self.doom.tick(input, &mut self.world);
        self.world.step(dt, &mut events);
        self.doom.passive_on_events(&mut events);
        let t = self.doom.time;
        self.history.extend(events.iter().map(|e| (t, *e)));
        events
    }

    /// Repeats the same input for `seconds`.
    pub fn run(&mut self, input: &Input, seconds: f32) -> Vec<Event> {
        let ticks = (seconds * self.doom.tun.tick_rate).round() as usize;
        (0..ticks).flat_map(|_| self.step(input)).collect()
    }
}
