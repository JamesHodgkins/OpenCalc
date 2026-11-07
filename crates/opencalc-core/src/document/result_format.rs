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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Saved", into = "Saved")]
pub struct ResultFormat {
    notation: Notation,
    decimals: Option<u8>,
}

/// The saved form; `from` brings an out-of-range count back into range.
#[derive(Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    notation: Notation,
    #[serde(default)]
    decimals: Option<u8>,
}

impl From<Saved> for ResultFormat {
    fn from(s: Saved) -> ResultFormat {
        ResultFormat::new(s.notation, s.decimals)
    }
}

impl From<ResultFormat> for Saved {
    fn from(f: ResultFormat) -> Saved {
        Saved {
            notation: f.notation,
            decimals: f.decimals,
        }
    }
}

impl ResultFormat {
    /// `decimals` of `None` is the default: up to three, trailing zeros dropped.
    pub fn new(notation: Notation, decimals: Option<u8>) -> ResultFormat {
        ResultFormat {
            notation,
            decimals: decimals.map(|d| d.min(MAX_DECIMALS)),
        }
    }

    pub fn notation(self) -> Notation {
        self.notation
    }

    pub fn decimals(self) -> Option<u8> {
        self.decimals
    }

    /// Whether this is the default, which is not saved.
    pub fn is_default(&self) -> bool {
        *self == ResultFormat::default()
    }

    /// One more decimal. From the default it starts at 4 (one more than the three
    /// the default shows) and stops at the most that means anything.
    pub fn more_decimals(self) -> ResultFormat {
        let next = self.decimals.map_or(4, |d| d.saturating_add(1));
        ResultFormat::new(self.notation, Some(next))
    }

    /// One fewer decimal, down to none.
    pub fn fewer_decimals(self) -> ResultFormat {
        let next = self.decimals.map_or(2, |d| d.saturating_sub(1));
        ResultFormat::new(self.notation, Some(next))
    }

    pub fn with_notation(self, notation: Notation) -> ResultFormat {
        ResultFormat::new(notation, self.decimals)
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
        let mut g = f;
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
    fn an_out_of_range_count_in_a_file_is_brought_into_range() {
        let f: ResultFormat =
            serde_json::from_str(r#"{"notation":"scientific","decimals":200}"#).unwrap();
        assert_eq!(f.decimals(), Some(MAX_DECIMALS));
        let d: ResultFormat = serde_json::from_str("{}").unwrap();
        assert!(d.is_default());
        assert!(serde_json::from_str::<ResultFormat>(r#"{"notation":"hex"}"#).is_err());
    }
}
