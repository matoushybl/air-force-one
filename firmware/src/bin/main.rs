#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(alloc_error_handler)]
#![allow(incomplete_features)]
#![feature(cell_update)]

use core::cell::Cell;

use air_force_one::board::Board;
use air_force_one::sensirion_i2c::SensirionI2c;
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
use embassy_nrf::Peripherals;
use shared::AirQuality;

static STATE: Forever<CriticalSectionMutex<Cell<AirQuality>>> = Forever::new();
static PAGE: Forever<CriticalSectionMutex<Cell<Page>>> = Forever::new();
static BUTTON_EVENTS: Forever<Channel<Noop, ButtonEvent, 1>> = Forever::new();

#[embassy::main(config = "air_force_one::embassy_config()")]
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

    let board = Board::new(p);

    let co2_sensor = SCD30::init(SensirionI2c::new(board.sensor_bus));
    let pm_sensor = Sps30::new(SensirionI2c::new(board.sensor_bus));
    let voc_sensor = Sgp40::init(SensirionI2c::new(board.sensor_bus));

    defmt::error!("Hello world");

    let channel = BUTTON_EVENTS.put(Channel::new());
    let (sender, receiver) = mpsc::split(channel);
    let state = STATE.put(CriticalSectionMutex::new(Cell::new(AirQuality::default())));
    let page = PAGE.put(CriticalSectionMutex::new(Cell::new(Page::Basic)));

    defmt::unwrap!(spawner.spawn(tasks::sensors::co2_task(co2_sensor, state)));
    defmt::unwrap!(spawner.spawn(tasks::sensors::pm_task(pm_sensor, state)));
    defmt::unwrap!(spawner.spawn(tasks::sensors::voc_task(voc_sensor, state)));
    defmt::unwrap!(spawner.spawn(tasks::display::render(board.display_bus, state, page)));
    defmt::unwrap!(spawner.spawn(tasks::display::navigation(receiver, page)));
    defmt::unwrap!(spawner.spawn(tasks::usb::communication(board.usb, state)));
    defmt::unwrap!(spawner.spawn(tasks::buttons::task(
        board.esc_button,
        board.prev_button,
        board.next_button,
        board.ok_button,
        sender
    )));
    defmt::unwrap!(spawner.spawn(tasks::reporting::task(state, board.buzzer)));
    defmt::unwrap!(spawner.spawn(tasks::bluetooth::softdevice_task(board.softdevice)));
    defmt::unwrap!(spawner.spawn(tasks::bluetooth::bluetooth_task(board.softdevice, state)));

    let mut led = board.led;
    loop {
        if led.is_set_high() {
            led.set_low();
        } else {
            led.set_high();
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}
