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

use crate::stereo::{Channels, Crossfeed};

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
    /// The routing of the channels, see [`Channels`].
    Channels,
    /// The strength of the crossfeed, see [`Crossfeed`].
    Crossfeed,
}

impl Param {
    /// All parameters, e.g. for tests.
    pub const ALL: [Self; 5] = [
        Self::Output,
        Self::Bypass,
        Self::BypassOnExport,
        Self::Channels,
        Self::Crossfeed,
    ];

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
            Self::Channels => "Channels",
            Self::Crossfeed => "Crossfeed",
        }
    }

    fn de(self) -> &'static str {
        match self {
            Self::Output => "Ausgang",
            Self::Bypass => "Umgehen",
            Self::BypassOnExport => "Beim Export umgehen",
            Self::Channels => "Kanäle",
            Self::Crossfeed => "Crossfeed",
        }
    }
}

impl Channels {
    /// The name of the routing, as the host and the window show it.
    #[must_use]
    pub fn text(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Self::Stereo, _) => "Stereo",
            (Self::Mono, _) => "Mono",
            (Self::Swapped, Lang::En) => "Swapped",
            (Self::Swapped, Lang::De) => "Vertauscht",
        }
    }

    /// The routing a user typed into the host: its name in either language, in any case, or its
    /// index.
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        find(&Self::ALL, text, &|channels, lang| channels.text(lang)).map(Self::from_index)
    }
}

impl Crossfeed {
    /// The name of the strength, as the host and the window show it.
    #[must_use]
    pub fn text(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Self::Off, Lang::En) => "Off",
            (Self::Off, Lang::De) => "Aus",
            (Self::Light, Lang::En) => "Light",
            (Self::Light, Lang::De) => "Leicht",
            (Self::Medium, Lang::En) => "Medium",
            (Self::Medium, Lang::De) => "Mittel",
            (Self::Strong, Lang::En) => "Strong",
            (Self::Strong, Lang::De) => "Stark",
        }
    }

    /// The strength a user typed into the host: its name in either language, in any case, or its
    /// index.
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        find(&Self::ALL, text, &|crossfeed, lang| crossfeed.text(lang)).map(Self::from_index)
    }
}

/// The index of the choice whose name in some language is `text`, or of the index `text` names.
fn find<T: Copy>(choices: &[T], text: &str, name: &dyn Fn(T, Lang) -> &'static str) -> Option<i32> {
    let text = text.trim();
    let named = (0..).zip(choices).find_map(|(index, choice)| {
        Lang::ALL
            .iter()
            .any(|lang| name(*choice, *lang).eq_ignore_ascii_case(text))
            .then_some(index)
    });
    named.or_else(|| {
        text.parse::<i32>()
            .ok()
            .filter(|index| usize::try_from(*index).is_ok_and(|index| index < choices.len()))
    })
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
    /// The tab with OPRA's presets.
    SourceOpra,
    /// The tab with the own measurement and target.
    SourceCurves,
    /// What the files of the own curves are, and that users download them themselves.
    CurvesHint,
    /// The label of the measurement's file.
    Measurement,
    /// The label of the target's file.
    Target,
    /// The file has no lines with a frequency and a level.
    NotCurve,
    /// The file has more points than AudioRef takes, see [`crate::autoeq::MAX_POINTS`].
    TooManyPoints,
    /// The button that makes the measurement and the target the correction.
    UseCurves,
    /// The name of the tilt, see [`crate::design::Settings`].
    Tilt,
    /// The name of the bass shelf.
    Bass,
    /// The name of the limit for boosts.
    MaxBoost,
    /// The unit of the tilt, with a space before it.
    DbPerOctave,
}

impl Text {
    /// All texts, e.g. for tests.
    pub const ALL: [Self; 24] = [
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
        Self::SourceOpra,
        Self::SourceCurves,
        Self::CurvesHint,
        Self::Measurement,
        Self::Target,
        Self::NotCurve,
        Self::TooManyPoints,
        Self::UseCurves,
        Self::Tilt,
        Self::Bass,
        Self::MaxBoost,
        Self::DbPerOctave,
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
            Self::SourceOpra => "OPRA presets",
            Self::SourceCurves => "Own curves (AutoEQ)",
            Self::CurvesHint => {
                "A measurement of your headphones and a target, as CSV files that you downloaded yourself, e.g. from AutoEQ's repository: measurements/oratory1990/data/over-ear/… and targets/…."
            }
            Self::Measurement => "Measurement",
            Self::Target => "Target",
            Self::NotCurve => {
                "This file isn't a curve: it has no lines with a frequency and a level."
            }
            Self::TooManyPoints => "This file has more than 100,000 points.",
            Self::UseCurves => "Use as correction",
            Self::Tilt => "Tilt",
            Self::Bass => "Bass",
            Self::MaxBoost => "Max. boost",
            Self::DbPerOctave => " dB/oct",
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
            Self::SourceOpra => "OPRA-Presets",
            Self::SourceCurves => "Eigene Kurven (AutoEQ)",
            Self::CurvesHint => {
                "Eine Messung deiner Kopfhörer und eine Zielkurve, als CSV-Dateien, die du selbst heruntergeladen hast, z. B. aus AutoEQs Repository: measurements/oratory1990/data/over-ear/… und targets/…."
            }
            Self::Measurement => "Messung",
            Self::Target => "Zielkurve",
            Self::NotCurve => {
                "Diese Datei ist keine Kurve: Sie hat keine Zeilen mit Frequenz und Pegel."
            }
            Self::TooManyPoints => "Diese Datei hat mehr als 100.000 Punkte.",
            Self::UseCurves => "Als Korrektur verwenden",
            Self::Tilt => "Neigung",
            Self::Bass => "Bass",
            Self::MaxBoost => "Max. Anhebung",
            Self::DbPerOctave => " dB/Okt.",
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

/// The texts of the window about the curves of a measurement and a target.
#[derive(Debug, Clone, PartialEq)]
pub enum CurveMessage<'a> {
    /// A curve was loaded, with so many points.
    Points(usize),
    /// The curve doesn't reach across [`crate::autoeq::REQUIRED_HZ`].
    TooNarrow {
        /// Its lowest frequency, in Hz.
        from_hz: f64,
        /// Its highest frequency, in Hz.
        to_hz: f64,
    },
    /// The correction designed from a measurement and a target, by their names.
    Designed {
        /// The name of the measurement.
        measurement: &'a str,
        /// The name of the target.
        target: &'a str,
    },
}

impl CurveMessage<'_> {
    /// The text in a language.
    #[must_use]
    pub fn text(&self, lang: Lang) -> String {
        match (self, lang) {
            (Self::Points(points), Lang::En) => format!("{} points", number(*points, lang)),
            (Self::Points(points), Lang::De) => format!("{} Punkte", number(*points, lang)),
            (Self::TooNarrow { from_hz, to_hz }, Lang::En) => format!(
                "The curve only reaches from {} to {} Hz, but needs 100 to 10,000 Hz.",
                hertz(*from_hz, lang),
                hertz(*to_hz, lang)
            ),
            (Self::TooNarrow { from_hz, to_hz }, Lang::De) => format!(
                "Die Kurve reicht nur von {} bis {} Hz, braucht aber 100 bis 10.000 Hz.",
                hertz(*from_hz, lang),
                hertz(*to_hz, lang)
            ),
            (
                Self::Designed {
                    measurement,
                    target,
                },
                Lang::En,
            ) => format!("{measurement}, corrected to {target}"),
            (
                Self::Designed {
                    measurement,
                    target,
                },
                Lang::De,
            ) => format!("{measurement}, korrigiert auf {target}"),
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

/// A frequency in whole Hz, with the thousands separated as the language does.
fn hertz(frequency: f64, lang: Lang) -> String {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "frequencies of curves, from 1 Hz to 100 kHz"
    )]
    let whole = frequency.round().max(0.0) as usize;
    number(whole, lang)
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
    use super::{CurveMessage, Lang, Message, Notice, Param, Text};
    use crate::stereo::{Channels, Crossfeed};

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
        assert_eq!(CurveMessage::Points(695).text(Lang::En), "695 points");
        assert_eq!(CurveMessage::Points(1695).text(Lang::De), "1.695 Punkte");
        let narrow = CurveMessage::TooNarrow {
            from_hz: 199.6,
            to_hz: 20_000.0,
        };
        assert_eq!(
            narrow.text(Lang::En),
            "The curve only reaches from 200 to 20,000 Hz, but needs 100 to 10,000 Hz."
        );
        assert_eq!(
            narrow.text(Lang::De),
            "Die Kurve reicht nur von 200 bis 20.000 Hz, braucht aber 100 bis 10.000 Hz."
        );
        let designed = CurveMessage::Designed {
            measurement: "AKG K240 Studio",
            target: "Harman over-ear 2018",
        };
        assert_eq!(
            designed.text(Lang::En),
            "AKG K240 Studio, corrected to Harman over-ear 2018"
        );
        assert_eq!(
            designed.text(Lang::De),
            "AKG K240 Studio, korrigiert auf Harman over-ear 2018"
        );
    }

    #[test]
    fn choices_have_names_and_hosts_find_them_again() {
        for lang in Lang::ALL {
            for channels in Channels::ALL {
                assert_eq!(Channels::from_text(channels.text(lang)), Some(channels));
                let index = channels.index().to_string();
                assert_eq!(Channels::from_text(&index), Some(channels));
            }
            for crossfeed in Crossfeed::ALL {
                assert_eq!(Crossfeed::from_text(crossfeed.text(lang)), Some(crossfeed));
                let index = crossfeed.index().to_string();
                assert_eq!(Crossfeed::from_text(&index), Some(crossfeed));
            }
        }
        assert_eq!(Channels::from_text(" vertauscht "), Some(Channels::Swapped));
        assert_eq!(Crossfeed::from_text("STRONG"), Some(Crossfeed::Strong));
        assert_eq!(Channels::Swapped.text(Lang::De), "Vertauscht");
        assert_eq!(Crossfeed::Off.text(Lang::De), "Aus");
        for wrong in ["", "3", "-1", "wide", "99999999999"] {
            assert_eq!(Channels::from_text(wrong), None, "{wrong}");
        }
        assert_eq!(Crossfeed::from_text("4"), None);
    }
}
