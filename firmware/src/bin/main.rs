#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use core::cell::Cell;

use air_force_one::sgp40::Sgp40;
use air_force_one::{self as _, scd30::SCD30, sps30::Sps30};
use air_force_one::{tasks, ButtonEvent, Page};

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
    ClassSet1, State,
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
use embedded_hal::digital::v2::{OutputPin, StatefulOutputPin};
use futures_intrusive::sync::LocalMutex;
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
static USB_STATE: Forever<
    State<
        'static,
        Usbd<UsbPeripheral<'static>>,
        ClassSet1<
            Usbd<UsbPeripheral<'static>>,
            UsbSerial<'static, 'static, Usbd<UsbPeripheral<'static>>>,
        >,
        interrupt::USBD,
    >,
> = Forever::new();
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

#[embassy::main]
async fn main(spawner: Spawner, p: Peripherals) {
    defmt::info!("Hello World!");

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

    loop {
        if defmt::unwrap!(led.is_set_high()) {
            defmt::unwrap!(led.set_low());
        } else {
            defmt::unwrap!(led.set_high());
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}
