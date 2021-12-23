use core::cell::Cell;

use embassy::blocking_mutex::CriticalSectionMutex;
use embassy::blocking_mutex::Mutex;
use embassy::time::{Duration, Timer};
use embassy_nrf::peripherals;
use embassy_nrf::twim::Twim;
use shared::AirQuality;

use crate::scd30::SCD30;
use crate::sps30::Sps30;

#[embassy::task]
pub async fn co2_task(
    mut sensor: SCD30<'static, Twim<'static, peripherals::TWISPI0>>,
    state: &'static CriticalSectionMutex<Cell<AirQuality>>,
) {
    defmt::info!(
        "SCD30: {}",
        defmt::unwrap!(sensor.get_temperature_offset().await)
    );
    defmt::unwrap!(sensor.set_temperature_offset(3.8).await);
    loop {
        if sensor.get_data_ready().await.unwrap() {
            let measurement = sensor.read_measurement().await.unwrap();
            state.lock(|data| {
                let mut raw = data.get();
                raw.co2_concentration = measurement.co2;
                raw.temperature = measurement.temperature;
                raw.humidity = measurement.humidity;
                data.set(raw)
            });
            defmt::info!("SCD30: co2 {}", measurement.co2);
        }
    }
}

#[embassy::task]
pub async fn pm_task(
    mut sensor: Sps30<'static, Twim<'static, peripherals::TWISPI0>>,
    state: &'static CriticalSectionMutex<Cell<AirQuality>>,
) {
    let version = defmt::unwrap!(sensor.read_version().await);

    defmt::error!("SPS30: version {:x}", version);

    defmt::unwrap!(sensor.start_measurement().await);

    Timer::after(Duration::from_millis(2000)).await;

    loop {
        Timer::after(Duration::from_millis(500)).await;

        if defmt::unwrap!(sensor.is_ready().await) {
            let measured = defmt::unwrap!(sensor.read_measured_data().await);
            state.lock(|data| {
                let mut raw = data.get();
                raw.mass_pm1_0 = measured.mass_pm1_0;
                raw.mass_pm2_5 = measured.mass_pm2_5;
                raw.mass_pm4_0 = measured.mass_pm4_0;
                raw.mass_pm10 = measured.mass_pm10;
                raw.number_pm0_5 = measured.number_pm0_5;
                raw.number_pm1_0 = measured.number_pm1_0;
                raw.number_pm2_5 = measured.number_pm2_5;
                raw.number_pm4_0 = measured.number_pm4_0;
                raw.number_pm10 = measured.number_pm10;
                raw.typical_particulate_matter_size = measured.typical_size;
                data.set(raw)
            });
            defmt::info!("SPS30: data: {}", measured);
        }
    }
}
