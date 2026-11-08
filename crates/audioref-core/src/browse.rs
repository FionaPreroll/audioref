//! What the window's file dialog asks for: its title, the files it lists, and the folder it opens
//! in. The plugin opens the system's dialog with these, see `crates/audioref/src/browse.rs`.

use std::path::Path;

use crate::i18n::{Lang, Text};

/// The files the dialog lists when every file may be picked.
pub const ALL_FILES: &[&str] = &["*"];

/// A file the window picks with the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pick {
    /// OPRA's `database_v1.jsonl`.
    Database,
    /// The measurement of the headphones.
    Measurement,
    /// The target.
    Target,
}

impl Pick {
    /// All files the window picks, e.g. for tests.
    pub const ALL: [Self; 3] = [Self::Database, Self::Measurement, Self::Target];

    /// The title of the dialog.
    #[must_use]
    pub const fn title(self) -> Text {
        match self {
            Self::Database => Text::PickDatabase,
            Self::Measurement => Text::PickMeasurement,
            Self::Target => Text::PickTarget,
        }
    }

    /// The name of the kind of files the dialog lists first, in a language, and their
    /// extensions, without dots.
    #[must_use]
    pub fn filter(self, lang: Lang) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Database => (Text::DatabaseFiles.text(lang), &["jsonl", "json"]),
            Self::Measurement | Self::Target => {
                (Text::CurveFiles.text(lang), &["csv", "txt", "tsv"])
            }
        }
    }
}

/// The folder the dialog opens in: that of the file in the field, if it names one. Whether the
/// folder exists is up to the plugin, which may look.
#[must_use]
pub fn folder_of(field: &str) -> Option<&Path> {
    let path = field.trim();
    if path.is_empty() {
        return None;
    }
    Path::new(path)
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ALL_FILES, Pick, folder_of};
    use crate::i18n::{Lang, Text};

    #[test]
    fn every_pick_has_a_title_and_files() {
        for pick in Pick::ALL {
            for lang in Lang::ALL {
                assert!(!pick.title().text(lang).is_empty(), "{pick:?} in {lang:?}");
                let (name, extensions) = pick.filter(lang);
                assert!(!name.is_empty(), "{pick:?} in {lang:?}");
                assert!(!extensions.is_empty(), "{pick:?}");
                for extension in extensions {
                    assert!(!extension.starts_with('.'), "{extension}");
                }
            }
        }
        assert_eq!(Pick::Database.filter(Lang::En).1.first(), Some(&"jsonl"));
        assert_eq!(Pick::Target.filter(Lang::De).1.first(), Some(&"csv"));
        assert_eq!(Pick::Measurement.title(), Text::PickMeasurement);
        assert_eq!(ALL_FILES, ["*"]);
    }

    #[test]
    fn titles_differ() {
        for lang in Lang::ALL {
            for (index, pick) in Pick::ALL.iter().enumerate() {
                for other in Pick::ALL.iter().skip(index + 1) {
                    assert_ne!(pick.title().text(lang), other.title().text(lang));
                }
            }
        }
    }

    #[test]
    fn the_folder_of_the_file_in_the_field() {
        assert_eq!(
            folder_of("/home/me/opra/database_v1.jsonl"),
            Some(Path::new("/home/me/opra"))
        );
        assert_eq!(
            folder_of("  /home/me/target.csv \n"),
            Some(Path::new("/home/me"))
        );
        assert_eq!(folder_of("/target.csv"), Some(Path::new("/")));
    }

    #[test]
    fn no_folder_without_one_in_the_field() {
        for field in ["", "   ", "target.csv", "/"] {
            assert_eq!(folder_of(field), None, "{field:?}");
        }
    }
}
