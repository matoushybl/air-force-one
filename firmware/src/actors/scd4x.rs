use ector::{actor, Actor, Address, Inbox};
use embassy_nrf::peripherals;
use embassy_nrf::twim::Twim;
use embassy_time::Duration;
use embassy_time::Timer;
use sensirion_async::scd4x::{Celsius, Meter, Scd4x};

use afo_shared::{Humidity, Temperature};

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

const ALTITUDE: Meter = Meter(230);
const TEMPERATURE_OFFSET: Celsius = Celsius(2.5);

#[actor]
impl Actor for Scd4xReader {
    type Message<'m> = ();

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, _: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        defmt::unwrap!(self.sensor.stop_periodic_measurement().await);
        Timer::after(Duration::from_millis(500)).await;

        let serial_number = defmt::unwrap!(self.sensor.read_serial_number().await);
        defmt::warn!("SCD4x serial number: {:x}", serial_number);

        let configured_altitude = defmt::unwrap!(self.sensor.get_sensor_altitude().await);
        if configured_altitude != ALTITUDE {
            defmt::unwrap!(self.sensor.set_sensor_altitude(ALTITUDE).await);
        }

        let configured_offset = defmt::unwrap!(self.sensor.get_temperature_offset().await);
        if configured_offset != TEMPERATURE_OFFSET {
            defmt::unwrap!(self.sensor.set_temperature_offset(TEMPERATURE_OFFSET).await);
        }

        defmt::unwrap!(self.sensor.start_periodic_measurement().await);
        Timer::after(Duration::from_millis(500)).await;

        loop {
            if defmt::unwrap!(self.sensor.data_ready().await) {
                let measurement = self.sensor.read().await;
                match measurement {
                    Ok(measurement) => {
                        self.consumer
                            .notify(Scd4xData {
                                co2: measurement.co2 as f32,
                                humidity: Humidity(measurement.humidity),
                                temperature: Temperature(measurement.temperature),
                            })
                            .await
                    }
                    Err(err) => {
                        defmt::error!("Error accessing Scd4x: {}", err);
                    }
                }
            }

            Timer::after(Duration::from_secs(6)).await;
        }
    }
}
