use core::cell::Cell;

use embassy::blocking_mutex::{CriticalSectionMutex, Mutex};
use embassy::time::{Duration, Timer};
use futures::future::select;
use futures::pin_mut;
use nrf_softdevice::ble::peripheral;
use nrf_softdevice::Softdevice;
use postcard::to_slice;
use shared::{AirQuality, AirQualityAdvertisement};

#[embassy::task]
pub async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}

#[embassy::task]
pub async fn bluetooth_task(
    sd: &'static Softdevice,
    state: &'static CriticalSectionMutex<Cell<AirQuality>>,
) {
    #[rustfmt::skip]
    let scan_data = &[
        0x03, 0x03, 0x09, 0x18,
    ];

    loop {
        let mut adv_data = [0u8; 31];
        let mut adv_offset = 0;

        adv_offset += shared::fill_adv_data(
            &mut adv_data,
            0x01,
            &[nrf_softdevice::raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8],
        );
        adv_offset += shared::fill_adv_data(&mut adv_data[adv_offset..], 0x03, &[0x09, 0x18]);
        adv_offset += shared::fill_adv_data(&mut adv_data[adv_offset..], 0x09, &[b'A', b'F', b'O']);

        let mut buffer = [0u8; 31];
        buffer[0] = 0xff;
        buffer[1] = 0xff;
        let data = state.lock(|cell| AirQualityAdvertisement::from(cell.get()));

        defmt::error!("wtf: {:?}", data);

        let serialized_len = to_slice(&data, &mut buffer[2..]).unwrap().len();

        adv_offset += shared::fill_adv_data(
            &mut adv_data[adv_offset..],
            0xff,
            &buffer[..2 + serialized_len],
        );
        let config = peripheral::Config::default();
        let adv = peripheral::NonconnectableAdvertisement::ScannableUndirected {
            adv_data: &adv_data[..adv_offset],
            scan_data,
        };
        let adv_fut = peripheral::advertise(sd, adv, &config);
        let timeout_fut = Timer::after(Duration::from_secs(5));

        pin_mut!(adv_fut);

        let result = select(adv_fut, timeout_fut).await;
        match result {
            futures::future::Either::Left((_, _)) => {}
            futures::future::Either::Right(_) => defmt::error!("adv_conn timeout"),
        }
    }
}
