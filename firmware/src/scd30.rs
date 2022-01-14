use embassy::traits::i2c::{I2c, SevenBitAddress};

use crate::sensirion_i2c::{Error, SensirionCommand, SensirionI2c};

const SENSOR_ADDR: u8 = 0x61;

#[derive(Clone, Copy)]
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

#[derive(Copy, Clone, Default)]
pub struct SensorData {
    pub co2: f32,
    pub temperature: f32,
    pub humidity: f32,
}

pub struct SCD30<'a, T>
where
    T: I2c<SevenBitAddress>,
    T::Error: defmt::Format,
{
    bus: SensirionI2c<'a, T>,
}

impl<'a, T> SCD30<'a, T>
where
    T: I2c<SevenBitAddress>,
    T::Error: defmt::Format,
{
    pub fn init(bus: SensirionI2c<'a, T>) -> Self {
        Self { bus }
    }

    pub async fn read_fw_version(&mut self) -> Result<u16, Error<T::Error>> {
        self.bus
            .read_word(SENSOR_ADDR, Scd30Command::ReadFWVersion, true)
            .await
    }

    pub async fn set_measurement_interval(&mut self, seconds: u16) -> Result<(), Error<T::Error>> {
        self.bus
            .write_word(SENSOR_ADDR, Scd30Command::SetInterval, seconds)
            .await
    }

    pub async fn start_continuous_measurement(
        &mut self,
        pressure: u16,
    ) -> Result<(), Error<T::Error>> {
        self.bus
            .write_word(SENSOR_ADDR, Scd30Command::StartContMeasurement, pressure)
            .await
    }

    pub async fn get_data_ready(&mut self) -> Result<bool, Error<T::Error>> {
        let result = self
            .bus
            .read_word(SENSOR_ADDR, Scd30Command::DataReady, true)
            .await?;
        Ok(result == 1)
    }

    pub async fn read_measurement(&mut self) -> Result<SensorData, Error<T::Error>> {
        let mut result = [0u8; 18];
        self.bus
            .read_raw(SENSOR_ADDR, Scd30Command::ReadMeasurement, &mut result)
            .await?;
        Ok(SensorData {
            co2: self.slice_to_f32(&result[..6])?,
            temperature: self.slice_to_f32(&result[6..12])?,
            humidity: self.slice_to_f32(&result[12..])?,
        })
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

    fn slice_to_f32(&mut self, slice: &[u8]) -> Result<f32, Error<T::Error>> {
        if slice.len() != 6
            || self.bus.calculate_crc(&slice[..2]) != slice[2]
            || self.bus.calculate_crc(&slice[3..5]) != slice[5]
        {
            return Err(Error::Crc);
        }
        let mut buffer = [0u8; 4];
        buffer[0] = slice[0];
        buffer[1] = slice[1];
        buffer[2] = slice[3];
        buffer[3] = slice[4];
        Ok(f32::from_be_bytes(buffer))
    }
}
