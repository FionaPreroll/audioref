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

pub mod background;
pub mod editor;

use std::num::NonZeroU32;
use std::sync::{Arc, RwLock};

use audioref_core::engine::Engine;
use audioref_core::i18n::{Lang, Param};
use audioref_core::render::{self, Processing};
use audioref_core::state::{SavedCorrection, SavedDesign};
use audioref_core::stereo::{Channels, Crossfeed};
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;

use crate::background::{AudioEnds, Shared, Worker};

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
    /// The sample rate the host last activated the plugin with, in Hz.
    sample_rate: f64,
    /// What the background and the window share with the plugin.
    shared: Arc<Shared>,
    /// The audio thread's ends of the queues of corrections.
    queues: AudioEnds,
    /// The background thread, joined when the plugin is dropped.
    _worker: Worker,
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
    /// The routing of the channels: stereo, mono or swapped, see `audioref_core::stereo`.
    #[id = "channels"]
    pub channels: IntParam,
    /// The strength of the crossfeed, off by default.
    #[id = "crossfeed"]
    pub crossfeed: IntParam,
    /// The limiter at the end, on by default, see `audioref_core::limiter`.
    #[id = "limiter"]
    pub limiter: BoolParam,
    /// The chosen preset with its bands, so the project opens with it without OPRA's file. Its
    /// values are checked again before it is used, see `audioref_core::state`.
    #[persist = "correction"]
    pub correction: RwLock<Option<SavedCorrection>>,
    /// The path of OPRA's file, to read it again when the window opens.
    #[persist = "opra-path"]
    pub opra_path: RwLock<String>,
    /// The measurement and the target a correction is designed from, with its settings. If there
    /// is one, it is the correction, not the preset; its values are checked again before it is
    /// used, see `audioref_core::state`.
    #[persist = "design"]
    pub design: RwLock<Option<SavedDesign>>,
    /// The paths of the files of the measurement and the target, for the window's fields.
    #[persist = "measurement-path"]
    pub measurement_path: RwLock<String>,
    #[persist = "target-path"]
    pub target_path: RwLock<String>,
    /// The size of the window.
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
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
            channels: IntParam::new(
                Param::Channels.name(lang),
                Channels::default().index(),
                IntRange::Linear {
                    min: 0,
                    max: Channels::Swapped.index(),
                },
            )
            .with_value_to_string(Arc::new(move |index| {
                Channels::from_index(index).text(lang).to_owned()
            }))
            .with_string_to_value(Arc::new(|text| {
                Channels::from_text(text).map(Channels::index)
            })),
            crossfeed: IntParam::new(
                Param::Crossfeed.name(lang),
                Crossfeed::default().index(),
                IntRange::Linear {
                    min: 0,
                    max: Crossfeed::Strong.index(),
                },
            )
            .with_value_to_string(Arc::new(move |index| {
                Crossfeed::from_index(index).text(lang).to_owned()
            }))
            .with_string_to_value(Arc::new(|text| {
                Crossfeed::from_text(text).map(Crossfeed::index)
            })),
            limiter: BoolParam::new(Param::Limiter.name(lang), true),
            correction: RwLock::new(None),
            opra_path: RwLock::new(String::new()),
            design: RwLock::new(None),
            measurement_path: RwLock::new(String::new()),
            target_path: RwLock::new(String::new()),
            editor_state: editor::default_state(),
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
        let lang = system_lang();
        let (shared, queues) = Shared::new(lang);
        let params = Arc::new(AudioRefParams::new(lang));
        let worker = Worker::start(shared.clone(), params.clone());
        Self {
            params,
            engine: Engine::new(48_000.0),
            processing: Processing::Realtime,
            sample_rate: 48_000.0,
            shared,
            queues,
            _worker: worker,
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
    // AudioRef runs its own background, see `background`.
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        // Keeping the executor would keep the plugin alive, see `background`.
        editor::create(self.params.clone(), self.shared.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.engine.set_sample_rate(buffer_config.sample_rate);
        self.sample_rate = f64::from(buffer_config.sample_rate);
        self.shared.set_sample_rate(buffer_config.sample_rate);
        // Outside the audio thread, and after loading a project too: corrections that were on
        // their way are for the sample rate or the project before. The one saved is designed
        // here, so the first block has it.
        while self.queues.incoming.pop().is_ok() {}
        drop(
            self.engine
                .set_correction(background::design(&self.params, buffer_config.sample_rate)),
        );
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
        self.receive_corrections();
        self.apply_params();
        self.engine.process(buffer.as_slice());
        self.shared.note_reduction(self.engine.take_reduction_db());
        ProcessStatus::Normal
    }
}

impl AudioRef {
    /// Hands the corrections from the background to the engine, and those it no longer needs
    /// back. Takes one only while there is room to send one back, so nothing is freed here.
    fn receive_corrections(&mut self) {
        while self.queues.returns.slots() > 0 {
            let Ok(correction) = self.queues.incoming.pop() else {
                break;
            };
            let unused = match correction {
                // Designed for another sample rate, before the host activated the plugin again.
                Some(filter) if filter.sample_rate().to_bits() != self.sample_rate.to_bits() => {
                    Some(filter)
                }
                correction => self.engine.set_correction(correction),
            };
            if let Some(filter) = unused {
                // There is room, see above, so this hands nothing back.
                drop(self.queues.returns.push(filter));
            }
        }
    }

    /// Hands the parameters to the engine, which fades to them.
    fn apply_params(&mut self) {
        self.engine
            .set_channels(Channels::from_index(self.params.channels.value()));
        self.engine
            .set_crossfeed(Crossfeed::from_index(self.params.crossfeed.value()));
        self.engine.set_output_db(self.params.output.value());
        self.engine.set_limiter(self.params.limiter.value());
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
    use audioref_core::stereo::{Channels, Crossfeed};
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
            assert_eq!(params.channels.name(), Param::Channels.name(lang));
            assert_eq!(params.crossfeed.name(), Param::Crossfeed.name(lang));
            assert_eq!(params.limiter.name(), Param::Limiter.name(lang));
            // Hosts show the choices by name, and take them back typed in.
            assert_eq!(params.channels.to_string(), Channels::Stereo.text(lang));
            assert_eq!(params.crossfeed.to_string(), Crossfeed::Off.text(lang));
            assert_eq!(
                params
                    .crossfeed
                    .string_to_normalized_value(Crossfeed::Strong.text(lang)),
                Some(1.0)
            );
        }
    }

    #[test]
    fn defaults_leave_the_signal_unchanged() {
        let params = AudioRefParams::new(Lang::En);
        assert!(params.output.value().abs() < f32::EPSILON);
        assert!(!params.bypass.value());
        assert!(params.bypass_on_export.value());
        assert_eq!(params.channels.value(), Channels::Stereo.index());
        assert_eq!(params.crossfeed.value(), Crossfeed::Off.index());
        assert!(params.limiter.value());
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
        assert_eq!(
            ids,
            [
                "output",
                "bypass",
                "bypass_on_export",
                "channels",
                "crossfeed",
                "limiter"
            ]
        );
        // Whatever the system's locale, it is one of the languages.
        assert!(Lang::ALL.contains(&system_lang()));
    }
}
