//! The text of OPRA's database, see `audioref_core::fuzz::opra_database`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| audioref_core::fuzz::opra_database(data));
