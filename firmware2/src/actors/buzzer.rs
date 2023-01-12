use embassy_time::Duration;

use ector::{actor, Actor, Address, Inbox};
use embassy_nrf::gpio::{AnyPin, Output};
use embassy_time::Timer;

pub struct Buzzer {
    buzzer: Output<'static, AnyPin>,
}

impl Buzzer {
    pub fn new(buzzer: Output<'static, AnyPin>) -> Self {
        Self { buzzer }
    }
}

pub struct Bzzz;

#[actor]
impl Actor for Buzzer {
    type Message<'m> = Bzzz;

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        loop {
            let _ = inbox.next().await;
            self.buzzer.set_high();
            Timer::after(Duration::from_millis(40)).await;
            self.buzzer.set_low();
        }
    }
}
