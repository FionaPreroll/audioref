//! The render guard: with "Bypass on export" on, the default, AudioRef bypasses itself while the
//! host exports offline, so the correction meant for the headphones doesn't end up in a mix.
//!
//! Hosts that export in real time don't tell the plugin, which is why the window also shows the
//! notice [`Notice::MonitoringOnly`](crate::i18n::Notice::MonitoringOnly). In VST3 the plugin
//! can't tell either, see [`Format::knows_offline`].
//!
//! ```
//! use audioref_core::render::{Format, Processing, bypassed, processing};
//!
//! assert!(bypassed(false, true, processing(true, Format::Clap)));
//! assert!(!bypassed(false, true, processing(false, Format::Clap)));
//! assert!(!bypassed(false, true, processing(true, Format::Vst3)));
//! ```

/// How the host runs the plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Processing {
    /// In real time, as while playing, or ahead of time at the pace of real time.
    Realtime,
    /// Offline, as fast as it can, as while exporting.
    Offline,
}

/// The format the host loaded AudioRef in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// CLAP.
    Clap,
    /// VST3.
    Vst3,
    /// On its own, without a host.
    Standalone,
}

impl Format {
    /// Whether AudioRef learns in this format that the host processes offline. Not in VST3:
    /// nih-plug's VST3 wrapper hands the plugin the mode of the host's call before the last one,
    /// so after the host set up offline processing once, AudioRef would take playing for an
    /// export and bypass itself.
    #[must_use]
    pub fn knows_offline(self) -> bool {
        self != Self::Vst3
    }
}

/// How the host runs the plugin, from what it reported and the format it loaded AudioRef in.
#[must_use]
pub fn processing(reported_offline: bool, format: Format) -> Processing {
    if reported_offline && format.knows_offline() {
        Processing::Offline
    } else {
        Processing::Realtime
    }
}

/// Whether AudioRef bypasses itself: when the user asks, with `bypass`, or while the host exports
/// offline with `bypass_on_export` on.
#[must_use]
pub fn bypassed(bypass: bool, bypass_on_export: bool, processing: Processing) -> bool {
    bypass || (bypass_on_export && processing == Processing::Offline)
}

#[cfg(test)]
mod tests {
    use super::{Format, Processing, bypassed, processing};

    #[test]
    fn bypasses_offline_only_with_the_guard_on() {
        assert!(bypassed(false, true, Processing::Offline));
        assert!(!bypassed(false, false, Processing::Offline));
        assert!(!bypassed(false, true, Processing::Realtime));
        assert!(!bypassed(false, false, Processing::Realtime));
    }

    #[test]
    fn trusts_the_reported_mode_except_in_vst3() {
        for format in [Format::Clap, Format::Standalone] {
            assert!(format.knows_offline());
            assert_eq!(processing(true, format), Processing::Offline);
            assert_eq!(processing(false, format), Processing::Realtime);
        }
        assert!(!Format::Vst3.knows_offline());
        assert_eq!(processing(true, Format::Vst3), Processing::Realtime);
        assert_eq!(processing(false, Format::Vst3), Processing::Realtime);
    }

    #[test]
    fn the_users_bypass_always_wins() {
        for guard in [false, true] {
            for processing in [Processing::Realtime, Processing::Offline] {
                assert!(bypassed(true, guard, processing));
            }
        }
    }
}
