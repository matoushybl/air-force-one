use core::future::Future;
use ector::{Actor, Address, Inbox};
use embedded_hal::digital::OutputPin;

#[derive(Clone, Copy)]
pub enum LedMessage {
    On,
    Off,
    Toggle,
    State(bool),
}

pub struct Led<P>
where
    P: OutputPin,
{
    led: P,
    state: bool,
}

impl<P> Led<P>
where
    P: OutputPin,
{
    pub fn new(led: P) -> Self {
        Self { led, state: false }
    }
}

impl<P> Actor for Led<P>
where
    P: OutputPin,
{
    type Message<'m> = LedMessage where Self: 'm;
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm where Self: 'm, M: Inbox<LedMessage> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<LedMessage>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<LedMessage> + 'm,
    {
        async move {
            loop {
                let new_state = match inbox.next().await {
                    LedMessage::On => true,
                    LedMessage::Off => false,
                    LedMessage::State(state) => state,
                    LedMessage::Toggle => !self.state,
                };
                if self.state != new_state {
                    if new_state {
                        self.led.set_high().ok();
                    } else {
                        self.led.set_low().ok();
                    }
                }
                self.state = new_state;
            }
        }
    }
}
