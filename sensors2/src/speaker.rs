use embedded_hal::PwmPin;
use uom::si::{ratio::ratio};

pub struct Speaker<P>
where
    P: PwmPin,
{
    pwm_pin: P,
}

impl<P> Speaker<P>
where
    P: PwmPin,
{
    pub fn new(pwm_pin: P) -> Self {
        Self { pwm_pin }
    }
}

impl<P> Speaker<P>
where
    P: PwmPin<Duty = u16>,
{
    pub fn apply(&mut self, duty_ratio: ratio) {
        if duty_ratio > 1.0 {
            duty_ratio = 1.0;
        }
        else if duty_ratio < 0.0 {
            duty_ratio = 0.0;
        }
        self.pwm_pin.set_duty(((1.0 - duty_ratio) * self.pwm_pin.get_max_duty() as f32) as u16);
    }
}
