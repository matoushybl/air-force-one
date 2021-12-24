#![no_std]
#![feature(type_alias_impl_trait)]

pub mod tasks;

pub mod scd30;

pub mod sgp40;
pub mod sps30;
pub mod vocalg;

use core::sync::atomic::{AtomicUsize, Ordering};

use defmt_rtt as _; // global logger

use panic_probe as _;

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
