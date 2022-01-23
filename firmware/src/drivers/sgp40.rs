use embassy::time::Duration;
use embassy_traits::i2c::{I2c, SevenBitAddress};

use crate::drivers::sensirion_i2c::{Error, SensirionCommand, SensirionI2c};
use crate::drivers::vocalg::VocAlgorithm;

#[allow(unused)]
#[repr(u16)]
#[derive(Clone, Copy)]
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

const ADDRESS: u8 = 0x59;

pub struct Sgp40<'a, T>
where
    T: I2c<SevenBitAddress>,
    T::Error: defmt::Format,
{
    bus: SensirionI2c<'a, T>,
    voc: VocAlgorithm,
}

impl<'a, T> Sgp40<'a, T>
where
    T: I2c<SevenBitAddress>,
    T::Error: defmt::Format,
{
    pub fn init(i2c: SensirionI2c<'a, T>) -> Self {
        Self {
            bus: i2c,
            voc: VocAlgorithm::default(),
        }
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

    pub async fn measure_raw(
        &mut self,
        humidity: f32,
        temperature: f32,
    ) -> Result<u16, Error<T::Error>> {
        let command = Command::MeasureRaw.raw();
        let raw_command = command.to_be_bytes();
        let raw_humidity = (((humidity / 100.0) * 65535.0) as u16).to_be_bytes();
        let raw_temp = ((((temperature + 45.0) / 175.0) * 65535.0) as u16).to_be_bytes();
        let write_data = [
            raw_command[0],
            raw_command[1],
            raw_humidity[0],
            raw_humidity[1],
            self.bus.calculate_crc(&raw_humidity),
            raw_temp[0],
            raw_temp[1],
            self.bus.calculate_crc(&raw_temp),
        ];
        let mut result = [0u8; 3];
        self.bus
            .write_read_raw(
                ADDRESS,
                &write_data,
                &mut result,
                Some(Duration::from_millis(30)),
            )
            .await?;
        Ok(u16::from_be_bytes((&result[..2]).try_into().unwrap()))
    }

    pub async fn measure_voc_index(
        &mut self,
        humidity: f32,
        temperature: f32,
    ) -> Result<u16, Error<T::Error>> {
        let raw = self.measure_raw(humidity, temperature).await?;
        Ok(self.voc.process(raw as i32) as u16)
    }
}
