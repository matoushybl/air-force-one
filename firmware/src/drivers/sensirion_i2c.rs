use crc_all::Crc;
use embassy::time::{Duration, Timer};
use embassy_traits::i2c::{I2c, SevenBitAddress};
use futures_intrusive::sync::LocalMutex;

pub trait SensirionCommand {
    fn raw(&self) -> u16;
}

#[derive(defmt::Format)]
pub enum Error<Inner: defmt::Format> {
    Bus(Inner),
    Crc,
}

impl<T: defmt::Format> From<T> for Error<T> {
    fn from(inner: T) -> Self {
        Self::Bus(inner)
    }
}

pub struct SensirionI2c<'a, T>
where
    T: I2c<SevenBitAddress>,
    T::Error: defmt::Format,
{
    bus: &'a LocalMutex<T>,
    crc: Crc<u8>,
}

impl<'a, T> SensirionI2c<'a, T>
where
    T: I2c<SevenBitAddress>,
    T::Error: defmt::Format,
{
    pub fn new(bus: &'a LocalMutex<T>) -> SensirionI2c<'a, T> {
        Self {
            bus,
            crc: crc_all::Crc::<u8>::new(0x31, 8, 0xff, 0x00, false),
        }
    }

    pub async fn read_word<Command: SensirionCommand>(
        &mut self,
        address: u8,
        command: Command,
        check_crc: bool,
    ) -> Result<u16, Error<T::Error>> {
        let mut bus = self.bus.lock().await;
        bus.write(address, &command.raw().to_be_bytes()).await?;

        let mut buffer = [0; 3];
        bus.read(address, &mut buffer).await?;

        if check_crc {
            let crc = self.calculate_crc(&buffer[..2]);
            if crc != buffer[2] {
                return Err(Error::Crc);
            }
        }

        Ok(u16::from_be_bytes(buffer[..2].try_into().unwrap()))
    }

    pub async fn write_word<Command: SensirionCommand>(
        &mut self,
        address: u8,
        command: Command,
        word: u16,
    ) -> Result<(), Error<T::Error>> {
        let mut buffer = [0u8; 5];

        buffer[0..2].copy_from_slice(&command.raw().to_be_bytes());
        buffer[2..4].copy_from_slice(&word.to_be_bytes());

        buffer[4] = self.calculate_crc(&buffer[2..4]);

        self.bus.lock().await.write(address, &buffer).await?;

        Ok(())
    }

    pub async fn write_read_raw(
        &mut self,
        address: u8,
        write_buffer: &[u8],
        read_buffer: &mut [u8],
        delay: Option<Duration>,
    ) -> Result<(), Error<T::Error>> {
        let mut bus = self.bus.lock().await;
        bus.write(address, write_buffer).await?;
        if let Some(time) = delay {
            Timer::after(time).await;
        }
        bus.read(address, read_buffer).await?;
        Ok(())
    }

    pub async fn read_raw<Command: SensirionCommand>(
        &mut self,
        address: u8,
        command: Command,
        buffer: &mut [u8],
    ) -> Result<(), Error<T::Error>> {
        let mut bus = self.bus.lock().await;
        bus.write(address, &command.raw().to_be_bytes()).await?;

        bus.read(address, buffer).await?;

        Ok(())
    }

    pub fn calculate_crc(&mut self, data: &[u8]) -> u8 {
        self.crc.init();
        self.crc.update(data);
        self.crc.finish()
    }
}
