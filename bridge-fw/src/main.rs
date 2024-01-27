#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;

use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::{Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_nrf::gpio::{AnyPin, Level, Output, OutputDrive, Pin as _};
use embassy_nrf::usb::vbus_detect::SoftwareVbusDetect;
use embassy_nrf::usb::Driver;
use embassy_nrf::{self as _, bind_interrupts, peripherals, usb};
use embassy_sync::blocking_mutex::ThreadModeMutex;
use embassy_time::Timer;
use embassy_usb::class::cdc_ncm::embassy_net::State as NetState;
use embassy_usb::class::cdc_ncm::embassy_net::{Device, Runner};
use embassy_usb::class::cdc_ncm::CdcNcmClass;
use embassy_usb::UsbDevice;
use nrf_softdevice::ble::central;
use nrf_softdevice::{raw, SocEvent, Softdevice};

use heapless::Vec;
use rust_mqtt::client::client::MqttClient;
use shared::AirQualityAdvertisement;
use static_cell::make_static;

use core::cell::RefCell;
use core::fmt::Write;
use core::{mem, slice};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
});

#[cfg(feature = "dev")]
use panic_probe as _;

#[cfg(not(feature = "dev"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    defmt::error!("panic!");
    cortex_m::peripheral::SCB::sys_reset();
}

#[cortex_m_rt::exception]
unsafe fn HardFault(_frame: &cortex_m_rt::ExceptionFrame) -> ! {
    cortex_m::peripheral::SCB::sys_reset()
}

type UsbDriver = Driver<'static, peripherals::USBD, &'static SoftwareVbusDetect>;

const MTU: usize = 1514;

const MEASUREMENT_COUNT: usize = 2;

struct AppState {
    measurements: [AirQualityAdvertisement; MEASUREMENT_COUNT],
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_nrf::config::Config::default();
    config.hfclk_source = embassy_nrf::config::HfclkSource::ExternalXtal;
    config.lfclk_source = embassy_nrf::config::LfclkSource::InternalRC;
    config.time_interrupt_priority = embassy_nrf::interrupt::Priority::P2;
    config.gpiote_interrupt_priority = embassy_nrf::interrupt::Priority::P7;

    unsafe { reinitialize_reset() };

    let p = embassy_nrf::init(config);

    let config = softdevice_config();

    let software_vbus = make_static!(SoftwareVbusDetect::new(true, true));

    let state = make_static!(ThreadModeMutex::new(RefCell::new(AppState {
        measurements: [AirQualityAdvertisement::default(); MEASUREMENT_COUNT],
    })));

    let sd = Softdevice::enable(&config);
    defmt::unwrap!(spawner.spawn(softdevice_task(sd, software_vbus)));
    defmt::unwrap!(spawner.spawn(scan_task(sd, state)));

    let driver = Driver::new(p.USBD, Irqs, &*software_vbus);

    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("MatousHybl");
    config.product = Some("AFO-Bridge");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Required for Windows support.
    config.composite_with_iads = true;
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;

    let mut builder = embassy_usb::Builder::new(
        driver,
        config,
        make_static!([0; 256]),
        make_static!([0; 256]),
        make_static!([0; 256]),
        make_static!([0; 128]),
        make_static!([0; 128]),
    );

    // Our MAC addr.
    let our_mac_addr = [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC];
    // Host's MAC addr. This is the MAC the host "thinks" its USB-to-ethernet adapter has.
    let host_mac_addr = [0x88, 0x88, 0x88, 0x88, 0x88, 0x88];

    let class = CdcNcmClass::new(
        &mut builder,
        make_static!(embassy_usb::class::cdc_ncm::State::new()),
        host_mac_addr,
        64,
    );

    let usb = builder.build();

    defmt::unwrap!(spawner.spawn(usb_task(usb)));

    let (runner, device) = class.into_embassy_net_device::<MTU, 4, 4>(
        make_static!(NetState::<MTU, 4, 4>::new()),
        our_mac_addr,
    );
    defmt::unwrap!(spawner.spawn(usb_ncm_task(runner)));

    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(10, 42, 0, 61), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(10, 42, 0, 1)),
    });

    // Generate random seed
    // wait for rnd to have enough entropy?
    Timer::after_secs(1).await;
    let mut raw_seed = [0u8; 8];
    nrf_softdevice::random_bytes(&sd, &mut raw_seed).unwrap();
    let seed = u64::from_le_bytes(raw_seed);

    let resources = make_static!(StackResources::<2>::new());
    let stack = make_static!(Stack::<Device<'static, MTU>>::new(
        device, config, resources, seed,
    ));

    defmt::unwrap!(spawner.spawn(net_task(stack)));
    defmt::unwrap!(spawner.spawn(send_measurements_task(sd, stack, state)));
    defmt::unwrap!(spawner.spawn(blink_task(Output::new(
        p.P1_15.degrade(),
        Level::Low,
        OutputDrive::Standard
    ))));
}

/// Periodically sends the measurements over MQTT
#[embassy_executor::task]
async fn send_measurements_task(
    sd: &'static Softdevice,
    stack: &'static Stack<Device<'static, MTU>>,
    state: &'static ThreadModeMutex<RefCell<AppState>>,
) {
    let rx_buffer = make_static!([0; 512]);
    let tx_buffer = make_static!([0; 512]);

    loop {
        let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);
        if socket
            .connect((Ipv4Address::new(10, 42, 0, 1), 1883))
            .await
            .is_err()
        {
            defmt::error!("failed to connect to MQTT broker");
            Timer::after_secs(2).await;
            continue;
        }

        let mut config = rust_mqtt::client::client_config::ClientConfig::new(
            rust_mqtt::client::client_config::MqttVersion::MQTTv5,
            SoftdeviceRng { sd },
        );
        config.add_max_subscribe_qos(rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS0);
        config.add_client_id("afo-bridge");
        config.max_packet_size = 100;
        let mut recv_buffer = [0; 80];
        let mut write_buffer = [0; 80];

        let mut client =
            MqttClient::<_, 5, _>::new(socket, &mut write_buffer, 80, &mut recv_buffer, 80, config);

        if client.connect_to_broker().await.is_err() {
            defmt::error!("failed to connect to MQTT broker");
            Timer::after_secs(2).await;
            continue;
        }

        for sensor_id in 0..MEASUREMENT_COUNT {
            let mut json = heapless::String::<64>::new();

            {
                let s = state.lock(|c| c.borrow().measurements[sensor_id]);
                write!(
                    &mut json,
                    r#"{{"co2": "{}", "temperature": "{:.1}", "humidity": "{}"}}"#,
                    s.co2_concentration,
                    s.temperature as f32 * 0.1,
                    s.humidity
                )
                .unwrap();
            }

            let mut topic = heapless::String::<10>::new();
            write!(topic, "afo-{}", sensor_id).unwrap();

            if client
                .send_message(
                    topic.as_str(),
                    json.as_bytes(),
                    rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS0,
                    true,
                )
                .await
                .is_err()
            {
                defmt::error!("failed to send MQTT message");
                Timer::after_secs(2).await;
                continue;
            }
        }

        // do not remove as the mqtt message will not be sent.
        // rust mqtt doesn't support flushing at the moment
        Timer::after_secs(2).await;
    }
}

/// Scans for AFO devices and saves their measurements to be sent over MQTT
#[embassy_executor::task]
async fn scan_task(sd: &'static Softdevice, state: &'static ThreadModeMutex<RefCell<AppState>>) {
    let config = central::ScanConfig::default();
    let res = central::scan(sd, &config, |params| unsafe {
        let mut data = slice::from_raw_parts(params.data.p_data, params.data.len as usize);
        let mut afo = false;
        while data.len() != 0 {
            let len = data[0] as usize;
            if data.len() < len + 1 {
                defmt::warn!("Advertisement data truncated?");
                break;
            }
            if len < 1 {
                defmt::warn!("Advertisement data malformed?");
                break;
            }
            let key = data[1];
            let value = &data[2..len + 1];
            data = &data[len + 1..];
            // device has sent a name
            if key == 9 && value.len() == 3 && value == b"AFO" {
                afo = true;
            }
            if afo && key == 0xff {
                let adv = postcard::from_bytes::<AirQualityAdvertisement>(&value[2..]).unwrap();
                state.lock(|c| {
                    c.borrow_mut().measurements[adv.sensor_id as usize] = adv;
                });
                defmt::trace!("AFO: {:?}", adv);
            }
        }
        None
    })
    .await;
    defmt::unwrap!(res);
    defmt::info!("Scan returned");
}

/// Blink the LED for a very short time, to avoid the blinking being distracting at night
#[embassy_executor::task]
async fn blink_task(mut led: Output<'static, AnyPin>) {
    loop {
        led.set_high();
        Timer::after_millis(10).await;
        led.set_low();
        Timer::after_secs(60).await;
    }
}

/// Runs the softdevice, while listening for USB VBUS detect events
#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice, software_vbus: &'static SoftwareVbusDetect) -> ! {
    unsafe {
        nrf_softdevice::raw::sd_power_usbdetected_enable(1);
        nrf_softdevice::raw::sd_power_usbpwrrdy_enable(1);
        nrf_softdevice::raw::sd_power_usbremoved_enable(1);
        nrf_softdevice::raw::sd_clock_hfclk_request();
    };
    sd.run_with_callback(|event| {
        match event {
            SocEvent::PowerUsbRemoved => software_vbus.detected(false),
            SocEvent::PowerUsbDetected => software_vbus.detected(true),
            SocEvent::PowerUsbPowerReady => software_vbus.ready(),
            _ => {}
        };
    })
    .await
}

#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, UsbDriver>) -> ! {
    device.run().await
}

#[embassy_executor::task]
async fn usb_ncm_task(class: Runner<'static, UsbDriver, MTU>) -> ! {
    class.run().await
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<Device<'static, MTU>>) -> ! {
    stack.run().await
}

fn softdevice_config() -> nrf_softdevice::Config {
    nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 6,
            event_length: 6,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 128 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: raw::BLE_GATTS_ATTR_TAB_SIZE_DEFAULT,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
            central_role_count: 3,
            central_sec_count: 0,
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: b"HelloRust" as *const u8 as _,
            current_len: 9,
            max_len: 9,
            write_perm: unsafe { mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    }
}

/// A random number generator that uses the softdevice to generate random numbers.
struct SoftdeviceRng {
    sd: &'static Softdevice,
}

impl rand_core::RngCore for SoftdeviceRng {
    fn next_u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        self.fill_bytes(&mut buf);
        u32::from_le_bytes(buf)
    }

    fn next_u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        self.fill_bytes(&mut buf);
        u64::from_le_bytes(buf)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        nrf_softdevice::random_bytes(self.sd, dest).unwrap();
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        nrf_softdevice::random_bytes(self.sd, dest).unwrap();
        Ok(())
    }
}

/// Reinitializes reset pin in the hardware.
///
/// ```
///
/// # Safety
/// We are directly accessing registers using raw pointers which is unsafe.
/// .
pub unsafe fn reinitialize_reset() {
    let nvmc = &*embassy_nrf::pac::NVMC::ptr();
    if *(0x10001200 as *mut u32) != 18 || *(0x10001204 as *mut u32) != 18 {
        nvmc.config.write(|w| w.wen().wen());
        while nvmc.ready.read().ready().is_busy() {}
        core::ptr::write_volatile(0x10001200 as *mut u32, 18);
        while nvmc.ready.read().ready().is_busy() {}
        core::ptr::write_volatile(0x10001204 as *mut u32, 18);
        while nvmc.ready.read().ready().is_busy() {}
        nvmc.config.reset();
        while nvmc.ready.read().ready().is_busy() {}
        cortex_m::peripheral::SCB::sys_reset();
    }
}
