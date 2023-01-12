#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_nrf::config::Config;
use embassy_nrf::gpio::Output;
use embassy_nrf::twim::{self, Twim};

use embassy_executor::Spawner;
use embassy_nrf::interrupt;
use embassy_time::{Duration, Timer};
use interrupt::InterruptExt;

use sensirion_async::sht4x::{self, Sht4x};

use example_embassy_nrf as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Config::default());
    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    irq.set_priority(interrupt::Priority::P2);
    let mut config = twim::Config::default();
    config.frequency = twim::Frequency::K100;
    config.scl_pullup = true;
    config.sda_pullup = true;
    let mut twi = Twim::new(p.TWISPI0, irq, p.P0_12, p.P0_13, config);
    // let mut sensor = Sht4x::new(twi);

    let mut read_buffer = [0u8; 6];
    defmt::unwrap!(twi.write_read(0x44, &[0x89], &mut read_buffer).await);

    defmt::error!("loop {}", read_buffer);

    loop {
        // match sensor.read_serial_number().await {
        //     Ok(v) => defmt::warn!("Version {:x}", v),
        //     Err(e) => defmt::error!("error {}", e),
        // }

        Timer::after(Duration::from_secs(6)).await;
        // let measurement = defmt::unwrap!(sensor.read().await);
        // defmt::warn!("data: {}", measurement);
    }
}
