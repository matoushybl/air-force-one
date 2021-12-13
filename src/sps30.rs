use defmt::Format;
use embassy_traits::i2c::{I2c, SevenBitAddress};
use futures_intrusive::sync::LocalMutex;

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

impl From<Sps30Command> for u16 {
    fn from(raw: Sps30Command) -> Self {
        match raw {
            Sps30Command::StartMeasurement => 0x0010,
            Sps30Command::StopMeasurement => 0x0104,
            Sps30Command::ReadDataReadyFlag => 0x0202,
            Sps30Command::ReadMeasuredValues => 0x0300,
            Sps30Command::Sleep => 0x1001,
            Sps30Command::WakeUp => 0x1103,
            Sps30Command::StartFanCleaning => 0x5607,
            Sps30Command::AutoCleaningInterval => 0x8004,
            Sps30Command::ReadProductType => 0xd002,
            Sps30Command::ReadSerialNumber => 0xd033,
            Sps30Command::ReadVersion => 0xd100,
            Sps30Command::ReadDeviceStatusRegister => 0xd206,
            Sps30Command::ClearDeviceStatusRegister => 0xd210,
            Sps30Command::Reset => 0xd304,
        }
    }
}

pub enum MeasurementOutputFormat {
    Float,
    Integer,
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

pub struct Sps30<'a, T>(&'a LocalMutex<T>);

impl<'a, T> Sps30<'a, T>
where
    T: I2c<SevenBitAddress>,
    T::Error: defmt::Format,
{
    pub fn new(i2c: &'a LocalMutex<T>) -> Self {
        Self(i2c)
    }

    pub async fn read_version(&mut self) -> Result<[u8; 2], Error<T::Error>> {
        let mut buffer = [0u8; 3];
        self.read(Sps30Command::ReadVersion, &mut buffer, true)
            .await?;
        Ok(buffer[..2].try_into().unwrap())
    }

    pub async fn start_measurement(&mut self) -> Result<(), Error<T::Error>> {
        self.write(
            Sps30Command::StartMeasurement,
            Some(&[MeasurementOutputFormat::Float.into(), 0x00]),
        )
        .await
    }

    pub async fn is_ready(&mut self) -> Result<bool, Error<T::Error>> {
        let mut buffer = [0u8; 3];
        self.read(Sps30Command::ReadDataReadyFlag, &mut buffer[..3], true)
            .await?;
        Ok(buffer[1] == 1)
    }

    pub async fn read_measured_data(&mut self) -> Result<AirInfo, Error<T::Error>> {
        let mut buffer: [u8; 60] = [0; 60];
        self.read(Sps30Command::ReadMeasuredValues, &mut buffer, false)
            .await?;

        defmt::trace!("SPS30: raw data from sensor: {:x}", &buffer[..]);
        Ok(AirInfo {
            mass_pm1_0: f32::from_be_bytes(buffer[..4].try_into().unwrap()),
            mass_pm2_5: f32::from_be_bytes(buffer[6..10].try_into().unwrap()),
            mass_pm4_0: f32::from_be_bytes(buffer[12..16].try_into().unwrap()),
            mass_pm10: f32::from_be_bytes(buffer[18..22].try_into().unwrap()),
            number_pm0_5: f32::from_be_bytes(buffer[24..28].try_into().unwrap()),
            number_pm1_0: f32::from_be_bytes(buffer[30..34].try_into().unwrap()),
            number_pm2_5: f32::from_be_bytes(buffer[36..40].try_into().unwrap()),
            number_pm4_0: f32::from_be_bytes(buffer[42..46].try_into().unwrap()),
            number_pm10: f32::from_be_bytes(buffer[48..52].try_into().unwrap()),
            typical_size: f32::from_be_bytes(buffer[54..58].try_into().unwrap()),
        })
    }

    async fn write(
        &mut self,
        command: Sps30Command,
        payload: Option<&[u8]>,
    ) -> Result<(), Error<T::Error>> {
        let mut buffer = [0u8; 10];

        let sensor_command: u16 = command.into();
        let sensor_command = sensor_command.to_be_bytes();

        let mut offset = 0;
        buffer[..2].copy_from_slice(&sensor_command);
        offset += 2;

        if let Some(payload) = payload {
            buffer[offset..][..payload.len()].copy_from_slice(payload);
            offset += payload.len();

            buffer[offset] = Self::crc(&buffer[2..offset]);
            offset += 1;
        }

        self.0
            .lock()
            .await
            .write(SENSOR_ADDR, &buffer[..offset])
            .await?;

        Ok(())
    }

    async fn read(
        &mut self,
        command: Sps30Command,
        buffer: &mut [u8],
        check_crc: bool,
    ) -> Result<(), Error<T::Error>> {
        let command: u16 = command.into();
        let mut bus = self.0.lock().await;
        bus.write(SENSOR_ADDR, &command.to_be_bytes()).await?;

        bus.read(SENSOR_ADDR, buffer).await?;

        if check_crc {
            let crc = Self::crc(&buffer[..buffer.len() - 1]);
            if crc != buffer[buffer.len() - 1] {
                return Err(Error::Crc);
            }
        }

        Ok(())
    }

    fn crc(data: &[u8]) -> u8 {
        let mut crc = crc_all::Crc::<u8>::new(0x31, 8, 0xff, 0x00, false);
        crc.update(&data);
        crc.finish()
    }
}

#[derive(Format)]
pub struct AirInfo {
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
