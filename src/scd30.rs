use core::cell::RefCell;
use core::convert::TryInto;
use defmt::{panic, *};
use embassy::time::{Duration, Timer};
use embassy::traits::i2c::{AddressMode, I2c, SevenBitAddress};
use embassy_nrf::twim::{self, Instance, Twim};
use embassy_nrf::{interrupt, Peripherals};
use embedded_hal::blocking::i2c::{Read, Write};
use futures_intrusive::sync::LocalMutex;

const SENSOR_ADDR: u8 = 0x61;

pub enum SCD30Command {
    ReadFWVersion,
    StartContMeasurement,
    DataReady,
    SetInterval,
    ReadMeasurement,
}

impl From<SCD30Command> for u16 {
    fn from(raw: SCD30Command) -> Self {
        match raw {
            SCD30Command::ReadFWVersion => 0xd100,
            SCD30Command::StartContMeasurement => 0x0010,
            SCD30Command::DataReady => 0x0202,
            SCD30Command::ReadMeasurement => 0x0300,
            SCD30Command::SetInterval => 0x4600,
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct SensorData {
    pub co2: f32,
    pub temperature: f32,
    pub humidity: f32,
}

pub struct SCD30<'a, T>(&'a LocalMutex<T>);

impl<'a, T> SCD30<'a, T>
where
    T: I2c<SevenBitAddress>,
{
    pub fn init(i2c: &'a LocalMutex<T>) -> Self {
        Self(i2c)
    }

    pub async fn read_fw_version(&mut self) -> Result<[u8; 2], T::Error> {
        let command: u16 = SCD30Command::ReadFWVersion.into();
        let mut bus = self.0.lock().await;
        bus.write(SENSOR_ADDR, &command.to_be_bytes()).await?;
        let mut result = [0u8; 2];
        bus.read(SENSOR_ADDR, &mut result).await?;
        Ok(result)
    }

    pub async fn set_measurement_interval(&mut self, seconds: u16) -> Result<(), T::Error> {
        let sensor_command: u16 = SCD30Command::SetInterval.into();
        let sensor_command = sensor_command.to_be_bytes();
        let raw_interval = seconds.to_be_bytes();
        let mut command: [u8; 5] = [
            sensor_command[0],
            sensor_command[1],
            raw_interval[0],
            raw_interval[1],
            0,
        ];

        let mut crc = crc_all::Crc::<u8>::new(0x31, 8, 0xff, 0x00, false);
        crc.update(&raw_interval);
        command[4] = crc.finish();

        self.0.lock().await.write(SENSOR_ADDR, &command).await?;

        Ok(())
    }

    pub async fn start_continuous_measurement(&mut self, pressure: u16) -> Result<(), T::Error> {
        let sensor_command: u16 = SCD30Command::StartContMeasurement.into();
        let sensor_command = sensor_command.to_be_bytes();
        let raw_pressure = pressure.to_be_bytes();
        let mut command: [u8; 5] = [
            sensor_command[0],
            sensor_command[1],
            raw_pressure[0],
            raw_pressure[1],
            0,
        ];

        let mut crc = crc_all::Crc::<u8>::new(0x31, 8, 0xff, 0x00, false);
        crc.update(&raw_pressure);
        command[4] = crc.finish();

        self.0.lock().await.write(SENSOR_ADDR, &command).await?;

        Ok(())
    }

    pub async fn get_data_ready(&mut self) -> Result<bool, T::Error> {
        let command: u16 = SCD30Command::DataReady.into();
        let mut bus = self.0.lock().await;
        bus.write(SENSOR_ADDR, &command.to_be_bytes()).await?;
        let mut result = [0u8; 3];
        bus.read(SENSOR_ADDR, &mut result).await?;
        Ok(u16::from_be_bytes((&result[0..2]).try_into().unwrap()) == 1)
    }

    pub async fn read_measurement(&mut self) -> Result<SensorData, T::Error> {
        let command: u16 = SCD30Command::ReadMeasurement.into();
        let mut bus = self.0.lock().await;
        bus.write(SENSOR_ADDR, &command.to_be_bytes()).await?;
        let mut result = [0u8; 18];
        bus.read(SENSOR_ADDR, &mut result).await?;
        defmt::trace!("res: {}", result);
        Ok(SensorData {
            co2: Self::slice_to_f32(&result[..6]).unwrap(),
            temperature: Self::slice_to_f32(&result[6..12]).unwrap(),
            humidity: Self::slice_to_f32(&result[12..]).unwrap(),
        })
    }

    // TODO check CRC
    fn slice_to_f32(slice: &[u8]) -> Result<f32, ()> {
        if slice.len() != 6 {
            return Err(());
        }
        let mut buffer = [0u8; 4];
        buffer[0] = slice[0];
        buffer[1] = slice[1];
        buffer[2] = slice[3];
        buffer[3] = slice[4];
        Ok(f32::from_be_bytes(buffer))
    }
}
