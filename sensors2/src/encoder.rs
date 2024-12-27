use crate::wait_ok;
use core::convert::Infallible;
use core::marker::PhantomData;
use embedded_hal::{
    blocking::delay::DelayMs, blocking::spi::Transfer, digital::v2::OutputPin, timer::CountDown,
    Qei,
};
use nb::block;
use uom::si::{angle::revolution, f32::Angle, f32::AngularVelocity};

pub struct MA702GQ<Q>
where
    Q: Qei,
{
    qei: Q,
    before_count: u16,
}

impl<Q> MA702GQ<Q>
where
    Q: Qei,
{
    const RESOLUTION_PER_ROTATION: f32 = 1024.0;

    pub fn new(qei: Q) -> Self {
        Self {
            qei,
            before_count: 0,
        }
    }

    #[inline]
    fn get_lower_bits(&self, val: u32) -> u16 {
        (val & core::u16::MAX as u32) as u16
    }
}

impl<Q> MA702GQ<Q>
where
    Q: Qei,
    Q::Count: Into<u32>,
{
    pub fn angle(&mut self) -> nb::Result<Angle, core::convert::Infallible> {
        let after_count = self.get_lower_bits(self.qei.count().into());
        let relative_count = if after_count > self.before_count {
            (after_count - self.before_count) as i16
        } else {
            -((self.before_count - after_count) as i16)
        };
        self.before_count = after_count;
        Ok(Angle::new::<revolution>(
            relative_count as f32 / Self::RESOLUTION_PER_ROTATION,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AS5055AError;

impl From<Infallible> for AS5055AError {
    fn from(_error: Infallible) -> Self {
        Self
    }
}

pub struct AS5055A<T> {
    cs: T,
    angle: Angle,
    prev_angle: Angle,
}

impl<T> AS5055A<T>
where
    T: OutputPin,
{
    //RA: register address
    const ANGLE_OUT: u16 = 0xFFFF;
    const SCALE_FACTOR: Angle = Angle {
        dimension: PhantomData,
        units: PhantomData,
        value: 0.001_534_355_3, // 2*pi/4095
    };

    pub fn new<S, V, W>(spi: &mut S, cs: T, delay: &mut V, timer: &mut W) -> Self
    where
        S: Transfer<u8>,
        V: DelayMs<u32>,
        W: CountDown,
    {
        let mut as5055a = Self {
            cs,
            angle: Default::default(),
            prev_angle: Default::default(),
        };

        as5055a.init(spi, delay, timer);

        as5055a
    }

    pub fn init<S, V, W>(&mut self, spi: &mut S, delay: &mut V, timer: &mut W)
    where
        S: Transfer<u8>,
        V: DelayMs<u32>,
        W: CountDown,
    {
        self.angle = self.angle(spi).unwrap();
        self.prev_angle = self.angle;
    }

    fn assert(&mut self) -> Result<(), AS5055AError> {
        self.cs.set_low().map_err(|_| AS5055AError)
    }

    fn deassert(&mut self) -> Result<(), AS5055AError> {
        self.cs.set_high().map_err(|_| AS5055AError)
    }

    //size of buffer should be equal to {data length}+1
    fn read_from_registers<'w, S: Transfer<u8>>(
        &mut self,
        spi: &mut S,
        address: u16,
        buffer: &'w mut [u8],
    ) -> Result<&'w [u8], AS5055AError> {
        self.assert()?;
        let res = Self::_read_from_registers(spi, address, buffer);
        self.deassert()?;
        res
    }

    fn _read_from_registers<'w, S: Transfer<u8>>(
        spi: &mut S,
        address: u16,
        buffer: &'w mut [u8],
    ) -> Result<&'w [u8], AS5055AError> {
        buffer[0] = (address >> 8) as u8;
        buffer[1] = address as u8;
        let buffer = spi.transfer(buffer).map_err(|_| AS5055AError)?;
        Ok(&buffer[1..])
    }

    #[inline]
    fn connect_raw_data(&self, higher: u8, lower: u8) -> u16 {
        (((higher as u16) << 8 | lower as u16) & 0x3FFC >> 2) as u16
    }

    fn convert_raw_data_to_angle(&mut self, raw_value: u16) -> Angle {
        Self::SCALE_FACTOR * raw_value as f32
    }

    pub fn angle<S: Transfer<u8>>(&mut self, spi: &mut S) -> nb::Result<Angle, AS5055AError> {
        let mut buffer = [0; 4];
        let buffer = self.read_from_registers(spi, Self::ANGLE_OUT, &mut buffer)?;
        self.angle = -self.convert_raw_data_to_angle(self.connect_raw_data(buffer[0], buffer[1]));
        Ok(self.angle)
    }

    pub fn dist_angle<S: Transfer<u8>>(&mut self, spi: &mut S) -> nb::Result<Angle, AS5055AError> {
        self.angle(spi).unwrap();
        Ok(self.angle - self.prev_angle)
    }
}
