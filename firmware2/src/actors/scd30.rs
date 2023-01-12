use ector::{actor, Actor, Address, Inbox};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_nrf::peripherals;
use embassy_nrf::twim::Twim;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_time::Duration;
use embassy_time::Timer;
use sensirion_async::scd30::Scd30;

use crate::models::{Humidity, Temperature};

#[derive(defmt::Format)]
pub struct Scd30Data {
    pub co2: f32,
    pub humidity: Humidity,
    pub temperature: Temperature,
}

type Sensor = Scd30<I2cDevice<'static, ThreadModeRawMutex, Twim<'static, peripherals::TWISPI0>>>;

pub struct Scd30Reader {
    sensor: Sensor,
    consumer: Address<Scd30Data>,
}

impl Scd30Reader {
    pub fn new(sensor: Sensor, consumer: Address<Scd30Data>) -> Self {
        Self { sensor, consumer }
    }
}

#[actor]
impl Actor for Scd30Reader {
    type Message<'m> = ();

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, _: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        {
            if let Ok(version) = self.sensor.read_version().await {
                defmt::error!("SCD30 version {:x}", version);
            } else {
                defmt::error!("Failed to read SCD30 version.");
            }
        }
        loop {
            {
                if self.sensor.is_measurement_ready().await.unwrap_or(false) {
                    let measurement = self.sensor.read().await;
                    match measurement {
                        Ok(measurement) => {
                            self.consumer
                                .notify(Scd30Data {
                                    co2: measurement.co2,
                                    humidity: Humidity(measurement.humidity),
                                    temperature: Temperature(measurement.temperature),
                                })
                                .await
                        }
                        Err(err) => {
                            defmt::error!("Error accessing Scd30: {:?}", err);
                        }
                    }
                }
            }
            Timer::after(Duration::from_millis(500)).await;
        }
    }
}
