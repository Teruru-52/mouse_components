use core::marker::PhantomData;

use embedded_hal::adc::{Channel, OneShot};
use nb::block;
use spin::Mutex;
use uom::si::{
    electric_potential::volt,
    f32::{ElectricPotential, Frequency, Time},
    ratio::ratio,
};

pub struct Voltmeter<T, ADC, PIN>
where
    T: OneShot<ADC, u16, PIN>,
    PIN: Channel<ADC>,
{
    adc: Mutex<T>,
    adc_pin: PIN,
    voltage: ElectricPotential,
    alpha: f32,
    ratio: f32,
    _adc_marker: PhantomData<ADC>,
}

impl<T, ADC, PIN> Voltmeter<T, ADC, PIN>
where
    T: OneShot<ADC, u16, PIN>,
    PIN: Channel<ADC>,
    <T as OneShot<ADC, u16, PIN>>::Error: core::fmt::Debug,
{
    const AVDD_VOLTAGE: ElectricPotential = ElectricPotential {
        dimension: PhantomData,
        units: PhantomData,
        value: 3.3,
    };
    const MAX_ADC_VALUE: f32 = 4096.0;
    const SUM_NUM: u16 = 100;

    pub fn new(
        adc: Mutex<T>,
        adc_pin: PIN,
        period: Time,
        cut_off_frequency: Frequency,
        battery_ratio: f32,
    ) -> Self {
        let alpha =
            1.0 / (2.0 * core::f32::consts::PI * (period * cut_off_frequency).get::<ratio>() + 1.0);
        let mut voltmeter = Self {
            adc,
            adc_pin,
            voltage: ElectricPotential::new::<volt>(0.0),
            alpha,
            ratio: battery_ratio,
            _adc_marker: PhantomData,
        };

        let mut sum = ElectricPotential::default();
        for _ in 0..Self::SUM_NUM {
            sum += voltmeter.current_voltage();
        }
        voltmeter.voltage = sum / Self::SUM_NUM as f32;
        voltmeter
    }

    #[allow(unused)]
    pub fn update_voltage(&mut self) {
        self.voltage = self.alpha * self.voltage + (1.0 - self.alpha) * self.current_voltage();
    }

    fn current_voltage(&mut self) -> ElectricPotential {
        let value = block!(self.adc.lock().read(&mut self.adc_pin)).unwrap() as f32;
        value * Self::AVDD_VOLTAGE * self.ratio / Self::MAX_ADC_VALUE
    }

    pub fn voltage(&self) -> ElectricPotential {
        self.voltage
    }
}
