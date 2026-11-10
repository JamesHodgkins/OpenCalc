//! How a math region shows its result: the number of decimals and the notation.
//!
//! This is display only. Values are never rounded; two regions with different
//! formats show the same number differently and calculate with the same one.

use serde::{Deserialize, Serialize};

/// Most decimals a result can be asked to show (an `f64` has about 15 digits).
pub const MAX_DECIMALS: u8 = 15;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Notation {
    /// Plain numbers, scientific only for very large or small ones.
    #[default]
    General,
    /// One digit before the point: `1.5e4`.
    Scientific,
    /// The exponent is a multiple of three: `15e3`.
    Engineering,
}

/// A unit a result can be asked to be shown in. It applies to results of the same
/// kind (lengths for the length units, forces for the force units); a result of
/// any other kind is shown as it would be without the choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayUnit {
    #[serde(rename = "m")]
    Metre,
    #[serde(rename = "mm")]
    Millimetre,
    #[serde(rename = "in")]
    Inch,
    #[serde(rename = "N")]
    Newton,
    #[serde(rename = "kN")]
    Kilonewton,
}

impl DisplayUnit {
    /// The unit's symbol, as the evaluator knows it.
    pub fn symbol(self) -> &'static str {
        match self {
            DisplayUnit::Metre => "m",
            DisplayUnit::Millimetre => "mm",
            DisplayUnit::Inch => "in",
            DisplayUnit::Newton => "N",
            DisplayUnit::Kilonewton => "kN",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Saved", into = "Saved")]
pub struct ResultFormat {
    notation: Notation,
    decimals: Option<u8>,
    unit: Option<DisplayUnit>,
    /// A unit typed as an expression (`N/mm^2`); it wins over `unit`.
    custom: Option<String>,
}

/// Longest unit expression kept.
const MAX_CUSTOM_LEN: usize = 64;

/// The saved form; `from` brings an out-of-range count back into range.
#[derive(Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    notation: Notation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decimals: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unit: Option<DisplayUnit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    custom: Option<String>,
}

impl From<Saved> for ResultFormat {
    fn from(s: Saved) -> ResultFormat {
        let f = ResultFormat::new(s.notation, s.decimals).with_unit(s.unit);
        match s.custom {
            Some(c) => f.with_custom_unit(Some(c)),
            None => f,
        }
    }
}

impl From<ResultFormat> for Saved {
    fn from(f: ResultFormat) -> Saved {
        Saved {
            notation: f.notation,
            decimals: f.decimals,
            unit: f.unit,
            custom: f.custom,
        }
    }
}

impl ResultFormat {
    /// `decimals` of `None` is the default: up to three, trailing zeros dropped.
    pub fn new(notation: Notation, decimals: Option<u8>) -> ResultFormat {
        ResultFormat {
            notation,
            decimals: decimals.map(|d| d.min(MAX_DECIMALS)),
            unit: None,
            custom: None,
        }
    }

    pub fn notation(&self) -> Notation {
        self.notation
    }

    pub fn decimals(&self) -> Option<u8> {
        self.decimals
    }

    /// Whether this is the default, which is not saved.
    pub fn is_default(&self) -> bool {
        *self == ResultFormat::default()
    }

    /// One more decimal. From the default it starts at 4 (one more than the three
    /// the default shows) and stops at the most that means anything.
    pub fn more_decimals(&self) -> ResultFormat {
        let next = self.decimals.map_or(4, |d| d.saturating_add(1));
        ResultFormat {
            decimals: Some(next.min(MAX_DECIMALS)),
            ..self.clone()
        }
    }

    /// One fewer decimal, down to none.
    pub fn fewer_decimals(&self) -> ResultFormat {
        let next = self.decimals.map_or(2, |d| d.saturating_sub(1));
        ResultFormat {
            decimals: Some(next),
            ..self.clone()
        }
    }

    pub fn unit(&self) -> Option<DisplayUnit> {
        self.unit
    }

    /// The unit expression typed for this region, such as `N/mm^2`.
    pub fn custom_unit(&self) -> Option<&str> {
        self.custom.as_deref()
    }

    /// Show results in `unit` (`None`: chosen automatically). Clears any typed unit.
    pub fn with_unit(self, unit: Option<DisplayUnit>) -> ResultFormat {
        ResultFormat {
            unit,
            custom: None,
            ..self
        }
    }

    /// Show results in a unit typed as an expression. Blank means none. It is
    /// applied to results of the same kind only, like [`DisplayUnit`].
    pub fn with_custom_unit(self, text: Option<String>) -> ResultFormat {
        let custom = text
            .map(|t| t.trim().chars().take(MAX_CUSTOM_LEN).collect::<String>())
            .filter(|t| !t.is_empty());
        ResultFormat {
            unit: None,
            custom,
            ..self
        }
    }

    pub fn with_notation(self, notation: Notation) -> ResultFormat {
        ResultFormat { notation, ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_step_from_the_default_and_stay_in_range() {
        let f = ResultFormat::default();
        assert_eq!(f.more_decimals().decimals(), Some(4));
        assert_eq!(f.fewer_decimals().decimals(), Some(2));
        let mut g = f.clone();
        for _ in 0..40 {
            g = g.more_decimals();
        }
        assert_eq!(g.decimals(), Some(MAX_DECIMALS));
        for _ in 0..40 {
            g = g.fewer_decimals();
        }
        assert_eq!(g.decimals(), Some(0));
    }

    #[test]
    fn a_typed_unit_replaces_a_preset_and_survives_saving() {
        let f = ResultFormat::default()
            .with_unit(Some(DisplayUnit::Metre))
            .with_custom_unit(Some("  N/mm^2 ".into()));
        assert_eq!(f.unit(), None);
        assert_eq!(f.custom_unit(), Some("N/mm^2"));
        let json = serde_json::to_string(&f).unwrap();
        assert_eq!(serde_json::from_str::<ResultFormat>(&json).unwrap(), f);
        assert!(f.clone().with_custom_unit(Some("  ".into())).is_default());
        assert_eq!(f.with_unit(None).custom_unit(), None);
    }

    #[test]
    fn an_out_of_range_count_in_a_file_is_brought_into_range() {
        let f: ResultFormat =
            serde_json::from_str(r#"{"notation":"scientific","decimals":200}"#).unwrap();
        assert_eq!(f.decimals(), Some(MAX_DECIMALS));
        let d: ResultFormat = serde_json::from_str("{}").unwrap();
        assert!(d.is_default());
        assert!(serde_json::from_str::<ResultFormat>(r#"{"notation":"hex"}"#).is_err());
    }
}
