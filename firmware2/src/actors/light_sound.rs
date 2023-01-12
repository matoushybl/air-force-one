// use crate::drivers::neopixel::rgbw::{Rgbw8, GREEN, RED};
// use ector::{actor, Actor, Address, Inbox};
// use embassy_futures::select::select;
// use embassy_time::{Duration, Instant, Timer};

// use crate::models::AirQuality;

// use super::buzzer::Bzzz;

// pub struct LightSoundReactor {
//     buzzer: Address<Bzzz>,
//     neopixel: Option<Address<Rgbw8>>,
//     bzzz_time: Option<Instant>,
// }

// impl LightSoundReactor {
//     pub fn new(buzzer: Address<Bzzz>, neopixel: Option<Address<Rgbw8>>) -> Self {
//         Self {
//             buzzer,
//             neopixel,
//             bzzz_time: None,
//         }
//     }

//     async fn process_air_quality(&mut self, air_quality: &AirQuality) {
//         let rgb = if air_quality.co2.0 < 800.0 {
//             GREEN
//         } else if air_quality.co2.0 < 1500.0 {
//             Rgbw8::new(0xff, 0xa5, 0x00, 0)
//         } else {
//             RED
//         };

//         if air_quality.co2.0 > 1500.0 {
//             if self.bzzz_time.is_none() {
//                 self.bzzz_time = Some(Instant::now() + Duration::from_secs(10));
//             }
//         } else {
//             self.bzzz_time = None;
//         }

//         if let Some(neopixel) = self.neopixel.as_ref() {
//             neopixel.notify(rgb).await;
//         }
//     }
// }

// #[actor]
// impl Actor for LightSoundReactor {
//     type Message<'m> = AirQuality;

//     async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
//     where
//         M: Inbox<Self::Message<'m>> + 'm,
//     {
//         loop {
//             if let Some(bzzz_time) = self.bzzz_time {
//                 match select(inbox.next(), Timer::at(bzzz_time)).await {
//                     embassy_futures::select::Either::First(air_quality) => {
//                         self.process_air_quality(&air_quality).await
//                     }
//                     embassy_futures::select::Either::Second(_bzzz) => {
//                         self.buzzer.notify(Bzzz).await;
//                         self.bzzz_time = None;
//                     }
//                 }
//             } else {
//                 let air_quality = inbox.next().await;
//                 self.process_air_quality(&air_quality).await
//             }
//         }
//     }
// }
