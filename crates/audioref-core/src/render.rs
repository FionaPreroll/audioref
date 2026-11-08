//! The render guard: with "Bypass on export" on, the default, AudioRef bypasses itself while the
//! host exports offline, so the correction meant for the headphones doesn't end up in a mix.
//!
//! Hosts that export in real time don't tell the plugin, which is why the window also shows the
//! notice [`Notice::MonitoringOnly`](crate::i18n::Notice::MonitoringOnly).
//!
//! ```
//! use audioref_core::render::{Processing, bypassed};
//!
//! assert!(bypassed(false, true, Processing::Offline));
//! assert!(!bypassed(false, true, Processing::Realtime));
//! ```

/// How the host runs the plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Processing {
    /// In real time, as while playing, or ahead of time at the pace of real time.
    Realtime,
    /// Offline, as fast as it can, as while exporting.
    Offline,
}

/// Whether AudioRef bypasses itself: when the user asks, with `bypass`, or while the host exports
/// offline with `bypass_on_export` on.
#[must_use]
pub fn bypassed(bypass: bool, bypass_on_export: bool, processing: Processing) -> bool {
    bypass || (bypass_on_export && processing == Processing::Offline)
}

#[cfg(test)]
mod tests {
    use super::{Processing, bypassed};

    #[test]
    fn bypasses_offline_only_with_the_guard_on() {
        assert!(bypassed(false, true, Processing::Offline));
        assert!(!bypassed(false, false, Processing::Offline));
        assert!(!bypassed(false, true, Processing::Realtime));
        assert!(!bypassed(false, false, Processing::Realtime));
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
