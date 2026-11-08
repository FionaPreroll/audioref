//! Where `cargo xtask fetch-profiles` finds the files of headphones: OPRA's database, AutoEQ's
//! target and oratory1990's measurements in AutoEQ's repository, found through its name index.
//!
//! This module only knows the addresses and reads the index; the xtask downloads and writes the
//! files, outside the repository, as AudioRef ships no measurements.
//!
//! ```
//! use audioref_core::profiles::NameIndex;
//!
//! let index = NameIndex::parse(
//!     "url\tsource_name\tname\tform\trig\n\
//!      https://x/K240.pdf\tAKG K240 Studio\tAKG K240 Studio\tover-ear\tGRAS 45BC\n",
//! );
//! let model = index.find("k240").map_err(|_| "not found")?;
//! assert_eq!(model.rig, "GRAS 45BC");
//! assert_eq!(
//!     model.url(),
//!     "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/\
//!      measurements/oratory1990/data/over-ear/AKG%20K240%20Studio.csv"
//! );
//! assert_eq!(model.file_name(), "AKG K240 Studio.csv");
//! # Ok::<(), &str>(())
//! ```

/// OPRA's database, the latest on its main branch.
pub const OPRA_DATABASE_URL: &str =
    "https://raw.githubusercontent.com/opra-project/OPRA/main/dist/database_v1.jsonl";

/// OPRA's repository, as the window links it.
pub const OPRA_REPOSITORY: &str = "https://github.com/opra-project/OPRA";

/// OPRA's database as the window links it: GitHub's download of the file on the main branch.
pub const OPRA_DATABASE_LINK: &str =
    "https://github.com/opra-project/OPRA/raw/refs/heads/main/dist/database_v1.jsonl";

/// AutoEQ's repository, as the window links it.
pub const AUTOEQ_REPOSITORY: &str = "https://github.com/jaakkopasanen/AutoEq";

/// The start of every address of a file in AutoEQ's repository, on its main branch.
pub const AUTOEQ_URL: &str = "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/";

/// The name index of oratory1990's measurements, in AutoEQ's repository.
pub const NAME_INDEX: &str = "measurements/oratory1990/name_index.tsv";

/// The target fetched with the measurements, in AutoEQ's repository: Harman's for over-ear
/// headphones, defined on a GRAS rig as oratory1990 uses.
pub const TARGET: &str = "targets/Harman over-ear 2018.csv";

/// The headphones fetched when none are named: those AudioRef was first built for.
pub const DEFAULT_HEADPHONES: [&str; 3] =
    ["AKG K240 Studio", "Bose QuietComfort 25", "Sony WH-1000XM4"];

/// The forms of headphones the index lists measurements of: each is a folder of AutoEQ's data.
pub const FORMS: [&str; 3] = ["over-ear", "in-ear", "earbud"];

/// The most lines of the index that are read.
pub const MAX_LINES: usize = 100_000;

/// A measured model in the name index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// The name oratory1990 gave it.
    pub source_name: String,
    /// The name AutoEQ gives it, which names its file.
    pub name: String,
    /// One of [`FORMS`].
    pub form: String,
    /// The measurement rig, e.g. GRAS 43AG; may be empty.
    pub rig: String,
}

impl Model {
    /// The path of its measurement in AutoEQ's repository.
    #[must_use]
    pub fn path(&self) -> String {
        format!(
            "measurements/oratory1990/data/{}/{}.csv",
            self.form, self.name
        )
    }

    /// The address of its measurement.
    #[must_use]
    pub fn url(&self) -> String {
        url(&self.path())
    }

    /// The name of the file to save it as: its name, with the characters that systems don't
    /// allow in file names, or that would leave the folder, replaced by `_`.
    #[must_use]
    pub fn file_name(&self) -> String {
        let name: String = self
            .name
            .chars()
            .map(|c| {
                if c.is_control()
                    || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
                {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        // Not hidden, and never "." or "..".
        let name = name.trim_start_matches('.');
        if name.is_empty() {
            "_.csv".to_owned()
        } else {
            format!("{name}.csv")
        }
    }
}

/// Why no model fits a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindError {
    /// No model has the name, or a name that contains it.
    NotFound,
    /// Several models contain it, by these names.
    Ambiguous(Vec<String>),
}

/// The name index of oratory1990's measurements.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameIndex {
    models: Vec<Model>,
    skipped: usize,
}

impl NameIndex {
    /// Reads the index: lines of tab-separated `url, source_name, name, form, rig`, after a
    /// header. Lines of another form, e.g. `ignore`, or without a name, are left out and counted;
    /// a model listed twice is kept once. Never fails: an index without models finds nothing.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut index = Self::default();
        for line in text.lines().take(MAX_LINES) {
            let line = line.trim_end_matches('\r');
            let mut fields = line.split('\t').map(str::trim);
            let (Some(_url), Some(source_name), Some(name), Some(form)) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                if !line.trim().is_empty() {
                    index.skipped += 1;
                }
                continue;
            };
            if name == "name" && form == "form" {
                continue;
            }
            if name.is_empty() || !FORMS.contains(&form) {
                index.skipped += 1;
                continue;
            }
            let model = Model {
                source_name: source_name.to_owned(),
                name: name.to_owned(),
                form: form.to_owned(),
                rig: fields.next().unwrap_or_default().to_owned(),
            };
            if !index
                .models
                .iter()
                .any(|known| known.name == model.name && known.form == model.form)
            {
                index.models.push(model);
            }
        }
        index
    }

    /// The models, in the order of the index.
    #[must_use]
    pub fn models(&self) -> &[Model] {
        &self.models
    }

    /// How many lines were left out.
    #[must_use]
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    /// The model with a name, AutoEQ's or oratory1990's, in any case; else the only one whose
    /// name contains it.
    ///
    /// # Errors
    ///
    /// [`FindError::NotFound`] if none fits, [`FindError::Ambiguous`] with their names if several
    /// contain it.
    pub fn find(&self, query: &str) -> Result<&Model, FindError> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Err(FindError::NotFound);
        }
        let names = |model: &Model| [model.name.to_lowercase(), model.source_name.to_lowercase()];
        if let Some(model) = self
            .models
            .iter()
            .find(|model| names(model).contains(&query))
        {
            return Ok(model);
        }
        let matches: Vec<&Model> = self
            .models
            .iter()
            .filter(|model| names(model).iter().any(|name| name.contains(&query)))
            .collect();
        match matches.as_slice() {
            [] => Err(FindError::NotFound),
            [model] => Ok(model),
            several => Err(FindError::Ambiguous(
                several.iter().map(|model| model.name.clone()).collect(),
            )),
        }
    }
}

/// The address of a path in AutoEQ's repository, each part of it percent-encoded.
#[must_use]
pub fn url(path: &str) -> String {
    let encoded: Vec<String> = path.split('/').map(encode).collect();
    format!("{AUTOEQ_URL}{}", encoded.join("/"))
}

/// Percent-encodes all but the unreserved characters of URLs.
fn encode(part: &str) -> String {
    let mut encoded = String::new();
    for byte in part.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            for nibble in [byte >> 4, byte & 0x0f] {
                // A nibble is always a hexadecimal digit.
                if let Some(digit) = char::from_digit(u32::from(nibble), 16) {
                    encoded.push(digit.to_ascii_uppercase());
                }
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = "url\tsource_name\tname\tform\trig\r\n\
        https://x/a.pdf\t64 Audio N8\t\tignore\t\n\
        https://x/k.pdf?dl=1\tAKG K240 Studio\tAKG K240 Studio\tover-ear\tGRAS 45BC \n\
        https://x/k.pdf\tAKG K240 Studio\tAKG K240 Studio\tover-ear\tGRAS 45BC\n\
        https://x/q.pdf\tBose QC25\tBose QuietComfort 25\tover-ear\tGRAS 43AG\n\
        https://x/p.pdf\tBose QC25 (ANC off)\tBose QuietComfort 25 (ANC off)\tover-ear\n\
        https://x/s.pdf\tSony WH-1000XM4\tSony WH-1000XM4\tover-ear\tGRAS 45BC-10\n\
        \n\
        a line without tabs\n\
        https://x/e.pdf\tE\tE 1\tearbud\tB&K 5128\n";

    #[test]
    fn reads_the_models_and_leaves_out_the_rest() {
        let index = NameIndex::parse(INDEX);
        let names: Vec<&str> = index.models().iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "AKG K240 Studio",
                "Bose QuietComfort 25",
                "Bose QuietComfort 25 (ANC off)",
                "Sony WH-1000XM4",
                "E 1"
            ]
        );
        // "ignore" and the line without tabs.
        assert_eq!(index.skipped(), 2);
        let first = &index.models()[0];
        assert_eq!(first.rig, "GRAS 45BC");
        assert_eq!(index.models()[2].rig, "");
        assert_eq!(NameIndex::parse(""), NameIndex::default());
    }

    #[test]
    fn finds_by_either_name_or_a_part_of_it() {
        let index = NameIndex::parse(INDEX);
        for query in DEFAULT_HEADPHONES {
            assert_eq!(index.find(query).unwrap().name, query);
        }
        assert_eq!(
            index.find(" bose qc25 ").unwrap().name,
            "Bose QuietComfort 25"
        );
        assert_eq!(index.find("XM4").unwrap().name, "Sony WH-1000XM4");
        assert_eq!(
            index.find("QuietComfort"),
            Err(FindError::Ambiguous(vec![
                "Bose QuietComfort 25".to_owned(),
                "Bose QuietComfort 25 (ANC off)".to_owned()
            ]))
        );
        assert_eq!(index.find("HD 600"), Err(FindError::NotFound));
        assert_eq!(index.find("  "), Err(FindError::NotFound));
    }

    #[test]
    fn addresses_and_file_names() {
        let index = NameIndex::parse(INDEX);
        let earbud = index.find("E 1").unwrap();
        assert_eq!(
            earbud.path(),
            "measurements/oratory1990/data/earbud/E 1.csv"
        );
        assert_eq!(
            earbud.url(),
            format!("{AUTOEQ_URL}measurements/oratory1990/data/earbud/E%201.csv")
        );
        assert_eq!(
            url(TARGET),
            format!("{AUTOEQ_URL}targets/Harman%20over-ear%202018.csv")
        );
        assert_eq!(url("ä&(x)~"), format!("{AUTOEQ_URL}%C3%A4%26%28x%29~"));
        let odd = Model {
            source_name: String::new(),
            name: "../a/b\\c:d*e?f\"g<h>i|j\u{7}".to_owned(),
            form: "in-ear".to_owned(),
            rig: String::new(),
        };
        assert_eq!(odd.file_name(), "_a_b_c_d_e_f_g_h_i_j_.csv");
        let dots = Model {
            name: "..".to_owned(),
            ..odd
        };
        assert_eq!(dots.file_name(), "_.csv");
    }

    #[test]
    fn reads_no_more_than_its_lines() {
        let line = "u\ts\tn\tover-ear\tr\n";
        let index = NameIndex::parse(&line.repeat(MAX_LINES + 5));
        assert_eq!(index.models().len(), 1);
    }
}
