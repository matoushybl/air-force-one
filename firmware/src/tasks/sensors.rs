use embassy::time::{Duration, Timer};
use embassy_nrf::peripherals;
use embassy_nrf::twim::Twim;

use crate::app::App;
use crate::drivers::scd30::SCD30;
use crate::drivers::sgp40::Sgp40;
use crate::drivers::sps30::Sps30;

#[embassy::task]
pub async fn voc_task(mut sensor: Sgp40<'static, Twim<'static, peripherals::TWISPI0>>, app: App) {
    defmt::error!(
        "voc version: {=u64:x}",
        defmt::unwrap!(sensor.get_serial_number().await)
    );

    loop {
        let air_quality = app.air_quality();
        let (temperature, humidity) = (air_quality.temperature, air_quality.humidity);
        if humidity > 1.0 {
            if let Ok(voc) = sensor.measure_voc_index(humidity, temperature).await {
                app.update_voc(voc);
                defmt::info!("voc: {}", voc);
            } else {
                defmt::error!("Failed to read data from VOC.");
            }
        }
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy::task]
pub async fn co2_task(mut sensor: SCD30<'static, Twim<'static, peripherals::TWISPI0>>, app: App) {
    defmt::info!(
        "Scd30: {}",
        defmt::unwrap!(sensor.get_temperature_offset().await)
    );
    defmt::unwrap!(sensor.set_temperature_offset(3.8).await);
    loop {
        if let Ok(true) = sensor.get_data_ready().await {
            if let Ok(measurement) = sensor.read_measurement().await {
                app.update_co2(
                    measurement.co2,
                    measurement.temperature,
                    measurement.humidity,
                );
                defmt::info!("Scd30: co2 {}", measurement.co2);
            } else {
                defmt::error!("Scd30: measurement error.")
            }
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy::task]
pub async fn pm_task(mut sensor: Sps30<'static, Twim<'static, peripherals::TWISPI0>>, app: App) {
    let version = defmt::unwrap!(sensor.read_version().await);

    defmt::error!("SPS30: version {:x}", version);

    defmt::unwrap!(sensor.start_measurement().await);

    Timer::after(Duration::from_millis(2000)).await;

    loop {
        Timer::after(Duration::from_millis(500)).await;

        if defmt::unwrap!(sensor.is_ready().await) {
            let measured = defmt::unwrap!(sensor.read_measured_data().await);
            defmt::info!("SPS30: data: {}", measured);
            app.update_pm(measured);
        }
    }
}
