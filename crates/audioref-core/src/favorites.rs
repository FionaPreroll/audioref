//! The favorites among OPRA's headphones and presets: marked with a star in the window, they come
//! first in their lists, see [`crate::opra::Database::search`] and
//! [`crate::opra::Database::presets_by_favorites`].
//!
//! Favorites aren't saved with a project but in a file of their own, `favorites.json`, so they
//! hold in every project: [`file()`] says where it is on each system, [`Favorites::parse`] reads
//! its text and [`Favorites::to_json`] writes it. The plugin reads and writes the file, see
//! `crates/audioref/src/background.rs`.
//!
//! ```
//! use audioref_core::favorites::{Favorites, Kind};
//!
//! let mut favorites = Favorites::default();
//! assert!(favorites.set(Kind::Headphones, "akg::k240_studio", true));
//! let again = Favorites::parse(&favorites.to_json()).map_err(|error| format!("{error:?}"))?;
//! assert!(again.contains(Kind::Headphones, "akg::k240_studio"));
//! assert!(!again.contains(Kind::Preset, "akg::k240_studio"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;

use serde_json::{Map, Value};

/// The most favorites of each kind: more than anyone marks, and few enough that the file stays
/// small.
pub const MAX_FAVORITES: usize = 1000;

/// The longest ID a favorite may have, in bytes. OPRA's longest are about 110.
pub const MAX_ID_BYTES: usize = 256;

/// The most of the file the plugin reads, in bytes: more than [`MAX_FAVORITES`] of each kind with
/// IDs of [`MAX_ID_BYTES`] take.
pub const MAX_FILE_BYTES: u64 = 1 << 20;

/// The name of the file.
pub const FILE_NAME: &str = "favorites.json";

/// The version of the file's format, written into it, so a later version of AudioRef can tell.
const VERSION: u64 = 1;

/// What can be a favorite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Headphones, a product of OPRA's, by its ID.
    Headphones,
    /// A preset of OPRA's, by its ID.
    Preset,
}

impl Kind {
    /// Both kinds, e.g. for tests.
    pub const ALL: [Self; 2] = [Self::Headphones, Self::Preset];

    /// The key of the kind's list in the file.
    const fn key(self) -> &'static str {
        match self {
            Self::Headphones => "headphones",
            Self::Preset => "presets",
        }
    }
}

/// Why a text isn't a file of favorites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// The text isn't a JSON object.
    NotFavorites,
}

/// The favorite headphones and presets, each by their IDs in OPRA's database.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Favorites {
    headphones: BTreeSet<String>,
    presets: BTreeSet<String>,
}

impl Favorites {
    /// Reads the text of the file: a JSON object with a list of IDs under `headphones` and one
    /// under `presets`. A list that is missing or isn't one counts as empty; entries that aren't
    /// strings, are empty or longer than [`MAX_ID_BYTES`] are left out, as are those after the
    /// first [`MAX_FAVORITES`] of a kind. Other keys, e.g. `version`, are passed over.
    ///
    /// # Errors
    ///
    /// If the text isn't a JSON object.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let Ok(Value::Object(object)) = serde_json::from_str::<Value>(text) else {
            return Err(ParseError::NotFavorites);
        };
        let ids = |kind: Kind| -> BTreeSet<String> {
            object
                .get(kind.key())
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|id| is_valid(id))
                .take(MAX_FAVORITES)
                .map(str::to_owned)
                .collect()
        };
        Ok(Self {
            headphones: ids(Kind::Headphones),
            presets: ids(Kind::Preset),
        })
    }

    /// The text of the file, which [`Self::parse`] reads again: its version and both lists,
    /// indented, with a newline at the end.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut object = Map::new();
        object.insert(String::from("version"), Value::from(VERSION));
        for kind in Kind::ALL {
            let ids = self.ids(kind).iter().cloned().map(Value::String).collect();
            object.insert(kind.key().to_owned(), Value::Array(ids));
        }
        format!("{:#}\n", Value::Object(object))
    }

    /// Whether the headphones or the preset with this ID is a favorite.
    #[must_use]
    pub fn contains(&self, kind: Kind, id: &str) -> bool {
        self.ids(kind).contains(id)
    }

    /// Marks the headphones or the preset with this ID as a favorite, or unmarks it, and says
    /// whether that changed anything. An ID that is empty or too long, or one more than
    /// [`MAX_FAVORITES`] of its kind, isn't marked.
    pub fn set(&mut self, kind: Kind, id: &str, favorite: bool) -> bool {
        let ids = match kind {
            Kind::Headphones => &mut self.headphones,
            Kind::Preset => &mut self.presets,
        };
        if !favorite {
            return ids.remove(id);
        }
        if !is_valid(id) || ids.len() >= MAX_FAVORITES {
            return false;
        }
        ids.insert(id.to_owned())
    }

    /// The IDs of the favorites of a kind, in their order.
    #[must_use]
    pub fn ids(&self, kind: Kind) -> &BTreeSet<String> {
        match kind {
            Kind::Headphones => &self.headphones,
            Kind::Preset => &self.presets,
        }
    }

    /// Whether there is no favorite of either kind.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.headphones.is_empty() && self.presets.is_empty()
    }
}

/// Whether an ID may be a favorite: not empty, and at most [`MAX_ID_BYTES`] long.
fn is_valid(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID_BYTES
}

/// The systems, which keep the settings of programs in different folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum System {
    /// Windows.
    Windows,
    /// macOS.
    MacOs,
    /// Linux and other Unix systems, which follow the XDG Base Directory Specification.
    Unix,
}

/// Where the file of the favorites is: in the folder where the system keeps the settings of
/// programs, `%APPDATA%\AudioRef\favorites.json` on Windows,
/// `~/Library/Application Support/AudioRef/favorites.json` on macOS, and
/// `$XDG_CONFIG_HOME/audioref/favorites.json`, by default `~/.config/audioref/favorites.json`,
/// elsewhere. `var` gives the system's environment variables. A variable that is empty or isn't
/// an absolute path is passed over, as the XDG specification asks; without any, there is no file.
#[must_use]
pub fn file(system: System, var: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let folder = |name: &str| {
        var(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let settings = match system {
        System::Windows => folder("APPDATA")?.join("AudioRef"),
        System::MacOs => folder("HOME")?
            .join("Library")
            .join("Application Support")
            .join("AudioRef"),
        System::Unix => folder("XDG_CONFIG_HOME")
            .or_else(|| folder("HOME").map(|home| home.join(".config")))?
            .join("audioref"),
    };
    Some(settings.join(FILE_NAME))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::{Favorites, Kind, MAX_FAVORITES, MAX_ID_BYTES, ParseError, System, file};

    fn parse(text: &str) -> Favorites {
        Favorites::parse(text).expect("accepted")
    }

    #[test]
    fn marks_and_unmarks_favorites() {
        let mut favorites = Favorites::default();
        assert!(favorites.is_empty());
        assert!(favorites.set(Kind::Headphones, "acme::one", true));
        assert!(!favorites.set(Kind::Headphones, "acme::one", true));
        assert!(favorites.set(Kind::Preset, "acme:one::tester", true));
        assert!(favorites.contains(Kind::Headphones, "acme::one"));
        assert!(!favorites.contains(Kind::Preset, "acme::one"));
        assert!(favorites.contains(Kind::Preset, "acme:one::tester"));
        assert!(!favorites.is_empty());
        assert!(favorites.set(Kind::Headphones, "acme::one", false));
        assert!(!favorites.set(Kind::Headphones, "acme::one", false));
        assert!(!favorites.contains(Kind::Headphones, "acme::one"));
        assert!(favorites.set(Kind::Preset, "acme:one::tester", false));
        assert!(favorites.is_empty());
    }

    #[test]
    fn refuses_ids_that_are_empty_or_too_long_and_too_many() {
        let mut favorites = Favorites::default();
        assert!(!favorites.set(Kind::Headphones, "", true));
        let long = "x".repeat(MAX_ID_BYTES + 1);
        assert!(!favorites.set(Kind::Preset, &long, true));
        assert!(favorites.set(Kind::Preset, &long[1..], true));
        for index in 0..MAX_FAVORITES {
            assert!(favorites.set(Kind::Headphones, &format!("acme::{index}"), true));
        }
        assert!(!favorites.set(Kind::Headphones, "acme::more", true));
        assert_eq!(favorites.ids(Kind::Headphones).len(), MAX_FAVORITES);
        // Unmarking still works, and makes room.
        assert!(favorites.set(Kind::Headphones, "acme::0", false));
        assert!(favorites.set(Kind::Headphones, "acme::more", true));
    }

    #[test]
    fn writes_what_it_reads_again() {
        let mut favorites = Favorites::default();
        for (kind, id) in [
            (Kind::Headphones, "acme::two"),
            (Kind::Headphones, "acme::one"),
            (Kind::Preset, "acme:one::\"quoted\" \\ ünïcode"),
        ] {
            favorites.set(kind, id, true);
        }
        let json = favorites.to_json();
        assert!(json.ends_with("}\n"), "{json}");
        assert!(json.contains("\"version\": 1"), "{json}");
        assert_eq!(parse(&json), favorites);
        assert_eq!(parse(&Favorites::default().to_json()), Favorites::default());
    }

    #[test]
    fn reads_the_file() {
        let favorites = parse(
            "\u{feff}{\"version\":7,\"headphones\":[\"acme::one\",\"acme::one\",\"\",3,null,\"acme::two\"],\"presets\":[\"acme:one::tester\"],\"later\":{}}",
        );
        let headphones: Vec<&str> = favorites
            .ids(Kind::Headphones)
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(headphones, ["acme::one", "acme::two"]);
        assert!(favorites.contains(Kind::Preset, "acme:one::tester"));
    }

    #[test]
    fn a_list_that_is_missing_or_not_one_is_empty() {
        for text in [
            "{}",
            r#"{"headphones":"acme::one","presets":{"acme:one::tester":true}}"#,
        ] {
            assert!(parse(text).is_empty(), "{text}");
        }
        let long = "x".repeat(MAX_ID_BYTES + 1);
        assert!(parse(&format!(r#"{{"presets":["{long}"]}}"#)).is_empty());
    }

    #[test]
    fn reads_at_most_so_many_of_each_kind() {
        let ids: Vec<String> = (0..=MAX_FAVORITES)
            .map(|index| format!("\"acme::{index:04}\""))
            .collect();
        let text = format!(r#"{{"headphones":[{0}],"presets":[{0}]}}"#, ids.join(","));
        let favorites = parse(&text);
        for kind in Kind::ALL {
            assert_eq!(favorites.ids(kind).len(), MAX_FAVORITES, "{kind:?}");
            assert!(!favorites.contains(kind, &format!("acme::{MAX_FAVORITES:04}")));
        }
    }

    #[test]
    fn refuses_a_text_that_is_no_object() {
        for text in ["", "not json", "[]", "null", "\"favorites\"", "{", "{} {}"] {
            assert_eq!(
                Favorites::parse(text),
                Err(ParseError::NotFavorites),
                "{text}"
            );
        }
    }

    /// A folder that is absolute on every system the tests run on.
    fn absolute() -> PathBuf {
        std::env::temp_dir()
    }

    /// The environment variables of a test: these, and no others.
    fn vars(set: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let set: Vec<(String, OsString)> = set
            .iter()
            .map(|(name, value)| ((*name).to_owned(), OsString::from(value)))
            .collect();
        move |name| {
            set.iter()
                .find(|(set, _)| set == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn the_file_is_where_each_system_keeps_settings() {
        let base = absolute();
        let text = base.to_str().expect("a temporary folder in UTF-8");
        let all = vars(&[("APPDATA", text), ("HOME", text), ("XDG_CONFIG_HOME", text)]);
        assert_eq!(
            file(System::Windows, &all),
            Some(base.join("AudioRef").join("favorites.json"))
        );
        assert_eq!(
            file(System::MacOs, &all),
            Some(
                base.join("Library")
                    .join("Application Support")
                    .join("AudioRef")
                    .join("favorites.json")
            )
        );
        assert_eq!(
            file(System::Unix, &all),
            Some(base.join("audioref").join("favorites.json"))
        );
        let home = vars(&[("HOME", text), ("XDG_CONFIG_HOME", "")]);
        assert_eq!(
            file(System::Unix, &home),
            Some(base.join(".config").join("audioref").join("favorites.json"))
        );
    }

    #[test]
    fn no_file_without_an_absolute_folder() {
        let relative = vars(&[
            ("APPDATA", "relative"),
            ("HOME", "relative"),
            ("XDG_CONFIG_HOME", "relative/config"),
        ]);
        let empty = vars(&[("APPDATA", ""), ("HOME", "")]);
        for system in [System::Windows, System::MacOs, System::Unix] {
            assert_eq!(file(system, &relative), None, "{system:?}");
            assert_eq!(file(system, &empty), None, "{system:?}");
            assert_eq!(file(system, &vars(&[])), None, "{system:?}");
        }
    }
}
