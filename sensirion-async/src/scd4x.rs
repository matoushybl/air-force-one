use embedded_hal_async::i2c::I2c;

use crate::{Error, SensirionCommand, SensirionI2c};

const SENSOR_ADDR: u8 = 0x62;

enum Command {
    StartPeriodicMeasurement,
    StopPeriodicMeasurement,
    ReadMeasurement,
    SetTemperatureOffset,
    GetTemperatureOffset,
    SetAmbientPressure,
    GetSerialNumber,
    GetDataReady,
    PersistSettings,
    SetSensorAltitude,
    GetSensorAltitude,
}

impl SensirionCommand for Command {
    fn raw(&self) -> u16 {
        match self {
            Command::StartPeriodicMeasurement => 0x21b1,
            Command::StopPeriodicMeasurement => 0x3f86,
            Command::ReadMeasurement => 0xec05,
            Command::SetTemperatureOffset => 0x241d,
            Command::GetTemperatureOffset => 0x2318,
            Command::SetAmbientPressure => 0xe000,
            Command::GetSerialNumber => 0x3682,
            Command::GetDataReady => 0xe4b8,
            Command::PersistSettings => 0x3615,
            Command::SetSensorAltitude => 0x2427,
            Command::GetSensorAltitude => 0x2322,
        }
    }
}

pub struct Scd4x<T>
where
    T: I2c,
{
    bus: SensirionI2c<T>,
}

impl<T> Scd4x<T>
where
    T: I2c,
{
    pub fn new(bus: T) -> Self {
        Self {
            bus: SensirionI2c::new(bus),
        }
    }

    // TODO refactor to calculations module
    pub async fn read_serial_number(&mut self) -> Result<u64, Error<T::Error>> {
        let mut result = [0u8; 9];
        self.bus
            .read_raw(SENSOR_ADDR, Command::GetSerialNumber, &mut result)
            .await?;

        if self.bus.crc.calculate(&result[..2]) != result[2] {
            return Err(Error::Parsing(crate::ParsingError::Crc));
        }

        if self.bus.crc.calculate(&result[3..5]) != result[5] {
            return Err(Error::Parsing(crate::ParsingError::Crc));
        }

        if self.bus.crc.calculate(&result[6..8]) != result[8] {
            return Err(Error::Parsing(crate::ParsingError::Crc));
        }

        let word0 = u16::from_be_bytes(result[..2].try_into().unwrap()) as u64;
        let word1 = u16::from_be_bytes(result[3..5].try_into().unwrap()) as u64;
        let word2 = u16::from_be_bytes(result[6..8].try_into().unwrap()) as u64;
        let serial_number = word0 << 32 | word1 << 16 | word2;

        Ok(serial_number)
    }

    pub async fn start_periodic_measurement(&mut self) -> Result<(), Error<T::Error>> {
        self.bus
            .write_command(SENSOR_ADDR, Command::StartPeriodicMeasurement)
            .await
    }

    pub async fn stop_periodic_measurement(&mut self) -> Result<(), Error<T::Error>> {
        self.bus
            .write_command(SENSOR_ADDR, Command::StopPeriodicMeasurement)
            .await
    }

    // TODO test
    pub async fn set_ambient_pressure(&mut self, pressure: Pascal) -> Result<(), Error<T::Error>> {
        self.bus
            .write_word(SENSOR_ADDR, Command::SetAmbientPressure, pressure.0)
            .await
    }

    // TODO test
    pub async fn set_temperature_offset(
        &mut self,
        temperature: Celsius,
    ) -> Result<(), Error<T::Error>> {
        self.bus
            .write_word(
                SENSOR_ADDR,
                Command::SetTemperatureOffset,
                (temperature.0 * u16::MAX as f32 / 175.0) as u16,
            )
            .await
    }

    pub async fn get_temperature_offset(&mut self) -> Result<Celsius, Error<T::Error>> {
        let raw = self
            .bus
            .read_word(SENSOR_ADDR, Command::GetTemperatureOffset, true)
            .await?;
        Ok(Celsius((175.0 * raw as f32) / u16::MAX as f32))
    }

    pub async fn get_sensor_altitude(&mut self) -> Result<Meter, Error<T::Error>> {
        self.bus
            .read_word(SENSOR_ADDR, Command::GetSensorAltitude, true)
            .await
            .map(Meter)
    }

    pub async fn set_sensor_altitude(&mut self, altitude: Meter) -> Result<(), Error<T::Error>> {
        self.bus
            .write_word(SENSOR_ADDR, Command::SetSensorAltitude, altitude.0)
            .await
    }

    // must wait for 800 ms after issuing
    pub async fn persist_settings(&mut self) -> Result<(), Error<T::Error>> {
        self.bus
            .write_command(SENSOR_ADDR, Command::PersistSettings)
            .await
    }

    pub async fn read(&mut self) -> Result<Measurement, Error<T::Error>> {
        let mut result = [0u8; 9];
        self.bus
            .read_raw(SENSOR_ADDR, Command::ReadMeasurement, &mut result)
            .await?;

        defmt::error!("result: {:x}", result);

        let raw_temp = u16::from_be_bytes(result[3..5].try_into().unwrap());
        let raw_humidity = u16::from_be_bytes(result[6..8].try_into().unwrap());

        Ok(Measurement {
            co2: u16::from_be_bytes(result[..2].try_into().unwrap()),
            temperature: -45.0 + 175.0 * (raw_temp as f32) / (u16::MAX as f32),
            humidity: 100.0 * raw_humidity as f32 / u16::MAX as f32,
        })
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Meter(pub u16);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Celsius(pub f32);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Pascal(pub u16);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Measurement {
    pub co2: u16,
    pub temperature: f32,
    pub humidity: f32,
}
