// use ector::{actor, Actor, Address, Inbox};
// use embassy_nrf::peripherals;
// use embassy_nrf::usb::{Driver, Instance, SignalledSupply, UsbSupply};
// use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
// use embassy_usb::driver::EndpointError;
// use embassy_usb::{Builder, Config};
// use futures::future::join;

// use crate::models::AirQuality;

// pub struct UsbSerial {
//     pub driver: Option<Driver<'static, peripherals::USBD, &'static SignalledSupply>>,
// }

// #[actor]
// impl Actor for UsbSerial {
//     type Message<'m> = AirQuality;

//     async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
//     where
//         M: Inbox<Self::Message<'m>> + 'm,
//     {
//         // Create embassy-usb Config
//         let mut config = Config::new(0xc0de, 0xcafe);
//         config.manufacturer = Some("Embassy");
//         config.product = Some("USB-serial example");
//         config.serial_number = Some("12345678");
//         config.max_power = 100;
//         config.max_packet_size_0 = 64;

//         // Required for windows compatiblity.
//         // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
//         config.device_class = 0xEF;
//         config.device_sub_class = 0x02;
//         config.device_protocol = 0x01;
//         config.composite_with_iads = true;

//         // Create embassy-usb DeviceBuilder using the driver and config.
//         // It needs some buffers for building the descriptors.
//         let mut device_descriptor = [0; 256];
//         let mut config_descriptor = [0; 256];
//         let mut bos_descriptor = [0; 256];
//         let mut control_buf = [0; 64];

//         let mut state = State::new();

//         let mut builder = Builder::new(
//             self.driver.take().unwrap(),
//             config,
//             &mut device_descriptor,
//             &mut config_descriptor,
//             &mut bos_descriptor,
//             &mut control_buf,
//             None,
//         );

//         // Create classes on the builder.
//         let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);

//         // Build the builder.
//         let mut usb = builder.build();

//         // Run the USB device.
//         let usb_fut = usb.run();

//         // Do stuff with the class!
//         let echo_fut = async {
//             loop {
//                 class.wait_connection().await;
//                 defmt::info!("Connected");
//                 loop {
//                     let message = inbox.next().await;

//                     defmt::error!("msg");
//                     if class.write_packet(&[0xde, 0xad, 0xbe, 0xef]).await.is_err() {
//                         defmt::info!("err");
//                         break;
//                     }
//                 }
//                 defmt::info!("Disconnected");
//             }
//         };

//         // Run everything concurrently.
//         // If we had made everything `'static` above instead, we could do this using separate tasks instead.
//         join(usb_fut, echo_fut).await;
//     }
// }

// struct Disconnected {}

// impl From<EndpointError> for Disconnected {
//     fn from(val: EndpointError) -> Self {
//         match val {
//             EndpointError::BufferOverflow => panic!("Buffer overflow"),
//             EndpointError::Disabled => Disconnected {},
//         }
//     }
// }

// async fn echo<'d, T: Instance + 'd, P: UsbSupply + 'd>(
//     inbox: impl Inbox<AirQuality>,
//     class: &mut CdcAcmClass<'d, Driver<'d, T, P>>,
// ) -> Result<(), Disconnected> {
//     let mut buf = [0; 64];
//     loop {
//         let n = class.read_packet(&mut buf).await?;
//         let data = &buf[..n];
//         defmt::info!("data: {:x}", data);
//         class.write_packet(data).await?;
//     }
// }
