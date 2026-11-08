//! The correction a project saved, see `audioref_core::fuzz::saved_correction`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| audioref_core::fuzz::saved_correction(data));
