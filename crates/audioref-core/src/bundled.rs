//! The presets built into AudioRef: OPRA's whole database, `data/opra/database_v1.jsonl`, so all
//! of its headphones correct without a download. See `data/opra/README.md` for the source, the
//! authors and how to update it.
//!
//! The file is about 12.6 MB of text. The plugin's build script packs it with [`pack`], to about
//! 1.2 MB, and the plugin keeps it packed until [`database`] unpacks and parses it, once, in the
//! background; nothing is read from a file at run time.
//!
//! ```
//! use audioref_core::bundled::{database, pack};
//!
//! let text = r#"{"type":"vendor","id":"akg","data":{"name":"AKG"}}"#;
//! assert_eq!(database(&pack(text)).vendors().len(), 1);
//! ```
//!
//! Like all of OPRA's data, the database is licensed under CC BY-SA 4.0, not under AudioRef's
//! GPLv3: wherever a preset is used, its author is named.

use crate::opra::Database;

/// OPRA's `database_v1.jsonl`, as text. Only the plugin's build script and the tests use it; the
/// plugin holds it packed, see [`pack`].
pub const DATABASE: &str = include_str!("../../../data/opra/database_v1.jsonl");

/// The most bytes [`database`] unpacks, far more than OPRA's database, so packed data that grows
/// without end, which the build never makes, can't take all memory.
pub const MAX_UNPACKED: usize = 256 << 20;

/// How hard [`pack`] compresses: level 9 of 10 gains only 2 % more and takes twice as long.
const LEVEL: u8 = 6;

/// Packs the text of OPRA's database with deflate, for the plugin to hold.
#[must_use]
pub fn pack(text: &str) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec(text.as_bytes(), LEVEL)
}

/// Unpacks what [`pack`] packed; `None` if it isn't deflate, unpacks to more than
/// [`MAX_UNPACKED`] bytes or isn't UTF-8.
#[must_use]
pub fn unpack(packed: &[u8]) -> Option<String> {
    unpack_at_most(packed, MAX_UNPACKED)
}

/// Unpacks what [`pack`] packed, if it is at most so many bytes of UTF-8.
fn unpack_at_most(packed: &[u8], limit: usize) -> Option<String> {
    let bytes = miniz_oxide::inflate::decompress_to_vec_with_limit(packed, limit).ok()?;
    String::from_utf8(bytes).ok()
}

/// The built-in presets, unpacked and parsed. Empty if they couldn't be, which the tests rule out
/// for the database in the repository.
#[must_use]
pub fn database(packed: &[u8]) -> Database {
    unpack(packed)
        .and_then(|text| Database::parse(&text).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{DATABASE, database, pack, unpack, unpack_at_most};
    use crate::opra::{Database, Form};

    /// The headphones AudioRef was first tried with, in all their modes in OPRA.
    const TRIED: [&str; 7] = [
        "akg::k240_studio",
        "bose::qc25",
        "bose::quietcomfort_25",
        "bose::quietcomfort_25_passive",
        "sony::wh_1000xm4",
        "sony::wh_1000xm4_anc_off",
        "sony::wh_1000xm4_anc_on",
    ];

    /// How many lines of each kind OPRA's database has.
    fn count(kind: &str) -> usize {
        let start = format!(r#"{{"type":"{kind}","#);
        DATABASE
            .lines()
            .filter(|line| line.starts_with(&start))
            .count()
    }

    #[test]
    fn every_line_is_understood() {
        let built_in = database(&pack(DATABASE));
        assert_eq!(built_in.skipped(), 0);
        assert_eq!(built_in.vendors().len(), count("vendor"));
        assert_eq!(built_in.products().len(), count("product"));
        assert_eq!(built_in.presets().len(), count("eq"));
        assert_eq!(
            count("vendor") + count("product") + count("eq"),
            DATABASE.lines().count()
        );
        assert_eq!(Database::parse(DATABASE), Ok(built_in));
    }

    #[test]
    fn packs_to_a_tenth() {
        let packed = pack(DATABASE);
        assert!(packed.len() < DATABASE.len() / 8, "{} bytes", packed.len());
        assert_eq!(unpack(&packed).as_deref(), Some(DATABASE));
    }

    #[test]
    fn holds_the_whole_database() {
        let built_in = database(&pack(DATABASE));
        assert!(built_in.presets().len() > 10_000);
        assert!(built_in.products().len() > 5_000);
        for product in TRIED {
            assert!(
                built_in.presets_of(product).next().is_some(),
                "no preset for {product}"
            );
        }
        assert_eq!(built_in.headphones("akg::k240_studio"), "AKG K240 Studio");
        assert!(
            built_in
                .products()
                .values()
                .any(|product| product.form == Form::InEar)
        );
    }

    #[test]
    fn names_every_author() {
        let built_in = database(&pack(DATABASE));
        for (id, preset) in built_in.presets() {
            assert!(!preset.author.is_empty(), "{id}");
        }
        let oratory = built_in
            .presets()
            .values()
            .filter(|preset| preset.author == "oratory1990")
            .count();
        assert!(oratory >= 1_000, "{oratory} presets by oratory1990");
    }

    #[test]
    fn refuses_what_it_did_not_pack() {
        assert_eq!(unpack(b"not deflate"), None);
        assert_eq!(database(b"not deflate"), Database::default());
        // Deflate, but not UTF-8.
        let latin1 = miniz_oxide::deflate::compress_to_vec(&[0xe4, 0xf6, 0xfc], 6);
        assert_eq!(unpack(&latin1), None);
        // UTF-8, but not OPRA's.
        assert_eq!(
            database(&pack("frequency,raw\n20,0\n")),
            Database::default()
        );
        // More than the limit.
        let packed = pack("0123456789");
        assert_eq!(unpack_at_most(&packed, 9), None);
        assert_eq!(unpack_at_most(&packed, 10).as_deref(), Some("0123456789"));
    }
}
