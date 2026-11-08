//! Entry points for the fuzz targets in `fuzz/`, compiled with `--cfg fuzzing`, which cargo fuzz
//! sets, and for the tests, which run each of them on a few inputs: a smoke test that needs
//! neither nightly Rust nor cargo-fuzz. See `fuzz/README.md`.

use crate::eq::{FREQUENCY_HZ, GAIN_DB, MAX_BANDS, ParametricEq};
use crate::opra::Database;

/// The text of OPRA's database, as bytes.
///
/// Checks beyond not crashing: every preset that [`Database::parse`] accepts has at most
/// [`MAX_BANDS`] bands and its values in their ranges, the IDs aren't empty, and every line that
/// isn't empty is either an entry or counted as skipped.
///
/// # Panics
///
/// If one of these checks fails, which is what the fuzz target looks for.
pub fn opra_database(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(database) = Database::parse(text) else {
        return;
    };
    for preset in database.presets().values() {
        check_eq(&preset.eq);
    }
    let ids = database
        .vendors()
        .keys()
        .chain(database.products().keys())
        .chain(database.presets().keys());
    assert!(ids.clone().all(|id| !id.is_empty()));
    let lines = text.lines().filter(|line| !line.trim().is_empty()).count();
    assert_eq!(ids.count() + database.skipped(), lines);
}

/// Asserts that an EQ holds no value outside its range.
fn check_eq(eq: &ParametricEq) {
    assert!(GAIN_DB.contains(&eq.gain_db()));
    assert!(eq.bands().len() <= MAX_BANDS);
    for band in eq.bands() {
        assert!(band.is_valid(), "{band:?}");
        assert!(FREQUENCY_HZ.contains(&band.frequency()));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn opra_database_smoke_test() {
        let inputs: [&[u8]; 8] = [
            b"",
            b"\xff\xfe",
            b"{}",
            br#"{"type":"vendor","id":"acme","data":{"name":"Acme"}}"#,
            br#"{"type":"vendor","id":"acme","data":{"name":"Acme"}}
{"type":"vendor","id":"acme","data":{"name":"Acme"}}
not json"#,
            br#"{"type":"eq","id":"a::b::c","data":{"author":"x","type":"parametric_eq","product_id":"a::b","parameters":{"gain_db":-1,"bands":[{"type":"peak_dip","frequency":100,"gain_db":2,"q":1}]}}}"#,
            br#"{"type":"eq","id":"a::b::c","data":{"author":"x","type":"parametric_eq","product_id":"a::b","parameters":{"gain_db":-1,"bands":[{"type":"peak_dip","frequency":100,"gain_db":99,"q":1}]}}}"#,
            "\u{feff}{\"type\":\"product\",\"id\":\"a::b\",\"data\":{\"name\":\"B\",\"vendor_id\":\"a\",\"subtype\":\"in_ear\"}}\r\n".as_bytes(),
        ];
        for input in inputs {
            super::opra_database(input);
        }
    }
}
