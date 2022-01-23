use arrayvec::ArrayString;
use embassy::blocking_mutex::kind::Noop;
use embassy::channel::mpsc::Receiver;
use embassy::time::{Duration, Timer};
use embassy_nrf::peripherals;
use embassy_nrf::twim::Twim;
use embedded_graphics::{
    drawable::Drawable,
    fonts::{Font6x8, Text},
    pixelcolor::BinaryColor,
    prelude::Point,
    style::TextStyleBuilder,
};
use futures::future::{select, Either};
use ssd1306::prelude::{DisplayRotation, DisplaySize128x32, GraphicsMode};
use ssd1306::{Builder, I2CDIBuilder};

use crate::app::{App, ButtonEvent, Page};

#[embassy::task]
pub async fn navigation(mut receiver: Receiver<'static, Noop, ButtonEvent, 1>, app: App) {
    loop {
        let sel = select(receiver.recv(), Timer::after(Duration::from_secs(10))).await;
        match sel {
            Either::Left((Some(event), _)) => app.button_pressed(event),
            Either::Right(_) => app.button_timed_out(),
            _ => {}
        }
    }
}

#[embassy::task]
pub async fn render(twim: Twim<'static, peripherals::TWISPI1>, app: App) {
    use core::fmt::Write;
    let interface = I2CDIBuilder::new().init(twim);
    let mut disp: GraphicsMode<_, _> = Builder::new()
        .size(DisplaySize128x32)
        .with_rotation(DisplayRotation::Rotate0)
        .connect(interface)
        .into();

    if disp.init().is_err() {
        defmt::error!("Display initialization failed.");
        return;
    }

    let text_style = TextStyleBuilder::new(Font6x8)
        .text_color(BinaryColor::On)
        .build();

    loop {
        disp.clear();

        let mut buf = ArrayString::<[_; 128]>::new();
        let data = app.air_quality();
        match app.page() {
            Page::Basic => write!(
                &mut buf,
                "Temp: {:.1} C\nHumi: {:.1} %\nCO2:  {:.0} ppm",
                data.temperature, data.humidity, data.co2_concentration
            )
            .unwrap(),
            Page::Pm => write!(
                &mut buf,
                "1.0: {:.1} ug/m3\n2.5: {:.1} ug/m3\n4.0: {:.1} ug/m3\n10:  {:.1} ug/m3",
                data.mass_pm1_0, data.mass_pm2_5, data.mass_pm4_0, data.mass_pm10
            )
            .unwrap(),
            Page::Voc => write!(
                &mut buf,
                "size: {:.1}\nvoc:  {}",
                data.typical_particulate_matter_size, data.voc_index,
            )
            .unwrap(),
            Page::Settings => write!(
                &mut buf,
                "bzzz: {}",
                "ON" // if (unsafe { Uicr::read(0) } & 0x01) > 0 {
                     //     "ON"
                     // } else {
                     //     "OFF"
                     // }
            )
            .unwrap(),
        }
        Text::new(&buf, Point::zero())
            .into_styled(text_style)
            .draw(&mut disp)
            .unwrap();

        disp.flush().unwrap();
        defmt::info!("DISP: displaying {}", data);
        Timer::after(Duration::from_millis(100)).await;
    }
}
