use ector::{actor, Actor, Address, Inbox};
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Ticker};
use futures::StreamExt;

use crate::models::{AirQuality, Co2};

use super::flash::LogCommand;
use super::scd4x::Scd4xData;

pub enum Message {
    NewScd4xData(Scd4xData),
    EnableLogging(bool),
}

impl TryFrom<Scd4xData> for Message {
    type Error = ();

    fn try_from(data: Scd4xData) -> Result<Self, Self::Error> {
        Ok(Self::NewScd4xData(data))
    }
}

pub struct Reactor {
    usb: Option<Address<AirQuality>>,
    ble: Option<Address<AirQuality>>,
    logger: Option<Address<LogCommand>>,
    air_quality: AirQuality,
}

impl Reactor {
    pub fn new(
        usb: Option<Address<AirQuality>>,
        ble: Option<Address<AirQuality>>,
        logger: Option<Address<LogCommand>>,
    ) -> Self {
        Self {
            air_quality: Default::default(),
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
                    Message::NewScd4xData(data) => {
                        self.air_quality.co2 = Co2(data.co2);
                        self.air_quality.temperature = data.temperature;
                        self.air_quality.humidity = data.humidity;
                    }
                    Message::EnableLogging(enable) => {
                        if let Some(logger) = self.logger.as_ref() {
                            logger.notify(LogCommand::EnableLogging(enable)).await
                        }
                    }
                },
                Either::Second(_tick) => {
                    if let Some(usb) = self.usb.as_ref() {
                        usb.try_notify(self.air_quality).ok();
                    }
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
