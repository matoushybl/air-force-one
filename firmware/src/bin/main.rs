#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(alloc_error_handler)]
#![allow(incomplete_features)]
#![feature(cell_update)]

use core::cell::Cell;

use air_force_one::sensirion_i2c::SensirionI2c;
use air_force_one::sgp40::Sgp40;
use air_force_one::{self as _, scd30::SCD30, sps30::Sps30};
use air_force_one::{tasks, ButtonEvent, Page, StaticSerialClassSet1, StaticUsb};

use embassy::blocking_mutex::kind::Noop;
use embassy::blocking_mutex::Mutex;
use embassy::channel::mpsc::{self, Channel};
use embassy::interrupt::InterruptExt;
use embassy::{
    blocking_mutex::CriticalSectionMutex,
    executor::Spawner,
    time::{Duration, Timer},
    util::Forever,
};
use embassy_hal_common::usb::{usb_serial::UsbSerial, State};
use embassy_nrf::gpio::{Input, Pull};
use embassy_nrf::gpiote::InputChannel;
use embassy_nrf::peripherals::{TWISPI0, USBD};
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    interrupt,
    twim::{self, Twim},
    usb::UsbBus,
    Peripherals,
};
use futures::future::select;
use futures::pin_mut;
use futures_intrusive::sync::LocalMutex;
use nrf_softdevice::ble::peripheral;
use nrf_softdevice::Softdevice;
use nrf_usbd::Usbd;
use postcard::to_slice;
use shared::{AirQuality, AirQualityAdvertisement};
use usb_device::{
    class_prelude::UsbBusAllocator,
    device::{UsbDeviceBuilder, UsbVidPid},
};

static SENSOR_BUS: Forever<LocalMutex<Twim<'static, TWISPI0>>> = Forever::new();
static USB_READ_BUFFER: Forever<[u8; 128]> = Forever::new();
static USB_WRITE_BUFFER: Forever<[u8; 128]> = Forever::new();
static USB_ALLOCATOR: Forever<UsbBusAllocator<Usbd<UsbBus<'static, USBD>>>> = Forever::new();
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

#[embassy::task]
async fn bluetooth_task(
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

    if let Some(msg) = panic_persist::get_panic_message_bytes() {
        defmt::error!(
            "panic_raw: {} {:x}",
            unsafe { core::str::from_utf8_unchecked(msg) },
            msg
        );
    }

    unsafe { air_force_one::reinitialize_reset() };

    Timer::after(Duration::from_millis(1000)).await;

    let config = twim::Config::default();
    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    irq.set_priority(interrupt::Priority::P2);
    let twi = Twim::new(p.TWISPI0, irq, p.P0_12, p.P0_11, config);

    let display_twi_config = twim::Config::default();
    let display_twi_irq = interrupt::take!(SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1);
    display_twi_irq.set_priority(interrupt::Priority::P2);
    let display_twi = Twim::new(
        p.TWISPI1,
        display_twi_irq,
        p.P1_08,
        p.P0_07,
        display_twi_config,
    );

    let usb_irq = interrupt::take!(USBD);
    usb_irq.set_priority(interrupt::Priority::P2);

    let bus = USB_ALLOCATOR.put(UsbBus::new(p.USBD));

    let read_buf = USB_READ_BUFFER.put([0u8; 128]);
    let write_buf = USB_WRITE_BUFFER.put([0u8; 128]);
    let serial = UsbSerial::new(bus, read_buf, write_buf);

    let device = UsbDeviceBuilder::new(bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("AFO")
        .device_class(0x02)
        .build();

    let state = USB_STATE.put(embassy_hal_common::usb::State::new());
    let usb = unsafe { embassy_hal_common::usb::Usb::new(state, device, serial, usb_irq) };

    let sd = Softdevice::enable(&air_force_one::softdevice_config());

    defmt::unwrap!(spawner.spawn(softdevice_task(sd)));

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

    let sensor_bus = LocalMutex::new(twi, true);

    let sensor_bus = SENSOR_BUS.put(sensor_bus);

    let co2_sensor = SCD30::init(SensirionI2c::new(sensor_bus));
    let pm_sensor = Sps30::new(SensirionI2c::new(sensor_bus));
    let voc_sensor = Sgp40::init(SensirionI2c::new(sensor_bus));

    defmt::error!("Hello world");

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
    defmt::unwrap!(spawner.spawn(bluetooth_task(sd, state)));

    loop {
        if led.is_set_high() {
            led.set_low();
        } else {
            led.set_high();
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}
