use core::num::Wrapping;

use ector::{actor, Actor, Address, Inbox};
use embassy::time::{Duration, Timer};
use embassy::util::{select, Either};
use embassy_nrf::usb::SignalledSupply;
use nrf_softdevice::ble::{gatt_server, peripheral};
use nrf_softdevice::Softdevice;
use nrf_softdevice::{gatt_server, raw};
use postcard::to_slice;

use crate::models::AirQuality;

#[embassy::task]
pub async fn softdevice_task(sd: &'static Softdevice, supply: &'static SignalledSupply) {
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

#[nrf_softdevice::gatt_service(uuid = "180f")]
pub struct BatteryService {
    #[characteristic(uuid = "2a19", read, notify)]
    battery_level: u8,
}

#[nrf_softdevice::gatt_service(uuid = "9e7312e0-2354-11eb-9f10-fbc30a62cf38")]
pub struct FooService {
    #[characteristic(
        uuid = "9e7312e0-2354-11eb-9f10-fbc30a63cf38",
        read,
        write,
        notify,
        indicate
    )]
    foo: u16,
}

#[nrf_softdevice::gatt_server]
pub struct Server {
    bas: BatteryService,
    foo: FooService,
}

pub struct Ble {
    pub softdevice: &'static Softdevice,
    pub server: Server,
    pub air_quality: AirQuality,
}

#[actor]
impl Actor for Ble {
    type Message<'m> = AirQuality;
    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        let mut battery_level = Wrapping(0u8);
        let mut notifications_enabled = false;
        loop {
            let config = peripheral::Config::default();

            let mut adv_data = [0u8; 31];
            let adv_len = build_adv_data(&self.air_quality, &mut adv_data);

            let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
                adv_data: &adv_data[..adv_len],
                scan_data: &[0x03, 0x03, 0x09, 0x18],
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
                        battery_level += 1;
                        if notifications_enabled {
                            defmt::unwrap!(self
                                .server
                                .bas
                                .battery_level_notify(&conn, battery_level.0));
                        } else {
                            defmt::unwrap!(self.server.bas.battery_level_set(battery_level.0));
                        }

                        // Run the GATT server on the connection. This returns when the connection gets disconnected.
                        let server = gatt_server::run(&conn, &self.server, |e| match e {
                            ServerEvent::Bas(e) => match e {
                                BatteryServiceEvent::BatteryLevelCccdWrite { notifications } => {
                                    defmt::info!("battery notifications: {}", notifications);
                                    notifications_enabled = notifications;
                                }
                            },
                            ServerEvent::Foo(e) => match e {
                                FooServiceEvent::FooWrite(val) => {
                                    defmt::info!("wrote foo: {}", val);
                                    if let Err(e) = self.server.foo.foo_notify(&conn, val + 1) {
                                        defmt::info!("send notification error: {:?}", e);
                                    }
                                }
                                FooServiceEvent::FooCccdWrite {
                                    indications,
                                    notifications,
                                } => {
                                    defmt::info!(
                                        "foo indications: {}, notifications: {}",
                                        indications,
                                        notifications
                                    )
                                }
                            },
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
                            }
                        }
                    }
                }
                Either::Second(air_quality) => {
                    defmt::error!("new aq");
                    self.air_quality = air_quality;
                }
            }
        }
    }
}

fn build_adv_data(air_quality: &AirQuality, adv_data: &mut [u8; 31]) -> usize {
    let mut adv_offset = 0;

    adv_offset += fill_adv_data(
        &mut adv_data[..],
        0x01,
        &[nrf_softdevice::raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8],
    );
    adv_offset += fill_adv_data(&mut adv_data[adv_offset..], 0x03, &[0x09, 0x18]);
    adv_offset += fill_adv_data(&mut adv_data[adv_offset..], 0x09, &[b'A', b'F', b'O']);

    let mut buffer = [0u8; 31];
    buffer[0] = 0xff;
    buffer[1] = 0xff;
    let data = AirQualityAdvertisement::from(*air_quality);

    defmt::error!("wtf: {:?}", data);

    let serialized_len = to_slice(&data, &mut buffer[2..]).unwrap().len();

    adv_offset += fill_adv_data(
        &mut adv_data[adv_offset..],
        0xff,
        &buffer[..2 + serialized_len],
    );
    adv_offset
}

#[derive(
    defmt::Format, serde::Serialize, serde::Deserialize, Eq, PartialEq, Default, Clone, Copy,
)]
pub struct AirQualityAdvertisement {
    pub co2_concentration: u16,
    pub temperature: i16, // scaled by 0.1
    pub humidity: u8,
    pub mass_pm1_0: u16,
    pub mass_pm2_5: u16,
    pub mass_pm4_0: u16,
    pub mass_pm10: u16,
    pub voc_index: u16,
}

impl From<AirQuality> for AirQualityAdvertisement {
    fn from(raw: AirQuality) -> Self {
        AirQualityAdvertisement {
            co2_concentration: raw.co2.0 as u16,
            temperature: (raw.temperature.0 / 0.1) as i16,
            humidity: raw.humidity.0 as u8,
            mass_pm1_0: raw.pm.mass_10 as u16,
            mass_pm2_5: raw.pm.mass_25 as u16,
            mass_pm4_0: raw.pm.mass_40 as u16,
            mass_pm10: raw.pm.mass_100 as u16,
            voc_index: raw.voc.index as u16,
        }
    }
}

/// Fills adv data with data type, length and data, return size of the filled data
pub fn fill_adv_data(adv_data: &mut [u8], data_type: u8, data: &[u8]) -> usize {
    let offset = 1 + data.len();
    adv_data[0] = offset as u8;
    adv_data[1] = data_type;
    adv_data[2..][..data.len()].copy_from_slice(data);
    offset + 1
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use postcard::{from_bytes_cobs, to_slice, to_slice_cobs};

//     #[test]
//     fn it_works() {
//         // let data = AirQuality {
//         //     co2_concentration: 400.0,
//         //     temperature: 22.5,
//         //     humidity: 40.0,
//         //     mass_pm1_0: 10.0,
//         //     mass_pm2_5: 10.0,
//         //     mass_pm4_0: 10.0,
//         //     mass_pm10: 10.0,
//         //     number_pm0_5: 2.0,
//         //     number_pm1_0: 2.0,
//         //     number_pm2_5: 2.0,
//         //     number_pm4_0: 2.0,
//         //     number_pm10: 2.0,
//         //     typical_particulate_matter_size: 1.3,
//         //     voc_index: 100,
//         // };
//         // let mut buffer = [0u8; 100];
//         // let output = to_slice_cobs(&data, &mut buffer).unwrap();

//         // println!("size: {}", output.len());

//         // assert!(output.len() > 0);
//         // assert!(output.len() < 100);

//         // let data = from_bytes_cobs::<AirQuality>(output).unwrap();
//         // assert!(data.typical_particulate_matter_size > 1.2);
//         // assert!(data.typical_particulate_matter_size < 1.4);
//     }

//     #[test]
//     fn adv() {
//         let data = AirQualityAdvertisement {
//             co2_concentration: 400,
//             temperature: 225,
//             humidity: 40,
//             mass_pm1_0: 10,
//             mass_pm2_5: 10,
//             mass_pm4_0: 10,
//             mass_pm10: 10,
//             voc_index: 100,
//         };
//         let mut buffer = [0u8; 100];
//         let output = to_slice(&data, &mut buffer).unwrap();

//         // println!("size: {}", output.len());
//     }

//     #[test]
//     fn fill_adv_data_test() {
//         let mut data = [0u8; 31];
//         let mut offset = 0;

//         offset += fill_adv_data(&mut data, 0x01, &[0x03]);
//         offset += fill_adv_data(&mut data[offset..], 0x09, &[b'A', b'F', b'O']);
//         offset += fill_adv_data(&mut data[offset..], 0xff, &[0xff, 0xff]);

//         #[rustfmt::skip]
//         let correct_data = &[
//             0x02, 0x01, 0x03,
//             0x04, 0x09, b'A', b'F', b'O',
//             0x03, 0xff, 0xff, 0xff,
//         ];

//         assert_eq!(&data[..offset], &correct_data[..])
//     }
// }
