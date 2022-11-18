use ector::{actor, Actor, Address, Inbox};
use embassy::time::{Duration, Ticker};
use embassy::util::{select, Either};
use futures::StreamExt;

use crate::models::{AirQuality, Co2, Humidity, Pm, Temperature, TemperatureAndHumidity, Voc};

use super::flash::LogCommand;
use super::scd30::Scd30Data;
use super::sgp40;

pub enum Message {
    NewScd30Data(Scd30Data),
    NewPmData(Pm),
    NewVocData(Voc),
    EnableLogging(bool),
}

impl TryFrom<Scd30Data> for Message {
    type Error = ();

    fn try_from(data: Scd30Data) -> Result<Self, Self::Error> {
        Ok(Self::NewScd30Data(data))
    }
}

impl TryFrom<Voc> for Message {
    type Error = ();

    fn try_from(voc: Voc) -> Result<Self, Self::Error> {
        Ok(Self::NewVocData(voc))
    }
}

impl TryFrom<Pm> for Message {
    type Error = ();

    fn try_from(value: Pm) -> Result<Self, Self::Error> {
        Ok(Self::NewPmData(value))
    }
}

pub struct Reactor {
    voc_sensor: Address<sgp40::Message>,
    light_sound: Address<AirQuality>,
    ui: Address<AirQuality>,
    usb: Address<AirQuality>,
    ble: Option<Address<AirQuality>>,
    logger: Option<Address<LogCommand>>,
    air_quality: AirQuality,
}

impl Reactor {
    pub fn new(
        voc_sensor: Address<sgp40::Message>,
        ui: Address<AirQuality>,
        light_sound: Address<AirQuality>,
        usb: Address<AirQuality>,
        ble: Option<Address<AirQuality>>,
        logger: Option<Address<LogCommand>>,
    ) -> Self {
        Self {
            voc_sensor,
            ui,
            air_quality: AirQuality {
                co2: Co2(0.0),
                temperature: Temperature(0.0),
                humidity: Humidity(0.0),
                pm: Pm {
                    mass_10: 0.0,
                    mass_25: 0.0,
                    mass_40: 0.0,
                    mass_100: 0.0,
                    average_particle_size: 0.0,
                },
                voc: Voc { index: 0, raw: 0 },
            },
            light_sound,
            usb,
            logger,
            ble,
        }
    }
}

const REACTOR_UPDATE_PERIOD: u64 = 1000;

#[actor]
impl Actor for Reactor {
    type Message<'m> = Message;

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        let mut ticker = Ticker::every(Duration::from_millis(REACTOR_UPDATE_PERIOD));
        loop {
            match select(inbox.next(), ticker.next()).await {
                Either::First(message) => match message {
                    Message::NewScd30Data(data) => {
                        self.air_quality.co2 = Co2(data.co2);
                        self.air_quality.temperature = data.temperature;
                        self.air_quality.humidity = data.humidity;

                        self.voc_sensor
                            .notify(sgp40::Message::UpdateTemperatureAndHumidity(
                                TemperatureAndHumidity {
                                    temperature: data.temperature,
                                    humidity: data.humidity,
                                },
                            ))
                            .await;
                    }
                    Message::NewVocData(voc) => {
                        self.air_quality.voc = voc;
                    }
                    Message::NewPmData(pm) => {
                        self.air_quality.pm = pm;
                    }
                    Message::EnableLogging(enable) => {
                        if let Some(logger) = self.logger.as_ref() {
                            logger.notify(LogCommand::EnableLogging(enable)).await
                        }
                    }
                },
                Either::Second(_tick) => {
                    self.ui.notify(self.air_quality).await;
                    self.light_sound.notify(self.air_quality).await;
                    self.usb.try_notify(self.air_quality).ok();
                    if let Some(ble) = self.ble.as_ref() {
                        ble.try_notify(self.air_quality).ok();
                    }
                    if let Some(logger) = self.logger.as_ref() {
                        logger
                            .try_notify(LogCommand::LogValue(self.air_quality))
                            .ok();
                    }
                }
            }
        }
    }
}
