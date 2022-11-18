use drogue_device::drivers::led::neopixel::filter::{Brightness, Filter, Gamma};
use drogue_device::drivers::led::neopixel::rgbw::{NeoPixelRgbw, Rgbw8, BLACK};
use ector::{actor, Actor, Address, Inbox};
use embassy::time::{Duration, Ticker};
use embassy::util::select;
use embassy_nrf::peripherals;
use futures::StreamExt;

pub struct NeoPixel {
    pub neopixel: NeoPixelRgbw<'static, peripherals::PWM0, 1>,
}

#[actor]
impl Actor for NeoPixel {
    type Message<'m> = Rgbw8;

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        // let cyclic = CyclicBrightness::new(1, 100, 5);
        let brightness = Brightness(64);
        let mut filter = brightness.and(Gamma);

        let mut ticker = Ticker::every(Duration::from_millis(20));

        let mut color = BLACK;
        loop {
            match select(inbox.next(), ticker.next()).await {
                embassy::util::Either::First(new_color) => color = new_color,
                embassy::util::Either::Second(_tick) => {
                    self.neopixel
                        .set_with_filter(&[color], &mut filter)
                        .await
                        .ok();
                }
            }
        }
    }
}
