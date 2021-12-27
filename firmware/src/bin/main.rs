#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(alloc_error_handler)]
#![allow(incomplete_features)]

use core::cell::Cell;

use air_force_one::sgp40::Sgp40;
use air_force_one::{self as _, scd30::SCD30, sps30::Sps30};
use air_force_one::{tasks, ButtonEvent, Page, StaticSerialClassSet1, StaticUsb};

use embassy::blocking_mutex::kind::Noop;
use embassy::channel::mpsc::{self, Channel};
use embassy::{
    blocking_mutex::CriticalSectionMutex,
    executor::Spawner,
    time::{Duration, Timer},
    util::Forever,
};
use embassy_hal_common::usb::{
    usb_serial::{UsbSerial, USB_CLASS_CDC},
    State,
};
use embassy_nrf::gpio::{Input, Pull};
use embassy_nrf::gpiote::InputChannel;
use embassy_nrf::peripherals::TWISPI0;
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    interrupt,
    twim::{self, Twim},
    usbd::UsbPeripheral,
    Peripherals,
};
use futures_intrusive::sync::LocalMutex;
use nrf_softdevice::ble::{gatt_server, peripheral};
use nrf_softdevice::{raw, Softdevice};
use nrf_usbd::Usbd;
use shared::AirQuality;
use usb_device::{
    class_prelude::UsbBusAllocator,
    device::{UsbDeviceBuilder, UsbVidPid},
};

static SENSOR_BUS: Forever<LocalMutex<Twim<'static, TWISPI0>>> = Forever::new();
static USB_READ_BUFFER: Forever<[u8; 128]> = Forever::new();
static USB_WRITE_BUFFER: Forever<[u8; 128]> = Forever::new();
static USB_ALLOCATOR: Forever<UsbBusAllocator<Usbd<UsbPeripheral>>> = Forever::new();
static USB_STATE: Forever<State<'static, StaticUsb, StaticSerialClassSet1, interrupt::USBD>> =
    Forever::new();
static STATE: Forever<CriticalSectionMutex<Cell<AirQuality>>> = Forever::new();
static PAGE: Forever<CriticalSectionMutex<Cell<Page>>> = Forever::new();
static BUTTON_EVENTS: Forever<Channel<Noop, ButtonEvent, 1>> = Forever::new();

// led 1 - p1.15 - red
// led 2 - p1.10 - blue
// buzz - p0.14
// esc - p0.13
// prev - p0.15
// next - p0.24
// ok - p.025

#[embassy::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}

#[nrf_softdevice::gatt_service(uuid = "180f")]
struct BatteryService {
    #[characteristic(uuid = "2a19", read, notify)]
    battery_level: u8,
}

#[nrf_softdevice::gatt_service(uuid = "9e7312e0-2354-11eb-9f10-fbc30a62cf38")]
struct FooService {
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
struct Server {
    bas: BatteryService,
    foo: FooService,
}

#[embassy::task]
async fn bluetooth_task(sd: &'static Softdevice) {
    let server: Server = defmt::unwrap!(gatt_server::register(sd));

    #[rustfmt::skip]
    let adv_data = &[
        0x02, 0x01, raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8,
        0x03, 0x03, 0x09, 0x18,
        0x0e, 0x09, b'A', b'i', b'r', b'F', b'o', b'r', b'c', b'e', b'O', b'n', b'e', b'V', b'1'
    ];
    #[rustfmt::skip]
    let scan_data = &[
        0x03, 0x03, 0x09, 0x18,
    ];

    loop {
        let config = peripheral::Config::default();
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data,
            scan_data,
        };
        let conn = defmt::unwrap!(peripheral::advertise_connectable(sd, adv, &config).await);

        defmt::info!("advertising done!");

        // Run the GATT server on the connection. This returns when the connection gets disconnected.
        let res = gatt_server::run(&conn, &server, |e| match e {
            ServerEvent::Bas(e) => match e {
                BatteryServiceEvent::BatteryLevelCccdWrite { notifications } => {
                    defmt::info!("battery notifications: {}", notifications)
                }
            },
            ServerEvent::Foo(e) => match e {
                FooServiceEvent::FooWrite(val) => {
                    defmt::info!("wrote foo: {}", val);
                    if let Err(e) = server.foo.foo_notify(&conn, val + 1) {
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
        })
        .await;

        if let Err(e) = res {
            defmt::info!("gatt_server run exited with error: {:?}", e);
        }
    }
}

pub fn embassy_config() -> embassy_nrf::config::Config {
    let mut config = embassy_nrf::config::Config::default();
    config.hfclk_source = embassy_nrf::config::HfclkSource::Internal;
    config.lfclk_source = embassy_nrf::config::LfclkSource::InternalRC;
    config.time_interrupt_priority = interrupt::Priority::P2;
    // if we see button misses lower this
    config.gpiote_interrupt_priority = interrupt::Priority::P7;
    config
}

#[embassy::main(config = "embassy_config()")]
async fn main(spawner: Spawner, p: Peripherals) {
    defmt::info!("Hello World!");

    let config = nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 4,
            rc_temp_ctiv: 2,
            accuracy: 7,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 6,
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: 32768,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
            central_role_count: 3,
            central_sec_count: 0,
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: b"AirForceOneV1" as *const u8 as _,
            current_len: 13,
            max_len: 13,
            write_perm: unsafe { core::mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    };

    let sd = Softdevice::enable(&config);

    let mut led = Output::new(p.P1_10, Level::Low, OutputDrive::Standard);
    let buzz = Output::new(p.P0_14, Level::Low, OutputDrive::Standard);
    let esc = InputChannel::new(
        p.GPIOTE_CH0,
        Input::new(p.P0_13, Pull::Up),
        embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
    );

    let prev = InputChannel::new(
        p.GPIOTE_CH1,
        Input::new(p.P0_15, Pull::Up),
        embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
    );

    let next = InputChannel::new(
        p.GPIOTE_CH2,
        Input::new(p.P0_24, Pull::Up),
        embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
    );
    let ok = InputChannel::new(
        p.GPIOTE_CH3,
        Input::new(p.P0_25, Pull::Up),
        embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
    );

    Timer::after(Duration::from_millis(500)).await;
    let config = twim::Config::default();
    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    let twi = Twim::new(p.TWISPI0, irq, p.P0_12, p.P0_11, config);

    let sensor_bus = LocalMutex::new(twi, true);

    let sensor_bus = SENSOR_BUS.put(sensor_bus);

    let co2_sensor = SCD30::init(sensor_bus);
    let pm_sensor = Sps30::new(sensor_bus);
    let voc_sensor = Sgp40::init(sensor_bus);

    let display_twi_config = twim::Config::default();
    let display_twi_irq = interrupt::take!(SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1);
    let display_twi = Twim::new(
        p.TWISPI1,
        display_twi_irq,
        p.P1_08,
        p.P0_07,
        display_twi_config,
    );

    let bus = USB_ALLOCATOR.put(Usbd::new(UsbPeripheral::new(p.USBD)));

    defmt::error!("Hello world");

    let read_buf = USB_READ_BUFFER.put([0u8; 128]);
    let write_buf = USB_WRITE_BUFFER.put([0u8; 128]);
    let serial = UsbSerial::new(bus, read_buf, write_buf);

    let device = UsbDeviceBuilder::new(bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("AFO")
        .device_class(USB_CLASS_CDC)
        .build();

    let state = USB_STATE.put(embassy_hal_common::usb::State::new());

    // sprinkle some unsafe

    let usb =
        unsafe { embassy_hal_common::usb::Usb::new(state, device, serial, interrupt::take!(USBD)) };

    unsafe {
        (*embassy_nrf::pac::USBD::ptr()).intenset.write(|w| {
            w.sof().set_bit();
            w.usbevent().set_bit();
            w.ep0datadone().set_bit();
            w.ep0setup().set_bit();
            w.usbreset().set_bit()
        })
    };

    let channel = BUTTON_EVENTS.put(Channel::new());
    let (sender, receiver) = mpsc::split(channel);
    let state = STATE.put(CriticalSectionMutex::new(Cell::new(AirQuality::default())));
    let page = PAGE.put(CriticalSectionMutex::new(Cell::new(Page::Basic)));
    defmt::unwrap!(spawner.spawn(tasks::sensors::co2_task(co2_sensor, state)));
    defmt::unwrap!(spawner.spawn(tasks::sensors::pm_task(pm_sensor, state)));
    defmt::unwrap!(spawner.spawn(tasks::sensors::voc_task(voc_sensor, state)));
    defmt::unwrap!(spawner.spawn(tasks::display::render(display_twi, state, page)));
    defmt::unwrap!(spawner.spawn(tasks::display::navigation(receiver, page)));
    defmt::unwrap!(spawner.spawn(tasks::usb::communication(usb, state)));
    defmt::unwrap!(spawner.spawn(tasks::buttons::task(esc, prev, next, ok, sender)));
    defmt::unwrap!(spawner.spawn(tasks::reporting::task(state, buzz)));
    defmt::unwrap!(spawner.spawn(softdevice_task(sd)));
    defmt::unwrap!(spawner.spawn(bluetooth_task(sd)));

    loop {
        if led.is_set_high() {
            led.set_low();
        } else {
            led.set_high();
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}
