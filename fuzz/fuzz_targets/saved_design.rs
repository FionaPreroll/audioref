//! The design of a correction in the plugin's saved state, see
//! `audioref_core::fuzz::saved_design`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| audioref_core::fuzz::saved_design(data));
