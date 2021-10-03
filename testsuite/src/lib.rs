#![no_std]
#![cfg_attr(test, no_main)]

use air_force_one as _; // memory layout + panic handler

#[defmt_test::tests]
mod tests {}
