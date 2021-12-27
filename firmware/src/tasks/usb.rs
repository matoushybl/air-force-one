use core::cell::Cell;

use embassy::blocking_mutex::{CriticalSectionMutex, Mutex};
use embassy::time::{Duration, Timer};
use embassy_nrf::interrupt;
use futures::pin_mut;
use shared::AirQuality;

use crate::{StaticSerialClassSet1, StaticUsb};

#[embassy::task]
pub async fn communication(
    usb: embassy_hal_common::usb::Usb<'static, StaticUsb, StaticSerialClassSet1, interrupt::USBD>,
    state: &'static CriticalSectionMutex<Cell<AirQuality>>,
) {
    use embassy::io::AsyncWriteExt;
    pin_mut!(usb);
    let (mut _read_interface, mut write_interface) = usb.as_ref().take_serial_0();
    let mut buffer = [0u8; 100];
    loop {
        Timer::after(Duration::from_millis(500)).await;
        let data = state.lock(|data| data.get());
        if let Ok(raw) = postcard::to_slice_cobs(&data, &mut buffer) {
            defmt::unwrap!(write_interface.write_all(raw).await);
        } else {
            defmt::error!("failed to serialize the state to raw data.");
        }
    }
}
