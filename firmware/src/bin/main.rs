#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(alloc_error_handler)]
#![allow(incomplete_features)]
#![feature(cell_update)]

use air_force_one::app::App;

use embassy::{
    executor::Spawner,
    time::{Duration, Timer},
};
use embassy_nrf::Peripherals;

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

    App::run(spawner, p).await
}
