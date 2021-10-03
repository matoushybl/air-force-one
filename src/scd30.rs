use core::convert::TryInto;
use embedded_hal::blocking::i2c::{Read, Write};

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

pub struct SCD30<T: Read + Write>(T);

impl<T, E> SCD30<T>
where
    T: Read<Error = E> + Write<Error = E>,
{
    pub fn init(i2c: T) -> Self {
        Self(i2c)
    }

    pub fn read_fw_version(&mut self) -> Result<[u8; 2], E> {
        let command: u16 = SCD30Command::ReadFWVersion.into();
        self.0.write(SENSOR_ADDR, &command.to_be_bytes())?;
        let mut result = [0u8; 2];
        self.0.read(SENSOR_ADDR, &mut result)?;
        Ok(result)
    }

    pub fn set_measurement_interval(&mut self, seconds: u16) -> Result<(), E> {
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

        self.0.write(SENSOR_ADDR, &command)?;

        Ok(())
    }

    pub fn start_continuous_measurement(&mut self, pressure: u16) -> Result<(), E> {
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

        self.0.write(SENSOR_ADDR, &command)?;

        Ok(())
    }

    pub fn get_data_ready(&mut self) -> Result<bool, E> {
        let command: u16 = SCD30Command::DataReady.into();
        self.0.write(SENSOR_ADDR, &command.to_be_bytes())?;
        let mut result = [0u8; 3];
        self.0.read(SENSOR_ADDR, &mut result)?;
        Ok(u16::from_be_bytes((&result[0..2]).try_into().unwrap()) == 1)
    }

    pub fn read_measurement(&mut self) -> Result<SensorData, E> {
        let command: u16 = SCD30Command::ReadMeasurement.into();
        self.0.write(SENSOR_ADDR, &command.to_be_bytes())?;
        let mut result = [0u8; 18];
        self.0.read(SENSOR_ADDR, &mut result)?;
        defmt::error!("res: {}", result);
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
