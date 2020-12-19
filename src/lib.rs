#![cfg_attr(not(test), no_std)]
#![feature(type_alias_impl_trait)]

extern crate alloc;

mod administrator;
mod agent;
mod controller;
mod estimator;
mod maze;
mod obstacle_detector;
pub mod operators;
mod pattern;
pub mod prelude;
mod solver;
mod tracker;
mod trajectory_generator;
pub mod utils;

pub mod traits {
    use super::*;

    pub use administrator::{
        Agent, DirectionInstructor, Graph, GraphConverter, NodeConverter, ObstacleInterpreter,
        Operator, Solver,
    };
    pub use agent::{ObstacleDetector, StateEstimator, Tracker, TrajectoryGenerator};
    pub use tracker::{Logger, RotationController, TranslationController};
    pub use utils::math::Math;
}

pub mod sensors {
    use super::*;

    pub use estimator::{Encoder, IMU};
    pub use obstacle_detector::distance_sensor::DistanceSensor;
    pub use tracker::Motor;
}

pub mod data_types {
    use super::*;

    pub use administrator::{FastRun, Idle, Search, Select};
    pub use agent::Pose;
    pub use maze::{
        AbsoluteDirection, NodeId, Position, RelativeDirection, SearchNodeId, WallDirection,
        WallPosition,
    };
    pub use obstacle_detector::Obstacle;
    pub use pattern::Pattern;
    pub use tracker::{AngleState, LengthState, State};
    pub use trajectory_generator::{AngleTarget, LengthTarget, Target};
}

pub mod impls {
    use super::*;

    pub use administrator::operator::SearchOperator;
    pub use administrator::Administrator;
    pub use agent::Agent;
    pub use controller::{
        RotationController, RotationControllerBuilder, TranslationController,
        TranslationControllerBuilder,
    };
    pub use estimator::{Estimator, EstimatorBuilder};
    pub use maze::{Maze, MazeBuilder};
    pub use obstacle_detector::ObstacleDetector;
    pub use solver::Solver;
    pub use tracker::{NullLogger, Tracker, TrackerBuilder};
    pub use trajectory_generator::{TrajectoryGenerator, TrajectoryGeneratorBuilder};
}

pub mod defaults {
    use super::{data_types::*, impls::*};

    pub type DefaultTracker<LeftMotor, RightMotor, Logger, Math> =
        Tracker<LeftMotor, RightMotor, TranslationController, RotationController, Logger, Math>;

    pub type DefaultAgent<
        LeftMotor,
        RightMotor,
        LeftEncoder,
        RightEncoder,
        Imu,
        DistanceSensor,
        DistanceSensorNum,
        Logger,
        Math,
    > = Agent<
        State,
        Target,
        Pose,
        Obstacle,
        RelativeDirection,
        ObstacleDetector<DistanceSensor, DistanceSensorNum>,
        Estimator<LeftEncoder, RightEncoder, Imu, Math>,
        DefaultTracker<LeftMotor, RightMotor, Logger, Math>,
        TrajectoryGenerator<Math>,
    >;

    pub type DefaultMaze<MazeWidth, Math> = Maze<MazeWidth, fn(Pattern) -> u16, Math>;

    pub type DefaultSolver<MazeWidth, MaxSize, GoalSize> =
        Solver<NodeId<MazeWidth>, SearchNodeId<MazeWidth>, MaxSize, GoalSize>;

    pub type DefaultSearchOperator<
        LeftMotor,
        RightMotor,
        LeftEncoder,
        RightEncoder,
        Imu,
        DistanceSensor,
        DistanceSensorNum,
        MazeWidth,
        MaxSize,
        GoalSize,
        Logger,
        Math,
    > = SearchOperator<
        NodeId<MazeWidth>,
        SearchNodeId<MazeWidth>,
        u16,
        RelativeDirection,
        Obstacle,
        Pose,
        DefaultMaze<MazeWidth, Math>,
        DefaultAgent<
            LeftMotor,
            RightMotor,
            LeftEncoder,
            RightEncoder,
            Imu,
            DistanceSensor,
            DistanceSensorNum,
            Logger,
            Math,
        >,
        DefaultSolver<MazeWidth, MaxSize, GoalSize>,
    >;
}
