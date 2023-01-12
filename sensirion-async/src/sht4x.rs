use embedded_hal_async::i2c::I2c;

use crate::Error;

const SENSOR_ADDR: u8 = 0x44;

#[allow(unused)]
enum Command {
    ReadHighPrecisionMeasurement,
    ReadMediumPrecisionMeasurement,
    ReadLowPrecisionMeasurement,
    GetSerialNumber,
}

impl From<Command> for u8 {
    fn from(value: Command) -> Self {
        match value {
            Command::ReadHighPrecisionMeasurement => 0xfd,
            Command::ReadMediumPrecisionMeasurement => 0xf6,
            Command::ReadLowPrecisionMeasurement => 0xe0,
            Command::GetSerialNumber => 0x89,
        }
    }
}

pub struct Sht4x<T>
where
    T: I2c,
{
    bus: T,
}

impl<T> Sht4x<T>
where
    T: I2c,
{
    pub fn new(bus: T) -> Self {
        Self { bus }
    }

    pub async fn read_serial_number(&mut self) -> Result<[u8; 6], Error<T::Error>> {
        let mut read_buffer = [0u8; 6];
        self.bus
            .write_read(
                SENSOR_ADDR,
                &[Command::GetSerialNumber.into()],
                &mut read_buffer,
            )
            .await?;

        Ok(read_buffer)
    }
}
