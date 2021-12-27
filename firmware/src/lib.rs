#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(alloc_error_handler)]
#![allow(incomplete_features)]

pub mod tasks;

pub mod scd30;

pub mod sgp40;
pub mod sps30;
pub mod vocalg;

use core::alloc::Layout;
use core::sync::atomic::{AtomicUsize, Ordering};

use alloc_cortex_m::CortexMHeap;
use embassy_hal_common::usb::usb_serial::UsbSerial;
use embassy_hal_common::usb::ClassSet1;
use embassy_nrf::usbd::UsbPeripheral;
use nrf_softdevice_defmt_rtt as _; // global logger

use nrf_usbd::Usbd;
use panic_probe as _;

use nrf_softdevice as _;

// same panicking *behavior* as `panic-probe` but doesn't print a panic message
// this prevents the panic message being printed *twice* when `defmt::panic` is invoked
#[defmt::panic_handler]
fn panic() -> ! {
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

pub type StaticUsb = Usbd<UsbPeripheral<'static>>;
pub type StaticSerialClassSet1 = ClassSet1<
    Usbd<UsbPeripheral<'static>>,
    UsbSerial<'static, 'static, Usbd<UsbPeripheral<'static>>>,
>;

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
