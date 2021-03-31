use core::marker::PhantomData;

use uom::si::angle::degree;
use uom::si::f32::{Angle, AngularAcceleration, AngularJerk, AngularVelocity, Time};

use super::spin_generator::{SpinGenerator, SpinTrajectory};
use super::Target;
use crate::nodes::RotationKind;
use crate::trajectory_managers::TrackingTrajectoryGenerator;
use crate::utils::builder::{ok_or, RequiredFieldEmptyError};
use crate::utils::math::{LibmMath, Math};

/// An implementation of
/// [TrackingTrajectoryGenerator](crate::trajectory_managers::TrackingTrajectoryGenerator).
pub struct ReturnSetupTrajectoryGenerator<M> {
    spin_generator: SpinGenerator<M>,
}

impl<M> ReturnSetupTrajectoryGenerator<M> {
    pub fn new(
        max_angular_velocity: AngularVelocity,
        max_angular_acceleration: AngularAcceleration,
        max_angular_jerk: AngularJerk,
        period: Time,
    ) -> Self {
        let spin_generator = SpinGenerator::<M>::new(
            max_angular_velocity,
            max_angular_acceleration,
            max_angular_jerk,
            period,
        );
        Self { spin_generator }
    }
}

impl<M: Math> TrackingTrajectoryGenerator<RotationKind> for ReturnSetupTrajectoryGenerator<M> {
    type Target = Target;
    type Trajectory = SpinTrajectory;

    fn generate(&self, command: &RotationKind) -> Self::Trajectory {
        use RotationKind::*;

        let theta = match command {
            Front => Default::default(),
            Right => Angle::new::<degree>(-90.0),
            Left => Angle::new::<degree>(90.0),
            Back => Angle::new::<degree>(180.0),
        };
        self.spin_generator.generate(Default::default(), theta)
    }
}

/// A builder for [ReturnSetupTrajectoryGenerator](ReturnSetupTrajectoryGenerator).
pub struct ReturnSetupTrajectoryGeneratorBuilder<M = LibmMath> {
    max_angular_velocity: Option<AngularVelocity>,
    max_angular_acceleration: Option<AngularAcceleration>,
    max_angular_jerk: Option<AngularJerk>,
    period: Option<Time>,
    _math: PhantomData<fn() -> M>,
}

impl Default for ReturnSetupTrajectoryGeneratorBuilder<LibmMath> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> ReturnSetupTrajectoryGeneratorBuilder<M> {
    pub fn new() -> Self {
        Self {
            max_angular_velocity: None,
            max_angular_acceleration: None,
            max_angular_jerk: None,
            period: None,
            _math: PhantomData,
        }
    }

    pub fn max_angular_velocity(mut self, max_angular_velocity: AngularVelocity) -> Self {
        self.max_angular_velocity = Some(max_angular_velocity);
        self
    }

    pub fn max_angular_acceleration(
        mut self,
        max_angular_acceleration: AngularAcceleration,
    ) -> Self {
        self.max_angular_acceleration = Some(max_angular_acceleration);
        self
    }

    pub fn max_angular_jerk(mut self, max_angular_jerk: AngularJerk) -> Self {
        self.max_angular_jerk = Some(max_angular_jerk);
        self
    }

    pub fn period(mut self, period: Time) -> Self {
        self.period = Some(period);
        self
    }

    pub fn build(self) -> Result<ReturnSetupTrajectoryGenerator<M>, RequiredFieldEmptyError> {
        Ok(ReturnSetupTrajectoryGenerator::new(
            ok_or(self.max_angular_velocity, "max_angular_velocity")?,
            ok_or(self.max_angular_acceleration, "max_angular_acceleration")?,
            ok_or(self.max_angular_jerk, "max_angular_jerk")?,
            ok_or(self.period, "period")?,
        ))
    }
}
