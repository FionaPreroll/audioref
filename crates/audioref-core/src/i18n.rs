//! The languages of AudioRef's texts: English and German.
//!
//! Every text that users see is a typed text: an enum with a variant for each text, and one
//! function per language with an exhaustive `match`. A text without a translation doesn't compile,
//! nothing is parsed at run time, and the tests reach every text in every language:
//!
//! ```
//! use audioref_core::i18n::{Lang, Param};
//!
//! assert_eq!(Param::Output.name(Lang::from_locale("de-AT")), "Ausgang");
//! assert_eq!(Param::Output.name(Lang::from_locale("en-GB")), "Output");
//! ```

/// A language of AudioRef's texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    /// English, also for every language that AudioRef doesn't speak.
    En,
    /// German.
    De,
}

impl Lang {
    /// All languages, e.g. for tests that every text exists in each of them.
    pub const ALL: [Self; 2] = [Self::En, Self::De];

    /// The language for a locale, e.g. `de-DE` as the operating system reports it, or a tag like
    /// `de_CH.UTF-8`: German for German locales, English for all others.
    #[must_use]
    pub fn from_locale(locale: &str) -> Self {
        match locale.split(['-', '_', '.']).next() {
            Some(language) if language.eq_ignore_ascii_case("de") => Self::De,
            _ => Self::En,
        }
    }
}

/// The names of the plugin's parameters, as hosts show them, e.g. in automation lanes.
///
/// Hosts store parameters by their IDs, which never change, not by these names: a project saved
/// in one language opens in the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Param {
    /// The output level, in dB.
    Output,
    /// Passes the signal through unchanged.
    Bypass,
    /// Bypasses AudioRef while the host exports offline, see `crate::render`.
    BypassOnExport,
}

impl Param {
    /// All parameters, e.g. for tests.
    pub const ALL: [Self; 3] = [Self::Output, Self::Bypass, Self::BypassOnExport];

    /// The parameter's name in a language.
    #[must_use]
    pub fn name(self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.en(),
            Lang::De => self.de(),
        }
    }

    fn en(self) -> &'static str {
        match self {
            Self::Output => "Output",
            Self::Bypass => "Bypass",
            Self::BypassOnExport => "Bypass on export",
        }
    }

    fn de(self) -> &'static str {
        match self {
            Self::Output => "Ausgang",
            Self::Bypass => "Umgehen",
            Self::BypassOnExport => "Beim Export umgehen",
        }
    }
}

/// Notices the plugin shows, e.g. in its window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Notice {
    /// The terms of use: AudioRef and the headphone data it reads are for private use only.
    NonCommercial,
    /// The correction belongs in the monitoring path, not in an exported mix.
    MonitoringOnly,
}

impl Notice {
    /// All notices, e.g. for tests.
    pub const ALL: [Self; 2] = [Self::NonCommercial, Self::MonitoringOnly];

    /// The notice in a language.
    #[must_use]
    pub fn text(self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.en(),
            Lang::De => self.de(),
        }
    }

    fn en(self) -> &'static str {
        match self {
            Self::NonCommercial => "Private, non-commercial use only",
            Self::MonitoringOnly => {
                "For the monitoring path only: bypass AudioRef before you export a mix or master."
            }
        }
    }

    fn de(self) -> &'static str {
        match self {
            Self::NonCommercial => "Nur für private, nicht-kommerzielle Nutzung",
            Self::MonitoringOnly => {
                "Nur für den Abhörweg: Umgehe AudioRef, bevor du einen Mix oder ein Master exportierst."
            }
        }
    }
}

/// The fixed texts of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Text {
    /// The heading of the part where OPRA's file is loaded.
    Database,
    /// What the file is for, that users download it themselves, and how to go back to the
    /// built-in presets.
    DatabaseHint,
    /// The button that loads the file.
    Load,
    /// While the file loads.
    Loading,
    /// The file is no database of OPRA's.
    NotOpra,
    /// The file couldn't be read; the system's reason follows.
    ReadFailed,
    /// The hint in the search field.
    Search,
    /// The search found no headphones.
    NoMatches,
    /// The heading of the presets of the chosen headphones.
    Presets,
    /// The heading of the current correction.
    Correction,
    /// No correction is chosen.
    NoCorrection,
    /// The button that turns the correction off.
    TurnOff,
}

impl Text {
    /// All texts, e.g. for tests.
    pub const ALL: [Self; 12] = [
        Self::Database,
        Self::DatabaseHint,
        Self::Load,
        Self::Loading,
        Self::NotOpra,
        Self::ReadFailed,
        Self::Search,
        Self::NoMatches,
        Self::Presets,
        Self::Correction,
        Self::NoCorrection,
        Self::TurnOff,
    ];

    /// The text in a language.
    #[must_use]
    pub fn text(self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.en(),
            Lang::De => self.de(),
        }
    }

    fn en(self) -> &'static str {
        match self {
            Self::Database => "OPRA database",
            Self::DatabaseHint => {
                "Optional, for more headphones: the path of database_v1.jsonl from OPRA's repository, which you downloaded yourself. Load with an empty path to go back to the built-in presets."
            }
            Self::Load => "Load",
            Self::Loading => "Loading…",
            Self::NotOpra => "This file isn't OPRA's database.",
            Self::ReadFailed => "The file couldn't be read:",
            Self::Search => "Search headphones",
            Self::NoMatches => "No headphones match.",
            Self::Presets => "Presets",
            Self::Correction => "Correction",
            Self::NoCorrection => "No correction: the signal passes unchanged.",
            Self::TurnOff => "Off",
        }
    }

    fn de(self) -> &'static str {
        match self {
            Self::Database => "OPRA-Datenbank",
            Self::DatabaseHint => {
                "Optional, für weitere Kopfhörer: der Pfad von database_v1.jsonl aus OPRAs Repository, die du selbst heruntergeladen hast. Laden mit leerem Pfad führt zurück zu den eingebauten Presets."
            }
            Self::Load => "Laden",
            Self::Loading => "Wird geladen …",
            Self::NotOpra => "Diese Datei ist nicht OPRAs Datenbank.",
            Self::ReadFailed => "Die Datei ließ sich nicht lesen:",
            Self::Search => "Kopfhörer suchen",
            Self::NoMatches => "Keine Kopfhörer passen.",
            Self::Presets => "Presets",
            Self::Correction => "Korrektur",
            Self::NoCorrection => "Keine Korrektur: Das Signal bleibt unverändert.",
            Self::TurnOff => "Aus",
        }
    }
}

/// The texts of the window with numbers or names in them.
#[derive(Debug, Clone, PartialEq)]
pub enum Message<'a> {
    /// The database was loaded, with so many presets for so many headphones.
    Loaded {
        /// The number of presets.
        presets: usize,
        /// The number of products.
        headphones: usize,
    },
    /// The built-in presets are in use: so many for so many headphones, see [`crate::bundled`].
    BuiltIn {
        /// The number of presets.
        presets: usize,
        /// The number of products.
        headphones: usize,
    },
    /// So many lines of the database were left out.
    Skipped(usize),
    /// The correction's gain was lowered by so many dB, see [`crate::cascade`].
    Headroom(f64),
    /// Whose preset it is and where it comes from, as OPRA's license asks.
    Attribution {
        /// Who made it.
        author: &'a str,
        /// What it is based on; may be empty.
        details: &'a str,
    },
}

impl Message<'_> {
    /// The text in a language.
    #[must_use]
    pub fn text(&self, lang: Lang) -> String {
        match (self, lang) {
            (
                Self::Loaded {
                    presets,
                    headphones,
                },
                Lang::En,
            ) => format!(
                "{} presets for {} headphones",
                number(*presets, lang),
                number(*headphones, lang)
            ),
            (
                Self::Loaded {
                    presets,
                    headphones,
                },
                Lang::De,
            ) => format!(
                "{} Presets für {} Kopfhörer",
                number(*presets, lang),
                number(*headphones, lang)
            ),
            (
                Self::BuiltIn {
                    presets,
                    headphones,
                },
                Lang::En,
            ) => format!(
                "Built in: {} presets for {} headphones",
                number(*presets, lang),
                number(*headphones, lang)
            ),
            (
                Self::BuiltIn {
                    presets,
                    headphones,
                },
                Lang::De,
            ) => format!(
                "Eingebaut: {} Presets für {} Kopfhörer",
                number(*presets, lang),
                number(*headphones, lang)
            ),
            (Self::Skipped(lines), Lang::En) => {
                format!(
                    "{} lines left out, which AudioRef doesn't understand",
                    number(*lines, lang)
                )
            }
            (Self::Skipped(lines), Lang::De) => {
                format!(
                    "{} Zeilen ausgelassen, die AudioRef nicht versteht",
                    number(*lines, lang)
                )
            }
            (Self::Headroom(db), Lang::En) => {
                format!("Lowered by {} dB, so it boosts nothing", decimal(*db, lang))
            }
            (Self::Headroom(db), Lang::De) => {
                format!(
                    "Um {} dB abgesenkt, damit nichts lauter wird",
                    decimal(*db, lang)
                )
            }
            (Self::Attribution { author, details }, Lang::En) => {
                format!(
                    "EQ by {author}{}, from OPRA, CC BY-SA 4.0",
                    in_parentheses(details)
                )
            }
            (Self::Attribution { author, details }, Lang::De) => {
                format!(
                    "EQ von {author}{}, aus OPRA, CC BY-SA 4.0",
                    in_parentheses(details)
                )
            }
        }
    }
}

/// A count, with the thousands separated as the language does: 12,594 or 12.594.
fn number(count: usize, lang: Lang) -> String {
    let separator = match lang {
        Lang::En => ',',
        Lang::De => '.',
    };
    let digits = count.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(separator);
        }
        grouped.push(digit);
    }
    grouped
}

/// A number with one decimal, with the language's decimal mark: 2.5 or 2,5.
fn decimal(value: f64, lang: Lang) -> String {
    let text = format!("{value:.1}");
    match lang {
        Lang::En => text,
        Lang::De => text.replace('.', ","),
    }
}

/// " (details)", or nothing for empty details.
fn in_parentheses(details: &str) -> String {
    if details.is_empty() {
        String::new()
    } else {
        format!(" ({details})")
    }
}

#[cfg(test)]
mod tests {
    use super::{Lang, Message, Notice, Param, Text};

    #[test]
    fn german_locales() {
        for locale in ["de", "DE", "de-AT", "de_CH", "de_DE.UTF-8", "de-DE-1996"] {
            assert_eq!(Lang::from_locale(locale), Lang::De, "{locale}");
        }
    }

    #[test]
    fn english_for_all_other_locales() {
        for locale in ["en", "en-US", "fr-FR", "dex", "", "-de", "C", "POSIX"] {
            assert_eq!(Lang::from_locale(locale), Lang::En, "{locale}");
        }
    }

    #[test]
    fn every_parameter_has_a_name_in_every_language() {
        for lang in Lang::ALL {
            for param in Param::ALL {
                assert!(!param.name(lang).is_empty(), "{param:?} in {lang:?}");
            }
        }
        assert_eq!(Param::Output.name(Lang::De), "Ausgang");
        assert_eq!(Param::Bypass.name(Lang::En), "Bypass");
    }

    #[test]
    fn names_differ_between_parameters() {
        for lang in Lang::ALL {
            for (index, param) in Param::ALL.iter().enumerate() {
                for other in Param::ALL.iter().skip(index + 1) {
                    assert_ne!(param.name(lang), other.name(lang), "{lang:?}");
                }
            }
        }
    }

    #[test]
    fn every_notice_exists_in_every_language() {
        for notice in Notice::ALL {
            assert_ne!(notice.text(Lang::En), notice.text(Lang::De), "{notice:?}");
            for lang in Lang::ALL {
                assert!(!notice.text(lang).is_empty(), "{notice:?} in {lang:?}");
            }
        }
        assert!(
            Notice::NonCommercial
                .text(Lang::En)
                .contains("non-commercial")
        );
        assert!(
            Notice::NonCommercial
                .text(Lang::De)
                .contains("nicht-kommerzielle")
        );
    }

    #[test]
    fn every_text_exists_in_every_language() {
        for text in Text::ALL {
            for lang in Lang::ALL {
                assert!(!text.text(lang).is_empty(), "{text:?} in {lang:?}");
            }
        }
        assert_eq!(Text::Load.text(Lang::De), "Laden");
    }

    #[test]
    fn messages_with_numbers_and_names() {
        let loaded = Message::Loaded {
            presets: 12_594,
            headphones: 6230,
        };
        assert_eq!(loaded.text(Lang::En), "12,594 presets for 6,230 headphones");
        assert_eq!(loaded.text(Lang::De), "12.594 Presets für 6.230 Kopfhörer");
        let few = Message::Loaded {
            presets: 7,
            headphones: 1_000_000,
        };
        assert_eq!(few.text(Lang::En), "7 presets for 1,000,000 headphones");
        let built_in = Message::BuiltIn {
            presets: 20,
            headphones: 7,
        };
        assert_eq!(
            built_in.text(Lang::En),
            "Built in: 20 presets for 7 headphones"
        );
        assert_eq!(
            built_in.text(Lang::De),
            "Eingebaut: 20 Presets für 7 Kopfhörer"
        );
        assert_eq!(
            Message::Skipped(3).text(Lang::En),
            "3 lines left out, which AudioRef doesn't understand"
        );
        assert_eq!(
            Message::Skipped(3).text(Lang::De),
            "3 Zeilen ausgelassen, die AudioRef nicht versteht"
        );
        assert_eq!(
            Message::Headroom(2.46).text(Lang::En),
            "Lowered by 2.5 dB, so it boosts nothing"
        );
        assert_eq!(
            Message::Headroom(2.46).text(Lang::De),
            "Um 2,5 dB abgesenkt, damit nichts lauter wird"
        );
        let full = Message::Attribution {
            author: "oratory1990",
            details: "Harman Target",
        };
        assert_eq!(
            full.text(Lang::En),
            "EQ by oratory1990 (Harman Target), from OPRA, CC BY-SA 4.0"
        );
        let bare = Message::Attribution {
            author: "AutoEQ",
            details: "",
        };
        assert_eq!(bare.text(Lang::De), "EQ von AutoEQ, aus OPRA, CC BY-SA 4.0");
    }
}
