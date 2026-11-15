//! Fuzzes the STEP reader, `partsmith_occt_sys::read_step` (delivery.md section 2.15), with an
//! interrupt that is never requested.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Every Ok and every Err passes. A crash, a memory error, a timeout or the memory limit fails
    // the input.
    let _ = partsmith_occt_sys::read_step(data, &partsmith_occt_sys::Interrupt::new());
});
