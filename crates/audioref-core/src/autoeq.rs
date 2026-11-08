//! Frequency responses in AutoEQ's CSV format: the measurements of headphones, e.g.
//! `measurements/oratory1990/data/over-ear/*.csv` in [AutoEQ's repository](https://github.com/jaakkopasanen/AutoEq),
//! and the targets they are corrected to, e.g. `targets/*.csv`.
//!
//! AutoEQ writes a header, `frequency,raw`, then one line per frequency in Hz with its level in
//! dB. [`Curve::parse`] reads that, and also what other tools write: more columns (it takes the
//! one named `raw`), no header, `;` with a decimal comma, tabs or spaces, and comments starting
//! with `#` or `*`. Lines it doesn't understand are left out and counted, so one bad line doesn't
//! cost all the others.
//!
//! ```
//! use audioref_core::autoeq::Curve;
//!
//! let curve = Curve::parse("frequency,raw\n20,-3\n1000,0\n20000,6\n")
//!     .map_err(|error| format!("{error:?}"))?;
//! assert_eq!(curve.points().len(), 3);
//! assert!((curve.value_db(1000.0)).abs() < 1e-9);
//! // Between two points, the level runs straight on a logarithmic frequency axis.
//! assert!((curve.value_db(4472.1) - 3.0).abs() < 0.01);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::ops::RangeInclusive;

/// The frequencies a curve may hold, in Hz.
pub const FREQUENCY_HZ: RangeInclusive<f64> = 1.0..=100_000.0;

/// The levels a curve may hold, in dB. AutoEQ's are relative to 1 kHz; other tools write sound
/// pressure levels of 80 to 120 dB, which [`Curve::value_db`] takes as they are, since a
/// correction only uses differences.
pub const LEVEL_DB: RangeInclusive<f64> = -200.0..=200.0;

/// The most points a curve may have. AutoEQ's have 695; measurements of other tools a few
/// thousand.
pub const MAX_POINTS: usize = 100_000;

/// The frequencies a curve must reach, at least, in Hz: below and above them, a correction
/// would guess.
pub const REQUIRED_HZ: RangeInclusive<f64> = 100.0..=10_000.0;

/// Why a text isn't a curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParseError {
    /// Fewer than two lines hold a frequency and a level: most likely another file.
    NoPoints,
    /// More than [`MAX_POINTS`] points.
    TooManyPoints,
    /// The points don't reach from [`REQUIRED_HZ`]'s start to its end.
    TooNarrow {
        /// The lowest frequency, in Hz.
        from_hz: f64,
        /// The highest frequency, in Hz.
        to_hz: f64,
    },
}

/// A frequency response: levels in dB at frequencies in Hz, rising.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    points: Vec<(f64, f64)>,
    skipped: usize,
}

/// How the fields of a line are separated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Separator {
    Comma,
    /// `;`, with a decimal comma or point.
    Semicolon,
    Whitespace,
}

impl Separator {
    /// The separator of a line: a comma if it has one and no `;`, `;` if it has one, else tabs or
    /// spaces.
    fn of(line: &str) -> Self {
        if line.contains(';') {
            Self::Semicolon
        } else if line.contains(',') {
            Self::Comma
        } else {
            Self::Whitespace
        }
    }

    /// The fields of a line, trimmed.
    fn fields(self, line: &str) -> Vec<&str> {
        match self {
            Self::Comma => line.split(',').map(str::trim).collect(),
            Self::Semicolon => line.split(';').map(str::trim).collect(),
            Self::Whitespace => line.split_whitespace().collect(),
        }
    }
}

/// A number of a field, with a decimal comma or point.
fn number(field: &str) -> Option<f64> {
    let value: f64 = field.replace(',', ".").parse().ok()?;
    value.is_finite().then_some(value)
}

/// The columns of the frequency and the level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Columns {
    frequency: usize,
    level: usize,
}

impl Columns {
    /// Without a header: the first two.
    const FIRST_TWO: Self = Self {
        frequency: 0,
        level: 1,
    };

    /// The columns a header names: `frequency` (or `freq`, `hz`) and `raw`, or else the first
    /// column after the frequency. `None` if it names no frequency.
    fn from_header(fields: &[&str]) -> Option<Self> {
        let names: Vec<String> = fields
            .iter()
            .map(|field| field.trim_matches('"').to_lowercase())
            .collect();
        let frequency = names.iter().position(|name| {
            ["frequency", "freq", "hz", "frequency (hz)", "freq(hz)"].contains(&name.as_str())
        })?;
        let level = names
            .iter()
            .position(|name| name == "raw")
            .unwrap_or(usize::from(frequency == 0));
        Some(Self { frequency, level })
    }

    /// The frequency and the level of a line's fields, if both are numbers.
    fn point(self, fields: &[&str]) -> Option<(f64, f64)> {
        let frequency = number(fields.get(self.frequency)?)?;
        let level = number(fields.get(self.level)?)?;
        Some((frequency, level))
    }
}

impl Curve {
    /// Reads a curve from the text of a CSV file. Empty lines and comments (`#`, `*`) are passed
    /// over; a line that isn't a frequency and a level in their ranges, or whose frequency isn't
    /// above the one before, is left out and counted in [`Self::skipped`].
    ///
    /// # Errors
    ///
    /// If fewer than two points are left, more than [`MAX_POINTS`], or they don't reach across
    /// [`REQUIRED_HZ`].
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut lines = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('*'))
            .peekable();
        let columns = lines
            .peek()
            .and_then(|line| {
                let fields = Separator::of(line).fields(line);
                let is_header = fields.first().is_some_and(|field| number(field).is_none());
                is_header.then(|| Columns::from_header(&fields))
            })
            .map_or(Columns::FIRST_TWO, |columns| {
                lines.next();
                columns.unwrap_or(Columns::FIRST_TWO)
            });
        let mut points: Vec<(f64, f64)> = Vec::new();
        let mut skipped = 0;
        for line in lines {
            let point =
                columns
                    .point(&Separator::of(line).fields(line))
                    .filter(|(frequency, level)| {
                        FREQUENCY_HZ.contains(frequency)
                            && LEVEL_DB.contains(level)
                            && points.last().is_none_or(|(last, _)| frequency > last)
                    });
            match point {
                Some(point) if points.len() < MAX_POINTS => points.push(point),
                Some(_) => return Err(ParseError::TooManyPoints),
                None => skipped += 1,
            }
        }
        let curve = Self::from_points(points)?;
        Ok(Self { skipped, ..curve })
    }

    /// A curve from points, with rising frequencies.
    ///
    /// # Errors
    ///
    /// As [`Self::parse`]: fewer than two points, more than [`MAX_POINTS`], or not across
    /// [`REQUIRED_HZ`]. Points out of their ranges or not rising count as missing.
    pub fn from_points(points: Vec<(f64, f64)>) -> Result<Self, ParseError> {
        let valid = points
            .iter()
            .all(|(frequency, level)| FREQUENCY_HZ.contains(frequency) && LEVEL_DB.contains(level))
            && points
                .iter()
                .zip(points.iter().skip(1))
                .all(|((a, _), (b, _))| a < b);
        let (Some((from_hz, _)), Some((to_hz, _)), true) = (points.first(), points.last(), valid)
        else {
            return Err(ParseError::NoPoints);
        };
        if points.len() < 2 {
            return Err(ParseError::NoPoints);
        }
        if points.len() > MAX_POINTS {
            return Err(ParseError::TooManyPoints);
        }
        if *from_hz > *REQUIRED_HZ.start() || *to_hz < *REQUIRED_HZ.end() {
            return Err(ParseError::TooNarrow {
                from_hz: *from_hz,
                to_hz: *to_hz,
            });
        }
        Ok(Self { points, skipped: 0 })
    }

    /// The points, frequency in Hz and level in dB, by rising frequency.
    #[must_use]
    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }

    /// How many lines [`Self::parse`] left out.
    #[must_use]
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    /// The level at a frequency in Hz, in dB: between two points straight on a logarithmic
    /// frequency axis, below the first and above the last the level of that point.
    #[must_use]
    pub fn value_db(&self, frequency: f64) -> f64 {
        let index = self.points.partition_point(|(f, _)| *f < frequency);
        match (
            index
                .checked_sub(1)
                .and_then(|below| self.points.get(below)),
            self.points.get(index),
        ) {
            (Some(&(f0, l0)), Some(&(f1, l1))) => {
                let t = (frequency / f0).ln() / (f1 / f0).ln();
                l0 + t * (l1 - l0)
            }
            (None, Some(&(_, level))) | (Some(&(_, level)), None) => level,
            (None, None) => 0.0,
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "the points are the very numbers of the text, unchanged"
)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    /// A curve on AutoEQ's grid with a level of `level(frequency)`, as AutoEQ writes it.
    fn autoeq_text(level: impl Fn(f64) -> f64) -> String {
        let mut text = String::from("frequency,raw\n");
        for frequency in crate::cascade::grid() {
            writeln!(text, "{frequency:.2},{:.2}", level(frequency)).unwrap();
        }
        text
    }

    #[test]
    fn reads_autoeq_files() {
        let curve = Curve::parse(&autoeq_text(|f| (f / 1000.0).log2())).unwrap();
        assert_eq!(curve.points().len(), 695);
        assert_eq!(curve.skipped(), 0);
        assert!(curve.value_db(1000.0).abs() < 0.01);
        assert!((curve.value_db(4000.0) - 2.0).abs() < 0.01);
        assert!((curve.value_db(125.0) + 3.0).abs() < 0.01);
    }

    #[test]
    fn takes_the_raw_column_of_results() {
        let text = "frequency,target,raw,smoothed\n20,9,-1,0\n1000,9,0,0\n20000,9,1,0\n";
        let curve = Curve::parse(text).unwrap();
        assert_eq!(
            curve.points(),
            &[(20.0, -1.0), (1000.0, 0.0), (20000.0, 1.0)]
        );
    }

    #[test]
    fn takes_the_column_after_the_frequency_without_raw() {
        let curve = Curve::parse("Freq(Hz);SPL(dB)\n20;80,5\n20000;70\n").unwrap();
        assert_eq!(curve.points(), &[(20.0, 80.5), (20000.0, 70.0)]);
        let curve = Curve::parse("SPL,Frequency\n80,20\n70,20000\n").unwrap();
        assert_eq!(curve.points(), &[(20.0, 80.0), (20000.0, 70.0)]);
    }

    #[test]
    fn reads_files_of_other_tools() {
        let rew = "\u{feff}* Measurement data measured by REW\r\n* Freq(Hz) SPL(dB)\r\n20.0\t85.1\r\n\r\n1000.0 90.0\r\n20000.0  80.0\r\n";
        let curve = Curve::parse(rew).unwrap();
        assert_eq!(
            curve.points(),
            &[(20.0, 85.1), (1000.0, 90.0), (20000.0, 80.0)]
        );
        let without_header = "# comment\n20,1\n20000,2\n";
        assert_eq!(Curve::parse(without_header).unwrap().points().len(), 2);
        let unknown_header = "a,b\n20,1\n20000,2\n";
        assert_eq!(Curve::parse(unknown_header).unwrap().points().len(), 2);
    }

    #[test]
    fn leaves_out_and_counts_bad_lines() {
        let text = "frequency,raw\n20,0\n10,0\n30,NaN\n40,inf\n50,300\n0.5,0\n60\n70,x\n100000.1,0\n1000,0\n1000,1\n20000,0\n";
        let curve = Curve::parse(text).unwrap();
        assert_eq!(
            curve.points(),
            &[(20.0, 0.0), (1000.0, 0.0), (20000.0, 0.0)]
        );
        assert_eq!(curve.skipped(), 9);
    }

    #[test]
    fn refuses_what_isnt_a_curve() {
        assert_eq!(Curve::parse(""), Err(ParseError::NoPoints));
        assert_eq!(Curve::parse("frequency,raw\n"), Err(ParseError::NoPoints));
        assert_eq!(
            Curve::parse("{\"type\":\"vendor\"}"),
            Err(ParseError::NoPoints)
        );
        assert_eq!(Curve::parse("20,1\n"), Err(ParseError::NoPoints));
        assert_eq!(
            Curve::parse("200,1\n20000,1\n"),
            Err(ParseError::TooNarrow {
                from_hz: 200.0,
                to_hz: 20000.0
            })
        );
        assert_eq!(
            Curve::parse("20,1\n8000,1\n"),
            Err(ParseError::TooNarrow {
                from_hz: 20.0,
                to_hz: 8000.0
            })
        );
    }

    #[test]
    fn refuses_too_many_points() {
        let mut text = String::new();
        let frequencies =
            || (0..=u32::try_from(MAX_POINTS).unwrap()).map(|i| 20.0 + f64::from(i) * 0.2);
        for frequency in frequencies() {
            writeln!(text, "{frequency},0").unwrap();
        }
        assert_eq!(Curve::parse(&text), Err(ParseError::TooManyPoints));
        let points = frequencies().map(|frequency| (frequency, 0.0)).collect();
        assert_eq!(Curve::from_points(points), Err(ParseError::TooManyPoints));
    }

    #[test]
    fn from_points_checks_them() {
        assert_eq!(Curve::from_points(vec![]), Err(ParseError::NoPoints));
        assert_eq!(
            Curve::from_points(vec![(20.0, 0.0), (20.0, 0.0), (20000.0, 0.0)]),
            Err(ParseError::NoPoints)
        );
        assert_eq!(
            Curve::from_points(vec![(20.0, f64::NAN), (20000.0, 0.0)]),
            Err(ParseError::NoPoints)
        );
        let curve = Curve::from_points(vec![(20.0, 1.0), (20000.0, 2.0)]).unwrap();
        assert_eq!(curve.skipped(), 0);
    }

    #[test]
    fn is_flat_beyond_its_ends() {
        let curve = Curve::from_points(vec![(50.0, 1.0), (15000.0, 2.0)]).unwrap();
        assert_eq!(curve.value_db(10.0), 1.0);
        assert_eq!(curve.value_db(50.0), 1.0);
        assert_eq!(curve.value_db(15000.0), 2.0);
        assert_eq!(curve.value_db(20000.0), 2.0);
        let empty = Curve {
            points: vec![],
            skipped: 0,
        };
        assert_eq!(empty.value_db(1000.0), 0.0);
    }

    #[test]
    fn separators() {
        assert_eq!(Separator::of("1;2,5"), Separator::Semicolon);
        assert_eq!(Separator::of("1,2"), Separator::Comma);
        assert_eq!(Separator::of("1 2"), Separator::Whitespace);
        assert_eq!(number(" 2,5 ".trim()), Some(2.5));
        assert_eq!(number("1e999"), None);
    }
}
