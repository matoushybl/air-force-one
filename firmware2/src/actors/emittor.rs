use drogue_device::actors::led::LedMessage;
use ector::{actor, Actor, Address, Inbox};
use embassy::time::{Duration, Timer};

pub struct Emitter {
    collector: Address<LedMessage>,
}

impl Emitter {
    pub fn new(collector: Address<LedMessage>) -> Self {
        Self { collector }
    }
}

#[actor]
impl Actor for Emitter {
    type Message<'m> = LedMessage;

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, _: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        loop {
            // self.collector.notify(LedMessage::Toggle).await;
            Timer::after(Duration::from_millis(500)).await;
        }
    }
}
