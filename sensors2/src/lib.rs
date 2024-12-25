#![no_std]

pub mod encoder;
pub mod imu;
pub mod infrared;
pub mod motor;
pub mod speaker;
pub mod tof;
pub mod voltmeter;

#[macro_export]
macro_rules! wait_ok {
    ($e: expr) => {
        while $e.is_err() {}
    };
}
