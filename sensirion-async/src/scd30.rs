use crate::{Error, SensirionCommand, SensirionI2c};
use embedded_hal_async::i2c::I2c;
use getset::CopyGetters;

const SENSOR_ADDR: u8 = 0x61;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Scd30Command {
    ReadFWVersion,
    StartContMeasurement,
    DataReady,
    SetInterval,
    ReadMeasurement,
    SetTemperatureOffset,
}

impl SensirionCommand for Scd30Command {
    fn raw(&self) -> u16 {
        match self {
            Self::ReadFWVersion => 0xd100,
            Self::StartContMeasurement => 0x0010,
            Self::DataReady => 0x0202,
            Self::ReadMeasurement => 0x0300,
            Self::SetInterval => 0x4600,
            Self::SetTemperatureOffset => 0x5403,
        }
    }
}

#[derive(Copy, Clone, Default, Debug, CopyGetters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[get_copy = "pub"]
pub struct Measurement {
    pub co2: f32,
    pub temperature: f32,
    pub humidity: f32,
}

pub struct Scd30<T>
where
    T: I2c,
{
    bus: SensirionI2c<T>,
}

impl<'a, T> Scd30<T>
where
    T: I2c,
{
    pub fn new(bus: T) -> Self {
        Self {
            bus: SensirionI2c::new(bus),
        }
    }

    pub async fn read_version(&mut self) -> Result<u16, Error<T::Error>> {
        self.bus
            .read_word(SENSOR_ADDR, Scd30Command::ReadFWVersion, true)
            .await
    }

    pub async fn start_measurement(&mut self, pressure: u16) -> Result<(), Error<T::Error>> {
        self.bus
            .write_word(SENSOR_ADDR, Scd30Command::StartContMeasurement, pressure)
            .await
    }

    pub async fn is_measurement_ready(&mut self) -> Result<bool, Error<T::Error>> {
        let result = self
            .bus
            .read_word(SENSOR_ADDR, Scd30Command::DataReady, true)
            .await?;
        Ok(result == 1)
    }

    pub async fn read(&mut self) -> Result<Measurement, Error<T::Error>> {
        let mut result = [0u8; 18];
        self.bus
            .read_raw(SENSOR_ADDR, Scd30Command::ReadMeasurement, &mut result)
            .await?;
        Ok(raw_data_processing::parse_measurement(
            &result,
            &mut self.bus.crc,
        )?)
    }

    pub async fn set_measurement_interval(&mut self, seconds: u16) -> Result<(), Error<T::Error>> {
        self.bus
            .write_word(SENSOR_ADDR, Scd30Command::SetInterval, seconds)
            .await
    }

    pub async fn get_temperature_offset(&mut self) -> Result<u16, Error<T::Error>> {
        self.bus
            .read_word(SENSOR_ADDR, Scd30Command::SetTemperatureOffset, true)
            .await
    }

    pub async fn set_temperature_offset(&mut self, degrees: f32) -> Result<(), Error<T::Error>> {
        let raw_offset = (degrees * 100.0) as u16;
        self.bus
            .write_word(SENSOR_ADDR, Scd30Command::SetTemperatureOffset, raw_offset)
            .await
    }
}

mod raw_data_processing {
    use super::*;
    use crate::{ParsingError, SensirionCrc};

    pub(super) fn parse_measurement(
        data: &[u8; 18],
        crc: &mut SensirionCrc,
    ) -> Result<Measurement, ParsingError> {
        Ok(Measurement {
            co2: slice_to_f32(&data[..6], crc)?,
            temperature: slice_to_f32(&data[6..12], crc)?,
            humidity: slice_to_f32(&data[12..], crc)?,
        })
    }

    fn slice_to_f32(slice: &[u8], crc: &mut SensirionCrc) -> Result<f32, ParsingError> {
        if slice.len() != 6
            || crc.calculate(&slice[..2]) != slice[2]
            || crc.calculate(&slice[3..5]) != slice[5]
        {
            return Err(ParsingError::Crc);
        }

        Ok(f32::from_be_bytes([slice[0], slice[1], slice[3], slice[4]]))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn measurement_parsing() {
            let mut crc = SensirionCrc::new();

            #[rustfmt::skip]
            let data = [
                0x43, 0xDB, 0xCB, 0x8C, 0x2E, 0x8F,
                0x41, 0xD9, 0x70, 0xE7, 0xFF, 0xF5, 
                0x42, 0x43, 0xBF, 0x3A, 0x1B, 0x74,
            ];

            let measurement = parse_measurement(&data, &mut crc);
            assert!(measurement.is_ok());
            let measurement = measurement.unwrap();

            assert_eq!(measurement.co2 as u16, 439);
            assert_eq!(measurement.humidity as u16, 48);
            assert_eq!(measurement.temperature as u16, 27);
        }
    }
}
