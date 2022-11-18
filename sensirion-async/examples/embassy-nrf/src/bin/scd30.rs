#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_nrf::config::Config;
use embassy_nrf::gpio::Output;
use embassy_nrf::twim::Twim;

use embassy_executor::Spawner;
use embassy_nrf::interrupt;
use embassy_time::{Duration, Timer};
use interrupt::InterruptExt;
use sensirion_async::scd30::Scd30;

use example_embassy_nrf as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Config::default());
    let mut led = Output::new(
        p.P1_10,
        embassy_nrf::gpio::Level::Low,
        embassy_nrf::gpio::OutputDrive::Standard,
    );
    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    irq.set_priority(interrupt::Priority::P2);
    let twi = Twim::new(p.TWISPI0, irq, p.P0_12, p.P0_11, Default::default());
    let mut sensor = Scd30::new(twi);

    defmt::unwrap!(sensor.start_measurement(1015).await);

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(200)).await;
        led.set_low();
        Timer::after(Duration::from_millis(200)).await;
        let version = defmt::unwrap!(sensor.read_version().await);
        defmt::warn!("Version {:x}", version);

        let measurement = defmt::unwrap!(sensor.read().await);
        defmt::warn!("data: {}", measurement);
    }
}
