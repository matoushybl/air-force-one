use core::future::Future;
use drogue_device::traits;
use ector::{Actor, Address, Inbox};

pub use drogue_device::traits::button::Event as ButtonEvent;
use embassy::time::{Duration, Timer};

//pub struct Button<P: Wait + InputPin, H: ButtonEventHandler> {
pub struct Button<P: traits::button::Button, H: 'static> {
    inner: P,
    handler: Address<H>,
}

//impl<P: Wait + InputPin, H: ButtonEventHandler> Button<P, H> {
impl<P: traits::button::Button, H> Button<P, H>
where
    H: 'static,
{
    pub fn new(inner: P, handler: Address<H>) -> Self {
        Self { inner, handler }
    }
}

impl<P: traits::button::Button, H> Actor for Button<P, H>
where
    H: Default + 'static,
{
    type Message<'m> = ();
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm where Self: 'm, M: Inbox<()> + 'm;
    fn on_mount<'m, M>(&'m mut self, _: Address<()>, _: M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<()> + 'm,
    {
        async move {
            loop {
                self.inner.wait_released().await;
                Timer::after(Duration::from_millis(10)).await;

                self.handler.try_notify(H::default()).ok();
            }
        }
    }
}
