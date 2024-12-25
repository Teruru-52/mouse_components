use core::marker::PhantomData;
use embedded_hal::{adc::Channel, adc::OneShot, PwmPin};
use nb::block;
// use uom::si::{f32::Length, length::meter, ratioratio::};
use spin::Mutex;

pub struct Infrared<T, ADC, AdcPin, TimPin>
where
    T: OneShot<ADC, u16, AdcPin>,
    AdcPin: Channel<ADC>,
    TimPin: PwmPin,
{
    adc: Mutex<T>,
    adc_pin: AdcPin,
    value: u16,
    _adc_marker: PhantomData<ADC>,
    tim_pin: Mutex<TimPin>,
    ratio: f32,
}

impl<T, ADC, AdcPin, TimPin> Infrared<T, ADC, AdcPin, TimPin>
where
    T: OneShot<ADC, u16, AdcPin>,
    AdcPin: Channel<ADC>,
    <T as OneShot<ADC, u16, AdcPin>>::Error: core::fmt::Debug,
    TimPin: PwmPin<Duty = u16>,
{
    pub fn new(adc: Mutex<T>, adc_pin: AdcPin, tim_pin: Mutex<TimPin>, duty_ratio: f32) -> Self {
        let mut infrared = Self {
            adc,
            adc_pin,
            value: 0,
            _adc_marker: PhantomData,
            tim_pin,
            ratio: duty_ratio,
        };

        infrared.init();
        infrared
    }

    pub fn init(&mut self) {
        self.apply(self.ratio);
    }

    pub fn apply(&mut self, mut duty_ratio: f32) {
        if duty_ratio > 1.0 {
            duty_ratio = 1.0;
        } else if duty_ratio < 0.0 {
            duty_ratio = 0.0;
        }
        self.tim_pin
            .lock()
            .set_duty(self.tim_pin.lock().get_max_duty());
        self.tim_pin
            .lock()
            .set_duty((duty_ratio * self.tim_pin.lock().get_max_duty() as f32) as u16);
    }

    #[allow(unused)]
    fn update_value(&mut self) {
        self.value = block!(self.adc.lock().read(&mut self.adc_pin)).unwrap() as u16;
    }

    #[allow(unused)]
    fn value(&self) -> u16 {
        self.value
    }
}
