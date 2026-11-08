//! OPRA's database of headphones and their EQ presets, `dist/database_v1.jsonl` in
//! [OPRA's repository](https://github.com/opra-project/OPRA): one JSON object per line, each a
//! vendor, a product or an EQ preset.
//!
//! [`Database::parse`] reads the text of the file. Lines it doesn't understand, e.g. of kinds that
//! a later version of OPRA adds, or with values outside the ranges of [`crate::eq`], are left out
//! and counted, so one bad line doesn't cost all the others.
//!
//! ```
//! use audioref_core::opra::Database;
//!
//! let text = r#"{"type":"vendor","id":"akg","data":{"name":"AKG"}}
//! {"type":"product","id":"akg::k240_studio","data":{"name":"K240 Studio","type":"headphones","subtype":"over_the_ear","vendor_id":"akg"}}
//! {"type":"eq","id":"akg:k240_studio::example","data":{"author":"Example","details":"Harman Target","type":"parametric_eq","product_id":"akg::k240_studio","parameters":{"gain_db":-6.2,"bands":[{"type":"low_shelf","frequency":105,"gain_db":6.3,"q":0.7}]}}}"#;
//! let database = Database::parse(text).map_err(|error| format!("{error:?}"))?;
//! let (_, preset) = database.presets_of("akg::k240_studio").next().ok_or("no preset")?;
//! assert_eq!(preset.author, "Example");
//! assert_eq!(preset.eq.bands().len(), 1);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! OPRA's data is licensed under CC BY-SA 4.0: wherever a preset is used, its author is named.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use serde::Deserialize;

use crate::eq::{Band, ParametricEq, Slope};

/// A maker of headphones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vendor {
    /// The name, e.g. "AKG".
    pub name: String,
}

/// How headphones are worn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    /// Over the ears, around them.
    OverTheEar,
    /// On the ears.
    OnEar,
    /// In the ear canal.
    InEar,
    /// In the outer ear, not sealed.
    Earbuds,
    /// A form that OPRA added after this version of AudioRef.
    #[serde(other)]
    Other,
}

/// A model of headphones. OPRA lists the modes of some as models of their own, e.g.
/// "WH-1000XM4 (ANC on)" and "WH-1000XM4 (ANC Off)".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    /// The ID of its vendor, a key of [`Database::vendors`].
    pub vendor_id: String,
    /// The name, without the vendor's, e.g. "K240 Studio".
    pub name: String,
    /// How it is worn.
    pub form: Form,
}

/// An EQ preset for a product.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    /// The ID of its product, a key of [`Database::products`].
    pub product_id: String,
    /// Who made it, e.g. "oratory1990" or "AutoEQ". OPRA's license asks to name them.
    pub author: String,
    /// What it is based on, e.g. "Harman Target" or "Measured by oratory1990".
    pub details: String,
    /// Where it comes from, if OPRA says.
    pub link: Option<String>,
    /// The bands and the overall gain.
    pub eq: ParametricEq,
}

/// Why a text isn't OPRA's database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// Not one line is a vendor, a product or a preset of OPRA's: most likely another file.
    NoEntries,
}

/// OPRA's vendors, products and presets, each by its ID.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Database {
    vendors: BTreeMap<String, Vendor>,
    products: BTreeMap<String, Product>,
    presets: BTreeMap<String, Preset>,
    skipped: usize,
}

impl Database {
    /// Reads the text of OPRA's database. Empty lines are passed over; a line that isn't a vendor,
    /// product or preset, or repeats the ID of an earlier one of its kind, is left out and counted
    /// in [`Self::skipped`].
    ///
    /// # Errors
    ///
    /// If no line is a vendor, product or preset.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut database = Self::default();
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            if !database.add(line) {
                database.skipped += 1;
            }
        }
        if database.vendors.is_empty()
            && database.products.is_empty()
            && database.presets.is_empty()
        {
            return Err(ParseError::NoEntries);
        }
        Ok(database)
    }

    /// Adds the entry of a line, and says whether it could.
    fn add(&mut self, line: &str) -> bool {
        let Ok(entry) = serde_json::from_str::<RawEntry>(line) else {
            return false;
        };
        match entry {
            RawEntry::Vendor { id, data } => {
                insert(&mut self.vendors, id, Some(Vendor { name: data.name }))
            }
            RawEntry::Product { id, data } => insert(
                &mut self.products,
                id,
                Some(Product {
                    vendor_id: data.vendor_id,
                    name: data.name,
                    form: data.subtype,
                }),
            ),
            RawEntry::Eq { id, data } => insert(&mut self.presets, id, data.into_preset()),
        }
    }

    /// The vendors, by ID.
    #[must_use]
    pub fn vendors(&self) -> &BTreeMap<String, Vendor> {
        &self.vendors
    }

    /// The products, by ID.
    #[must_use]
    pub fn products(&self) -> &BTreeMap<String, Product> {
        &self.products
    }

    /// The presets, by ID.
    #[must_use]
    pub fn presets(&self) -> &BTreeMap<String, Preset> {
        &self.presets
    }

    /// The presets of a product, with their IDs, in the order of their IDs.
    pub fn presets_of<'a>(
        &'a self,
        product_id: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a Preset)> {
        self.presets
            .iter()
            .filter(move |(_, preset)| preset.product_id == product_id)
            .map(|(id, preset)| (id.as_str(), preset))
    }

    /// The number of lines that were left out.
    #[must_use]
    pub fn skipped(&self) -> usize {
        self.skipped
    }
}

/// Inserts a value under an ID that isn't empty or taken yet, and says whether it did.
fn insert<T>(map: &mut BTreeMap<String, T>, id: String, value: Option<T>) -> bool {
    match (value, id.is_empty(), map.entry(id)) {
        (Some(value), false, Entry::Vacant(entry)) => {
            entry.insert(value);
            true
        }
        _ => false,
    }
}

/// A line of the database, as OPRA writes it.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawEntry {
    Vendor { id: String, data: RawVendor },
    Product { id: String, data: RawProduct },
    Eq { id: String, data: RawEq },
}

#[derive(Deserialize)]
struct RawVendor {
    name: String,
}

#[derive(Deserialize)]
struct RawProduct {
    name: String,
    vendor_id: String,
    subtype: Form,
}

#[derive(Deserialize)]
struct RawEq {
    product_id: String,
    author: String,
    #[serde(default)]
    details: String,
    link: Option<String>,
    /// Only parametric EQs so far; another kind makes the line fail.
    #[serde(rename = "type")]
    _kind: RawEqKind,
    parameters: RawParameters,
}

impl RawEq {
    /// The preset, if its values are in their ranges.
    fn into_preset(self) -> Option<Preset> {
        let bands = self
            .parameters
            .bands
            .into_iter()
            .map(RawBand::into_band)
            .collect::<Option<_>>()?;
        Some(Preset {
            product_id: self.product_id,
            author: self.author,
            details: self.details,
            link: self.link,
            eq: ParametricEq::new(self.parameters.gain_db, bands).ok()?,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawEqKind {
    ParametricEq,
}

#[derive(Deserialize)]
struct RawParameters {
    #[serde(default)]
    gain_db: f64,
    bands: Vec<RawBand>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawBand {
    PeakDip {
        frequency: f64,
        #[serde(default)]
        gain_db: f64,
        q: f64,
    },
    LowShelf {
        frequency: f64,
        #[serde(default)]
        gain_db: f64,
        q: f64,
    },
    HighShelf {
        frequency: f64,
        #[serde(default)]
        gain_db: f64,
        q: f64,
    },
    LowPass {
        frequency: f64,
        #[serde(default = "default_slope")]
        slope: f64,
    },
    HighPass {
        frequency: f64,
        #[serde(default = "default_slope")]
        slope: f64,
    },
    BandPass {
        frequency: f64,
        q: f64,
    },
    BandStop {
        frequency: f64,
        q: f64,
    },
}

/// The slope that OPRA's schema gives a low or high pass without one, in dB per octave.
fn default_slope() -> f64 {
    12.0
}

impl RawBand {
    /// The band, if its slope is one of those OPRA allows. [`ParametricEq::new`] checks the rest.
    fn into_band(self) -> Option<Band> {
        Some(match self {
            Self::PeakDip {
                frequency,
                gain_db,
                q,
            } => Band::Peak {
                frequency,
                gain_db,
                q,
            },
            Self::LowShelf {
                frequency,
                gain_db,
                q,
            } => Band::LowShelf {
                frequency,
                gain_db,
                q,
            },
            Self::HighShelf {
                frequency,
                gain_db,
                q,
            } => Band::HighShelf {
                frequency,
                gain_db,
                q,
            },
            Self::LowPass { frequency, slope } => Band::LowPass {
                frequency,
                slope: Slope::from_db_per_octave(slope)?,
            },
            Self::HighPass { frequency, slope } => Band::HighPass {
                frequency,
                slope: Slope::from_db_per_octave(slope)?,
            },
            Self::BandPass { frequency, q } => Band::BandPass { frequency, q },
            Self::BandStop { frequency, q } => Band::BandStop { frequency, q },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Database, Form, ParseError};
    use crate::eq::{Band, Slope};

    const VENDOR: &str = r#"{"type":"vendor","id":"acme","data":{"name":"Acme","logo":"assets/x.png","blurb":"Makes headphones."}}"#;
    const PRODUCT: &str = r#"{"type":"product","id":"acme::one","data":{"name":"One","type":"headphones","subtype":"over_the_ear","vendor_id":"acme","line_art_svg":"assets/y.svg"}}"#;

    /// A preset for `acme::one` with the bands given as JSON.
    fn preset(id: &str, bands: &str) -> String {
        format!(
            r#"{{"type":"eq","id":"{id}","data":{{"author":"Tester","details":"Synthetic","type":"parametric_eq","product_id":"acme::one","parameters":{{"gain_db":-3.5,"bands":[{bands}]}}}}}}"#
        )
    }

    fn parse(lines: &[&str]) -> Database {
        Database::parse(&lines.join(
            "
",
        ))
        .expect("accepted")
    }

    #[test]
    fn reads_vendors_products_and_presets() {
        let eq = preset(
            "acme:one::tester",
            r#"{"type":"peak_dip","frequency":1000,"gain_db":-2.5,"q":1.4}"#,
        );
        let database = parse(&[VENDOR, PRODUCT, &eq]);
        assert_eq!(
            database
                .vendors()
                .get("acme")
                .map(|vendor| vendor.name.as_str()),
            Some("Acme")
        );
        let product = database.products().get("acme::one").expect("present");
        assert_eq!(
            (product.vendor_id.as_str(), product.name.as_str()),
            ("acme", "One")
        );
        assert_eq!(product.form, Form::OverTheEar);
        let presets: Vec<_> = database.presets_of("acme::one").collect();
        assert_eq!(presets.len(), 1, "{presets:?}");
        let (id, preset) = presets[0];
        assert_eq!(id, "acme:one::tester");
        assert_eq!(
            (preset.author.as_str(), preset.details.as_str()),
            ("Tester", "Synthetic")
        );
        assert_eq!(preset.link, None);
        assert_eq!(preset.eq.gain_db().to_bits(), f64::to_bits(-3.5));
        assert_eq!(
            preset.eq.bands(),
            [Band::Peak {
                frequency: 1000.0,
                gain_db: -2.5,
                q: 1.4
            }]
        );
        assert_eq!(database.presets().len(), 1);
        assert_eq!(database.skipped(), 0);
        assert_eq!(database.presets_of("acme::two").count(), 0);
    }

    #[test]
    fn reads_every_kind_of_band() {
        let bands = [
            r#"{"type":"low_shelf","frequency":105,"gain_db":6,"q":0.7}"#,
            r#"{"type":"high_shelf","frequency":10000,"gain_db":-1.5,"q":0.7}"#,
            r#"{"type":"low_pass","frequency":12000,"slope":24}"#,
            r#"{"type":"high_pass","frequency":20}"#,
            r#"{"type":"band_pass","frequency":1000,"q":2}"#,
            r#"{"type":"band_stop","frequency":50,"q":10}"#,
            r#"{"type":"peak_dip","frequency":3000,"q":3}"#,
        ];
        let eq = preset("acme:one::all", &bands.join(","));
        let database = parse(&[&eq]);
        let preset = database.presets().get("acme:one::all").expect("present");
        assert_eq!(
            preset.eq.bands(),
            [
                Band::LowShelf {
                    frequency: 105.0,
                    gain_db: 6.0,
                    q: 0.7
                },
                Band::HighShelf {
                    frequency: 10_000.0,
                    gain_db: -1.5,
                    q: 0.7
                },
                Band::LowPass {
                    frequency: 12_000.0,
                    slope: Slope::Db24
                },
                // OPRA's schema gives a pass without a slope 12 dB per octave.
                Band::HighPass {
                    frequency: 20.0,
                    slope: Slope::Db12
                },
                Band::BandPass {
                    frequency: 1000.0,
                    q: 2.0
                },
                Band::BandStop {
                    frequency: 50.0,
                    q: 10.0
                },
                // And a peak without a gain 0 dB.
                Band::Peak {
                    frequency: 3000.0,
                    gain_db: 0.0,
                    q: 3.0
                },
            ]
        );
    }

    #[test]
    fn reads_the_link_and_forms_of_later_versions() {
        let eq = r#"{"type":"eq","id":"acme:one::linked","data":{"author":"Tester","link":"https://example.com/one.pdf","type":"parametric_eq","product_id":"acme::one","parameters":{"bands":[]}}}"#;
        let product = r#"{"type":"product","id":"acme::two","data":{"name":"Two","type":"headphones","subtype":"bone_conduction","vendor_id":"acme"}}"#;
        let database = parse(&[eq, product]);
        let preset = database.presets().get("acme:one::linked").expect("present");
        assert_eq!(preset.link.as_deref(), Some("https://example.com/one.pdf"));
        assert_eq!(preset.details, "");
        assert_eq!(preset.eq.gain_db().to_bits(), 0.0_f64.to_bits());
        assert_eq!(
            database
                .products()
                .get("acme::two")
                .map(|product| product.form),
            Some(Form::Other)
        );
    }

    #[test]
    fn leaves_out_and_counts_lines_it_does_not_understand() {
        let lines = [
            VENDOR,
            "not json",
            r#"{"type":"speaker","id":"acme::box","data":{"name":"Box"}}"#,
            r#"{"type":"vendor","id":"","data":{"name":"Nobody"}}"#,
            r#"{"type":"vendor","id":"acme","data":{"name":"Acme again"}}"#,
            r#"{"type":"vendor","id":"other"}"#,
            &preset(
                "acme:one::unknown_band",
                r#"{"type":"tilt","frequency":1000,"gain_db":1}"#,
            ),
            &preset(
                "acme:one::bad_slope",
                r#"{"type":"low_pass","frequency":8000,"slope":15}"#,
            ),
            &preset(
                "acme:one::bad_q",
                r#"{"type":"peak_dip","frequency":1000,"gain_db":1,"q":0}"#,
            ),
            &preset(
                "acme:one::huge",
                r#"{"type":"peak_dip","frequency":1e400,"gain_db":1,"q":1}"#,
            ),
            r#"{"type":"eq","id":"acme:one::graphic","data":{"author":"Tester","type":"graphic_eq","product_id":"acme::one","parameters":{"bands":[]}}}"#,
        ];
        let database = parse(&lines);
        assert_eq!(database.skipped(), lines.len() - 1);
        assert_eq!(
            database
                .vendors()
                .get("acme")
                .map(|vendor| vendor.name.as_str()),
            Some("Acme")
        );
        assert!(database.products().is_empty() && database.presets().is_empty());
    }

    #[test]
    fn passes_over_empty_lines_a_byte_order_mark_and_crlf() {
        let text = format!("\u{feff}{VENDOR}\r\n\r\n  \n{PRODUCT}\r\n");
        let database = Database::parse(&text).expect("accepted");
        assert_eq!(
            (database.vendors().len(), database.products().len()),
            (1, 1)
        );
        assert_eq!(database.skipped(), 0);
    }

    #[test]
    fn refuses_a_text_without_entries() {
        for text in [
            "",
            "\n\n",
            "frequency,raw\n20.0,1.5\n",
            "{}",
            r#"{"type":"vendor"}"#,
        ] {
            assert_eq!(Database::parse(text), Err(ParseError::NoEntries), "{text}");
        }
    }
}
