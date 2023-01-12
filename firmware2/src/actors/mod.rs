pub mod bluetooth;
pub mod button;
pub mod buzzer;
pub mod display;
pub mod emittor;
pub mod flash;
pub mod led;
pub mod light_sound;
pub mod neopixel;
pub mod reactor;
pub mod scd30;
pub mod scd4x;
pub mod sgp40;
pub mod sps30;
pub mod transformer;
pub mod usb_hid;
pub mod usb_serial;

// #[derive(Clone, Copy, defmt::Format)]
// pub enum ButtonPressed {
//     Esc,
//     Prev,
//     Next,
//     Ok,
// }

// impl TryFrom<AirQuality> for UiMessage {
//     type Error = ();

//     fn try_from(value: AirQuality) -> Result<Self, Self::Error> {
//         Ok(Self::NewAirQualityData(value))
//     }
// }

// #[derive(Clone, Copy, PartialEq, Eq)]
// pub enum Page {
//     Basic,
//     Pm,
//     Voc,
//     Settings,
// }

// impl Page {
//     pub fn next(&self) -> Self {
//         match self {
//             Self::Basic => Self::Pm,
//             Self::Pm => Self::Voc,
//             Self::Voc => Self::Settings,
//             Self::Settings => Self::Basic,
//         }
//     }

//     pub fn prev(&self) -> Self {
//         match self {
//             Page::Basic => Page::Settings,
//             Page::Pm => Page::Basic,
//             Page::Voc => Page::Pm,
//             Page::Settings => Page::Voc,
//         }
//     }
// }

// #[derive(Clone, Copy)]
// pub enum UiMessage {
//     NewAirQualityData(AirQuality),
//     ButtonPressed(ButtonPressed),
// }

// pub struct UiReactor {
//     air_quality: AirQuality,
//     display: Address<display::Message>,
//     hack: Address<reactor::Message>,
//     page: Page,
//     logging_enabled: bool,
// }

// impl UiReactor {
//     pub fn new(display: Address<display::Message>, hack: Address<reactor::Message>) -> Self {
//         Self {
//             air_quality: Default::default(),
//             display,
//             page: Page::Basic,
//             hack,
//             logging_enabled: false,
//         }
//     }

//     pub async fn display(&self) {
//         match self.page {
//             Page::Basic => {
//                 self.display
//                     .notify(display::Message::DisplayBasic(
//                         self.air_quality.co2,
//                         self.air_quality.temperature,
//                         self.air_quality.humidity,
//                     ))
//                     .await;
//             }
//             Page::Pm => {
//                 self.display
//                     .notify(display::Message::DisplayPm(self.air_quality.pm))
//                     .await;
//             }
//             Page::Voc => {
//                 self.display
//                     .notify(display::Message::DisplayVoc(self.air_quality.voc))
//                     .await;
//             }
//             Page::Settings => {
//                 self.display
//                     .notify(display::Message::DisplayLogging(self.logging_enabled))
//                     .await;
//             }
//         }
//     }
// }

// #[actor]
// impl Actor for UiReactor {
//     type Message<'m> = UiMessage;

//     async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
//     where
//         M: Inbox<Self::Message<'m>> + 'm,
//     {
//         loop {
//             match inbox.next().await {
//                 UiMessage::NewAirQualityData(air_quality) => {
//                     self.air_quality = air_quality;
//                     self.display().await;
//                 }
//                 UiMessage::ButtonPressed(button) => {
//                     match button {
//                         ButtonPressed::Esc => self.page = Page::Basic,
//                         ButtonPressed::Prev => self.page = self.page.prev(),
//                         ButtonPressed::Next => self.page = self.page.next(),
//                         ButtonPressed::Ok => {
//                             // self.hack
//                             //     .notify(reactor::Message::EnableLogging(true))
//                             //     .await;
//                             // self.logging_enabled = true;
//                         }
//                     }
//                     self.display().await;
//                 }
//             }
//         }
//     }
// }
