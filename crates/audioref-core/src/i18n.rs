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
    /// Turns the limiter on or off, see [`crate::limiter`].
    Limiter,
}

impl Param {
    /// All parameters, e.g. for tests.
    pub const ALL: [Self; 6] = [
        Self::Output,
        Self::Bypass,
        Self::BypassOnExport,
        Self::Channels,
        Self::Crossfeed,
        Self::Limiter,
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
            Self::Limiter => "Limiter",
        }
    }

    fn de(self) -> &'static str {
        match self {
            Self::Output => "Ausgang",
            Self::Bypass => "Umgehen",
            Self::BypassOnExport => "Beim Export umgehen",
            Self::Channels => "Kanäle",
            Self::Crossfeed => "Crossfeed",
            Self::Limiter => "Limiter",
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
    /// The heading of the current correction and its curve.
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
    /// The label of the display of the output level.
    Level,
    /// The unit of the tilt, with a space before it.
    DbPerOctave,
}

impl Text {
    /// All texts, e.g. for tests.
    pub const ALL: [Self; 25] = [
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
        Self::Level,
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
            Self::Correction => "Frequency response of the correction EQ",
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
            Self::Level => "Level",
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
            Self::Correction => "Frequenzgang des Korrektur-EQ",
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
            Self::Level => "Pegel",
            Self::DbPerOctave => " dB/Okt.",
        }
    }
}

/// Hints in context: what a control does, shown when the pointer rests on it, and what to do next
/// where the window waits for something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hint {
    /// The field of OPRA's file and its button.
    LoadDatabase,
    /// The search for headphones.
    Search,
    /// No headphones chosen yet: where to go on.
    PickHeadphones,
    /// A preset in the list.
    Preset,
    /// The button that turns the correction off.
    TurnOff,
    /// The curve of the correction.
    Curve,
    /// The correction was lowered by its largest boost.
    Headroom,
    /// The field of a measurement or a target and its button.
    LoadCurve,
    /// The button that makes the curves the correction.
    UseCurves,
    /// That button, while a curve is missing.
    LoadBoth,
    /// The slider of the tilt.
    Tilt,
    /// The slider of the bass.
    Bass,
    /// The slider of the limit for boosts.
    MaxBoost,
    /// The output level.
    Output,
    /// The bypass.
    Bypass,
    /// The render guard.
    BypassOnExport,
    /// The routing of the channels.
    Channels,
    /// The crossfeed.
    Crossfeed,
    /// The display of the output level.
    Level,
    /// The limiter and its display.
    Limiter,
    /// The limiter lowers the signal by much: what to do.
    LowerOutput,
    /// A click on a display turns the displays off.
    MetersOn,
    /// The displays are off; a click turns them on.
    MetersOff,
}

impl Hint {
    /// All hints, e.g. for tests.
    pub const ALL: [Self; 23] = [
        Self::LoadDatabase,
        Self::Search,
        Self::PickHeadphones,
        Self::Preset,
        Self::TurnOff,
        Self::Curve,
        Self::Headroom,
        Self::LoadCurve,
        Self::UseCurves,
        Self::LoadBoth,
        Self::Tilt,
        Self::Bass,
        Self::MaxBoost,
        Self::Output,
        Self::Bypass,
        Self::BypassOnExport,
        Self::Channels,
        Self::Crossfeed,
        Self::Level,
        Self::Limiter,
        Self::LowerOutput,
        Self::MetersOn,
        Self::MetersOff,
    ];

    /// The hint in a language.
    #[must_use]
    pub fn text(self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.en(),
            Lang::De => self.de(),
        }
    }

    fn en(self) -> &'static str {
        match self {
            Self::LoadDatabase => {
                "Reads OPRA's database_v1.jsonl at this path, for headphones that aren't built in. AudioRef remembers the path. With an empty field, it goes back to the built-in presets."
            }
            Self::Search => {
                "Type part of the name, e.g. WH-1000XM4. Headphones with several modes, e.g. ANC on and off, are listed once for each: pick the one you listen in."
            }
            Self::PickHeadphones => "Pick your headphones on the left, then one of their presets.",
            Self::Preset => {
                "A click makes this preset the correction. Presets by oratory1990, or \"Measured by oratory1990\", are a good start."
            }
            Self::TurnOff => {
                "Turns the correction off: the signal passes unchanged. The other settings stay."
            }
            Self::Curve => {
                "What the correction does at each frequency, from 20 Hz to 20 kHz. It is lowered so it boosts nothing."
            }
            Self::Headroom => {
                "The correction lifts some frequencies, so AudioRef lowers it as a whole by as much: nothing gets louder or clips. Raise the output level to make up for it."
            }
            Self::LoadCurve => {
                "Reads the CSV file at this path: a column of frequencies and one of levels, as in AutoEQ's repository."
            }
            Self::UseCurves => {
                "Computes the correction from the measurement and the target: smoothed, nothing above 16 kHz, boosts limited. It replaces the preset."
            }
            Self::LoadBoth => "Load a measurement and a target first.",
            Self::Tilt => {
                "Tilts the sound around 1 kHz: below 0 darker, above 0 brighter. 0.5 dB per octave is already clearly audible."
            }
            Self::Bass => {
                "Raises or lowers the bass below about 100 Hz, with the shelf of the Harman target."
            }
            Self::MaxBoost => {
                "How much the correction may lift frequencies your headphones lack. More boost lowers the whole level more, and may sound strained."
            }
            Self::Output => {
                "The level after the correction, from −24 to +6 dB. The limiter catches the peaks if you raise it."
            }
            Self::Bypass => "Passes the signal through unchanged, to compare.",
            Self::BypassOnExport => {
                "Bypasses AudioRef while your DAW exports offline, so the headphone correction doesn't end up in your mix. Leave it on."
            }
            Self::Channels => {
                "Mono: both channels in both ears, to check a mix in mono. Swapped: left and right exchanged, e.g. to check your headphones."
            }
            Self::Crossfeed => {
                "Each ear hears some of the other channel's bass, as from speakers, so wide mixes tire less. Light, Medium and Strong feed it 9.5, 6 and 4.5 dB lower."
            }
            Self::Level => {
                "The peak level that leaves AudioRef, in dB of full scale, from −60 dB. With the limiter on, it stays at or below −0.3 dB."
            }
            Self::Limiter => {
                "Keeps the output at or below −0.3 dBFS, without latency. Its bar shows by how much it lowers the signal: it only moves when the level would go above."
            }
            Self::LowerOutput => "Lower the output level",
            Self::MetersOn => {
                "Click to turn both displays off, e.g. on slow graphics: the window is then only drawn again when something in it changes."
            }
            Self::MetersOff => {
                "The displays are off, so the window isn't drawn again and again. Click to turn them on."
            }
        }
    }

    fn de(self) -> &'static str {
        match self {
            Self::LoadDatabase => {
                "Liest OPRAs database_v1.jsonl unter diesem Pfad, für Kopfhörer, die nicht eingebaut sind. AudioRef merkt sich den Pfad. Mit leerem Feld geht es zurück zu den eingebauten Presets."
            }
            Self::Search => {
                "Tippe einen Teil des Namens, z. B. WH-1000XM4. Kopfhörer mit mehreren Modi, z. B. ANC an und aus, stehen für jeden einmal da: Wähle den, mit dem du hörst."
            }
            Self::PickHeadphones => "Wähle links deine Kopfhörer, dann eines ihrer Presets.",
            Self::Preset => {
                "Ein Klick macht dieses Preset zur Korrektur. Presets von oratory1990 oder „Measured by oratory1990“ sind ein guter Anfang."
            }
            Self::TurnOff => {
                "Schaltet die Korrektur aus: Das Signal bleibt unverändert. Die übrigen Einstellungen bleiben."
            }
            Self::Curve => {
                "Was die Korrektur bei jeder Frequenz tut, von 20 Hz bis 20 kHz. Sie ist abgesenkt, damit sie nichts anhebt."
            }
            Self::Headroom => {
                "Die Korrektur hebt manche Frequenzen an, daher senkt AudioRef sie insgesamt um so viel ab: Nichts wird lauter oder übersteuert. Hebe den Ausgang an, um das auszugleichen."
            }
            Self::LoadCurve => {
                "Liest die CSV-Datei unter diesem Pfad: eine Spalte mit Frequenzen und eine mit Pegeln, wie in AutoEQs Repository."
            }
            Self::UseCurves => {
                "Berechnet die Korrektur aus Messung und Zielkurve: geglättet, nichts über 16 kHz, Anhebungen begrenzt. Sie ersetzt das Preset."
            }
            Self::LoadBoth => "Lade zuerst eine Messung und eine Zielkurve.",
            Self::Tilt => {
                "Neigt den Klang um 1 kHz: unter 0 dunkler, über 0 heller. 0,5 dB pro Oktave sind schon deutlich hörbar."
            }
            Self::Bass => {
                "Hebt oder senkt den Bass unter etwa 100 Hz, mit dem Shelf-Filter der Harman-Zielkurve."
            }
            Self::MaxBoost => {
                "Wie weit die Korrektur Frequenzen anheben darf, die deinen Kopfhörern fehlen. Mehr Anhebung senkt den ganzen Pegel mehr ab und kann angestrengt klingen."
            }
            Self::Output => {
                "Der Pegel nach der Korrektur, von −24 bis +6 dB. Hebst du ihn an, fängt der Limiter die Spitzen ab."
            }
            Self::Bypass => "Lässt das Signal unverändert durch, zum Vergleichen.",
            Self::BypassOnExport => {
                "Umgeht AudioRef, während deine DAW offline exportiert, damit die Kopfhörerkorrektur nicht in deinem Mix landet. Lass es an."
            }
            Self::Channels => {
                "Mono: beide Kanäle auf beiden Ohren, um einen Mix in Mono zu prüfen. Vertauscht: links und rechts getauscht, z. B. um deine Kopfhörer zu prüfen."
            }
            Self::Crossfeed => {
                "Jedes Ohr hört etwas vom Bass des anderen Kanals, wie von Lautsprechern, damit breite Mixe weniger ermüden. Leicht, Mittel und Stark mischen ihn 9,5, 6 und 4,5 dB leiser bei."
            }
            Self::Level => {
                "Der Spitzenpegel, der AudioRef verlässt, in dB relativ zur Vollaussteuerung, ab −60 dB. Mit Limiter bleibt er bei höchstens −0,3 dB."
            }
            Self::Limiter => {
                "Hält den Ausgang bei höchstens −0,3 dBFS, ohne Latenz. Sein Balken zeigt, um wie viel er das Signal absenkt: Er bewegt sich nur, wenn der Pegel darüber ginge."
            }
            Self::LowerOutput => "Senke den Ausgang",
            Self::MetersOn => {
                "Klicke, um beide Anzeigen abzuschalten, z. B. bei schwacher Grafik: Das Fenster wird dann nur neu gezeichnet, wenn sich darin etwas ändert."
            }
            Self::MetersOff => {
                "Die Anzeigen sind aus, damit das Fenster nicht ständig neu gezeichnet wird. Klicke, um sie einzuschalten."
            }
        }
    }
}

/// The lowest level the display of the output shows, in dB of full scale: below, it shows
/// silence.
pub const LEVEL_FLOOR_DB: f64 = -60.0;

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
    /// The limiter lowers the signal by so many dB, see [`crate::limiter`].
    Reduction(f64),
    /// The peak level of the output, in dB of full scale; nothing at or below [`LEVEL_FLOOR_DB`].
    Level(f64),
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
            // Without a minus while it rounds to nothing.
            (Self::Reduction(db), _) if *db < 0.05 => format!("{} dB", decimal(0.0, lang)),
            (Self::Reduction(db), _) => format!("−{} dB", decimal(*db, lang)),
            (Self::Level(db), _) if db.is_nan() || *db <= LEVEL_FLOOR_DB => "−∞ dB".to_owned(),
            (Self::Level(db), _) if *db <= -0.05 => format!("−{} dB", decimal(-db, lang)),
            (Self::Level(db), _) if *db < 0.05 => format!("{} dB", decimal(0.0, lang)),
            (Self::Level(db), _) => format!("+{} dB", decimal(*db, lang)),
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

/// The texts of the about window and of what opens it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum About {
    /// The button at the top right that opens the window.
    Button,
    /// The hint of the button and of the plugin's name, which opens the window too.
    Hint,
    /// The window's title.
    Title,
    /// What AudioRef is.
    Description,
    /// The label of the version.
    Version,
    /// The label of the git commit it was built from.
    Commit,
    /// The label of the time of the build.
    Built,
    /// What isn't known of a build, e.g. its commit outside a git repository.
    Unknown,
    /// The label of the contact address.
    Contact,
    /// The label of the homepage.
    Homepage,
    /// What AudioRef is written in.
    WrittenInRust,
    /// AudioRef's own license.
    License,
    /// Where the built-in presets come from, and their license.
    Presets,
    /// The heading of the third-party software.
    ThirdParty,
    /// What the list of third-party software is.
    ThirdPartyHint,
    /// The build couldn't list the third-party software.
    NoThirdParty,
}

impl About {
    /// All texts, e.g. for tests.
    pub const ALL: [Self; 16] = [
        Self::Button,
        Self::Hint,
        Self::Title,
        Self::Description,
        Self::Version,
        Self::Commit,
        Self::Built,
        Self::Unknown,
        Self::Contact,
        Self::Homepage,
        Self::WrittenInRust,
        Self::License,
        Self::Presets,
        Self::ThirdParty,
        Self::ThirdPartyHint,
        Self::NoThirdParty,
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
            Self::Button => "About",
            Self::Hint => "Version, contact and licenses",
            Self::Title => "About AudioRef",
            Self::Description => "Headphone correction for monitoring, as a VST3 and CLAP plugin.",
            Self::Version => "Version",
            Self::Commit => "Commit",
            Self::Built => "Built",
            Self::Unknown => "unknown",
            Self::Contact => "Contact",
            Self::Homepage => "Homepage",
            Self::WrittenInRust => "Written in Rust.",
            Self::License => {
                "AudioRef is free software under the GNU General Public License v3.0 only, for private, non-commercial use."
            }
            Self::Presets => {
                "The built-in presets are an excerpt of OPRA's database, by oratory1990 and AutoEQ, under CC BY-SA 4.0."
            }
            Self::ThirdParty => "Third-party software",
            Self::ThirdPartyHint => {
                "AudioRef is built with these crates, under their licenses. Open one to read its license."
            }
            Self::NoThirdParty => {
                "This build has no list of them: see the dependencies in Cargo.lock of AudioRef's source."
            }
        }
    }

    fn de(self) -> &'static str {
        match self {
            Self::Button => "Info",
            Self::Hint => "Version, Kontakt und Lizenzen",
            Self::Title => "Über AudioRef",
            Self::Description => "Kopfhörerkorrektur für den Abhörweg, als VST3- und CLAP-Plugin.",
            Self::Version => "Version",
            Self::Commit => "Commit",
            Self::Built => "Erstellt",
            Self::Unknown => "unbekannt",
            Self::Contact => "Kontakt",
            Self::Homepage => "Website",
            Self::WrittenInRust => "Geschrieben in Rust.",
            Self::License => {
                "AudioRef ist freie Software unter der GNU General Public License v3.0 only, für private, nicht-kommerzielle Nutzung."
            }
            Self::Presets => {
                "Die eingebauten Presets sind ein Auszug aus OPRAs Datenbank, von oratory1990 und AutoEQ, unter CC BY-SA 4.0."
            }
            Self::ThirdParty => "Software von Dritten",
            Self::ThirdPartyHint => {
                "AudioRef ist mit diesen Crates gebaut, unter ihren Lizenzen. Öffne eines, um seine Lizenz zu lesen."
            }
            Self::NoThirdParty => {
                "Dieser Build hat keine Liste davon: Siehe die Abhängigkeiten in Cargo.lock von AudioRefs Quelltext."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{About, CurveMessage, Hint, Lang, Message, Notice, Param, Text};
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
    fn every_text_of_the_about_window_exists_in_every_language() {
        for text in About::ALL {
            for lang in Lang::ALL {
                assert!(!text.text(lang).is_empty(), "{text:?} in {lang:?}");
            }
        }
        assert_eq!(About::Title.text(Lang::De), "Über AudioRef");
    }

    #[test]
    fn every_hint_exists_in_every_language() {
        for hint in Hint::ALL {
            assert_ne!(hint.text(Lang::En), hint.text(Lang::De), "{hint:?}");
            for lang in Lang::ALL {
                assert!(!hint.text(lang).is_empty(), "{hint:?} in {lang:?}");
            }
        }
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
        assert_eq!(Message::Reduction(2.46).text(Lang::En), "−2.5 dB");
        assert_eq!(Message::Reduction(0.04).text(Lang::De), "0,0 dB");
        assert_eq!(Message::Level(-12.34).text(Lang::De), "−12,3 dB");
        assert_eq!(Message::Level(-0.01).text(Lang::En), "0.0 dB");
        assert_eq!(Message::Level(1.5).text(Lang::En), "+1.5 dB");
        assert_eq!(Message::Level(-60.0).text(Lang::En), "−∞ dB");
        assert_eq!(Message::Level(-59.96).text(Lang::En), "−60.0 dB");
        assert_eq!(Message::Level(f64::NEG_INFINITY).text(Lang::De), "−∞ dB");
        assert_eq!(Message::Level(f64::NAN).text(Lang::En), "−∞ dB");
        assert_eq!(Message::Reduction(0.05).text(Lang::De), "−0,1 dB");
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
