mod motor;
mod state;

use core::ops::Div;

use quantities::{
    Angle, Distance, Frequency, Speed, SquaredFrequency, Time, TimeDifferentiable, Voltage,
};

use super::agent;
use super::trajectory_generator::Target;
use crate::{ddt, dt};
pub use motor::Motor;
pub use state::{State, SubState};

pub trait Controller<T>
where
    T: TimeDifferentiable,
    dt!(T): TimeDifferentiable,
{
    fn calculate(&mut self, r: dt!(T), dr: ddt!(T), y: dt!(T), dy: ddt!(T)) -> Voltage;
}

pub struct Tracker<LM, RM, TC, RC> {
    kx: Frequency,
    kdx: SquaredFrequency,
    ky: Frequency,
    kdy: SquaredFrequency,
    xi: Speed,
    period: Time,
    xi_threshold: Speed,
    translation_controller: TC,
    rotation_controller: RC,
    left_motor: LM,
    right_motor: RM,
}

impl<LM, RM, TC, RC> agent::Tracker<State, Target> for Tracker<LM, RM, TC, RC>
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Distance>,
    RC: Controller<Angle>,
{
    fn track(&mut self, state: State, target: Target) {
        let (left, right) = self.track_move(state, target);
        self.left_motor.apply(left);
        self.right_motor.apply(right);
    }
}

impl<LM, RM, TC, RC> Tracker<LM, RM, TC, RC>
where
    TC: Controller<Distance>,
    RC: Controller<Angle>,
{
    fn track_move(&mut self, state: State, target: Target) -> (Voltage, Voltage) {
        let cos_theta = state.theta.x.cos();
        let sin_theta = state.theta.x.sin();

        let vv = state.x.v * cos_theta + state.y.v * sin_theta;
        let va = state.x.a * cos_theta + state.y.a * sin_theta;

        //calculate control input for (x,y)
        let ux =
            target.x.a + self.kx * (target.x.v - state.x.v) + self.kdx * (target.x.x - state.x.x);
        let uy =
            target.y.a + self.ky * (target.y.v - state.y.v) + self.kdy * (target.y.x - state.y.x);
        let dux =
            target.x.j + self.kx * (target.x.a - state.x.a) + self.kdx * (target.x.v - state.x.v);
        let duy =
            target.y.j + self.ky * (target.y.a - state.y.a) + self.kdy * (target.y.v - state.y.v);

        //calculate xi
        let dxi = ux * cos_theta + uy * sin_theta;
        let xi = if self.xi > self.xi_threshold {
            self.xi
        } else {
            self.xi_threshold
        };

        //calculate control input for (v,w)
        let uv = self.xi;
        let uw = (uy * cos_theta - ux * sin_theta) / xi;
        let duv = dxi;
        let duw = -(2.0 * dxi * uw + dux * sin_theta - duy * cos_theta) / xi;

        self.xi += self.period * dxi;
        //calculate motor voltage
        let vol_v = self.translation_controller.calculate(uv, duv, vv, va);
        let vol_w = self
            .rotation_controller
            .calculate(uw, duw, state.theta.v, state.theta.a);
        (vol_v - vol_w, vol_v + vol_w)
    }
}

pub struct TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI> {
    kx: KX,
    kdx: KDX,
    ky: KY,
    kdy: KDY,
    xi_threshold: XIT,
    translation_controller: TC,
    rotation_controller: RC,
    left_motor: LM,
    right_motor: RM,
    period: P,
    xi: XI,
}

impl TrackerBuilder<(), (), (), (), (), (), (), (), (), (), ()> {
    pub fn new() -> Self {
        Self {
            kx: (),
            kdx: (),
            ky: (),
            kdy: (),
            xi_threshold: (),
            translation_controller: (),
            rotation_controller: (),
            left_motor: (),
            right_motor: (),
            period: (),
            xi: (),
        }
    }
}

impl<TC, RC, LM, RM> TrackerBuilder<f32, f32, f32, f32, Speed, TC, RC, LM, RM, Time, ()>
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Distance>,
    RC: Controller<Angle>,
{
    pub fn build(self) -> Tracker<LM, RM, TC, RC> {
        Tracker {
            kx: Frequency::from_hertz(self.kx),
            kdx: SquaredFrequency::from_squared_hertz(self.kdx),
            ky: Frequency::from_hertz(self.ky),
            kdy: SquaredFrequency::from_squared_hertz(self.kdy),
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: Default::default(),
        }
    }
}

impl<TC, RC, LM, RM> TrackerBuilder<f32, f32, f32, f32, Speed, TC, RC, LM, RM, Time, Speed>
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Distance>,
    RC: Controller<Angle>,
{
    pub fn build(self) -> Tracker<LM, RM, TC, RC> {
        Tracker {
            kx: Frequency::from_hertz(self.kx),
            kdx: SquaredFrequency::from_squared_hertz(self.kdx),
            ky: Frequency::from_hertz(self.ky),
            kdy: SquaredFrequency::from_squared_hertz(self.kdy),
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
        }
    }
}

impl<KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI>
    TrackerBuilder<(), KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI>
{
    pub fn kx(self, kx: f32) -> TrackerBuilder<f32, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI> {
        TrackerBuilder {
            kx,
            kdx: self.kdx,
            ky: self.ky,
            kdy: self.kdy,
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
        }
    }
}

impl<KX, KY, KDY, XIT, TC, RC, LM, RM, P, XI>
    TrackerBuilder<KX, (), KY, KDY, XIT, TC, RC, LM, RM, P, XI>
{
    pub fn kdx(self, kdx: f32) -> TrackerBuilder<KX, f32, KY, KDY, XIT, TC, RC, LM, RM, P, XI> {
        TrackerBuilder {
            kx: self.kx,
            kdx,
            ky: self.ky,
            kdy: self.kdy,
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
        }
    }
}

impl<KX, KDX, KDY, XIT, TC, RC, LM, RM, P, XI>
    TrackerBuilder<KX, KDX, (), KDY, XIT, TC, RC, LM, RM, P, XI>
{
    pub fn ky(self, ky: f32) -> TrackerBuilder<KX, KDX, f32, KDY, XIT, TC, RC, LM, RM, P, XI> {
        TrackerBuilder {
            kx: self.kx,
            kdx: self.kdx,
            ky,
            kdy: self.kdy,
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
        }
    }
}

impl<KX, KDX, KY, XIT, TC, RC, LM, RM, P, XI>
    TrackerBuilder<KX, KDX, KY, (), XIT, TC, RC, LM, RM, P, XI>
{
    pub fn kdy(self, kdy: f32) -> TrackerBuilder<KX, KDX, KY, f32, XIT, TC, RC, LM, RM, P, XI> {
        TrackerBuilder {
            kx: self.kx,
            kdx: self.kdx,
            ky: self.ky,
            kdy,
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
        }
    }
}

impl<KX, KDX, KY, KDY, TC, RC, LM, RM, P, XI>
    TrackerBuilder<KX, KDX, KY, KDY, (), TC, RC, LM, RM, P, XI>
{
    pub fn valid_control_lower_bound(
        self,
        xi_threshold: Speed,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, Speed, TC, RC, LM, RM, P, XI> {
        TrackerBuilder {
            kx: self.kx,
            kdx: self.kdx,
            ky: self.ky,
            kdy: self.kdy,
            xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, RC, LM, RM, P, XI>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, (), RC, LM, RM, P, XI>
{
    pub fn translation_controller<TC>(
        self,
        translation_controller: TC,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI>
    where
        TC: Controller<Distance>,
    {
        TrackerBuilder {
            kx: self.kx,
            kdx: self.kdx,
            ky: self.ky,
            kdy: self.kdy,
            xi_threshold: self.xi_threshold,
            translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, LM, RM, P, XI>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, (), LM, RM, P, XI>
{
    pub fn rotation_controller<RC>(
        self,
        rotation_controller: RC,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI>
    where
        RC: Controller<Angle>,
    {
        TrackerBuilder {
            kx: self.kx,
            kdx: self.kdx,
            ky: self.ky,
            kdy: self.kdy,
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, RM, P, XI>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, (), RM, P, XI>
{
    pub fn left_motor<LM>(
        self,
        left_motor: LM,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI>
    where
        LM: Motor,
    {
        TrackerBuilder {
            kx: self.kx,
            kdx: self.kdx,
            ky: self.ky,
            kdy: self.kdy,
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, P, XI>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, (), P, XI>
{
    pub fn right_motor<RM>(
        self,
        right_motor: RM,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI>
    where
        RM: Motor,
    {
        TrackerBuilder {
            kx: self.kx,
            kdx: self.kdx,
            ky: self.ky,
            kdy: self.kdy,
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor,
            period: self.period,
            xi: self.xi,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, XI>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, (), XI>
{
    pub fn period(
        self,
        period: Time,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, Time, XI> {
        TrackerBuilder {
            kx: self.kx,
            kdx: self.kdx,
            ky: self.ky,
            kdy: self.kdy,
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period,
            xi: self.xi,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, ()>
{
    pub fn initial_speed(
        self,
        xi: Speed,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, Speed> {
        TrackerBuilder {
            kx: self.kx,
            kdx: self.kdx,
            ky: self.ky,
            kdy: self.kdy,
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::marker::PhantomData;

    struct IMotor;

    impl Motor for IMotor {
        fn apply(&mut self, _voltage: Voltage) {}
    }

    struct IController<T> {
        _phantom: PhantomData<fn() -> T>,
    }

    impl<T> IController<T> {
        fn new() -> Self {
            Self {
                _phantom: PhantomData,
            }
        }
    }

    impl<T> Controller<T> for IController<T>
    where
        T: TimeDifferentiable,
        dt!(T): TimeDifferentiable,
    {
        fn calculate(&mut self, _r: dt!(T), _dr: ddt!(T), _y: dt!(T), _dy: ddt!(T)) -> Voltage {
            Voltage::from_volts(1.0)
        }
    }

    fn build_tracker() -> Tracker<IMotor, IMotor, IController<Distance>, IController<Angle>> {
        TrackerBuilder::new()
            .kx(1.0)
            .kdx(1.0)
            .ky(1.0)
            .kdy(1.0)
            .initial_speed(Speed::from_meter_per_second(0.0))
            .valid_control_lower_bound(Speed::from_meter_per_second(0.001))
            .right_motor(IMotor)
            .left_motor(IMotor)
            .period(Time::from_seconds(0.001))
            .translation_controller(IController::<Distance>::new())
            .rotation_controller(IController::<Angle>::new())
            .build()
    }

    #[test]
    fn test_build() {
        let _tracker = build_tracker();
    }
}
