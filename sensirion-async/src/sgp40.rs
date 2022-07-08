use embedded_hal_async::delay::DelayUs;
use embedded_hal_async::i2c::I2c;
use getset::CopyGetters;

use crate::vocalg::VocAlgorithm;
use crate::{Error, SensirionCommand, SensirionI2c};

#[allow(unused)]
#[repr(u16)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum Command {
    MeasureRaw = 0x260f,
    ExecuteSelfTest = 0x280e,
    TurnHeaterOff = 0x3615,
    GetSerialNumber = 0x3682,
}

impl SensirionCommand for Command {
    fn raw(&self) -> u16 {
        *self as u16
    }
}

#[derive(Clone, Copy, Debug, CopyGetters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[get_copy = "pub"]
pub struct Measurement {
    pub voc_index: u16,
    pub raw: u16,
}

const ADDRESS: u8 = 0x59;

pub struct Sgp40<T>
where
    T: I2c,
{
    bus: SensirionI2c<T>,
    voc: VocAlgorithm,
}

impl<T> Sgp40<T>
where
    T: I2c,
{
    pub fn new(i2c: T) -> Self {
        Self {
            bus: SensirionI2c::new(i2c),
            voc: VocAlgorithm::default(),
        }
    }

    pub async fn read(
        &mut self,
        humidity: f32,
        temperature: f32,
        delay: &mut impl DelayUs,
    ) -> Result<Measurement, Error<T::Error>> {
        let raw = self.read_raw(humidity, temperature, delay).await?;

        Ok(Measurement {
            voc_index: self.voc.process(raw as i32) as u16,
            raw,
        })
    }

    pub async fn get_serial_number(&mut self) -> Result<u64, Error<T::Error>> {
        let mut buffer = [0u8; 9];
        self.bus
            .read_raw(ADDRESS, Command::GetSerialNumber, &mut buffer)
            .await?;
        Ok(u64::from(buffer[0]) << 40
            | u64::from(buffer[1]) << 32
            | u64::from(buffer[3]) << 24
            | u64::from(buffer[4]) << 16
            | u64::from(buffer[6]) << 8
            | u64::from(buffer[7]))
    }

    pub async fn read_raw(
        &mut self,
        humidity: f32,
        temperature: f32,
        delay: &mut impl DelayUs,
    ) -> Result<u16, Error<T::Error>> {
        let write_data = raw_data_processing::compose_command(
            Command::MeasureRaw,
            humidity,
            temperature,
            &mut self.bus.crc,
        );
        let mut result = [0u8; 3];
        self.bus
            .write_read_raw(ADDRESS, &write_data, &mut result, 30, delay)
            .await?;
        Ok(raw_data_processing::parse_raw_measurement(
            &result,
            &mut self.bus.crc,
        )?)
    }
}

mod raw_data_processing {
    use crate::{ParsingError, SensirionCrc};

    use super::*;

    pub(super) fn compose_command(
        command: Command,
        humidity: f32,
        temperature: f32,
        crc: &mut SensirionCrc,
    ) -> [u8; 8] {
        let command = command.raw();
        let raw_command = command.to_be_bytes();
        let raw_humidity = (((humidity / 100.0) * 65535.0) as u16).to_be_bytes();
        let raw_temp = ((((temperature + 45.0) / 175.0) * 65535.0) as u16).to_be_bytes();
        [
            raw_command[0],
            raw_command[1],
            raw_humidity[0],
            raw_humidity[1],
            crc.calculate(&raw_humidity),
            raw_temp[0],
            raw_temp[1],
            crc.calculate(&raw_temp),
        ]
    }

    pub(super) fn parse_raw_measurement(
        data: &[u8; 3],
        crc: &mut SensirionCrc,
    ) -> Result<u16, ParsingError> {
        if crc.calculate(&data[..2]) != data[2] {
            return Err(ParsingError::Crc);
        }
        Ok(u16::from_be_bytes((&data[..2]).try_into().unwrap()))
    }
}
