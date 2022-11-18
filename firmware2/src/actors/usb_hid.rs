use ector::{actor, Actor, Address, Inbox};
use embassy::time::{Duration, Timer};
use embassy_nrf::peripherals;
use embassy_nrf::usb::embassy_usb::control::OutResponse;
use embassy_nrf::usb::embassy_usb::{Builder, Config};
use embassy_nrf::usb::{Driver, SignalledSupply};
use embassy_usb_hid::{HidWriter, ReportId, RequestHandler, State};
use futures::future::join;

use usbd_hid::descriptor::{AsInputReport, SerializedDescriptor};
use usbd_hid_macros::gen_hid_descriptor;

use crate::models::AirQuality;
use serde::ser::SerializeTuple;
use serde::{Serialize, Serializer};

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = 0x77, usage = 0x01) = {
        (usage = 0x55,) = {
            #[item_settings data,variable,absolute] co2=input;
        };
    }
)]
pub struct AirQualityReport {
    pub co2: u16,
}

#[allow(dead_code)]
pub struct UsbHid {
    pub driver: Option<Driver<'static, peripherals::USBD, &'static SignalledSupply>>,
}

#[actor]
impl Actor for UsbHid {
    type Message<'m> = AirQuality;

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, _: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        // Create embassy-usb Config
        let mut config = Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("Embassy");
        config.product = Some("HID mouse example");
        config.serial_number = Some("12345678");
        config.max_power = 100;
        config.max_packet_size_0 = 64;

        // Create embassy-usb DeviceBuilder using the driver and config.
        // It needs some buffers for building the descriptors.
        let mut device_descriptor = [0; 256];
        let mut config_descriptor = [0; 256];
        let mut bos_descriptor = [0; 256];
        let mut control_buf = [0; 64];
        let request_handler = MyRequestHandler {};

        let mut state = State::new();

        let mut builder = Builder::new(
            self.driver.take().unwrap(),
            config,
            &mut device_descriptor,
            &mut config_descriptor,
            &mut bos_descriptor,
            &mut control_buf,
            None,
        );

        // Create classes on the builder.
        let config = embassy_usb_hid::Config {
            report_descriptor: AirQualityReport::desc(),
            request_handler: Some(&request_handler),
            poll_ms: 60,
            max_packet_size: 8,
        };

        let mut writer = HidWriter::<_, 5>::new(&mut builder, &mut state, config);

        // Build the builder.
        let mut usb = builder.build();

        // Run the USB device.
        let usb_fut = usb.run();

        // Do stuff with the class!
        let hid_fut = async {
            let mut y: i8 = 5;
            loop {
                Timer::after(Duration::from_millis(500)).await;

                y = -y;
                // let report = MouseReport {
                //     buttons: 0,
                //     x: 0,
                //     y,
                //     wheel: 0,
                //     pan: 0,
                // };
                let report = AirQualityReport { co2: 0x66 };
                match writer.write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => defmt::warn!("Failed to send report: {:?}", e),
                }
            }
        };

        // Run everything concurrently.
        // If we had made everything `'static` above instead, we could do this using separate tasks instead.
        join(usb_fut, hid_fut).await;
    }
}

struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
    fn get_report(&self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        defmt::info!("Get report for {:?}", id);
        None
    }

    fn set_report(&self, id: ReportId, data: &[u8]) -> OutResponse {
        defmt::info!("Set report for {:?}: {=[u8]}", id, data);
        OutResponse::Accepted
    }

    fn get_idle(&self, id: Option<ReportId>) -> Option<Duration> {
        defmt::info!("Get idle rate for {:?}", id);
        let _ = id;
        None
    }

    fn set_idle(&self, id: Option<ReportId>, dur: Duration) {
        defmt::info!("Set idle rate for {:?} to {:?}", id, dur);
        let _ = (id, dur);
    }
}
