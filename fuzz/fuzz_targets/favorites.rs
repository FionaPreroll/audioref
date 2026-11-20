//! The file of the favorite headphones and presets, see `audioref_core::fuzz::favorites`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| audioref_core::fuzz::favorites(data));
