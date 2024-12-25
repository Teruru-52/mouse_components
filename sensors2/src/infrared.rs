use core::marker::PhantomData;
use embedded_hal::{adc::Channel, adc::OneShot, PwmPin};
use nb::block;
// use uom::si::{f32::Length, length::meter, ratioratio::};
use spin::Mutex;

pub struct Infrared<ADC, AdcPin>
where
    AdcPin: Channel<ADC>,
{
    adc_pin: AdcPin,
    value: u16,
    _adc_marker: PhantomData<ADC>,
    ratio: f32,
}

impl<ADC, AdcPin> Infrared<ADC, AdcPin>
where
    AdcPin: Channel<ADC>,
{
    pub fn new(adc_pin: AdcPin, duty_ratio: f32) -> Self {
        let mut infrared = Self {
            adc_pin,
            value: 0,
            _adc_marker: PhantomData,
            ratio: duty_ratio,
        };

        infrared
    }

    pub fn init<P>(&mut self, tim_pin: &mut P)
    where
        P: PwmPin<Duty = u16>,
    {
        self.apply(self.ratio, tim_pin);
    }

    pub fn apply<P>(&mut self, mut duty_ratio: f32, tim_pin: &mut P)
    where
        P: PwmPin<Duty = u16>,
    {
        if duty_ratio > 1.0 {
            duty_ratio = 1.0;
        } else if duty_ratio < 0.0 {
            duty_ratio = 0.0;
        }
        tim_pin.set_duty(tim_pin.get_max_duty());
        tim_pin.set_duty((duty_ratio * tim_pin.get_max_duty() as f32) as u16);
        self.ratio = duty_ratio;
    }

    #[allow(unused)]
    pub fn update_value<T>(&mut self, adc: &mut Mutex<T>)
    where
        T: OneShot<ADC, u16, AdcPin>,
        <T as OneShot<ADC, u16, AdcPin>>::Error: core::fmt::Debug,
    {
        self.value = block!(adc.lock().read(&mut self.adc_pin)).unwrap() as u16;
    }

    #[allow(unused)]
    pub fn value(&self) -> u16 {
        self.value
    }
}
