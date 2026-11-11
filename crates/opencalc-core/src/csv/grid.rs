//! Turning parsed CSV rows into a grid of numbers with optional headers.

use std::fmt;

use super::parse_number_with;

/// Numbers read from text, with any header row split off.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    /// The first row when it is not numbers (one entry per column).
    pub headers: Option<Vec<String>>,
    /// Rows of numbers, all the same length.
    pub rows: Vec<Vec<f64>>,
}

/// Why rows could not be made into a grid of numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GridError {
    Empty,
    /// A row has a different number of fields from the first. 1-based.
    Ragged {
        row: usize,
        found: usize,
        expected: usize,
    },
    /// A field that is not a number. 1-based row and column.
    NotANumber {
        row: usize,
        col: usize,
        text: String,
    },
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GridError::Empty => write!(f, "there is no data"),
            GridError::Ragged {
                row,
                found,
                expected,
            } => write!(f, "row {row} has {found} values, expected {expected}"),
            GridError::NotANumber { row, col, text } => {
                let shown: String = text.chars().take(24).collect();
                write!(f, "row {row}, column {col}: \"{shown}\" is not a number")
            }
        }
    }
}

impl std::error::Error for GridError {}

/// Makes rows of fields into numbers. The first row is taken as headers when none of
/// its fields is a number and a later row exists.
pub fn to_grid(rows: &[Vec<String>], decimal: char) -> Result<Grid, GridError> {
    let Some(first) = rows.first() else {
        return Err(GridError::Empty);
    };
    let width = first.len();
    let header = first
        .iter()
        .all(|f| parse_number_with(f, decimal).is_none());
    if header && rows.len() == 1 {
        return Err(GridError::Empty);
    }
    let start = usize::from(header);
    let mut out = Vec::with_capacity(rows.len() - start);
    for (i, r) in rows.iter().enumerate().skip(start) {
        if r.len() != width {
            return Err(GridError::Ragged {
                row: i + 1,
                found: r.len(),
                expected: width,
            });
        }
        let mut nums = Vec::with_capacity(width);
        for (j, f) in r.iter().enumerate() {
            match parse_number_with(f, decimal) {
                Some(v) => nums.push(v),
                None => {
                    return Err(GridError::NotANumber {
                        row: i + 1,
                        col: j + 1,
                        text: f.clone(),
                    })
                }
            }
        }
        out.push(nums);
    }
    if out.is_empty() {
        return Err(GridError::Empty);
    }
    Ok(Grid {
        headers: header.then(|| first.iter().map(|s| s.trim().to_owned()).collect()),
        rows: out,
    })
}

/// Splits a header such as `force [kN]` into its name and unit text.
pub fn split_header(header: &str) -> (String, String) {
    let h = header.trim();
    if let Some(open) = h.rfind('[') {
        if let Some(inner) = h[open + 1..].strip_suffix(']') {
            return (h[..open].trim().to_owned(), inner.trim().to_owned());
        }
    }
    (h.to_owned(), String::new())
}
