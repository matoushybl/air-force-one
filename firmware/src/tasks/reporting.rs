use core::cell::Cell;

use embassy::blocking_mutex::{CriticalSectionMutex, Mutex};
use embassy::time::{Duration, Timer};
use embassy_nrf::gpio::Output;
use embassy_nrf::peripherals;
use shared::AirQuality;

#[embassy::task]
pub async fn task(
    state: &'static CriticalSectionMutex<Cell<AirQuality>>,
    mut buzz: Output<'static, peripherals::P0_14>,
) {
    let mut count = 0;
    loop {
        Timer::after(Duration::from_secs(1)).await;
        if state.lock(|state| state.get().co2_concentration) > 1500.0 {
            count += 1;
        } else {
            count = 0;
        }

        if count >= 10 {
            // buzz.set_high();
            Timer::after(Duration::from_millis(200)).await;
            buzz.set_low();
            count = 0;
        }
    }
}
