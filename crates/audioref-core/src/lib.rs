//! The logic of AudioRef, headphone correction for monitoring: the parsers of headphone data, the
//! signal processing and the texts it shows, in English and German.
//!
//! The crate knows nothing of plugin APIs, GUIs or files, so its tests need no host. The plugin,
//! `crates/audioref`, connects it to the hosts through nih-plug. The dependencies of this crate are
//! only crates without plugin APIs, GUI or I/O; `make lint` checks that.

pub mod autoeq;
pub mod biquad;
pub mod bundled;
pub mod cascade;
pub mod design;
pub mod engine;
pub mod eq;
pub mod fir;
#[cfg(any(test, fuzzing))]
pub mod fuzz;
pub mod gain;
pub mod i18n;
pub mod opra;
pub mod render;
pub mod state;
