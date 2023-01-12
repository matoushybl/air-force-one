use ector::{Actor, Address, Inbox};
use embassy_time::{Duration, Timer};
use futures::Future;

pub struct Emitter<T>
where
    T: 'static,
{
    collector: Address<T>,
    value: T,
}

impl<T> Emitter<T> {
    pub fn new(collector: Address<T>, value: T) -> Self {
        Self { collector, value }
    }
}

impl<T> Actor for Emitter<T>
where
    T: Copy + 'static,
{
    type OnMountFuture<'m, M>
    = impl Future<Output = ()> + 'm where Self: 'm, M: Inbox<Self::Message<'m>> + 'm;

    type Message<'m> = T where Self: 'm;

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
                self.collector.notify(self.value).await;
                Timer::after(Duration::from_millis(500)).await;
            }
        }
    }
}
