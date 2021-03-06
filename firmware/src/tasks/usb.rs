use embassy::time::{Duration, Timer};
use futures::pin_mut;

use crate::app::App;

#[embassy::task]
pub async fn communication(usb: crate::Usb, app: App) {
    use embassy::io::AsyncWriteExt;
    pin_mut!(usb);
    let (mut _read_interface, mut write_interface) = usb.as_ref().take_serial_0();
    let mut buffer = [0u8; 100];
    loop {
        Timer::after(Duration::from_millis(500)).await;
        let data = app.air_quality();
        if let Ok(raw) = postcard::to_slice_cobs(&data, &mut buffer) {
            defmt::unwrap!(write_interface.write_all(raw).await);
        } else {
            defmt::error!("failed to serialize the state to raw data.");
        }
    }
}
