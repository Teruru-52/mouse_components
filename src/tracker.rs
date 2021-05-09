//! An implementation of [Tracker](crate::robot::Tracker).

mod state;

use core::marker::PhantomData;

#[allow(unused_imports)]
use micromath::F32Ext;
use serde::{Deserialize, Serialize};
use uom::si::{
    angle::radian,
    f32::{
        Acceleration, Angle, AngularAcceleration, AngularVelocity, ElectricPotential, Frequency,
        Length, Time, Velocity,
    },
    frequency::hertz,
    Quantity, ISQ, SI,
};
use uom::{typenum::*, Kind};

use super::robot::Tracker as ITracker;
use super::trajectory_generators::Target;
use crate::utils::builder::{ok_or, RequiredFieldEmptyError};
use crate::{Construct, Deconstruct};
pub use state::{AngleState, LengthState, RobotState};

pub trait Motor {
    fn apply(&mut self, electric_potential: ElectricPotential);
}

type GainType = Quantity<ISQ<Z0, Z0, N2, Z0, Z0, Z0, Z0, dyn Kind>, SI<f32>, f32>;
type BType = Quantity<ISQ<N2, Z0, Z0, Z0, Z0, Z0, Z0, dyn Kind>, SI<f32>, f32>;

pub trait Controller<T, U> {
    fn calculate(&mut self, r: T, dr: U, y: T, dy: U) -> ElectricPotential;
}

/// An implementation of [Tracker](crate::robot::Tracker).
pub struct Tracker<
    LM,
    RM,
    TC = crate::controllers::TranslationalController,
    RC = crate::controllers::RotationalController,
> {
    kx: GainType,
    kdx: Frequency,
    ky: GainType,
    kdy: Frequency,
    xi: Velocity,
    period: Time,
    xi_threshold: Velocity,
    fail_safe_distance: Length,
    translation_controller: TC,
    rotation_controller: RC,
    left_motor: LM,
    right_motor: RM,
    zeta: f32,
    b: BType,
}

impl<LM, RM, TC, RC> Tracker<LM, RM, TC, RC> {
    pub fn release(self) -> (LM, RM) {
        let Self {
            left_motor,
            right_motor,
            ..
        } = self;
        (left_motor, right_motor)
    }
}

/// Config for [Tracker].
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TrackerConfig {
    pub kx: f32,
    pub kdx: f32,
    pub ky: f32,
    pub kdy: f32,
    pub period: Time,
    pub valid_control_lower_bound: Velocity,
    pub fail_safe_distance: Length,
    pub low_zeta: f32,
    pub low_b: f32,
}

/// Resource for [Tracker].
#[derive(PartialEq, Debug)]
pub struct TrackerResource<LeftMotor, RightMotor> {
    pub left_motor: LeftMotor,
    pub right_motor: RightMotor,
}

impl<LeftMotor, RightMotor, TC, RC, Config, State, Resource> Construct<Config, State, Resource>
    for Tracker<LeftMotor, RightMotor, TC, RC>
where
    TC: Construct<Config, State, Resource> + Controller<Velocity, Acceleration>,
    RC: Construct<Config, State, Resource> + Controller<AngularVelocity, AngularAcceleration>,
    LeftMotor: Motor,
    RightMotor: Motor,
    Config: AsRef<TrackerConfig>,
    Resource: AsMut<Option<TrackerResource<LeftMotor, RightMotor>>>,
{
    fn construct<'a>(config: &'a Config, state: &'a State, resource: &'a mut Resource) -> Self {
        let translation_controller = TC::construct(config, state, resource);
        let rotation_controller = RC::construct(config, state, resource);
        let config = config.as_ref();
        let TrackerResource {
            left_motor,
            right_motor,
        } = resource.as_mut().take().unwrap_or_else(|| unimplemented!());
        TrackerBuilder::new()
            .translation_controller(translation_controller)
            .rotation_controller(rotation_controller)
            .left_motor(left_motor)
            .right_motor(right_motor)
            .kx(config.kx)
            .kdx(config.kdx)
            .ky(config.ky)
            .kdy(config.kdy)
            .period(config.period)
            .valid_control_lower_bound(config.valid_control_lower_bound)
            .fail_safe_distance(config.fail_safe_distance)
            .low_zeta(config.low_zeta)
            .low_b(config.low_b)
            .build()
            .expect("Should never panic")
    }
}

impl<LeftMotor, RightMotor, TC, RC, State, Resource> Deconstruct<State, Resource>
    for Tracker<LeftMotor, RightMotor, TC, RC>
where
    State: Default,
    Resource: From<TrackerResource<LeftMotor, RightMotor>>,
{
    fn deconstruct(self) -> (State, Resource) {
        let (left_motor, right_motor) = self.release();
        (
            Default::default(),
            TrackerResource {
                left_motor,
                right_motor,
            }
            .into(),
        )
    }
}

impl<LM, RM, TC, RC> core::fmt::Debug for Tracker<LM, RM, TC, RC> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "Tracker{{ xi:{:?} }}", self.xi)
    }
}

/// Error on [Tracker](Tracker).
#[derive(Clone, PartialEq, Debug)]
pub struct FailSafeError {
    state: RobotState,
    target: Target,
}

impl<LM, RM, TC, RC> ITracker<RobotState, Target> for Tracker<LM, RM, TC, RC>
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Velocity, Acceleration>,
    RC: Controller<AngularVelocity, AngularAcceleration>,
{
    type Error = FailSafeError;

    fn track(&mut self, state: &RobotState, target: &Target) -> Result<(), Self::Error> {
        let (left, right) = self.track_move(state, target)?;
        self.left_motor.apply(left);
        self.right_motor.apply(right);
        Ok(())
    }
}

// normalize angle to [-pi, pi].
fn normalize_angle(angle: Angle) -> Angle {
    use core::f32::consts::{PI, TAU};

    let raw_angle = angle.value.rem_euclid(TAU);

    Angle::new::<radian>(if raw_angle > PI {
        raw_angle - TAU
    } else {
        raw_angle
    })
}

// calculate sin(x)/x
fn sinc(x: f32) -> f32 {
    let xx = x * x;
    let xxxx = xx * xx;
    xxxx * xxxx / 362880.0 - xxxx * xx / 5040.0 + xxxx / 120.0 - xx / 6.0 + 1.0
}

impl<LM, RM, TC, RC> Tracker<LM, RM, TC, RC>
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Velocity, Acceleration>,
    RC: Controller<AngularVelocity, AngularAcceleration>,
{
    pub fn stop(&mut self)
    where
        LM: Motor,
        RM: Motor,
    {
        self.left_motor.apply(Default::default());
        self.right_motor.apply(Default::default());
    }

    fn fail_safe(&mut self, state: &RobotState, target: &Target) -> Result<(), FailSafeError> {
        let x_diff = state.x.x - target.x.x;
        let y_diff = state.y.x - target.y.x;

        let distance =
            Length::new::<uom::si::length::meter>((x_diff * x_diff + y_diff * y_diff).value.sqrt());
        if distance >= self.fail_safe_distance {
            Err(FailSafeError {
                state: state.clone(),
                target: target.clone(),
            })
        } else {
            Ok(())
        }
    }

    fn track_move(
        &mut self,
        state: &RobotState,
        target: &Target,
    ) -> Result<(ElectricPotential, ElectricPotential), FailSafeError> {
        self.fail_safe(state, target)?;

        let sin_th = state.theta.x.value.sin();
        let cos_th = state.theta.x.value.cos();

        let vv = state.x.v * cos_th + state.y.v * sin_th;
        let va = state.x.a * cos_th + state.y.a * sin_th;

        //calculate control input for (x,y)
        let ux =
            target.x.a + self.kdx * (target.x.v - state.x.v) + self.kx * (target.x.x - state.x.x);
        let uy =
            target.y.a + self.kdy * (target.y.v - state.y.v) + self.ky * (target.y.x - state.y.x);
        let dux =
            target.x.j + self.kdx * (target.x.a - state.x.a) + self.kx * (target.x.v - state.x.v);
        let duy =
            target.y.j + self.kdy * (target.y.a - state.y.a) + self.ky * (target.y.v - state.y.v);

        let dxi = ux * cos_th + uy * sin_th;
        let (uv, uw, duv, duw) = if self.xi.abs() > self.xi_threshold {
            let uv = self.xi;
            let uw = AngularVelocity::from((uy * cos_th - ux * sin_th) / self.xi);
            let duv = dxi;
            let duw = -AngularAcceleration::from(
                (2.0 * dxi * uw + dux * sin_th - duy * cos_th) / self.xi,
            );
            (uv, uw, duv, duw)
        } else {
            let sin_th_r = target.theta.x.value.sin();
            let cos_th_r = target.theta.x.value.cos();
            let theta_d = normalize_angle(target.theta.x - state.theta.x);
            let cos_th_d = theta_d.value.cos();
            let xd = target.x.x - state.x.x;
            let yd = target.y.x - state.y.x;

            let vr = target.x.v * cos_th_r + target.y.v * sin_th_r;
            let wr = target.theta.v;

            let k1 = 2.0
                * self.zeta
                * AngularVelocity::from(crate::utils::math::sqrt(wr * wr + self.b * vr * vr));
            let k2 = self.b;
            let k3 = k1;

            let uv = vr * cos_th_d + k1 * (xd * cos_th + yd * sin_th);
            let uw =
                wr + AngularVelocity::from(
                    k2 * vr * (-xd * sin_th + yd * cos_th) * sinc(theta_d.value),
                ) + AngularVelocity::from(k3 * theta_d);
            (
                uv,
                uw,
                target.x.a * cos_th_r + target.y.a * sin_th_r,
                target.theta.a,
            )
        };

        self.xi += self.period * dxi;
        //calculate motor voltage
        let vol_v = self.translation_controller.calculate(uv, duv, vv, va);
        let vol_w = self
            .rotation_controller
            .calculate(uw, duw, state.theta.v, state.theta.a);
        Ok((vol_v - vol_w, vol_v + vol_w))
    }
}

pub struct TrackerBuilder<TC, RC, LM, RM> {
    kx: Option<GainType>,
    kdx: Option<Frequency>,
    ky: Option<GainType>,
    kdy: Option<Frequency>,
    xi_threshold: Option<Velocity>,
    translation_controller: Option<TC>,
    rotation_controller: Option<RC>,
    left_motor: Option<LM>,
    right_motor: Option<RM>,
    period: Option<Time>,
    xi: Option<Velocity>,
    fail_safe_distance: Option<Length>,
    zeta: Option<f32>,
    b: Option<BType>,
}

impl<TC, RC, LM, RM> TrackerBuilder<TC, RC, LM, RM> {
    pub fn new() -> Self {
        Self {
            kx: None,
            kdx: None,
            ky: None,
            kdy: None,
            xi_threshold: None,
            translation_controller: None,
            rotation_controller: None,
            left_motor: None,
            right_motor: None,
            period: None,
            xi: Some(Default::default()),
            fail_safe_distance: None,
            zeta: None,
            b: None,
        }
    }

    pub fn kx(&mut self, kx: f32) -> &mut Self {
        self.kx = Some(GainType {
            value: kx,
            dimension: PhantomData,
            units: PhantomData,
        });
        self
    }

    pub fn kdx(&mut self, kdx: f32) -> &mut Self {
        self.kdx = Some(Frequency::new::<hertz>(kdx));
        self
    }

    pub fn ky(&mut self, ky: f32) -> &mut Self {
        self.ky = Some(GainType {
            value: ky,
            dimension: PhantomData,
            units: PhantomData,
        });
        self
    }

    pub fn kdy(&mut self, kdy: f32) -> &mut Self {
        self.kdy = Some(Frequency::new::<hertz>(kdy));
        self
    }

    pub fn valid_control_lower_bound(&mut self, xi_threshold: Velocity) -> &mut Self {
        self.xi_threshold = Some(xi_threshold);
        self
    }

    pub fn translation_controller(&mut self, translation_controller: TC) -> &mut Self
    where
        TC: Controller<Velocity, Acceleration>,
    {
        self.translation_controller = Some(translation_controller);
        self
    }

    pub fn rotation_controller(&mut self, rotation_controller: RC) -> &mut Self
    where
        RC: Controller<AngularVelocity, AngularAcceleration>,
    {
        self.rotation_controller = Some(rotation_controller);
        self
    }

    pub fn left_motor(&mut self, left_motor: LM) -> &mut Self
    where
        LM: Motor,
    {
        self.left_motor = Some(left_motor);
        self
    }

    pub fn right_motor(&mut self, right_motor: RM) -> &mut Self
    where
        RM: Motor,
    {
        self.right_motor = Some(right_motor);
        self
    }

    pub fn period(&mut self, period: Time) -> &mut Self {
        self.period = Some(period);
        self
    }

    pub fn initial_velocity(&mut self, xi: Velocity) -> &mut Self {
        self.xi = Some(xi);
        self
    }

    pub fn fail_safe_distance(&mut self, fail_safe_distance: Length) -> &mut Self {
        self.fail_safe_distance = Some(fail_safe_distance);
        self
    }

    pub fn low_zeta(&mut self, zeta: f32) -> &mut Self {
        self.zeta = Some(zeta);
        self
    }

    pub fn low_b(&mut self, b: f32) -> &mut Self {
        self.b = Some(BType {
            value: b,
            ..Default::default()
        });
        self
    }

    pub fn build(&mut self) -> Result<Tracker<LM, RM, TC, RC>, RequiredFieldEmptyError> {
        Ok(Tracker {
            kx: ok_or(self.kx, "kx")?,
            kdx: ok_or(self.kdx, "kdx")?,
            ky: ok_or(self.ky, "ky")?,
            kdy: ok_or(self.kdy, "kdy")?,
            xi_threshold: ok_or(self.xi_threshold, "xi_threshold")?,
            translation_controller: ok_or(
                self.translation_controller.take(),
                "translation_controller",
            )?,
            rotation_controller: ok_or(self.rotation_controller.take(), "rotation_controller")?,
            left_motor: ok_or(self.left_motor.take(), "left_motor")?,
            right_motor: ok_or(self.right_motor.take(), "right_motor")?,
            period: ok_or(self.period, "period")?,
            xi: self.xi.expect("Should never None"),
            fail_safe_distance: ok_or(self.fail_safe_distance, "fail_safe_distance")?,
            zeta: ok_or(self.zeta, "zeta")?,
            b: ok_or(self.b, "b")?,
        })
    }
}

impl<TC, RC, LM, RM> Default for TrackerBuilder<TC, RC, LM, RM> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::{
        electric_potential::volt, length::meter, time::second, velocity::meter_per_second,
    };

    struct IMotor;

    impl Motor for IMotor {
        fn apply(&mut self, _voltage: ElectricPotential) {}
    }

    struct IController;

    impl<T, U> Controller<T, U> for IController {
        fn calculate(&mut self, _: T, _: U, _: T, _: U) -> ElectricPotential {
            ElectricPotential::new::<volt>(1.0)
        }
    }

    fn build_tracker() -> Tracker<IMotor, IMotor, IController, IController> {
        TrackerBuilder::default()
            .kx(1.0)
            .kdx(1.0)
            .ky(1.0)
            .kdy(1.0)
            .initial_velocity(Velocity::new::<meter_per_second>(0.0))
            .valid_control_lower_bound(Velocity::new::<meter_per_second>(0.001))
            .right_motor(IMotor)
            .left_motor(IMotor)
            .period(Time::new::<second>(0.001))
            .translation_controller(IController)
            .rotation_controller(IController)
            .low_zeta(1.0)
            .low_b(1e-3)
            .fail_safe_distance(Length::new::<meter>(0.02))
            .build()
            .unwrap()
    }

    #[test]
    fn test_build() {
        let _tracker = build_tracker();
    }

    #[test]
    fn test_normalize_angle() {
        use approx::assert_relative_eq;
        use uom::si::angle::degree;

        let test_cases = vec![
            (45.0, 45.0),
            (180.0, 180.0),
            (-45.0, -45.0),
            (-300.0, 60.0),
            (-660.0, 60.0),
        ];

        for (angle, expected) in test_cases {
            let angle = Angle::new::<degree>(angle);
            let expected = Angle::new::<degree>(expected);
            assert_relative_eq!(
                normalize_angle(angle).value,
                expected.value,
                epsilon = 0.001
            );
        }
    }
}
