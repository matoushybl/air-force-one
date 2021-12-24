use embassy::time::{Duration, Timer};
use embassy_traits::i2c::{I2c, SevenBitAddress};
use futures_intrusive::sync::LocalMutex;

use crate::sps30::Error;
use crate::vocalg::VocAlgorithm;

#[repr(u16)]
enum Command {
    MeasureRaw = 0x260f,
    ExecuteSelfTest = 0x280e,
    TurnHeaterOff = 0x3615,
    GetSerialNumber = 0x3682,
}

impl From<Command> for u16 {
    fn from(raw: Command) -> Self {
        raw as u16
    }
}

const ADDRESS: u8 = 0x59;

pub struct Sgp40<'a, T> {
    bus: &'a LocalMutex<T>,
    voc: VocAlgorithm,
}

impl<'a, T> Sgp40<'a, T>
where
    T: I2c<SevenBitAddress>,
    T::Error: defmt::Format,
{
    pub fn init(i2c: &'a LocalMutex<T>) -> Self {
        Self {
            bus: i2c,
            voc: VocAlgorithm::new(),
        }
    }

    pub async fn get_serial_number(&mut self) -> Result<u64, Error<T::Error>> {
        let mut buffer = [0u8; 9];
        self.read(Command::GetSerialNumber, &mut buffer, false)
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
        let command: u16 = Command::MeasureRaw.into();
        let raw_command = command.to_be_bytes();
        let raw_humidity = (((humidity / 100.0) * 65535.0) as u16).to_be_bytes();
        let raw_temp = ((((temperature + 45.0) / 175.0) * 65535.0) as u16).to_be_bytes();
        let write_data = [
            raw_command[0],
            raw_command[1],
            raw_humidity[0],
            raw_humidity[1],
            Self::crc(&raw_humidity),
            raw_temp[0],
            raw_temp[1],
            Self::crc(&raw_temp),
        ];
        let mut bus = self.bus.lock().await;
        bus.write(ADDRESS, &write_data).await?;
        Timer::after(Duration::from_millis(30)).await;
        let mut result = [0u8; 3];
        bus.read(ADDRESS, &mut result).await?;
        Ok(u16::from_be_bytes((&result[0..2]).try_into().unwrap()))
    }

    pub async fn measure_voc_index(
        &mut self,
        humidity: f32,
        temperature: f32,
    ) -> Result<u16, Error<T::Error>> {
        let raw = self.measure_raw(humidity, temperature).await?;
        Ok(self.voc.process(raw as i32) as u16)
    }

    fn crc(data: &[u8]) -> u8 {
        let mut crc = crc_all::Crc::<u8>::new(0x31, 8, 0xff, 0x00, false);
        crc.update(&data);
        crc.finish()
    }

    async fn read(
        &mut self,
        command: Command,
        buffer: &mut [u8],
        check_crc: bool,
    ) -> Result<(), Error<T::Error>> {
        let command: u16 = command.into();
        let mut bus = self.bus.lock().await;
        bus.write(ADDRESS, &command.to_be_bytes()).await?;

        bus.read(ADDRESS, buffer).await?;

        if check_crc {
            let crc = Self::crc(&buffer[..buffer.len() - 1]);
            if crc != buffer[buffer.len() - 1] {
                return Err(Error::Crc);
            }
        }

        Ok(())
    }
}
