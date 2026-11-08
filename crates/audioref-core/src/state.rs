//! The correction a project saves: the chosen preset with its bands ([`SavedCorrection`]), or the
//! measurement and the target a correction is designed from, with its settings ([`SavedDesign`]),
//! so a project opens with its correction on a computer without the files, and the window can
//! name where it comes from.
//!
//! nih-plug stores it as JSON in the plugin's state, which a host hands back as it got it, or not:
//! [`SavedCorrection::eq`] and [`SavedDesign::correction`] check the values again, and a saved
//! correction outside their ranges isn't used.
//!
//! ```
//! use audioref_core::eq::{Band, ParametricEq};
//! use audioref_core::state::SavedCorrection;
//!
//! let json = r#"{"preset_id":"acme:one::x","headphones":"Acme One","author":"Tester","details":"",
//!     "gain_db":-3.0,"bands":[{"type":"peak","frequency":1000.0,"gain_db":2.0,"q":1.0}]}"#;
//! let saved: SavedCorrection = serde_json::from_str(json)?;
//! assert_eq!(saved.eq().map(|eq| eq.bands().len()), Some(1));
//! # Ok::<(), serde_json::Error>(())
//! ```

use serde::{Deserialize, Serialize};

use crate::autoeq::Curve;
use crate::cascade::grid;
use crate::design::{Correction, Settings};
use crate::eq::{Band, ParametricEq};
use crate::opra::Database;

/// A preset as a project saves it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedCorrection {
    /// The ID of the preset in OPRA's database.
    pub preset_id: String,
    /// The vendor and the model of the headphones, e.g. "Sony WH-1000XM4 (ANC on)".
    pub headphones: String,
    /// Who made the preset, as OPRA's license asks to name them.
    pub author: String,
    /// What the preset is based on, e.g. "Harman Target".
    pub details: String,
    /// The overall gain, in dB.
    pub gain_db: f64,
    /// The bands, in the order of the preset.
    pub bands: Vec<Band>,
}

impl SavedCorrection {
    /// The preset of a database with this ID, with the names of its headphones, if there is one.
    #[must_use]
    pub fn from_preset(database: &Database, preset_id: &str) -> Option<Self> {
        let preset = database.presets().get(preset_id)?;
        Some(Self {
            preset_id: preset_id.to_owned(),
            headphones: database.headphones(&preset.product_id),
            author: preset.author.clone(),
            details: preset.details.clone(),
            gain_db: preset.eq.gain_db(),
            bands: preset.eq.bands().to_vec(),
        })
    }

    /// The EQ, if all its values are in their ranges.
    #[must_use]
    pub fn eq(&self) -> Option<ParametricEq> {
        ParametricEq::new(self.gain_db, self.bands.clone()).ok()
    }
}

/// A correction designed from a measurement and a target, as a project saves it: both curves on
/// AutoEQ's grid ([`grid`]), so it can be designed again with other settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedDesign {
    /// The name of the measurement, e.g. its file's, "AKG K240 Studio".
    pub measurement: String,
    /// The name of the target, e.g. "Harman over-ear 2018".
    pub target: String,
    /// The measurement's level at each frequency of the grid, in dB.
    pub measurement_db: Vec<f64>,
    /// The target's level at each frequency of the grid, in dB.
    pub target_db: Vec<f64>,
    /// The tilt, the bass and the limit for boosts.
    #[serde(default)]
    pub settings: Settings,
}

impl SavedDesign {
    /// A measurement and a target with their names, sampled on the grid, with settings.
    #[must_use]
    pub fn new(measurement: (&str, &Curve), target: (&str, &Curve), settings: Settings) -> Self {
        let on_grid = |curve: &Curve| grid().map(|frequency| curve.value_db(frequency)).collect();
        Self {
            measurement: measurement.0.to_owned(),
            target: target.0.to_owned(),
            measurement_db: on_grid(measurement.1),
            target_db: on_grid(target.1),
            settings,
        }
    }

    /// The correction, designed with the settings clamped to their ranges, if both curves have a
    /// level for each frequency of the grid, within [`crate::autoeq::LEVEL_DB`].
    #[must_use]
    pub fn correction(&self) -> Option<Correction> {
        let curve = |levels: &[f64]| {
            (levels.len() == grid().count())
                .then(|| Curve::from_points(grid().zip(levels.iter().copied()).collect()).ok())
                .flatten()
        };
        let measurement = curve(&self.measurement_db)?;
        let target = curve(&self.target_db)?;
        Some(Correction::design(&measurement, &target, self.settings))
    }
}

#[cfg(test)]
mod tests {
    use super::{SavedCorrection, SavedDesign};
    use crate::autoeq::Curve;
    use crate::design::Settings;
    use crate::eq::{Band, Slope};
    use crate::opra::Database;

    const DATABASE: &str = r#"{"type":"vendor","id":"acme","data":{"name":"Acme"}}
{"type":"product","id":"acme::one","data":{"name":"One (ANC on)","subtype":"over_the_ear","vendor_id":"acme"}}
{"type":"eq","id":"acme:one::tester","data":{"author":"Tester","details":"Harman Target","type":"parametric_eq","product_id":"acme::one","parameters":{"gain_db":-4,"bands":[{"type":"low_shelf","frequency":105,"gain_db":5,"q":0.7},{"type":"high_pass","frequency":20,"slope":24}]}}}"#;

    #[test]
    fn saves_a_preset_with_its_headphones_and_author() {
        let database = Database::parse(DATABASE).expect("accepted");
        let saved = SavedCorrection::from_preset(&database, "acme:one::tester").expect("present");
        assert_eq!(saved.headphones, "Acme One (ANC on)");
        assert_eq!(
            (saved.author.as_str(), saved.details.as_str()),
            ("Tester", "Harman Target")
        );
        let eq = saved.eq().expect("valid");
        assert_eq!(
            eq.bands(),
            [
                Band::LowShelf {
                    frequency: 105.0,
                    gain_db: 5.0,
                    q: 0.7
                },
                Band::HighPass {
                    frequency: 20.0,
                    slope: Slope::Db24
                },
            ]
        );
        assert!(SavedCorrection::from_preset(&database, "acme:one::other").is_none());
    }

    #[test]
    fn a_saved_correction_reads_back_the_same() {
        let database = Database::parse(DATABASE).expect("accepted");
        let saved = SavedCorrection::from_preset(&database, "acme:one::tester").expect("present");
        let json = serde_json::to_string(&saved).expect("serializable");
        assert!(
            json.contains(r#""type":"high_pass""#) && json.contains(r#""slope":"db24""#),
            "{json}"
        );
        let read: SavedCorrection = serde_json::from_str(&json).expect("readable");
        assert_eq!(read, saved);
    }

    #[test]
    fn values_out_of_range_are_not_used() {
        let saved = SavedCorrection {
            preset_id: "x".to_owned(),
            headphones: "X".to_owned(),
            author: "Y".to_owned(),
            details: String::new(),
            gain_db: 0.0,
            bands: vec![Band::Peak {
                frequency: 1000.0,
                gain_db: 99.0,
                q: 1.0,
            }],
        };
        assert!(saved.eq().is_none());
        assert!(serde_json::from_str::<SavedCorrection>(r#"{"preset_id":"x"}"#).is_err());
    }

    fn design() -> SavedDesign {
        let measurement =
            Curve::from_points(vec![(20.0, -6.0), (150.0, 0.0), (20_000.0, 0.0)]).expect("a curve");
        let target = Curve::from_points(vec![(20.0, 0.0), (20_000.0, 0.0)]).expect("a curve");
        SavedDesign::new(
            ("Measured", &measurement),
            ("Flat", &target),
            Settings::default(),
        )
    }

    #[test]
    fn a_design_reads_back_the_same_and_corrects() {
        let saved = design();
        assert_eq!(saved.measurement_db.len(), 695);
        let json = serde_json::to_string(&saved).expect("serializable");
        let read: SavedDesign = serde_json::from_str(&json).expect("readable");
        // JSON keeps the levels to within the last digit.
        assert_eq!(
            (&read.measurement, &read.target),
            (&saved.measurement, &saved.target)
        );
        assert_eq!(read.settings, saved.settings);
        let close = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-12);
        assert!(close(&read.measurement_db, &saved.measurement_db));
        assert!(close(&read.target_db, &saved.target_db));
        let correction = read.correction().expect("valid");
        assert!((correction.response_db(20.0) - 6.0).abs() < 0.1);
        // Older projects without settings get the defaults.
        let without: SavedDesign = serde_json::from_str(
            r#"{"measurement":"M","target":"T","measurement_db":[],"target_db":[]}"#,
        )
        .expect("readable");
        assert_eq!(without.settings, Settings::default());
    }

    #[test]
    fn a_design_with_bad_curves_is_not_used() {
        let mut short = design();
        short.measurement_db.pop();
        assert!(short.correction().is_none());
        let mut loud = design();
        loud.target_db[10] = 1e6;
        assert!(loud.correction().is_none());
        let mut nan = design();
        nan.measurement_db[10] = f64::NAN;
        assert!(nan.correction().is_none());
    }
}
