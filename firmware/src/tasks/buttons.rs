use embassy::blocking_mutex::kind::Noop;
use embassy::channel::mpsc::Sender;
use embassy::time::Duration;
use embassy::time::Timer;
use embassy_nrf::gpiote::InputChannel;
use embassy_nrf::peripherals;
use embedded_hal::digital::v2::InputPin;
use futures::future::{select, Either};
use futures::pin_mut;

use crate::ButtonEvent;

macro_rules! naive_debounce {
    ($name:ident) => {
        async {
            loop {
                $name.wait().await;
                Timer::after(Duration::from_millis(10)).await;
                if defmt::unwrap!($name.is_high()) {
                    break;
                }
            }
        }
    };
}

#[embassy::task]
pub async fn task(
    esc: InputChannel<'static, peripherals::GPIOTE_CH0, peripherals::P0_13>,
    prev: InputChannel<'static, peripherals::GPIOTE_CH1, peripherals::P0_15>,
    next: InputChannel<'static, peripherals::GPIOTE_CH2, peripherals::P0_24>,
    ok: InputChannel<'static, peripherals::GPIOTE_CH3, peripherals::P0_25>,
    sender: Sender<'static, Noop, ButtonEvent, 1>,
) {
    loop {
        let esc_fut = naive_debounce!(esc);
        let prev_fut = naive_debounce!(prev);
        let next_fut = naive_debounce!(next);
        let ok_fut = naive_debounce!(ok);

        pin_mut!(esc_fut);
        pin_mut!(prev_fut);
        pin_mut!(next_fut);
        pin_mut!(ok_fut);
        let esc_ok_fut = select(esc_fut, ok_fut);
        let prev_next_fut = select(prev_fut, next_fut);
        let res = select(esc_ok_fut, prev_next_fut).await;
        match res {
            Either::Left((Either::Left(_), _)) => {
                defmt::unwrap!(sender.send(ButtonEvent::Esc).await);
            }
            Either::Left((Either::Right(_), _)) => {
                defmt::unwrap!(sender.send(ButtonEvent::Ok).await);
            }
            Either::Right((Either::Left(_), _)) => {
                defmt::unwrap!(sender.send(ButtonEvent::Prev).await);
            }
            Either::Right((Either::Right(_), _)) => {
                defmt::unwrap!(sender.send(ButtonEvent::Next).await);
            }
        }
    }
}
