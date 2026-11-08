//! The text of a measurement or target in AutoEQ's CSV format, see
//! `audioref_core::fuzz::autoeq_curve`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| audioref_core::fuzz::autoeq_curve(data));
