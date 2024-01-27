#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use core::cell::RefCell;
use defmt_rtt as _;
use static_cell::make_static;

use embassy_executor::Spawner;
use embassy_nrf::gpio::{AnyPin, Input, Output, Pin, Pull};
use embassy_nrf::twim::{self, Twim};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::{bind_interrupts, peripherals};
use embassy_sync::blocking_mutex::ThreadModeMutex;
use embassy_time::{with_timeout, Duration, Timer};

use nrf_softdevice::ble::peripheral;
use nrf_softdevice::Softdevice;

use sensirion_async::scd4x::{Celsius, Meter, Scd4x};
use shared::{fill_adv_data, AirQuality, AirQualityAdvertisement, Co2, Humidity, Temperature};

#[cfg(feature = "dev")]
use panic_probe as _;

#[cfg(not(feature = "dev"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    defmt::error!("panic!");
    cortex_m::peripheral::SCB::sys_reset();
}

#[cortex_m_rt::exception]
unsafe fn HardFault(_frame: &cortex_m_rt::ExceptionFrame) -> ! {
    cortex_m::peripheral::SCB::sys_reset()
}

bind_interrupts!(struct Irqs {
    SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0 => twim::InterruptHandler<peripherals::TWISPI0>;
});

#[derive(Clone, Copy, Default)]
struct State {
    measurement: AirQuality,
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_nrf::config::Config::default();
    config.hfclk_source = embassy_nrf::config::HfclkSource::Internal;
    config.lfclk_source = embassy_nrf::config::LfclkSource::InternalRC;
    config.time_interrupt_priority = interrupt::Priority::P2;
    config.gpiote_interrupt_priority = interrupt::Priority::P7;

    let p = embassy_nrf::init(config);

    unsafe { reinitialize_reset() };

    // set priority to avoid collisions with softdevice
    interrupt::SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0.set_priority(interrupt::Priority::P2);

    let sd = Softdevice::enable(&softdevice_config());
    spawner.spawn(softdevice_task(sd)).unwrap();

    let led = Output::new(
        p.P1_15.degrade(),
        embassy_nrf::gpio::Level::Low,
        embassy_nrf::gpio::OutputDrive::Standard,
    );
    spawner.spawn(blinky(led)).unwrap();

    let id_pin = Input::new(p.P0_28, Pull::Up);
    // First read was 0, lets wait a bit and read again
    Timer::after(Duration::from_millis(100)).await;

    let state = make_static!(ThreadModeMutex::new(RefCell::new(State::default())));

    let device_id = if id_pin.is_low() { 0 } else { 1 };
    spawner
        .spawn(advertising_task(device_id, state, sd))
        .unwrap();

    let twi = Twim::new(p.TWISPI0, Irqs, p.P0_12, p.P0_13, Default::default());
    let scd40 = Scd4x::new(twi);
    spawner.spawn(scd4x_task(scd40, state)).unwrap();

    defmt::info!("Starting with device id: {}", device_id);
}

/// Blink the LED for a very short time, to avoid the blinking being distracting at night
#[embassy_executor::task]
async fn blinky(mut led: Output<'static, AnyPin>) {
    loop {
        led.set_high();
        Timer::after_millis(50).await;
        led.set_low();
        Timer::after_secs(60).await;
    }
}

/// Updates the advertisement every second
#[embassy_executor::task]
async fn advertising_task(
    device_id: u8,
    state: &'static ThreadModeMutex<RefCell<State>>,
    softdevice: &'static Softdevice,
) {
    loop {
        let config = peripheral::Config::default();

        let mut adv_data = [0u8; 31];
        let adv_len = build_adv_data(
            device_id,
            &state.lock(|c| c.borrow().measurement),
            &mut adv_data,
        );

        let adv = peripheral::NonconnectableAdvertisement::ScannableUndirected {
            adv_data: &adv_data[..adv_len],
            scan_data: &[],
        };

        match with_timeout(
            Duration::from_secs(1),
            peripheral::advertise(softdevice, adv, &config),
        )
        .await
        {
            Ok(Err(e)) => defmt::error!("advertisement error: {}", e),
            _ => {}
        }
    }
}

/// Configures the CO2 sensor and reads the data from it every 6 seconds
#[embassy_executor::task]
async fn scd4x_task(mut sensor: Scd4x<Twim<'static, peripherals::TWISPI0>>, state: &'static ThreadModeMutex<RefCell<State>>) {
    const ALTITUDE: Meter = Meter(230);
    const TEMPERATURE_OFFSET: Celsius = Celsius(2.5);

    defmt::unwrap!(sensor.stop_periodic_measurement().await);
    Timer::after(Duration::from_millis(500)).await;

    let serial_number = defmt::unwrap!(sensor.read_serial_number().await);
    defmt::warn!("SCD4x serial number: {:x}", serial_number);

    let configured_altitude = defmt::unwrap!(sensor.get_sensor_altitude().await);
    if configured_altitude != ALTITUDE {
        defmt::unwrap!(sensor.set_sensor_altitude(ALTITUDE).await);
    }

    let configured_offset = defmt::unwrap!(sensor.get_temperature_offset().await);
    if configured_offset != TEMPERATURE_OFFSET {
        defmt::unwrap!(sensor.set_temperature_offset(TEMPERATURE_OFFSET).await);
    }

    defmt::unwrap!(sensor.start_periodic_measurement().await);
    Timer::after(Duration::from_millis(500)).await;

    loop {
        if defmt::unwrap!(sensor.data_ready().await) {
            let measurement = sensor.read().await;
            match measurement {
                Ok(measurement) => {
                    state.lock(|c| {
                        let mut state = c.borrow_mut();
                        state.measurement.co2 = Co2(measurement.co2 as f32);
                        state.measurement.humidity = Humidity(measurement.humidity);
                        state.measurement.temperature = Temperature(measurement.temperature);
                    });
                    defmt::info!(
                        "CO2: {}, Temperature: {}, Humidity: {}",
                        measurement.co2,
                        measurement.temperature,
                        measurement.humidity
                    );
                }
                Err(err) => {
                    defmt::error!("Error accessing Scd4x: {}", err);
                }
            }
        }

        Timer::after(Duration::from_secs(6)).await;
    }
}

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await
}

/// Basic configuration for the softdevice
fn softdevice_config() -> nrf_softdevice::Config {
    use nrf_softdevice::raw;
    nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_250_PPM as u8,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 6,
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: 32768,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
            central_role_count: 3,
            central_sec_count: 0,
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: b"AirForceOneV1" as *const u8 as _,
            current_len: 13,
            max_len: 13,
            write_perm: unsafe { core::mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    }
}

/// Encode measurement and device id into advertisement data
/// The data is encoded into the Manufacturer Specific Data in the advertisement
/// This method also encodes other BLE specific data in the advertisement - such as the device name
fn build_adv_data(device_id: u8, air_quality: &AirQuality, adv_data: &mut [u8; 31]) -> usize {
    let mut adv_offset = 0;

    adv_offset += fill_adv_data(
        &mut adv_data[..],
        0x01,
        &[nrf_softdevice::raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8],
    );
    adv_offset += fill_adv_data(&mut adv_data[adv_offset..], 0x09, &[b'A', b'F', b'O']);

    let mut buffer = [0u8; 31];
    buffer[0] = 0xff;
    buffer[1] = 0xff;
    let data = AirQualityAdvertisement::from((device_id, *air_quality));

    let serialized_len = postcard::to_slice(&data, &mut buffer[2..]).unwrap().len();

    adv_offset += fill_adv_data(
        &mut adv_data[adv_offset..],
        0xff,
        &buffer[..2 + serialized_len],
    );
    adv_offset
}

/// Reinitializes reset pin in the hardware.
///
/// ```
///
/// # Safety
/// We are directly accessing registers using raw pointers which is unsafe.
/// .
pub unsafe fn reinitialize_reset() {
    let nvmc = &*embassy_nrf::pac::NVMC::ptr();
    if *(0x10001200 as *mut u32) != 18 || *(0x10001204 as *mut u32) != 18 {
        nvmc.config.write(|w| w.wen().wen());
        while nvmc.ready.read().ready().is_busy() {}
        core::ptr::write_volatile(0x10001200 as *mut u32, 18);
        while nvmc.ready.read().ready().is_busy() {}
        core::ptr::write_volatile(0x10001204 as *mut u32, 18);
        while nvmc.ready.read().ready().is_busy() {}
        nvmc.config.reset();
        while nvmc.ready.read().ready().is_busy() {}
        cortex_m::peripheral::SCB::sys_reset();
    }
}
