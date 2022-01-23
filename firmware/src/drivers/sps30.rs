use defmt::Format;
use embassy_traits::i2c::{I2c, SevenBitAddress};

use crate::drivers::sensirion_i2c::{Error, SensirionCommand, SensirionI2c};

#[repr(u16)]
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

const SENSOR_ADDR: u8 = 0x69;

pub struct Sps30<'a, T>
where
    T: I2c<SevenBitAddress>,
    T::Error: defmt::Format,
{
    bus: SensirionI2c<'a, T>,
}

impl<'a, T> Sps30<'a, T>
where
    T: I2c<SevenBitAddress>,
    T::Error: defmt::Format,
{
    pub fn new(bus: SensirionI2c<'a, T>) -> Self {
        Self { bus }
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

    pub async fn is_ready(&mut self) -> Result<bool, Error<T::Error>> {
        let result = self
            .bus
            .read_word(SENSOR_ADDR, Sps30Command::ReadDataReadyFlag, true)
            .await?;
        Ok(result == 1)
    }

    pub async fn read_measured_data(&mut self) -> Result<PMInfo, Error<T::Error>> {
        let mut buffer: [u8; 60] = [0; 60];
        self.bus
            .read_raw(SENSOR_ADDR, Sps30Command::ReadMeasuredValues, &mut buffer)
            .await?;

        Ok(PMInfo {
            mass_pm1_0: self.process_data_slice(&buffer[..6]),
            mass_pm2_5: self.process_data_slice(&buffer[6..]),
            mass_pm4_0: self.process_data_slice(&buffer[12..]),
            mass_pm10: self.process_data_slice(&buffer[18..]),
            number_pm0_5: self.process_data_slice(&buffer[24..]),
            number_pm1_0: self.process_data_slice(&buffer[30..]),
            number_pm2_5: self.process_data_slice(&buffer[36..]),
            number_pm4_0: self.process_data_slice(&buffer[42..]),
            number_pm10: self.process_data_slice(&buffer[48..]),
            typical_size: self.process_data_slice(&buffer[54..]),
        })
    }

    fn process_data_slice(&mut self, buffer: &[u8]) -> f32 {
        let raw = [buffer[0], buffer[1], buffer[3], buffer[4]];
        let crc1 = self.bus.calculate_crc(&buffer[..2]);

        let crc2 = self.bus.calculate_crc(&buffer[3..5]);

        if crc1 != buffer[2] || crc2 != buffer[5] {
            defmt::error!("crc invalid");
        }
        f32::from_be_bytes(raw)
    }
}

#[derive(Format)]
pub struct PMInfo {
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
