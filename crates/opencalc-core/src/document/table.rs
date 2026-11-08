//! The Table region: a grid of numbers that defines a name.
//!
//! The cells are kept as the text the user typed, so a number round-trips exactly
//! and an unfinished cell is not lost. The table is turned into a matrix
//! expression, `name := [1 kN, 2 s; ...]`, and evaluated like any Math region, so
//! it takes part in the dependency graph, recalculation and unit checking with no
//! special cases. Each column has one unit.

use serde::{Deserialize, Serialize};

use crate::csv;
use crate::expr::{Expr, ParseError};
use crate::parser::parse;

/// The most cells a table holds (the matrix limit).
pub const MAX_CELLS: usize = 4096;
/// The most columns.
pub const MAX_COLUMNS: usize = 64;
/// The most characters in one cell, header, unit or name.
pub const MAX_TEXT: usize = 60;

/// One column: an optional heading and a unit (blank: none).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableColumn {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub header: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSpec {
    /// The name the table defines (blank: it defines nothing yet).
    #[serde(default)]
    pub name: String,
    pub columns: Vec<TableColumn>,
    /// Rows of cell text, each as long as `columns`.
    pub rows: Vec<Vec<String>>,
}

impl Default for TableSpec {
    /// A blank 3 by 2 table.
    fn default() -> TableSpec {
        TableSpec {
            name: String::new(),
            columns: vec![TableColumn::default(); 2],
            rows: vec![vec![String::new(); 2]; 3],
        }
    }
}

fn clip(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXT)
        .collect()
}

fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
}

impl TableSpec {
    pub fn cols(&self) -> usize {
        self.columns.len()
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Whether the shape is allowed: rectangular, within the limits.
    pub fn is_valid(&self) -> bool {
        let w = self.columns.len();
        (1..=MAX_COLUMNS).contains(&w)
            && !self.rows.is_empty()
            && self.rows.len() * w <= MAX_CELLS
            && self.rows.iter().all(|r| r.len() == w)
    }

    /// Makes an arbitrary value valid: cells clipped, rows padded or cut to the
    /// column count, too many columns or cells cut. Used on load.
    pub fn repaired(mut self) -> TableSpec {
        self.columns.truncate(MAX_COLUMNS);
        if self.columns.is_empty() {
            self.columns.push(TableColumn::default());
        }
        let w = self.columns.len();
        self.rows.truncate((MAX_CELLS / w).max(1));
        if self.rows.is_empty() {
            self.rows.push(Vec::new());
        }
        for r in &mut self.rows {
            r.resize(w, String::new());
            for c in r.iter_mut() {
                *c = clip(c);
            }
        }
        for c in &mut self.columns {
            c.header = clip(&c.header);
            c.unit = clip(&c.unit);
        }
        self.name = clip(&self.name);
        self
    }

    pub fn with_name(mut self, name: &str) -> TableSpec {
        self.name = clip(name.trim());
        self
    }

    pub fn with_cell(mut self, row: usize, col: usize, text: &str) -> TableSpec {
        if let Some(cell) = self.rows.get_mut(row).and_then(|r| r.get_mut(col)) {
            *cell = clip(text);
        }
        self
    }

    pub fn with_header(mut self, col: usize, text: &str) -> TableSpec {
        if let Some(c) = self.columns.get_mut(col) {
            c.header = clip(text);
        }
        self
    }

    pub fn with_unit(mut self, col: usize, text: &str) -> TableSpec {
        if let Some(c) = self.columns.get_mut(col) {
            c.unit = clip(text.trim());
        }
        self
    }

    /// One more blank row, unless the table is full.
    pub fn with_row_added(mut self) -> TableSpec {
        if (self.rows.len() + 1) * self.cols() <= MAX_CELLS {
            self.rows.push(vec![String::new(); self.cols()]);
        }
        self
    }

    /// Removes a row; the last row cannot be removed.
    pub fn with_row_removed(mut self, row: usize) -> TableSpec {
        if self.rows.len() > 1 && row < self.rows.len() {
            self.rows.remove(row);
        }
        self
    }

    /// One more blank column, unless the table is full.
    pub fn with_column_added(mut self) -> TableSpec {
        if self.cols() < MAX_COLUMNS && self.rows.len() * (self.cols() + 1) <= MAX_CELLS {
            self.columns.push(TableColumn::default());
            for r in &mut self.rows {
                r.push(String::new());
            }
        }
        self
    }

    /// Removes a column; the last column cannot be removed.
    pub fn with_column_removed(mut self, col: usize) -> TableSpec {
        if self.cols() > 1 && col < self.cols() {
            self.columns.remove(col);
            for r in &mut self.rows {
                r.remove(col);
            }
        }
        self
    }

    fn all_blank(&self) -> bool {
        self.rows.iter().flatten().all(|c| c.trim().is_empty())
    }

    /// The matrix expression the table stands for, or `None` when it defines
    /// nothing: no name, or no numbers typed yet. A table with a name and numbers
    /// that cannot be used gives an error that says which cell.
    pub fn expr(&self) -> Option<Expr> {
        if self.name.trim().is_empty() || self.all_blank() {
            return None;
        }
        Some(match self.source() {
            Ok(source) => parse(&source),
            Err(message) => Expr::Error(ParseError::new(message, 0, 0)),
        })
    }

    /// The text the table is evaluated from.
    pub fn source(&self) -> Result<String, String> {
        if !is_name(&self.name) {
            return Err(format!("\"{}\" is not a usable name", self.name));
        }
        if !self.is_valid() {
            return Err("the table has the wrong shape".into());
        }
        for (j, c) in self.columns.iter().enumerate() {
            let u = c.unit.trim();
            if u.contains([',', ';', '[', ']', ':']) {
                return Err(format!("column {}: \"{u}\" is not a unit", j + 1));
            }
            if !u.is_empty() && matches!(parse(&format!("1 {u}")), Expr::Error(_)) {
                return Err(format!("column {}: \"{u}\" is not a unit", j + 1));
            }
        }
        let mut out = format!("{} := [", self.name);
        for (i, row) in self.rows.iter().enumerate() {
            if i > 0 {
                out.push_str("; ");
            }
            for (j, cell) in row.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                let text = cell.trim();
                if csv::parse_number(text).is_none() {
                    let what = if text.is_empty() {
                        "is empty".to_owned()
                    } else {
                        format!("\"{text}\" is not a number")
                    };
                    return Err(format!("row {}, column {}: {what}", i + 1, j + 1));
                }
                out.push_str(text);
                let unit = self.columns[j].unit.trim();
                if !unit.is_empty() {
                    out.push(' ');
                    out.push_str(unit);
                }
            }
        }
        out.push(']');
        Ok(out)
    }

    /// Makes a table from delimited text. The first row is read as headings when
    /// it is not numbers; a unit may be written in brackets after the heading.
    pub fn from_csv(text: &str) -> Result<TableSpec, String> {
        let delimiter = csv::detect_delimiter(text);
        let rows = csv::parse(text, delimiter).map_err(|e| e.to_string())?;
        let decimal = csv::detect_decimal(&rows, delimiter);
        let grid = csv::to_grid(&rows, decimal).map_err(|e| e.to_string())?;
        let width = grid.rows[0].len();
        if width > MAX_COLUMNS {
            return Err(format!("more than {MAX_COLUMNS} columns"));
        }
        if grid.rows.len() * width > MAX_CELLS {
            return Err(format!("more than {MAX_CELLS} values"));
        }
        let columns = (0..width)
            .map(|j| match &grid.headers {
                Some(h) => {
                    let (header, unit) = csv::split_header(&h[j]);
                    TableColumn {
                        header: clip(&header),
                        unit: clip(&unit),
                    }
                }
                None => TableColumn::default(),
            })
            .collect();
        let rows = grid
            .rows
            .iter()
            .map(|r| r.iter().map(|v| csv::number_text(*v)).collect())
            .collect();
        Ok(TableSpec {
            name: String::new(),
            columns,
            rows,
        })
    }

    /// The table as delimited text: a heading row (`force [kN]`) when any column
    /// has a heading or a unit, then the numbers as typed.
    pub fn to_csv(&self, delimiter: char) -> String {
        let mut rows: Vec<Vec<String>> = Vec::with_capacity(self.rows.len() + 1);
        if self
            .columns
            .iter()
            .any(|c| !c.header.is_empty() || !c.unit.is_empty())
        {
            rows.push(
                self.columns
                    .iter()
                    .enumerate()
                    .map(|(j, c)| {
                        let h = if c.header.is_empty() {
                            format!("col{}", j + 1)
                        } else {
                            c.header.clone()
                        };
                        if c.unit.is_empty() {
                            h
                        } else {
                            format!("{h} [{}]", c.unit)
                        }
                    })
                    .collect(),
            );
        }
        rows.extend(self.rows.iter().cloned());
        csv::write(&rows, delimiter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled() -> TableSpec {
        TableSpec::default()
            .with_name("T")
            .with_unit(0, "m")
            .with_unit(1, "s")
            .with_cell(0, 0, "1")
            .with_cell(0, 1, "2")
            .with_cell(1, 0, "3")
            .with_cell(1, 1, "4.5")
            .with_cell(2, 0, "5")
            .with_cell(2, 1, "6")
    }

    #[test]
    fn a_blank_or_unnamed_table_defines_nothing() {
        assert!(TableSpec::default().expr().is_none());
        assert!(TableSpec::default().with_name("T").expr().is_none());
        let mut t = filled();
        t.name.clear();
        assert!(t.expr().is_none());
    }

    #[test]
    fn source_has_units_per_column() {
        assert_eq!(
            filled().source().unwrap(),
            "T := [1 m, 2 s; 3 m, 4.5 s; 5 m, 6 s]"
        );
        assert!(!matches!(filled().expr(), Some(Expr::Error(_))));
    }

    #[test]
    fn a_bad_cell_is_named() {
        let e = filled().with_cell(1, 1, "x").source().unwrap_err();
        assert_eq!(e, "row 2, column 2: \"x\" is not a number");
        let e = filled().with_cell(2, 0, "").source().unwrap_err();
        assert_eq!(e, "row 3, column 1: is empty");
        assert!(matches!(
            filled().with_cell(0, 0, "").expr(),
            Some(Expr::Error(_))
        ));
    }

    #[test]
    fn bad_names_and_units_are_refused() {
        assert!(filled().with_name("1x").source().is_err());
        assert!(filled().with_name("a := 5; b").source().is_err());
        assert!(filled().with_unit(0, "m, s").source().is_err());
        assert!(filled().with_unit(0, "m +").source().is_err());
    }

    #[test]
    fn editing_respects_the_limits() {
        let t = filled().with_row_added();
        assert_eq!(t.row_count(), 4);
        assert!(t.is_valid());
        let t = t.with_row_removed(0);
        assert_eq!(t.row_count(), 3);
        let t = t.with_column_added();
        assert_eq!(t.cols(), 3);
        assert!(t.rows.iter().all(|r| r.len() == 3));
        let t = t.with_column_removed(2);
        assert_eq!(t.cols(), 2);
        // the last row and column stay
        let one = TableSpec::default()
            .with_row_removed(0)
            .with_row_removed(0)
            .with_row_removed(0)
            .with_column_removed(0)
            .with_column_removed(0);
        assert_eq!((one.row_count(), one.cols()), (1, 1));
        // the cell limit stops growth
        let mut big = TableSpec::default();
        for _ in 0..5000 {
            big = big.with_row_added();
        }
        assert!(big.rows.len() * big.cols() <= MAX_CELLS);
        assert!(big.is_valid());
    }

    #[test]
    fn repair_fixes_hostile_shapes() {
        let wild = TableSpec {
            name: "n".repeat(500),
            columns: vec![TableColumn::default(); 200],
            rows: vec![vec!["1".into()]; 100_000],
        };
        let fixed = wild.repaired();
        assert!(fixed.is_valid());
        assert_eq!(fixed.name.chars().count(), MAX_TEXT);
        let empty = TableSpec {
            name: String::new(),
            columns: vec![],
            rows: vec![],
        }
        .repaired();
        assert!(empty.is_valid());
    }

    #[test]
    fn json_round_trip() {
        let t = filled();
        let back: TableSpec = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn csv_in_with_headings_and_units() {
        let t = TableSpec::from_csv("x [m];t [s]\n1,5;2\n3;4,25\n").unwrap();
        assert_eq!(t.columns[0].unit, "m");
        assert_eq!(t.columns[1].header, "t");
        assert_eq!(t.rows, vec![vec!["1.5", "2"], vec!["3", "4.25"]]);
        let t = t.with_name("D");
        assert_eq!(t.source().unwrap(), "D := [1.5 m, 2 s; 3 m, 4.25 s]");
    }

    #[test]
    fn csv_in_without_headings_and_errors() {
        let t = TableSpec::from_csv("1,2\n3,4\n").unwrap();
        assert!(t.columns.iter().all(|c| c.header.is_empty()));
        assert!(TableSpec::from_csv("1,2\n3,x\n")
            .unwrap_err()
            .contains("not a number"));
        let wide = vec!["1"; MAX_COLUMNS + 1].join(",");
        assert!(TableSpec::from_csv(&wide).unwrap_err().contains("columns"));
        let tall = "1,2\n".repeat(MAX_CELLS);
        assert!(TableSpec::from_csv(&tall).unwrap_err().contains("values"));
    }

    #[test]
    fn csv_out_and_back() {
        let t = filled();
        let text = t.to_csv(',');
        assert!(text.starts_with("col1 [m],col2 [s]\r\n1,2\r\n"));
        let back = TableSpec::from_csv(&text).unwrap();
        assert_eq!(back.rows, t.rows);
        assert_eq!(back.columns[0].unit, "m");
    }
}
