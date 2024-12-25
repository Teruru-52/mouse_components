use core::marker::PhantomData;
use embedded_hal::adc::Channel;
use nb::block;
use stm32f4xx_hal::adc::{Adc, AdcConfig, OneShot};
use stm32f4xx_hal::gpio::{Analog, Pin};
use uom::si::{
    electric_potential::volt,
    f32::{ElectricPotential, Frequency, Time},
    ratio::ratio,
};

pub struct Voltmeter<ADC, PIN>
where
    ADC: OneShot<Adc<ADC>, u16, PIN>,
    PIN: Channel<Adc<ADC>>,
{
    adc_pin: PIN,
    voltage: ElectricPotential,
    alpha: f32,
    ratio: f32,
    _adc_marker: PhantomData<ADC>,
}

impl<ADC, PIN> Voltmeter<ADC, PIN>
where
    ADC: OneShot<Adc<ADC>, u16, PIN>,
    PIN: Channel<Adc<ADC>>,
    <Adc<ADC> as OneShot<Adc<ADC>, u16, PIN>>::Error: core::fmt::Debug,
{
    const AVDD_VOLTAGE: ElectricPotential = ElectricPotential {
        dimension: PhantomData,
        units: PhantomData,
        value: 3.3,
    };
    const MAX_ADC_VALUE: f32 = 4096.0;
    const SUM_NUM: u16 = 100;

    pub fn new(
        adc_pin: PIN,
        period: Time,
        cut_off_frequency: Frequency,
        battery_ratio: f32,
    ) -> Self {
        let alpha =
            1.0 / (2.0 * core::f32::consts::PI * (period * cut_off_frequency).get::<ratio>() + 1.0);
        let voltmeter = Self {
            adc_pin,
            voltage: ElectricPotential::new::<volt>(0.0),
            alpha,
            ratio: battery_ratio,
            _adc_marker: PhantomData,
        };

        // let mut sum = ElectricPotential::default();
        // for _ in 0..Self::SUM_NUM {
        //     sum += voltmeter.current_voltage();
        // }
        // voltmeter.voltage = sum / Self::SUM_NUM as f32;
        voltmeter
    }

    pub fn update_voltage(&mut self, adc: &mut Adc<ADC>) {
        self.voltage = self.alpha * self.voltage + (1.0 - self.alpha) * self.current_voltage(adc);
    }

    fn current_voltage(&mut self, adc: &mut Adc<ADC>) -> ElectricPotential {
        let value = block!(adc.read(&mut self.adc_pin)).unwrap() as f32;
        value * Self::AVDD_VOLTAGE * self.ratio / Self::MAX_ADC_VALUE
    }

    pub fn voltage(&self) -> ElectricPotential {
        self.voltage
    }
}
