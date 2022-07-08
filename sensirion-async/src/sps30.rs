use embedded_hal_async::i2c::I2c;
use getset::CopyGetters;

use crate::{Error, SensirionCommand, SensirionI2c};

#[repr(u16)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Sps30Command {
    StartMeasurement,
    StopMeasurement,
    ReadDataReadyFlag,
    ReadMeasuredValues,
    Sleep,
    WakeUp,
    StartFanCleaning,
    AutoCleaningInterval,
    ReadProductType,
    ReadSerialNumber,
    ReadVersion,
    ReadDeviceStatusRegister,
    ClearDeviceStatusRegister,
    Reset,
}

impl SensirionCommand for Sps30Command {
    fn raw(&self) -> u16 {
        match self {
            Self::StartMeasurement => 0x0010,
            Self::StopMeasurement => 0x0104,
            Self::ReadDataReadyFlag => 0x0202,
            Self::ReadMeasuredValues => 0x0300,
            Self::Sleep => 0x1001,
            Self::WakeUp => 0x1103,
            Self::StartFanCleaning => 0x5607,
            Self::AutoCleaningInterval => 0x8004,
            Self::ReadProductType => 0xd002,
            Self::ReadSerialNumber => 0xd033,
            Self::ReadVersion => 0xd100,
            Self::ReadDeviceStatusRegister => 0xd206,
            Self::ClearDeviceStatusRegister => 0xd210,
            Self::Reset => 0xd304,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MeasurementOutputFormat {
    Float = 0x03,
    Integer = 0x05,
}

impl From<MeasurementOutputFormat> for u8 {
    fn from(raw: MeasurementOutputFormat) -> Self {
        match raw {
            MeasurementOutputFormat::Float => 0x03,
            MeasurementOutputFormat::Integer => 0x05,
        }
    }
}

#[derive(Clone, Copy, Debug, CopyGetters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[get_copy = "pub"]
pub struct Measurement {
    /// Mass Concentration PM1.0 [μg/m³]
    pub mass_pm1_0: f32,
    /// Mass Concentration PM2.5 [μg/m³]
    pub mass_pm2_5: f32,
    /// Mass Concentration PM4.0 [μg/m³]
    pub mass_pm4_0: f32,
    /// Mass Concentration PM10 [μg/m³]
    pub mass_pm10: f32,
    /// Number Concentration PM0.5 [#/cm³]
    pub number_pm0_5: f32,
    /// Number Concentration PM1.0 [#/cm³]
    pub number_pm1_0: f32,
    /// Number Concentration PM2.5 [#/cm³]
    pub number_pm2_5: f32,
    /// Number Concentration PM4.0 [#/cm³]
    pub number_pm4_0: f32,
    /// Number Concentration PM10 [#/cm³]
    pub number_pm10: f32,
    /// Typical Particle Size [μm]
    pub typical_size: f32,
}

const SENSOR_ADDR: u8 = 0x69;

pub struct Sps30<T>
where
    T: I2c,
{
    bus: SensirionI2c<T>,
}

impl<T> Sps30<T>
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
            .read_word(SENSOR_ADDR, Sps30Command::ReadVersion, true)
            .await
    }

    pub async fn start_measurement(&mut self) -> Result<(), Error<T::Error>> {
        self.bus
            .write_word(
                SENSOR_ADDR,
                Sps30Command::StartMeasurement,
                (MeasurementOutputFormat::Float as u16) << 8,
            )
            .await
    }

    pub async fn is_measurement_ready(&mut self) -> Result<bool, Error<T::Error>> {
        let result = self
            .bus
            .read_word(SENSOR_ADDR, Sps30Command::ReadDataReadyFlag, true)
            .await?;
        Ok(result == 1)
    }

    pub async fn read(&mut self) -> Result<Measurement, Error<T::Error>> {
        let mut buffer: [u8; 60] = [0; 60];
        self.bus
            .read_raw(SENSOR_ADDR, Sps30Command::ReadMeasuredValues, &mut buffer)
            .await?;

        Ok(raw_data_processing::parse_measurement(
            &buffer,
            &mut self.bus.crc,
        )?)
    }
}

mod raw_data_processing {
    use crate::{ParsingError, SensirionCrc};

    use super::*;

    pub(super) fn parse_measurement(
        data: &[u8; 60],
        crc: &mut SensirionCrc,
    ) -> Result<Measurement, ParsingError> {
        Ok(Measurement {
            mass_pm1_0: slice_to_f32(&data[..6], crc)?,
            mass_pm2_5: slice_to_f32(&data[6..], crc)?,
            mass_pm4_0: slice_to_f32(&data[12..], crc)?,
            mass_pm10: slice_to_f32(&data[18..], crc)?,
            number_pm0_5: slice_to_f32(&data[24..], crc)?,
            number_pm1_0: slice_to_f32(&data[30..], crc)?,
            number_pm2_5: slice_to_f32(&data[36..], crc)?,
            number_pm4_0: slice_to_f32(&data[42..], crc)?,
            number_pm10: slice_to_f32(&data[48..], crc)?,
            typical_size: slice_to_f32(&data[54..], crc)?,
        })
    }

    fn slice_to_f32(buffer: &[u8], crc: &mut SensirionCrc) -> Result<f32, ParsingError> {
        let crc1 = crc.calculate(&buffer[..2]);
        let crc2 = crc.calculate(&buffer[3..5]);

        if crc1 != buffer[2] || crc2 != buffer[5] {
            return Err(ParsingError::Crc);
        }
        Ok(f32::from_be_bytes([
            buffer[0], buffer[1], buffer[3], buffer[4],
        ]))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn measurement_parsing() {
            let mut crc = SensirionCrc::new();

            #[rustfmt::skip]
            let data = [
                0x40, 0x88, 0xcb, 0xe1, 0x6d, 0xf4, 
                0x40, 0x94, 0xf5, 0xb2, 0xea, 0xd3, 
                0x40, 0x98, 0x88, 0xd7, 0xed, 0x66, 
                0x40, 0x9b, 0xdb, 0x06, 0xca, 0x47, 
                0x41, 0xe4, 0xf9, 0xc5, 0x23, 0x89, 
                0x42, 0x05, 0x24, 0x43, 0x7e, 0xc2, 
                0x42, 0x06, 0x77, 0x68, 0x6c, 0x25,
                0x42, 0x06, 0x77, 0x93, 0x8c, 0xe6, 
                0x42, 0x06, 0x77, 0xa2, 0x9b, 0x74, 
                0x3f, 0x0d, 0xe6, 0x09, 0x1c, 0x7c,
            ];

            let measurement = parse_measurement(&data, &mut crc);
            assert!(measurement.is_ok());
            let measurement = measurement.unwrap();

            assert_eq!(measurement.mass_pm1_0 as u16, 4);
        }
    }
}
