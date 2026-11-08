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
}

impl Param {
    /// All parameters, e.g. for tests.
    pub const ALL: [Self; 2] = [Self::Output, Self::Bypass];

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
        }
    }

    fn de(self) -> &'static str {
        match self {
            Self::Output => "Ausgang",
            Self::Bypass => "Umgehen",
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

#[cfg(test)]
mod tests {
    use super::{Lang, Notice, Param};

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
            assert_ne!(Param::Output.name(lang), Param::Bypass.name(lang));
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
}
