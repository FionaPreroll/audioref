//! Entry points for the fuzz targets in `fuzz/`, compiled with `--cfg fuzzing`, which cargo fuzz
//! sets, and for the tests, which run each of them on a few inputs: a smoke test that needs
//! neither nightly Rust nor cargo-fuzz. See `fuzz/README.md`.

use crate::about::{code, packages};
use crate::autoeq::{Curve, FREQUENCY_HZ as CURVE_HZ, LEVEL_DB, MAX_POINTS, REQUIRED_HZ};
use crate::cascade::{Cascade, grid};
use crate::design::{MAX_BOOST_DB, MAX_CUT_DB};
use crate::eq::{FREQUENCY_HZ, GAIN_DB, MAX_BANDS, ParametricEq};
use crate::fir;
use crate::opra::Database;
use crate::profiles::{FORMS, MAX_LINES, NameIndex};
use crate::state::{SavedCorrection, SavedDesign};

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

/// The text of a measurement or target in AutoEQ's CSV format, as bytes.
///
/// Checks beyond not crashing: a curve that [`Curve::parse`] accepts has from 2 to [`MAX_POINTS`]
/// points, with rising frequencies and levels in their ranges, reaching across [`REQUIRED_HZ`];
/// its points and the lines it left out are no more than the lines of the text; and its level on
/// the grid is finite and between its lowest and its highest point.
///
/// # Panics
///
/// If one of these checks fails, which is what the fuzz target looks for.
pub fn autoeq_curve(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(curve) = Curve::parse(text) else {
        return;
    };
    let points = curve.points();
    assert!((2..=MAX_POINTS).contains(&points.len()));
    assert!(
        points
            .iter()
            .zip(points.iter().skip(1))
            .all(|((a, _), (b, _))| a < b)
    );
    assert!(
        points
            .iter()
            .all(|(frequency, level)| CURVE_HZ.contains(frequency) && LEVEL_DB.contains(level))
    );
    assert!(
        points
            .first()
            .is_some_and(|(from, _)| from <= REQUIRED_HZ.start())
    );
    assert!(points.last().is_some_and(|(to, _)| to >= REQUIRED_HZ.end()));
    assert!(points.len() + curve.skipped() <= text.lines().count());
    let lowest = points
        .iter()
        .map(|(_, level)| *level)
        .fold(f64::INFINITY, f64::min);
    let highest = points
        .iter()
        .map(|(_, level)| *level)
        .fold(f64::NEG_INFINITY, f64::max);
    for frequency in grid() {
        let level = curve.value_db(frequency);
        assert!(
            level.is_finite() && level >= lowest - 1e-9 && level <= highest + 1e-9,
            "{level} dB at {frequency} Hz"
        );
    }
}

/// The design a project saved, as the JSON that nih-plug stores in the plugin's state.
///
/// Checks beyond not crashing: the correction of a design that is read and used has a finite
/// level within its limits at each frequency of the grid, and its FIR at 44.1 kHz finite taps.
///
/// # Panics
///
/// If one of these checks fails, which is what the fuzz target looks for.
pub fn saved_design(data: &[u8]) {
    let Ok(saved) = serde_json::from_slice::<SavedDesign>(data) else {
        return;
    };
    let Some(correction) = saved.correction() else {
        return;
    };
    let limits = MAX_CUT_DB..=*MAX_BOOST_DB.end();
    assert!(
        correction
            .levels_db()
            .iter()
            .all(|level| limits.contains(level))
    );
    let taps = fir::minimum_phase(&correction, 44_100.0);
    assert!(taps.is_some_and(|taps| taps.iter().all(|tap| tap.is_finite())));
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

/// The text of AutoEQ's name index of oratory1990's measurements, as bytes.
///
/// Checks beyond not crashing: every model has a name and one of the known forms, no model is
/// listed twice, models and left-out lines are no more than the lines read, every model's file
/// name stays in its folder, and a model is found again by its name.
///
/// # Panics
///
/// If one of these checks fails, which is what the fuzz target looks for.
pub fn name_index(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let index = NameIndex::parse(text);
    let lines = text.lines().take(MAX_LINES).count();
    assert!(index.models().len() + index.skipped() <= lines);
    for (position, model) in index.models().iter().enumerate() {
        assert_ne!(model.name, "");
        assert!(FORMS.contains(&model.form.as_str()));
        assert!(index.models().iter().skip(position + 1).all(|other| {
            (other.name.as_str(), other.form.as_str()) != (model.name.as_str(), model.form.as_str())
        }));
        let file_name = model.file_name();
        assert!(!file_name.starts_with('.') && !file_name.contains(['/', '\\']));
        assert!(model.url().starts_with(crate::profiles::AUTOEQ_URL));
        assert!(index.find(&model.name).is_ok());
    }
}

/// The output of `cargo metadata`, which the plugin's build script reads for the about window.
///
/// Checks beyond not crashing: the crates found are sorted by name and version, each once, none
/// has a folder that ends in its manifest or a separator, and the Rust written of them has one
/// line per crate, so their strings are escaped.
///
/// # Panics
///
/// If one of these checks fails, which is what the fuzz target looks for.
pub fn cargo_metadata(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(found) = packages(text, "audioref") else {
        return;
    };
    for (position, package) in found.iter().enumerate() {
        assert!(found.iter().skip(position + 1).all(|later| {
            (&package.name, &package.version) <= (&later.name, &later.version) && later != package
        }));
        assert!(!package.dir.ends_with("/Cargo.toml") && !package.dir.ends_with(['/', '\\']));
    }
    let with_texts: Vec<_> = found
        .into_iter()
        .map(|package| {
            let text = format!("{}\n\"{}\"", package.name, package.license);
            (package, vec![text])
        })
        .collect();
    let rust = code(&with_texts);
    let texts = with_texts
        .iter()
        .map(|(_, texts)| texts)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    assert_eq!(rust.lines().count(), with_texts.len() + texts + 5);
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
    fn autoeq_curve_smoke_test() {
        let inputs: [&[u8]; 6] = [
            b"",
            b"\xff\xfe",
            b"frequency,raw\n20,0\n20000,0\n",
            b"frequency,raw\n20,0\n1000,1\n1000,2\nx\n20000,-3\n",
            b"* REW\n20.0\t85.1\n20000.0 80.0\n",
            b"Freq(Hz);SPL(dB)\n20;80,5\n200;1e400\n20000;70\n",
        ];
        for input in inputs {
            super::autoeq_curve(input);
        }
    }

    #[test]
    fn saved_design_smoke_test() {
        let flat = format!("[{}]", vec!["0"; 695].join(","));
        let loud = format!("[{}]", vec!["150"; 695].join(","));
        let inputs = [
            String::new(),
            "null".to_owned(),
            r#"{"measurement":"M","target":"T","measurement_db":[],"target_db":[]}"#.to_owned(),
            format!(
                r#"{{"measurement":"M","target":"T","measurement_db":{flat},"target_db":{loud}}}"#
            ),
            format!(
                r#"{{"measurement":"M","target":"T","measurement_db":{loud},"target_db":{flat},"settings":{{"tilt_db_per_octave":9,"bass_db":-99,"max_boost_db":1e300}}}}"#
            ),
        ];
        for input in inputs {
            super::saved_design(input.as_bytes());
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

    #[test]
    fn cargo_metadata_smoke_test() {
        let inputs: [&[u8]; 6] = [
            b"",
            b"\xff\xfe",
            b"{}",
            br#"{"packages": [{"id": "r", "name": "audioref", "version": "1", "source": null, "manifest_path": "/w/Cargo.toml"}]}"#,
            br#"{"packages": [
                {"id": "r", "name": "audioref", "version": "1", "source": null, "manifest_path": "/w/Cargo.toml"},
                {"id": "a", "name": "a\n\"b\"", "version": "1", "license": "MIT\u0000", "source": "registry", "manifest_path": "/r/a/Cargo.toml"},
                {"id": "b", "name": "a\n\"b\"", "version": "1", "license": "MIT\u0000", "source": "registry", "manifest_path": "/r/a/Cargo.toml"},
                {"id": "c", "name": "c", "version": "1", "source": "git", "manifest_path": "/g/c/Cargo.toml"}
            ], "resolve": {"nodes": [
                {"id": "r", "deps": [{"pkg": "a", "dep_kinds": [{"kind": null}]}, {"pkg": "b", "dep_kinds": [{"kind": null}]}, {"pkg": "c", "dep_kinds": [{"kind": null}]}, {"pkg": "r", "dep_kinds": [{"kind": null}]}]},
                {"id": "a", "deps": [{"pkg": "missing", "dep_kinds": [{"kind": null}]}]}
            ]}}"#,
            br#"{"packages": [{"id": "x", "name": "x", "version": "1", "source": null, "manifest_path": ""}], "resolve": null}"#,
        ];
        for input in inputs {
            super::cargo_metadata(input);
        }
    }

    #[test]
    fn name_index_smoke_test() {
        let inputs: [&[u8]; 6] = [
            b"",
            b"\xff\xfe",
            b"url\tsource_name\tname\tform\trig\n",
            b"u\ts\tA\tover-ear\nu\ts\tA\tin-ear\nu\tA\tB\tearbud\n",
            b"u\ts\tAKG K240 Studio\tover-ear\tGRAS 45BC\nu\ts\tAKG K240 Studio\tover-ear\n",
            b"u\ts\t../x\tin-ear\nu\ts\t\tearbud\nu\ts\tA\tignore\nu\tonly\n",
        ];
        for input in inputs {
            super::name_index(input);
        }
    }
}
