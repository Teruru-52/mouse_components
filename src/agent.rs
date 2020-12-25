use core::cell::{Cell, RefCell};

use heapless::{consts::*, spsc::Queue};
use uom::si::f32::{Angle, Length};

use crate::operators::{RunAgent, SearchAgent};
use crate::utils::mutex::Mutex;

pub trait ObstacleDetector<State> {
    type Obstacle;
    type Obstacles: IntoIterator<Item = Self::Obstacle>;

    fn detect(&mut self, state: &State) -> Self::Obstacles;
}

pub trait StateEstimator {
    type State;

    fn init(&mut self);
    fn estimate(&mut self) -> Self::State;
}

pub trait SearchTrajectoryGenerator<Pose, Kind> {
    type Target;
    type Trajectory: Iterator<Item = Self::Target>;

    fn generate_search(&self, pose: &Pose, kind: &Kind) -> Self::Trajectory;
}

pub trait Tracker<State, Target> {
    fn init(&mut self);
    fn track(&mut self, state: &State, target: &Target);
    fn stop(&mut self);
}

pub trait RunTrajectoryGenerator<Command> {
    type Target;
    type Trajectory: Iterator<Item = Self::Target>;

    fn generate<Commands: IntoIterator<Item = Command>>(
        &self,
        commands: Commands,
    ) -> Self::Trajectory;
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

//TODO: separate Agent to SearchAgent and RunAgent with AgentInner
//Initialize RunAgent from SearchAgent by implementing From<SearchAgent>
pub struct Agent<
    IObstacleDetector,
    IStateEstimator,
    ITracker,
    ITrajectoryGenerator,
    Trajectory,
    Target,
> {
    obstacle_detector: RefCell<IObstacleDetector>,
    state_estimator: RefCell<IStateEstimator>,
    tracker: RefCell<ITracker>,
    trajectory_generator: ITrajectoryGenerator,
    trajectories: Mutex<Queue<Trajectory, U3>>,
    run_trajectory: RefCell<Option<Trajectory>>,
    last_target: Cell<Option<Target>>,
}

impl<IObstacleDetector, IStateEstimator, ITracker, ITrajectoryGenerator, Trajectory, Target>
    Agent<IObstacleDetector, IStateEstimator, ITracker, ITrajectoryGenerator, Trajectory, Target>
{
    pub fn new(
        obstacle_detector: IObstacleDetector,
        state_estimator: IStateEstimator,
        tracker: ITracker,
        trajectory_generator: ITrajectoryGenerator,
    ) -> Self {
        Self {
            obstacle_detector: RefCell::new(obstacle_detector),
            state_estimator: RefCell::new(state_estimator),
            tracker: RefCell::new(tracker),
            trajectory_generator,
            trajectories: Mutex::new(Queue::new()),
            run_trajectory: RefCell::new(None),
            last_target: Cell::new(None),
        }
    }
}

impl<IObstacleDetector, IStateEstimator, ITracker, ITrajectoryGenerator, Trajectory, Target>
    Agent<IObstacleDetector, IStateEstimator, ITracker, ITrajectoryGenerator, Trajectory, Target>
where
    ITracker: Tracker<IStateEstimator::State, Target>,
    IStateEstimator: StateEstimator,
{
    pub fn stop(&self) {
        self.tracker.borrow_mut().stop();
    }
}

impl<Pose, Kind, IObstacleDetector, IStateEstimator, ITracker, ITrajectoryGenerator>
    SearchAgent<(Pose, Kind)>
    for Agent<
        IObstacleDetector,
        IStateEstimator,
        ITracker,
        ITrajectoryGenerator,
        ITrajectoryGenerator::Trajectory,
        ITrajectoryGenerator::Target,
    >
where
    Pose: Copy,
    ITrajectoryGenerator::Target: Copy,
    IObstacleDetector: ObstacleDetector<IStateEstimator::State>,
    IStateEstimator: StateEstimator,
    ITracker: Tracker<IStateEstimator::State, ITrajectoryGenerator::Target>,
    ITrajectoryGenerator: SearchTrajectoryGenerator<Pose, Kind>,
{
    type Error = ();
    type Obstacle = IObstacleDetector::Obstacle;
    type Obstacles = IObstacleDetector::Obstacles;

    fn get_obstacles(&self) -> Self::Obstacles {
        let state = self.state_estimator.borrow_mut().estimate();
        self.obstacle_detector.borrow_mut().detect(&state)
    }

    fn set_command(&self, command: &(Pose, Kind)) {
        let trajectory = self
            .trajectory_generator
            .generate_search(&command.0, &command.1);
        let mut trajectories = self.trajectories.lock();
        trajectories.enqueue(trajectory).ok();
    }

    fn track_next(&self) -> Result<(), Self::Error> {
        let state = self.state_estimator.borrow_mut().estimate();
        let target = {
            if let Ok(mut trajectories) = self.trajectories.try_lock() {
                loop {
                    if let Some(trajectory) = trajectories.iter_mut().next() {
                        if let Some(target) = trajectory.next() {
                            break Some(target);
                        }
                    } else {
                        break self.last_target.get();
                    }
                    trajectories.dequeue();
                }
            } else {
                self.last_target.get()
            }
        };
        if let Some(target) = target {
            self.tracker.borrow_mut().track(&state, &target);
            self.last_target.set(Some(target));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrackFinishError;

impl<Command, IObstacleDetector, IStateEstimator, ITracker, ITrajectoryGenerator> RunAgent<Command>
    for Agent<
        IObstacleDetector,
        IStateEstimator,
        ITracker,
        ITrajectoryGenerator,
        ITrajectoryGenerator::Trajectory,
        ITrajectoryGenerator::Target,
    >
where
    IObstacleDetector: ObstacleDetector<IStateEstimator::State>,
    IStateEstimator: StateEstimator,
    ITracker: Tracker<IStateEstimator::State, ITrajectoryGenerator::Target>,
    ITrajectoryGenerator: RunTrajectoryGenerator<Command>,
{
    type Error = TrackFinishError;

    fn set_commands<Commands: IntoIterator<Item = Command>>(&self, commands: Commands) {
        self.run_trajectory
            .replace(Some(self.trajectory_generator.generate(commands)));
    }

    fn track_next(&self) -> Result<(), Self::Error> {
        use core::ops::DerefMut;

        if let Some(trajectory) = self.run_trajectory.borrow_mut().deref_mut() {
            if let Some(target) = trajectory.next() {
                let state = self.state_estimator.borrow_mut().estimate();
                self.tracker.borrow_mut().track(&state, &target);
                return Ok(());
            }
        }
        Err(TrackFinishError)
    }
}
