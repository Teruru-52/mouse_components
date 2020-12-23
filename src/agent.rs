use core::cell::{Cell, RefCell};
use core::marker::PhantomData;

use heapless::{consts::*, spsc::Queue};
use uom::si::f32::{Angle, Length};

use crate::operators::search_operator::SearchAgent;
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

pub struct Agent<
    IObstacleDetector,
    IStateEstimator,
    ITracker,
    ITrajectoryGenerator,
    Pose = crate::data_types::Pose,
    Kind = crate::data_types::SearchKind,
> where
    IObstacleDetector: ObstacleDetector<IStateEstimator::State>,
    IStateEstimator: StateEstimator,
    ITracker: Tracker<IStateEstimator::State, ITrajectoryGenerator::Target>,
    ITrajectoryGenerator: SearchTrajectoryGenerator<Pose, Kind>,
{
    obstacle_detector: RefCell<IObstacleDetector>,
    state_estimator: RefCell<IStateEstimator>,
    tracker: RefCell<ITracker>,
    trajectory_generator: ITrajectoryGenerator,
    trajectories: Mutex<Queue<ITrajectoryGenerator::Trajectory, U3>>,
    last_target: Cell<Option<ITrajectoryGenerator::Target>>,
    _pose: PhantomData<fn() -> Pose>,
    _direction: PhantomData<fn() -> Kind>,
}

impl<Pose, Direction, IObstacleDetector, IStateEstimator, ITracker, ITrajectoryGenerator>
    Agent<IObstacleDetector, IStateEstimator, ITracker, ITrajectoryGenerator, Pose, Direction>
where
    IObstacleDetector: ObstacleDetector<IStateEstimator::State>,
    IStateEstimator: StateEstimator,
    ITracker: Tracker<IStateEstimator::State, ITrajectoryGenerator::Target>,
    ITrajectoryGenerator: SearchTrajectoryGenerator<Pose, Direction>,
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
            last_target: Cell::new(None),
            _pose: PhantomData,
            _direction: PhantomData,
        }
    }

    pub fn stop(&self) {
        self.tracker.borrow_mut().stop();
    }
}

impl<Pose, Kind, IObstacleDetector, IStateEstimator, ITracker, ITrajectoryGenerator>
    SearchAgent<(Pose, Kind)>
    for Agent<IObstacleDetector, IStateEstimator, ITracker, ITrajectoryGenerator, Pose, Kind>
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
