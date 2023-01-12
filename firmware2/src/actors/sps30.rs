use ector::{actor, Actor, Address, Inbox};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_nrf::peripherals;
use embassy_nrf::twim::Twim;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_time::Duration;
use embassy_time::Timer;
use sensirion_async::sps30::Sps30;

use crate::models::Pm;

type Sensor = Sps30<I2cDevice<'static, ThreadModeRawMutex, Twim<'static, peripherals::TWISPI0>>>;

pub struct Sps30Reader {
    sensor: Sensor,
    consumer: Address<Pm>,
}

impl Sps30Reader {
    pub fn new(sensor: Sensor, consumer: Address<Pm>) -> Self {
        Self { sensor, consumer }
    }
}

#[actor]
impl Actor for Sps30Reader {
    type Message<'m> = ();

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, _: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        if let Ok(version) = self.sensor.read_version().await {
            defmt::warn!("sps30 version {:x}", version);
        }

        defmt::unwrap!(self.sensor.start_measurement().await);
        loop {
            {
                if self.sensor.is_measurement_ready().await.unwrap_or(false) {
                    let measurement = self.sensor.read().await;
                    match measurement {
                        Ok(measurement) => {
                            self.consumer
                                .notify(Pm {
                                    mass_10: measurement.mass_pm1_0,
                                    mass_25: measurement.mass_pm2_5,
                                    mass_40: measurement.mass_pm4_0,
                                    mass_100: measurement.mass_pm10,
                                    average_particle_size: measurement.typical_size,
                                })
                                .await;
                            defmt::trace!("Measured PM: {:?}", measurement);
                        }
                        Err(err) => {
                            defmt::error!("Error accessing Sps30: {:?}", err);
                        }
                    }
                }
            }
            Timer::after(Duration::from_millis(500)).await;
        }
    }
}
