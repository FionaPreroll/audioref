//! The text of AutoEQ's name index of oratory1990's measurements, see
//! `audioref_core::fuzz::name_index`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| audioref_core::fuzz::name_index(data));
