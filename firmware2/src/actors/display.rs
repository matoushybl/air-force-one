use arrayvec::ArrayString;
use ector::{actor, Actor, Address, Inbox};
use embassy_nrf::peripherals;
use embassy_nrf::twim::Twim;
use embedded_graphics::drawable::Drawable;
use embedded_graphics::fonts::{Font6x8, Text};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::Point;
use embedded_graphics::style::TextStyleBuilder;
use ssd1306::I2CDIBuilder;
use ssd1306::{prelude::*, Builder};

use crate::models::{Co2, Humidity, Pm, Temperature, Voc};

pub struct Display {
    display: GraphicsMode<I2CInterface<Twim<'static, peripherals::TWISPI1>>, DisplaySize128x32>,
}

impl Display {
    pub fn new(twi: Twim<'static, peripherals::TWISPI1>) -> Self {
        let interface = I2CDIBuilder::new().init(twi);
        let mut display: GraphicsMode<I2CInterface<Twim<peripherals::TWISPI1>>, DisplaySize128x32> =
            Builder::new()
                .size(DisplaySize128x32)
                .with_rotation(DisplayRotation::Rotate0)
                .connect(interface)
                .into();

        if display.init().is_err() {
            defmt::panic!("Display initialization failed.");
        }

        Self { display }
    }
}

pub enum Message {
    DisplayBasic(Co2, Temperature, Humidity),
    DisplayPm(Pm),
    DisplayVoc(Voc),
    DisplayLogging(bool),
}

#[actor]
impl Actor for Display {
    type Message<'m> = Message;

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        use core::fmt::Write;
        let text_style = TextStyleBuilder::new(Font6x8)
            .text_color(BinaryColor::On)
            .build();
        loop {
            let mut buf = ArrayString::<[_; 128]>::new();
            match inbox.next().await {
                Message::DisplayBasic(co2, temp, humidity) => write!(
                    &mut buf,
                    "Temp: {:.1} C\nHumi: {:.1} %\nCO2:  {:.0} ppm",
                    temp.0, humidity.0, co2.0
                )
                .unwrap(),
                Message::DisplayPm(pm) => write!(
                    &mut buf,
                    "1.0: {:.1} ug/m3\n2.5: {:.1} ug/m3\n4.0: {:.1} ug/m3\n10:  {:.1} ug/m3\nsize:  {:.1}",
                    pm.mass_10, pm.mass_25, pm.mass_40, pm.mass_100, pm.average_particle_size
                )
                .unwrap(),
                Message::DisplayVoc(voc) => write!(&mut buf, "voc:  {}", voc.index,).unwrap(),
                Message::DisplayLogging(l) => write!(&mut buf, "logging:  {}", l,).unwrap(),
            }

            self.display.clear();
            Text::new(&buf, Point::zero())
                .into_styled(text_style)
                .draw(&mut self.display)
                .unwrap();

            self.display.flush().unwrap();
        }
    }
}
