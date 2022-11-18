use ector::{actor, Actor, Address, Inbox};
use embassy::blocking_mutex::raw::ThreadModeRawMutex;
use embassy::time::Ticker;
use embassy::time::{Delay, Duration};
use embassy::util::{select, Either};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_nrf::peripherals;
use embassy_nrf::twim::Twim;
use futures::StreamExt;
use sensirion_async::sgp40::Sgp40;

use crate::models::{Humidity, Temperature, TemperatureAndHumidity, Voc};

type Sensor = Sgp40<I2cDevice<'static, ThreadModeRawMutex, Twim<'static, peripherals::TWISPI0>>>;

pub struct Sgp40Reader {
    sensor: Sensor,
    humidity: Humidity,
    temperature: Temperature,
    consumer: Address<Voc>, // TODO T: From<Voc>?
}

impl Sgp40Reader {
    pub fn new(sensor: Sensor, consumer: Address<Voc>) -> Self {
        Self {
            sensor,
            humidity: Humidity(40.0),
            temperature: Temperature(25.0),
            consumer,
        }
    }
}

pub enum Message {
    UpdateTemperatureAndHumidity(TemperatureAndHumidity),
}

const SGP40_POLLING_PERIOD: u64 = 1000;

#[actor]
impl Actor for Sgp40Reader {
    type Message<'m> = Message;

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        if let Ok(version) = self.sensor.get_serial_number().await {
            defmt::error!("voc version: {=u64:x}", version);
        } else {
            defmt::error!("Failed to read VOC sensor version.")
        }

        let mut ticker = Ticker::every(Duration::from_millis(SGP40_POLLING_PERIOD));

        loop {
            let event = select(ticker.next(), inbox.next()).await;
            match event {
                Either::First(_tick) => {
                    if let Ok(voc) = self
                        .sensor
                        .read(self.humidity.0, self.temperature.0, &mut Delay)
                        .await
                    {
                        self.consumer
                            .notify(Voc {
                                index: voc.voc_index,
                                raw: voc.raw,
                            })
                            .await
                    } else {
                        defmt::error!("Failed to read data from VOC.");
                    }
                }
                Either::Second(command) => match command {
                    Message::UpdateTemperatureAndHumidity(temperature_and_humidity) => {
                        self.humidity = temperature_and_humidity.humidity;
                        self.temperature = temperature_and_humidity.temperature;
                    }
                },
            }
        }
    }
}
