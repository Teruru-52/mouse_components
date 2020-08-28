mod motor;
mod state;

use core::marker::PhantomData;
use core::ops::Div;

use super::agent;
use super::trajectory_generator::Target;
use crate::quantities::{
    dimensionless::radian,
    f32::{Angle, AngularVelocity, Frequency, Length, SquaredFrequency, Time, Velocity, Voltage},
    frequency::{hertz, radian_per_second},
    length::meter,
    squared_frequency::squared_hertz,
    velocity::meter_per_second,
};
use crate::{ddt, dt};
pub use motor::Motor;
pub use state::{AngleState, LengthState, State};

pub trait Controller<T>
where
    T: Div<Time>,
    dt!(T): Div<Time>,
{
    fn init(&mut self);
    fn calculate(&mut self, r: dt!(T), dr: ddt!(T), y: dt!(T), dy: ddt!(T)) -> Voltage;
}

pub trait Logger {
    fn log(&self, state: &State, target: &Target);
}

pub struct NullLogger;

impl Logger for NullLogger {
    fn log(&self, _state: &State, _target: &Target) {}
}

pub struct Tracker<LM, RM, TC, RC, L> {
    kx: SquaredFrequency,
    kdx: Frequency,
    ky: SquaredFrequency,
    kdy: Frequency,
    xi: Velocity,
    period: Time,
    xi_threshold: Velocity,
    fail_safe_distance: Length,
    translation_controller: TC,
    rotation_controller: RC,
    left_motor: LM,
    right_motor: RM,
    #[allow(unused)]
    logger: L,
    zeta: f32,
    b: f32,
}

impl<LM, RM, TC, RC, L> agent::Tracker<State, Target> for Tracker<LM, RM, TC, RC, L>
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Length>,
    RC: Controller<Angle>,
    L: Logger,
{
    fn stop(&mut self)
    where
        LM: Motor,
        RM: Motor,
    {
        self.left_motor.apply(Default::default());
        self.right_motor.apply(Default::default());
    }

    fn init(&mut self) {
        self.translation_controller.init();
        self.rotation_controller.init();
    }

    fn track(&mut self, state: State, target: Target) {
        #[cfg(feature = "log")]
        self.logger.log(&state, &target);
        let (left, right) = self.track_move(state, target);
        self.left_motor.apply(left);
        self.right_motor.apply(right);
    }
}

impl<LM, RM, TC, RC, L> Tracker<LM, RM, TC, RC, L>
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Length>,
    RC: Controller<Angle>,
    L: Logger,
{
    fn sinc(x: f32) -> f32 {
        let xx = x * x;
        let xxxx = xx * xx;
        xxxx * xxxx / 362880.0 - xxxx * xx / 5040.0 + xxxx / 120.0 - xx / 6.0 + 1.0
    }

    fn fail_safe(&mut self, state: &State, target: &Target) {
        use agent::Tracker;

        let x_diff = (state.x.x - target.x.x).get::<meter>();
        let y_diff = (state.y.x - target.y.x).get::<meter>();

        let distance = Length::new::<meter>(libm::sqrtf(x_diff * x_diff + y_diff * y_diff));
        if distance >= self.fail_safe_distance {
            self.stop();
            panic!("state: {:?}, target: {:?}", state, target);
        }
    }

    fn track_move(&mut self, state: State, target: Target) -> (Voltage, Voltage) {
        self.fail_safe(&state, &target);

        let cos_th = state.theta.x.cos();
        let sin_th = state.theta.x.sin();

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
        let (uv, uw, duv, duw) = if self.xi > self.xi_threshold {
            let uv = self.xi;
            let uw = (uy * cos_th - ux * sin_th) / self.xi;
            let duv = dxi;
            let duw = -(2.0 * dxi * uw + dux * sin_th - duy * cos_th) / self.xi;
            (uv, uw, duv, duw)
        } else {
            let sin_th_r = target.theta.x.sin();
            let cos_th_r = target.theta.x.cos();
            let theta_d = target.theta.x - state.theta.x;
            let cos_th_d = theta_d.cos();
            let xd = target.x.x - state.x.x;
            let yd = target.y.x - state.y.x;

            let vr = target.x.v * cos_th_r + target.y.v * sin_th_r;
            let wr = target.theta.v;

            let k1 = self.calculate_k1(wr, vr);
            let k2 = self.b;
            let k3 = k1;

            let e = xd * cos_th + yd * sin_th;
            let uv = vr * cos_th_d + k1 * e;
            let uw =
                wr + AngularVelocity::new::<radian_per_second>(
                    k2 * vr.get::<meter_per_second>()
                        * e.get::<meter>()
                        * Self::sinc(theta_d.get::<radian>()),
                ) + k3 * theta_d;
            (uv, uw, Default::default(), Default::default())
        };

        self.xi += self.period * dxi;
        //calculate motor voltage
        let vol_v = self.translation_controller.calculate(uv, duv, vv, va);
        let vol_w = self
            .rotation_controller
            .calculate(uw, duw, state.theta.v, state.theta.a);
        (vol_v - vol_w, vol_v + vol_w)
    }

    fn calculate_k1(&self, wr: AngularVelocity, vr: Velocity) -> Frequency {
        let wr_raw = wr.get::<radian_per_second>();
        let vr_raw = vr.get::<meter_per_second>();
        Frequency::new::<hertz>(
            2.0 * self.zeta * libm::sqrtf(wr_raw * wr_raw + self.b * vr_raw * vr_raw),
        )
    }
}

pub struct TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B> {
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
    fail_safe_distance: FS,
    logger: L,
    zeta: Z,
    b: B,
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
{
    const DEFAULT_FAIL_SAFE_DISTANCE: Length = Length {
        dimension: PhantomData,
        units: PhantomData,
        value: 0.02,
    };
}

impl TrackerBuilder<(), (), (), (), (), (), (), (), (), (), (), (), NullLogger, (), ()> {
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
            fail_safe_distance: (),
            logger: NullLogger,
            zeta: (),
            b: (),
        }
    }
}

impl<TC, RC, LM, RM, L>
    TrackerBuilder<f32, f32, f32, f32, Velocity, TC, RC, LM, RM, Time, (), (), L, f32, f32>
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Length>,
    RC: Controller<Angle>,
    L: Logger,
{
    pub fn build(self) -> Tracker<LM, RM, TC, RC, L> {
        Tracker {
            kx: SquaredFrequency::new::<squared_hertz>(self.kx),
            kdx: Frequency::new::<hertz>(self.kdx),
            ky: SquaredFrequency::new::<squared_hertz>(self.ky),
            kdy: Frequency::new::<hertz>(self.kdy),
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: Default::default(),
            fail_safe_distance: Self::DEFAULT_FAIL_SAFE_DISTANCE,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<TC, RC, LM, RM, L>
    TrackerBuilder<f32, f32, f32, f32, Velocity, TC, RC, LM, RM, Time, (), Length, L, f32, f32>
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Length>,
    RC: Controller<Angle>,
    L: Logger,
{
    pub fn build(self) -> Tracker<LM, RM, TC, RC, L> {
        Tracker {
            kx: SquaredFrequency::new::<squared_hertz>(self.kx),
            kdx: Frequency::new::<hertz>(self.kdx),
            ky: SquaredFrequency::new::<squared_hertz>(self.ky),
            kdy: Frequency::new::<hertz>(self.kdy),
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: Default::default(),
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<TC, RC, LM, RM, L>
    TrackerBuilder<f32, f32, f32, f32, Velocity, TC, RC, LM, RM, Time, Velocity, (), L, f32, f32>
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Length>,
    RC: Controller<Angle>,
{
    pub fn build(self) -> Tracker<LM, RM, TC, RC, L> {
        Tracker {
            kx: SquaredFrequency::new::<squared_hertz>(self.kx),
            kdx: Frequency::new::<hertz>(self.kdx),
            ky: SquaredFrequency::new::<squared_hertz>(self.ky),
            kdy: Frequency::new::<hertz>(self.kdy),
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
            fail_safe_distance: Self::DEFAULT_FAIL_SAFE_DISTANCE,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<TC, RC, LM, RM, L>
    TrackerBuilder<
        f32,
        f32,
        f32,
        f32,
        Velocity,
        TC,
        RC,
        LM,
        RM,
        Time,
        Velocity,
        Length,
        L,
        f32,
        f32,
    >
where
    LM: Motor,
    RM: Motor,
    TC: Controller<Length>,
    RC: Controller<Angle>,
    L: Logger,
{
    pub fn build(self) -> Tracker<LM, RM, TC, RC, L> {
        Tracker {
            kx: SquaredFrequency::new::<squared_hertz>(self.kx),
            kdx: Frequency::new::<hertz>(self.kdx),
            ky: SquaredFrequency::new::<squared_hertz>(self.ky),
            kdy: Frequency::new::<hertz>(self.kdy),
            xi_threshold: self.xi_threshold,
            translation_controller: self.translation_controller,
            rotation_controller: self.rotation_controller,
            left_motor: self.left_motor,
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
    TrackerBuilder<(), KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
{
    pub fn kx(
        self,
        kx: f32,
    ) -> TrackerBuilder<f32, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B> {
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
    TrackerBuilder<KX, (), KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
{
    pub fn kdx(
        self,
        kdx: f32,
    ) -> TrackerBuilder<KX, f32, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B> {
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
    TrackerBuilder<KX, KDX, (), KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
{
    pub fn ky(
        self,
        ky: f32,
    ) -> TrackerBuilder<KX, KDX, f32, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B> {
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
    TrackerBuilder<KX, KDX, KY, (), XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
{
    pub fn kdy(
        self,
        kdy: f32,
    ) -> TrackerBuilder<KX, KDX, KY, f32, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B> {
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, TC, RC, LM, RM, P, XI, FS, L, Z, B>
    TrackerBuilder<KX, KDX, KY, KDY, (), TC, RC, LM, RM, P, XI, FS, L, Z, B>
{
    pub fn valid_control_lower_bound(
        self,
        xi_threshold: Velocity,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, Velocity, TC, RC, LM, RM, P, XI, FS, L, Z, B> {
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, RC, LM, RM, P, XI, FS, L, Z, B>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, (), RC, LM, RM, P, XI, FS, L, Z, B>
{
    pub fn translation_controller<TC>(
        self,
        translation_controller: TC,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
    where
        TC: Controller<Length>,
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, LM, RM, P, XI, FS, L, Z, B>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, (), LM, RM, P, XI, FS, L, Z, B>
{
    pub fn rotation_controller<RC>(
        self,
        rotation_controller: RC,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, RM, P, XI, FS, L, Z, B>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, (), RM, P, XI, FS, L, Z, B>
{
    pub fn left_motor<LM>(
        self,
        left_motor: LM,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, P, XI, FS, L, Z, B>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, (), P, XI, FS, L, Z, B>
{
    pub fn right_motor<RM>(
        self,
        right_motor: RM,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, XI, FS, L, Z, B>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, (), XI, FS, L, Z, B>
{
    pub fn period(
        self,
        period: Time,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, Time, XI, FS, L, Z, B> {
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, FS, L, Z, B>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, (), FS, L, Z, B>
{
    pub fn initial_velocity(
        self,
        xi: Velocity,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, Velocity, FS, L, Z, B> {
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
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, L, Z, B>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, (), L, Z, B>
{
    pub fn fail_safe_distance(
        self,
        fail_safe_distance: Length,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, Length, L, Z, B> {
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
            xi: self.xi,
            fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, Z, B>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, NullLogger, Z, B>
{
    pub fn logger<L>(
        self,
        logger: L,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, B>
    where
        L: Logger,
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
            right_motor: self.right_motor,
            period: self.period,
            xi: self.xi,
            fail_safe_distance: self.fail_safe_distance,
            logger,
            zeta: self.zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, B>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, (), B>
{
    pub fn low_zeta(
        self,
        zeta: f32,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, f32, B> {
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
            xi: self.xi,
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta,
            b: self.b,
        }
    }
}

impl<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z>
    TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, ()>
{
    pub fn low_b(
        self,
        b: f32,
    ) -> TrackerBuilder<KX, KDX, KY, KDY, XIT, TC, RC, LM, RM, P, XI, FS, L, Z, f32> {
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
            xi: self.xi,
            fail_safe_distance: self.fail_safe_distance,
            logger: self.logger,
            zeta: self.zeta,
            b,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantities::{time::second, voltage::volt};
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
        T: Div<Time>,
        dt!(T): Div<Time>,
    {
        fn init(&mut self) {}

        fn calculate(&mut self, _r: dt!(T), _dr: ddt!(T), _y: dt!(T), _dy: ddt!(T)) -> Voltage {
            Voltage::new::<volt>(1.0)
        }
    }

    fn build_tracker<L>(
        logger: L,
    ) -> Tracker<IMotor, IMotor, IController<Length>, IController<Angle>, L>
    where
        L: Logger,
    {
        TrackerBuilder::new()
            .kx(1.0)
            .kdx(1.0)
            .ky(1.0)
            .kdy(1.0)
            .initial_velocity(Velocity::new::<meter_per_second>(0.0))
            .valid_control_lower_bound(Velocity::new::<meter_per_second>(0.001))
            .right_motor(IMotor)
            .left_motor(IMotor)
            .period(Time::new::<second>(0.001))
            .translation_controller(IController::<Length>::new())
            .rotation_controller(IController::<Angle>::new())
            .low_zeta(1.0)
            .low_b(1e-3)
            .fail_safe_distance(Length::new::<meter>(0.02))
            .logger(logger)
            .build()
    }

    #[test]
    fn test_build() {
        let _tracker = build_tracker(NullLogger);
    }

    #[cfg(feature = "log")]
    mod log_tests {
        use super::*;
        use std::cell::RefCell;
        use std::fmt::Write;
        use std::rc::Rc;

        struct ILogger {
            raw: Rc<RefCell<String>>,
        }

        impl ILogger {
            fn new(raw: Rc<RefCell<String>>) -> Self {
                Self { raw }
            }
        }

        impl Logger for ILogger {
            fn log(&self, state: &State, target: &Target) {
                write!(self.raw.borrow_mut(), "{:?},{:?}", state, target).unwrap();
            }
        }

        #[test]
        fn test_log() {
            use crate::prelude::*;

            let log = Rc::new(RefCell::new(String::new()));
            let state = State::default();
            let target = Target::default();
            let mut tracker = build_tracker(ILogger::new(Rc::clone(&log)));
            tracker.track(state.clone(), target.clone());
            assert_eq!(log.borrow().as_ref(), format!("{:?},{:?}", state, target));
        }
    }
}
