use embassy::time::{Duration, Timer};
use embassy_nrf::gpio::Output;
use embassy_nrf::peripherals;
use embassy_nrf::spim::Spim;

use crate::app::App;
use crate::drivers::neopixel::{colors, NeoPixel};

#[embassy::task]
pub async fn task(
    app: App,
    mut buzz: Output<'static, peripherals::P0_14>,
    mut neopixel: NeoPixel<Spim<'static, peripherals::SPI3>>,
) {
    let mut count = 0;
    loop {
        Timer::after(Duration::from_secs(1)).await;
        let air_quality = app.air_quality();
        let co2 = air_quality.co2_concentration;
        if co2 > 1500.0 {
            count += 1;
            neopixel.set_color(colors::RED);
        } else if co2 > 1000.0 {
            count = 0;
            neopixel.set_color(colors::ORANGE);
        } else {
            count = 0;
            neopixel.set_color(colors::GREEN);
        }

        if count >= 10 {
            if app.bzzz_enabled() {
                // buzz.set_high();
            }
            Timer::after(Duration::from_millis(200)).await;
            buzz.set_low();
            count = 0;
        }
    }
}
