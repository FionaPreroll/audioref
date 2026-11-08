//! AudioRef as a VST3 and CLAP plugin: connects the logic of `audioref-core` to the hosts through
//! nih-plug.
//!
//! This crate only translates between nih-plug and `audioref-core`; everything that can be
//! tested without a host lives there. clap-validator and pluginval check the hosts' side in CI,
//! see `docs/DEVELOPMENT.md`.

// The macros of vst3-sys, which nih_export_vst3! expands, end expressions with a semicolon:
// Rust 1.99 warns of it, a later Rust will refuse it. Goes once nih-plug's VST3 bindings are
// fixed, see Dependencies in docs/PLAN.md.
#![allow(
    unknown_lints,
    reason = "the lint below is new in Rust 1.99, older versions don't know it"
)]
#![allow(
    semicolon_in_expressions_from_non_local_macros,
    reason = "in the macros of vst3-sys, not in this crate"
)]

use std::num::NonZeroU32;
use std::sync::Arc;

use audioref_core::gain::Engine;
use audioref_core::i18n::{Lang, Param};
use audioref_core::render::{self, Processing};
use nih_plug::prelude::*;

/// The plugin's ID for CLAP hosts. Never change it: hosts find saved plugins by it.
pub const CLAP_ID: &str = "io.github.fionapreroll.audioref";

/// The plugin's ID for VST3 hosts. Never change it: hosts find saved plugins by it.
pub const VST3_CLASS_ID: [u8; 16] = [
    0xf0, 0x9d, 0xf6, 0xf7, 0x79, 0xba, 0x47, 0x82, 0x80, 0xe0, 0xe3, 0x85, 0x10, 0x26, 0xdb, 0xd7,
];

/// The lowest output level in dB.
pub const OUTPUT_MIN_DB: f32 = -24.0;
/// The highest output level in dB.
pub const OUTPUT_MAX_DB: f32 = 6.0;
// The default, 0 dB, must lie in the range.
const _: () = assert!(OUTPUT_MIN_DB < 0.0 && OUTPUT_MAX_DB > 0.0);

/// The plugin, as the hosts load it.
pub struct AudioRef {
    params: Arc<AudioRefParams>,
    engine: Engine,
    /// How the host runs the plugin, as it said when it last activated it.
    processing: Processing,
}

/// The parameters that hosts automate and save with a project. Their IDs never change; their
/// names are in the language of the operating system.
#[derive(Params)]
pub struct AudioRefParams {
    /// The output level in dB.
    #[id = "output"]
    pub output: FloatParam,
    /// Passes the signal through unchanged.
    #[id = "bypass"]
    pub bypass: BoolParam,
    /// Bypasses the plugin while the host exports offline, see `audioref_core::render`. A
    /// setting, not for automation.
    #[id = "bypass_on_export"]
    pub bypass_on_export: BoolParam,
}

impl AudioRefParams {
    /// The parameters with their names in a language.
    #[must_use]
    pub fn new(lang: Lang) -> Self {
        Self {
            output: FloatParam::new(
                Param::Output.name(lang),
                0.0,
                FloatRange::Linear {
                    min: OUTPUT_MIN_DB,
                    max: OUTPUT_MAX_DB,
                },
            )
            .with_unit(" dB")
            .with_step_size(0.1),
            bypass: BoolParam::new(Param::Bypass.name(lang), false).make_bypass(),
            bypass_on_export: BoolParam::new(Param::BypassOnExport.name(lang), true)
                .non_automatable(),
        }
    }
}

/// The language of the operating system's locale, English if it reports none.
#[must_use]
pub fn system_lang() -> Lang {
    sys_locale::get_locale().map_or(Lang::En, |locale| Lang::from_locale(&locale))
}

impl Default for AudioRef {
    fn default() -> Self {
        Self {
            params: Arc::new(AudioRefParams::new(system_lang())),
            engine: Engine::new(48_000.0),
            processing: Processing::Realtime,
        }
    }
}

impl Plugin for AudioRef {
    const NAME: &'static str = "AudioRef";
    const VENDOR: &'static str = "FionaPreroll";
    const URL: &'static str = "https://github.com/FionaPreroll/audioref";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.engine.set_sample_rate(buffer_config.sample_rate);
        // Hosts activate the plugin again whenever this changes, e.g. to export.
        self.processing = match buffer_config.process_mode {
            ProcessMode::Offline => Processing::Offline,
            ProcessMode::Realtime | ProcessMode::Buffered => Processing::Realtime,
        };
        true
    }

    fn reset(&mut self) {
        // The fades end here, so an export starts bypassed from its first sample.
        self.apply_params();
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.apply_params();
        self.engine.process(buffer.as_slice());
        ProcessStatus::Normal
    }
}

impl AudioRef {
    /// Hands the parameters to the engine, which fades to them.
    fn apply_params(&mut self) {
        self.engine.set_output_db(self.params.output.value());
        self.engine.set_bypass(render::bypassed(
            self.params.bypass.value(),
            self.params.bypass_on_export.value(),
            self.processing,
        ));
    }
}

impl ClapPlugin for AudioRef {
    const CLAP_ID: &'static str = CLAP_ID;
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Headphone correction for monitoring. Private, non-commercial use only.");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> =
        Some("https://github.com/FionaPreroll/audioref/issues");
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Equalizer,
        ClapFeature::Stereo,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for AudioRef {
    const VST3_CLASS_ID: [u8; 16] = VST3_CLASS_ID;
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Eq,
        Vst3SubCategory::Stereo,
    ];
}

nih_export_clap!(AudioRef);
nih_export_vst3!(AudioRef);

#[cfg(test)]
mod tests {
    use super::{AudioRef, AudioRefParams, system_lang};
    use audioref_core::i18n::{Lang, Param};
    use nih_plug::prelude::{Param as _, Plugin as _};

    #[test]
    fn parameters_have_names_in_both_languages() {
        for lang in Lang::ALL {
            let params = AudioRefParams::new(lang);
            assert_eq!(params.output.name(), Param::Output.name(lang));
            assert_eq!(params.bypass.name(), Param::Bypass.name(lang));
            assert_eq!(
                params.bypass_on_export.name(),
                Param::BypassOnExport.name(lang)
            );
        }
    }

    #[test]
    fn defaults_leave_the_signal_unchanged() {
        let params = AudioRefParams::new(Lang::En);
        assert!(params.output.value().abs() < f32::EPSILON);
        assert!(!params.bypass.value());
        assert!(params.bypass_on_export.value());
    }

    #[test]
    fn the_plugin_lists_its_parameters_by_their_ids() {
        let plugin = AudioRef::default();
        let ids: Vec<String> = plugin
            .params()
            .param_map()
            .into_iter()
            .map(|(id, ..)| id)
            .collect();
        assert_eq!(ids, ["output", "bypass", "bypass_on_export"]);
        // Whatever the system's locale, it is one of the languages.
        assert!(Lang::ALL.contains(&system_lang()));
    }
}
