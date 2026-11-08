//! The presets built into AudioRef: an excerpt of OPRA's database with the presets of the Bose
//! QC25, the Sony WH-1000XM4 and the AKG K240 Studio, in all their modes, so these correct without
//! a download. See `data/opra/README.md` for the source, the authors and how to update it.
//!
//! The file is compiled into the plugin; nothing is read at run time.
//!
//! ```
//! let database = audioref_core::bundled::database();
//! assert!(database.search("WH-1000XM4", 10).contains(&"sony::wh_1000xm4"));
//! ```
//!
//! Like all of OPRA's data, the excerpt is licensed under CC BY-SA 4.0, not under AudioRef's
//! GPLv3: wherever a preset is used, its author is named.

use crate::opra::Database;

/// The excerpt of OPRA's `database_v1.jsonl`, as text.
pub const PRESETS: &str = include_str!("../../../data/opra/presets.jsonl");

/// The built-in presets, parsed. Empty if the excerpt couldn't be parsed, which the tests rule out.
#[must_use]
pub fn database() -> Database {
    Database::parse(PRESETS).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{PRESETS, database};
    use crate::opra::Database;

    /// OPRA's IDs of the built-in headphones, as `utils/bundle-opra-presets.py` picks them.
    const PRODUCTS: [&str; 7] = [
        "akg::k240_studio",
        "bose::qc25",
        "bose::quietcomfort_25",
        "bose::quietcomfort_25_passive",
        "sony::wh_1000xm4",
        "sony::wh_1000xm4_anc_off",
        "sony::wh_1000xm4_anc_on",
    ];

    #[test]
    fn every_line_is_understood() {
        let parsed = Database::parse(PRESETS).expect("OPRA's format");
        assert_eq!(parsed.skipped(), 0);
        assert_eq!(parsed, database());
        assert_eq!(parsed.presets().len(), PRESETS.lines().count() - 10);
    }

    #[test]
    fn holds_the_three_headphones_in_all_their_modes() {
        let database = database();
        assert_eq!(database.products().keys().collect::<Vec<_>>(), PRODUCTS);
        assert_eq!(database.vendors().len(), 3);
        for product in PRODUCTS {
            assert!(
                database.presets_of(product).next().is_some(),
                "no preset for {product}"
            );
        }
        assert_eq!(database.search("", 100).len(), PRODUCTS.len());
        assert_eq!(database.headphones("akg::k240_studio"), "AKG K240 Studio");
    }

    #[test]
    fn names_every_author() {
        let database = database();
        for (id, preset) in database.presets() {
            assert!(!preset.author.is_empty(), "{id}");
        }
        let oratory = database
            .presets()
            .values()
            .filter(|preset| preset.author == "oratory1990")
            .count();
        assert!(oratory >= 3, "{oratory} presets by oratory1990");
    }
}
