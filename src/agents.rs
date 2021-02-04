mod run_agent;
pub mod search_agent;

use uom::si::f32::{Angle, Length};

pub use run_agent::{RunAgent, RunAgentError, RunTrajectoryGenerator};

pub trait StateEstimator<Diff> {
    type State;

    fn init(&mut self);
    fn estimate(&mut self);
    fn state(&self) -> &Self::State;
    fn correct_state<Diffs: IntoIterator<Item = Diff>>(&mut self, diffs: Diffs);
}

pub trait Tracker<State, Target> {
    type Error;

    fn init(&mut self);
    fn track(&mut self, state: &State, target: &Target) -> Result<(), Self::Error>;
    fn stop(&mut self);
}

pub trait Robot<Target> {
    type Error;

    fn track_and_update(&mut self, target: &Target) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Pose {
    pub x: Length,
    pub y: Length,
    pub theta: Angle,
}

impl Pose {
    pub fn new(x: Length, y: Length, theta: Angle) -> Self {
        Self { x, y, theta }
    }
}
