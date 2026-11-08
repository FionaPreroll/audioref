//! Entry points for the fuzz targets in `fuzz/`, compiled with `--cfg fuzzing`, which cargo fuzz
//! sets, and for the tests, which run each of them on a few inputs: a smoke test that needs
//! neither nightly Rust nor cargo-fuzz. See `fuzz/README.md`.

use crate::cascade::{Cascade, grid};
use crate::eq::{FREQUENCY_HZ, GAIN_DB, MAX_BANDS, ParametricEq};
use crate::opra::Database;
use crate::state::SavedCorrection;

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

/// The correction a project saved, as the JSON that nih-plug stores in the plugin's state.
///
/// Checks beyond not crashing: the EQ of a correction that is read and used has its values in
/// their ranges, and its cascade at 44.1 kHz has a finite response that boosts nothing on the grid
/// where the headroom is measured.
///
/// # Panics
///
/// If one of these checks fails, which is what the fuzz target looks for.
pub fn saved_correction(data: &[u8]) {
    let Ok(saved) = serde_json::from_slice::<SavedCorrection>(data) else {
        return;
    };
    let Some(eq) = saved.eq() else {
        return;
    };
    check_eq(&eq);
    let cascade = Cascade::new(&eq, 44_100.0);
    for frequency in grid().filter(|frequency| *frequency < 22_050.0) {
        let db = cascade.response_db(frequency);
        assert!(db.is_finite() && db < 0.01, "{db} dB at {frequency} Hz");
    }
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

    #[test]
    fn saved_correction_smoke_test() {
        let inputs: [&[u8]; 5] = [
            b"",
            b"null",
            br#"{"preset_id":"a","headphones":"A","author":"x","details":"","gain_db":-1,"bands":[]}"#,
            br#"{"preset_id":"a","headphones":"A","author":"x","details":"","gain_db":0,"bands":[{"type":"peak","frequency":1000,"gain_db":12,"q":0.5},{"type":"low_pass","frequency":8000,"slope":"db36"}]}"#,
            br#"{"preset_id":"a","headphones":"A","author":"x","details":"","gain_db":0,"bands":[{"type":"peak","frequency":1000,"gain_db":31,"q":1}]}"#,
        ];
        for input in inputs {
            super::saved_correction(input);
        }
    }
}
