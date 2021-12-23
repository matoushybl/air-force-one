use core::cell::Cell;

use embassy::blocking_mutex::{CriticalSectionMutex, Mutex};
use embassy::time::{Duration, Timer};
use embassy_hal_common::usb::usb_serial::UsbSerial;
use embassy_hal_common::usb::ClassSet1;
use embassy_nrf::interrupt;
use embassy_nrf::usbd::UsbPeripheral;
use futures::pin_mut;
use nrf_usbd::Usbd;
use shared::AirQuality;

#[embassy::task]
pub async fn communication(
    usb: embassy_hal_common::usb::Usb<
        'static,
        Usbd<UsbPeripheral<'static>>,
        ClassSet1<
            Usbd<UsbPeripheral<'static>>,
            UsbSerial<'static, 'static, Usbd<UsbPeripheral<'static>>>,
        >,
        interrupt::USBD,
    >,
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
            defmt::unwrap!(write_interface.write_all(&raw).await);
        } else {
            defmt::error!("failed to serialize the state to raw data.");
        }
    }
}
