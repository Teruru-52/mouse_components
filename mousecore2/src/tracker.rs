use core::marker::PhantomData;

#[allow(unused_imports)]
use micromath::F32Ext;
use typed_builder::TypedBuilder;
use uom::{
    si::{
        angle::radian,
        angular_velocity::radian_per_second,
        f32::{
            Acceleration, Angle, AngularAcceleration, AngularJerk, AngularVelocity, Frequency,
            Jerk, Length, Time, Velocity,
        },
        Quantity, ISQ, SI,
    },
    typenum::*,
    Kind,
};

use crate::estimator::State;

type GainType = Quantity<ISQ<Z0, Z0, N2, Z0, Z0, Z0, Z0, dyn Kind>, SI<f32>, f32>;
type BType = Quantity<ISQ<N2, Z0, Z0, Z0, Z0, Z0, Z0, dyn Kind>, SI<f32>, f32>;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Target {
    pub x: LengthTarget,
    pub y: LengthTarget,
    pub theta: AngleTarget,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LengthTarget {
    pub x: Length,
    pub v: Velocity,
    pub a: Acceleration,
    pub j: Jerk,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AngleTarget {
    pub x: Angle,
    pub v: AngularVelocity,
    pub a: AngularAcceleration,
    pub j: AngularJerk,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ControlTarget {
    pub v: Velocity,
    pub a: Acceleration,
    pub omega: AngularVelocity,
    pub alpha: AngularAcceleration,
}

#[derive(Debug, TypedBuilder)]
pub struct Tracker {
    #[builder(setter(transform = |value: f32| GainType{ value, dimension: PhantomData, units: PhantomData }))]
    gain: GainType,
    #[builder(setter(transform = |value: f32| Frequency{ value, dimension: PhantomData, units: PhantomData }))]
    dgain: Frequency,
    #[builder(default)]
    xi: Velocity,
    period: Time,
    xi_threshold: Velocity,
    zeta: f32,
    #[builder(setter(transform = |value: f32| BType{ value, dimension: PhantomData, units: PhantomData }))]
    b: BType,
}

impl Tracker {
    pub fn track(&mut self, state: &State, target: &Target) -> (ControlTarget, ControlTarget) {
        let sin_th = state.theta.x.value.sin();
        let cos_th = state.theta.x.value.cos();

        let vv = state.x.v * cos_th + state.y.v * sin_th;
        let va = state.x.a * cos_th + state.y.a * sin_th;

        //calculate control input for (x,y)
        let ux = target.x.a
            + self.dgain * (target.x.v - state.x.v)
            + self.gain * (target.x.x - state.x.x);
        let uy = target.y.a
            + self.dgain * (target.y.v - state.y.v)
            + self.gain * (target.y.x - state.y.x);
        let dux = target.x.j
            + self.dgain * (target.x.a - state.x.a)
            + self.gain * (target.x.v - state.x.v);
        let duy = target.y.j
            + self.dgain * (target.y.a - state.y.a)
            + self.gain * (target.y.v - state.y.v);

        let dxi = ux * cos_th + uy * sin_th;
        self.xi += self.period * dxi;

        let sin_th_r = target.theta.x.value.sin();
        let cos_th_r = target.theta.x.value.cos();
        let vr = target.x.v * cos_th_r + target.y.v * sin_th_r;

        let (uv, uw, duv, duw) =
            if vr.abs() > self.xi_threshold && self.xi.abs() > self.xi_threshold {
                let uv = self.xi;
                let uw = AngularVelocity::from((uy * cos_th - ux * sin_th) / self.xi);
                let duv = dxi;
                let duw = -AngularAcceleration::from(
                    (2.0 * dxi * uw + dux * sin_th - duy * cos_th) / self.xi,
                );
                (uv, uw, duv, duw)
            } else {
                let theta_d = normalize_angle(target.theta.x - state.theta.x);
                let cos_th_d = theta_d.value.cos();
                let xd = target.x.x - state.x.x;
                let yd = target.y.x - state.y.x;

                let wr = target.theta.v;

                let k1 = 2.0
                    * self.zeta
                    * AngularVelocity::new::<radian_per_second>(
                        (wr * wr + self.b * vr * vr).value.sqrt(),
                    );
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

        (
            ControlTarget {
                v: uv,
                a: duv,
                omega: uw,
                alpha: duw,
            },
            ControlTarget {
                v: vv,
                a: va,
                omega: state.theta.v,
                alpha: state.theta.a,
            },
        )
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

#[cfg(test)]
mod tests {
    use super::*;

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
