use core::future::Future;
use ector::{Actor, Address, Inbox};

use embassy_time::{Duration, Timer};
use embedded_hal::digital::InputPin;
use embedded_hal_async::digital::Wait;

pub struct Button<B, T>
where
    T: 'static,
{
    button: B,
    released_value: T,
    handler: Address<T>,
}

impl<B, T> Button<B, T>
where
    B: Wait + InputPin,
{
    pub fn new(button: B, released_value: T, handler: Address<T>) -> Self {
        Self {
            button,
            released_value,
            handler,
        }
    }
}

impl<B, T> Actor for Button<B, T>
where
    B: Wait + InputPin,
    T: Copy + 'static,
{
    type Message<'m> = T where Self: 'm;

    type OnMountFuture<'m, M>= impl Future<Output = ()> + 'm where Self: 'm, M: Inbox<Self::Message<'m>> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self::Message<'m>>,
        _: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        async move {
            loop {
                self.button.wait_for_any_edge().await.ok();
                if self.button.is_high().unwrap_or(false) {
                    self.handler.try_notify(self.released_value).ok();
                }
                Timer::after(Duration::from_millis(10)).await;
            }
        }
    }
}
