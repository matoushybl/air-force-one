use ector::{actor, Actor, Address, Inbox};
use embassy_nrf::peripherals;
use embassy_nrf::twim::Twim;
use embassy_time::Duration;
use embassy_time::Timer;
use sensirion_async::scd4x::Scd4x;

use crate::models::{Humidity, Temperature};

#[derive(defmt::Format)]
pub struct Scd4xData {
    pub co2: f32,
    pub humidity: Humidity,
    pub temperature: Temperature,
}

type Sensor = Scd4x<Twim<'static, peripherals::TWISPI0>>;

pub struct Scd4xReader {
    sensor: Sensor,
    consumer: Address<Scd4xData>,
}

impl Scd4xReader {
    pub fn new(sensor: Sensor, consumer: Address<Scd4xData>) -> Self {
        Self { sensor, consumer }
    }
}

#[actor]
impl Actor for Scd4xReader {
    type Message<'m> = ();

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, _: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        {
            if let Ok(version) = self.sensor.read_serial_number().await {
                defmt::error!("Scd4x SN {:x}", version);
            } else {
                defmt::error!("Failed to read Scd4x SN.");
            }
        }
        loop {
            defmt::error!("loopko");
            let measurement = self.sensor.read().await;
            defmt::error!("measuredko");
            match measurement {
                Ok(measurement) => {
                    defmt::error!("measuredkoko");
                    // self.consumer
                    //     .notify(Scd4xData {
                    //         co2: measurement.co2 as f32,
                    //         humidity: Humidity(measurement.humidity),
                    //         temperature: Temperature(measurement.temperature),
                    //     })
                    //     .await
                }
                Err(err) => {
                    defmt::error!("Error accessing Scd4x: {:?}", err);
                }
            }

            Timer::after(Duration::from_secs(6)).await;
        }
    }
}
