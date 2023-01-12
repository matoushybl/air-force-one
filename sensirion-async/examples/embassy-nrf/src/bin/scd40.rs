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

use sensirion_async::scd4x::{self, Celsius, Meter, Scd4x};

use example_embassy_nrf as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Config::default());
    let mut led = Output::new(
        p.P1_15,
        embassy_nrf::gpio::Level::Low,
        embassy_nrf::gpio::OutputDrive::Standard,
    );
    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    irq.set_priority(interrupt::Priority::P2);
    let mut config = twim::Config::default();
    config.frequency = twim::Frequency::K100;
    config.scl_pullup = true;
    config.sda_pullup = true;
    let twi = Twim::new(p.TWISPI0, irq, p.P0_12, p.P0_13, config);
    let mut sensor = Scd4x::new(twi);

    defmt::unwrap!(sensor.stop_periodic_measurement().await);

    Timer::after(Duration::from_millis(500)).await;

    defmt::error!("altitude: {:?}", sensor.get_sensor_altitude().await);
    defmt::error!(
        "altitude: {:?}",
        sensor.set_sensor_altitude(Meter(230)).await
    );
    defmt::error!("altitude: {:?}", sensor.get_sensor_altitude().await);

    defmt::error!("temp offset {:?}", sensor.get_temperature_offset().await);
    defmt::error!(
        "temp offset {:?}",
        sensor.set_temperature_offset(Celsius(2.0)).await
    );
    defmt::error!("temp offset {:?}", sensor.get_temperature_offset().await);

    let serial_number = defmt::unwrap!(sensor.read_serial_number().await);
    defmt::warn!("SCD4x serial number: {:x}", serial_number);

    defmt::unwrap!(sensor.start_periodic_measurement().await);

    defmt::error!("loop");

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(200)).await;
        led.set_low();
        Timer::after(Duration::from_millis(200)).await;
        // let version = defmt::unwrap!(sensor.read_version().await);
        // defmt::warn!("Version {:x}", version);

        Timer::after(Duration::from_secs(6)).await;
        let measurement = defmt::unwrap!(sensor.read().await);
        defmt::warn!("data: {}", measurement);
    }
}
