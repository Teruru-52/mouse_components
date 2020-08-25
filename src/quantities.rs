#[macro_use]
mod length {
    quantity! {
        /// Length (base unit meter, m).
        quantity: Length; "length";
        /// Length dimension, m.
        dimension: Q<
            P1,
            Z0,
            Z0
        >;
        units {
            @meter: 1.0E0; "m", "meter", "meters";
        }
    }
}

#[macro_use]
mod time {
    quantity! {
        /// Time (base unit second, s).
        quantity: Time; "time";
        /// Time dimension, s.
        dimension: Q<
            Z0,
            P1,
            Z0
        >;
        units {
            @second: 1.0; "s", "second", "seconds";
        }
    }
}

#[macro_use]
mod frequency {
    quantity! {
        quantity: Frequency; "frequency";
        dimension: Q<
            Z0,
            N1,
            Z0
        >;
        units {
            @hertz: 1.0; "Hz", "hertz", "hertz";
            @radian_per_second: 1.0 / (2.0 * core::f32::consts::PI); "rad/s", "radian per second", "radians per second";
            @degree_per_second: 1.0 / 360.0; "deg/s", "degree per second", "degrees per second";
        }
    }
    pub type AngularVelocity<U, V> = Frequency<U, V>;
}

#[macro_use]
mod angle {
    quantity! {
        quantity: Angle; "angle";
        dimension: Q<
            Z0,
            Z0,
            Z0
        >;
        units {
            @revolution: 1.0; "rev", "revolution", "revolutions";
            @radian: 1.0 / (2.0 * core::f32::consts::PI); "rad", "radian", "radians";
            @degree: 1.0 / 360.0; "deg", "degree", "degrees";
        }
    }
}

#[macro_use]
mod voltage {
    quantity! {
        quantity: Voltage; "voltage";
        dimension: Q<
            Z0,
            Z0,
            P1
        >;
        units {
            @volt: 1.0; "V", "volt", "volts";
        }
    }
}

#[macro_use]
mod velocity {
    quantity! {
        quantity: Velocity; "velocity";
        dimension: Q<
            P1,
            N1,
            Z0
        >;
        units {
            @meter_per_second: 1.0; "m/s", "meter per second", "meters per second";
        }
    }
}

#[macro_use]
mod acceleration {
    quantity! {
        quantity: Acceleration; "acceleration";
        dimension: Q<
            P1,
            N2,
            Z0
        >;
        units {
            @meter_per_second_squared: 1.0; "m/s^2", "meter per second squared", "meters per second squared";
        }
    }
}

#[macro_use]
mod jerk {
    quantity! {
        quantity: Jerk; "jerk";
        dimension: Q<
            P1,
            N3,
            Z0
        >;
        units {
            @meter_per_second_cubed: 1.0; "m/s^3", "meter per second cubed", "meters per second cubed";
        }
    }
}

#[macro_use]
mod squared_frequency {
    quantity! {
        quantity: SquaredFrequency; "squared frequency";
        dimension: Q<
            Z0,
            N2,
            Z0
        >;
        units {
            @squared_hertz: 1.0; "Hz^2", "hertz squared", "hertz squared";
            @radian_per_second_squared: 1.0 / (2.0 * core::f32::consts::PI); "rad/s^2", "radian per second squared", "radians per second squared";
            @degree_per_second_squared: 1.0 / 360.0; "deg/s^2", "degree per second squared", "degrees per second squared";
        }
    }
    pub type AngularAcceleration<U, V> = SquaredFrequency<U, V>;
}

#[macro_use]
mod cubed_frequency {
    quantity! {
        quantity: CubedFrequency; "cubed frequency";
        dimension: Q<
            Z0,
            N3,
            Z0
        >;
        units {
            @cubed_hertz: 1.0; "Hz^3", "hertz cubed", "hertz cubed";
            @radian_per_second_cubed: 1.0 / (2.0 * core::f32::consts::PI); "rad/s^3", "radian per second cubed", "radians per second cubed";
            @degree_per_second_cubed: 1.0 / 360.0; "deg/s^3", "degree per second cubed", "degrees per second cubed";
        }
    }
    pub type AngularJerk<U, V> = CubedFrequency<U, V>;
}

system! {
    quantities: Q {
        length: meter, L;
        time: second, T;
        voltage: volt, V;
    }

    units: U {
        mod length::Length,
        mod frequency::Frequency,
        mod frequency::AngularVelocity,
        mod time::Time,
        mod angle::Angle,
        mod velocity::Velocity,
        mod acceleration::Acceleration,
        mod jerk::Jerk,
        mod squared_frequency::SquaredFrequency,
        mod squared_frequency::AngularAcceleration,
        mod cubed_frequency::CubedFrequency,
        mod cubed_frequency::AngularJerk,
    }
}

mod f32 {
    mod com {
        pub use super::super::*;
    }

    Q!(self::com, f32);
}