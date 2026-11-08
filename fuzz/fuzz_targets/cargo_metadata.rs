//! The output of `cargo metadata`, which the plugin's build script reads for the about window,
//! see `audioref_core::fuzz::cargo_metadata`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| audioref_core::fuzz::cargo_metadata(data));
