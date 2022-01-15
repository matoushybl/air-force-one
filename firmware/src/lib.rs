#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(alloc_error_handler)]
#![allow(incomplete_features)]
#![feature(cell_update)]

pub mod tasks;

pub mod scd30;

pub mod board;
pub mod sensirion_i2c;
pub mod sgp40;
pub mod sps30;
pub mod vocalg;

use core::alloc::Layout;
#[allow(unused)]
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicUsize, Ordering};

use alloc_cortex_m::CortexMHeap;
use embassy_hal_common::usb::usb_serial::UsbSerial;
use embassy_hal_common::usb::ClassSet1;
use embassy_nrf::interrupt;
use embassy_nrf::peripherals::USBD;
use embassy_nrf::usb::UsbBus;
use nrf_softdevice::raw;
use nrf_softdevice_defmt_rtt as _; // global logger

use nrf_usbd::Usbd;

#[cfg(not(feature = "persist-panic"))]
use panic_probe as _;

use nrf_softdevice as _;

// same panicking *behavior* as `panic-probe` but doesn't print a panic message
// this prevents the panic message being printed *twice* when `defmt::panic` is invoked
#[defmt::panic_handler]
fn panic_defmt() -> ! {
    cortex_m::asm::udf()
}

static COUNT: AtomicUsize = AtomicUsize::new(0);
defmt::timestamp!("{=usize}", {
    // NOTE(no-CAS) `timestamps` runs with interrupts disabled
    let n = COUNT.load(Ordering::Relaxed);
    COUNT.store(n + 1, Ordering::Relaxed);
    n
});

// this is the allocator the application will use
#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

// define what happens in an Out Of Memory (OOM) condition
#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    panic!("Alloc error");
}

pub type StaticUsb = Usbd<UsbBus<'static, USBD>>;
pub type StaticSerialClassSet1 = ClassSet1<StaticUsb, UsbSerial<'static, 'static, StaticUsb>>;
pub type Usb =
    embassy_hal_common::usb::Usb<'static, StaticUsb, StaticSerialClassSet1, interrupt::USBD>;

pub enum ButtonEvent {
    Esc,
    Ok,
    Next,
    Prev,
}

#[derive(Clone, Copy)]
pub enum Page {
    Basic,
    Pm,
    Voc,
}

pub fn softdevice_config() -> nrf_softdevice::Config {
    nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 4,
            rc_temp_ctiv: 2,
            accuracy: 7,
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

#[cfg(feature = "persist-panic")]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    panic_persist::report_panic_info(info);

    cortex_m::asm::udf()
}

pub fn embassy_config() -> embassy_nrf::config::Config {
    let mut config = embassy_nrf::config::Config::default();
    config.hfclk_source = embassy_nrf::config::HfclkSource::Internal;
    config.lfclk_source = embassy_nrf::config::LfclkSource::InternalRC;
    config.time_interrupt_priority = interrupt::Priority::P2;
    // if we see button misses lower this
    config.gpiote_interrupt_priority = interrupt::Priority::P7;
    config
}

/// Reinitializes reset pin in the hardware.
///
/// # Examples
///
/// ```
/// use air_force_one::reinitialize_reset;
///
/// unsafe { reinitialize_reset() };
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
