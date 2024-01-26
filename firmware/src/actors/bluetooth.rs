use ector::{actor, Actor, Address, Inbox};

use embassy_futures::select::{select, Either};
use embassy_nrf::usb::SoftwareVbusDetect;
use nrf_softdevice::ble::{gatt_server, peripheral};
use nrf_softdevice::Softdevice;

use postcard::to_slice;

use afo_shared::{AirQuality, fill_adv_data, AirQualityAdvertisement};

#[embassy_executor::task]
pub async fn softdevice_task(sd: &'static Softdevice, supply: &'static SoftwareVbusDetect) {
    unsafe {
        nrf_softdevice::raw::sd_power_usbdetected_enable(1);
        nrf_softdevice::raw::sd_power_usbpwrrdy_enable(1);
        nrf_softdevice::raw::sd_power_usbremoved_enable(1);
    };

    sd.run_with_callback(|event| {
        defmt::error!("sd callback: {:?}", event);
        match event {
            nrf_softdevice::SocEvent::PowerUsbPowerReady => supply.ready(),
            nrf_softdevice::SocEvent::PowerUsbDetected => supply.detected(true),
            nrf_softdevice::SocEvent::PowerUsbRemoved => supply.detected(false),
            _ => {}
        }
    })
    .await;
}

#[nrf_softdevice::gatt_service(uuid = "181a")]
pub struct EnvironmentalService {
    #[characteristic(uuid = "2a6e", read)]
    temperature: u16,
}

#[nrf_softdevice::gatt_server]
pub struct Server {
    environment: EnvironmentalService,
}

pub struct Ble {
    pub softdevice: &'static Softdevice,
    pub server: Server,
    pub air_quality: AirQuality,
    pub device_id: u8,
}

#[actor]
impl Actor for Ble {
    type Message<'m> = AirQuality;
    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        loop {
            let config = peripheral::Config::default();

            let mut adv_data = [0u8; 31];
            let adv_len = build_adv_data(self.device_id, &self.air_quality, &mut adv_data);

            let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
                adv_data: &adv_data[..adv_len],
                scan_data: &[0x03, 0x03, 0x1a, 0x18],
            };

            match select(
                peripheral::advertise_connectable(self.softdevice, adv, &config),
                inbox.next(),
            )
            .await
            {
                Either::First(maybe_conn) => {
                    let conn = defmt::unwrap!(maybe_conn);

                    defmt::info!("advertising done!");

                    loop {
                        // Run the GATT server on the connection. This returns when the connection gets disconnected.
                        let server = gatt_server::run(&conn, &self.server, |e| match e {
                                _ => {}
                        });

                        match select(server, inbox.next()).await {
                            Either::First(res) => {
                                if let Err(e) = res {
                                    defmt::info!("gatt_server run exited with error: {:?}", e);
                                    break;
                                }
                            }
                            Either::Second(new_data) => {
                                self.air_quality = new_data;

                                // update GATT values
                                defmt::unwrap!(self.server.environment.temperature_set(
                                    &((self.air_quality.temperature.0 * 100.0) as u16)
                                ));
                            }
                        }
                    }
                }
                Either::Second(air_quality) => {
                    self.air_quality = air_quality;
                }
            }
        }
    }
}

fn build_adv_data(device_id: u8, air_quality: &AirQuality, adv_data: &mut [u8; 31]) -> usize {
    let mut adv_offset = 0;

    adv_offset += fill_adv_data(
        &mut adv_data[..],
        0x01,
        &[nrf_softdevice::raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8],
    );
    adv_offset += fill_adv_data(&mut adv_data[adv_offset..], 0x03, &[0x1a, 0x18]);
    adv_offset += fill_adv_data(&mut adv_data[adv_offset..], 0x09, &[b'A', b'F', b'O']);

    let mut buffer = [0u8; 31];
    buffer[0] = 0xff;
    buffer[1] = 0xff;
    let data = AirQualityAdvertisement::from((device_id, *air_quality));

    let serialized_len = to_slice(&data, &mut buffer[2..]).unwrap().len();

    adv_offset += fill_adv_data(
        &mut adv_data[adv_offset..],
        0xff,
        &buffer[..2 + serialized_len],
    );
    adv_offset
}

