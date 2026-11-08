//! Reading and writing delimited text (CSV and its relatives).
//!
//! Written from RFC 4180 and common practice; no external parser. Everything is
//! bounded, because the input is a file someone else made: a size limit on the text,
//! on the number of rows and columns, and on the length of one field. Nothing here
//! touches the file system; callers pass text in and take text out.

use std::fmt;

/// The most bytes of text accepted.
pub const MAX_BYTES: usize = 10 * 1024 * 1024;
/// The most rows accepted.
pub const MAX_ROWS: usize = 100_000;
/// The most columns accepted in a row.
pub const MAX_COLS: usize = 1024;
/// The longest single field accepted, in bytes.
pub const MAX_FIELD: usize = 64 * 1024;

/// Why text could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CsvError {
    TooLarge,
    TooManyRows,
    TooManyColumns {
        row: usize,
    },
    FieldTooLong {
        row: usize,
    },
    /// A quoted field that never closes.
    UnclosedQuote {
        row: usize,
    },
}

impl fmt::Display for CsvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CsvError::TooLarge => write!(f, "the file is larger than {} MB", MAX_BYTES >> 20),
            CsvError::TooManyRows => write!(f, "more than {MAX_ROWS} rows"),
            CsvError::TooManyColumns { row } => {
                write!(f, "row {row} has more than {MAX_COLS} columns")
            }
            CsvError::FieldTooLong { row } => write!(f, "a field in row {row} is too long"),
            CsvError::UnclosedQuote { row } => {
                write!(f, "a quoted field starting in row {row} never closes")
            }
        }
    }
}

impl std::error::Error for CsvError {}

/// Guesses the separator from the first lines: whichever of comma, semicolon, tab
/// and pipe gives the same, largest count (outside quotes) on the most lines.
/// Falls back to a comma.
pub fn detect_delimiter(text: &str) -> char {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut best = (',', 0usize);
    for d in [',', ';', '\t', '|'] {
        let counts: Vec<usize> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .take(20)
            .map(|l| count_outside_quotes(l, d))
            .collect();
        let first = counts.first().copied().unwrap_or(0);
        if first == 0 {
            continue;
        }
        let agreeing = counts.iter().filter(|&&c| c == first).count();
        let score = agreeing * 1000 + first;
        if score > best.1 {
            best = (d, score);
        }
    }
    best.0
}

fn count_outside_quotes(line: &str, d: char) -> usize {
    let mut quoted = false;
    let mut n = 0;
    for c in line.chars() {
        if c == '"' {
            quoted = !quoted;
        } else if c == d && !quoted {
            n += 1;
        }
    }
    n
}

/// Splits text into rows of fields.
///
/// Fields may be quoted with `"`, a doubled `""` inside quotes is one quote, and a
/// quoted field may hold the separator and line breaks. Line endings may be `\n`,
/// `\r\n` or a lone `\r`. A leading byte order mark is dropped. A blank line
/// (outside quotes) is skipped. Rows may differ in length; the caller decides.
pub fn parse(text: &str, delimiter: char) -> Result<Vec<Vec<String>>, CsvError> {
    if text.len() > MAX_BYTES {
        return Err(CsvError::TooLarge);
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false; // inside quotes
    let mut was_quoted = false; // this field had quotes, so it is not "blank"
    let mut quote_row = 0;
    let mut chars = text.chars().peekable();

    // Ends the field; ends the row when `end_row`.
    macro_rules! finish_field {
        () => {{
            if row.len() >= MAX_COLS {
                return Err(CsvError::TooManyColumns {
                    row: rows.len() + 1,
                });
            }
            row.push(std::mem::take(&mut field));
        }};
    }
    macro_rules! finish_row {
        () => {{
            let blank = row.is_empty() && field.is_empty() && !was_quoted;
            if !blank {
                finish_field!();
                if rows.len() >= MAX_ROWS {
                    return Err(CsvError::TooManyRows);
                }
                rows.push(std::mem::take(&mut row));
            }
        }};
    }

    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field.is_empty() && !was_quoted {
            quoted = true;
            was_quoted = true;
            quote_row = rows.len() + 1;
        } else if c == delimiter {
            finish_field!();
            was_quoted = false;
        } else if c == '\n' {
            finish_row!();
            was_quoted = false;
        } else if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            finish_row!();
            was_quoted = false;
        } else {
            field.push(c);
        }
        if field.len() > MAX_FIELD {
            return Err(CsvError::FieldTooLong {
                row: rows.len() + 1,
            });
        }
    }
    if quoted {
        return Err(CsvError::UnclosedQuote { row: quote_row });
    }
    finish_row!();
    Ok(rows)
}

/// Reads a number as written in a data file: optional sign, digits with a point,
/// an optional exponent, surrounding spaces ignored. Not `inf`, `nan` or hex, and
/// no thousands separators. Returns `None` for anything else.
pub fn parse_number(field: &str) -> Option<f64> {
    let t = field.trim();
    if t.is_empty() {
        return None;
    }
    let b = t.as_bytes();
    let mut i = 0;
    if b[i] == b'+' || b[i] == b'-' {
        i += 1;
    }
    let mut digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

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
pub fn to_grid(rows: &[Vec<String>]) -> Result<Grid, GridError> {
    let Some(first) = rows.first() else {
        return Err(GridError::Empty);
    };
    let width = first.len();
    let header = first.iter().all(|f| parse_number(f).is_none());
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
            match parse_number(f) {
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

/// Writes one field, quoting when needed. A field that starts like a spreadsheet
/// formula (`=`, `+`, `-`, `@`) and is not a number gets a leading apostrophe-free
/// guard: it is quoted and prefixed with a space, so opening the file in a
/// spreadsheet cannot run it.
fn write_field(out: &mut String, field: &str, delimiter: char) {
    let risky = field
        .chars()
        .next()
        .is_some_and(|c| matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r'))
        && parse_number(field).is_none();
    let needs = risky
        || field.contains(delimiter)
        || field.contains(['"', '\n', '\r'])
        || field.starts_with(' ')
        || field.ends_with(' ');
    if !needs {
        out.push_str(field);
        return;
    }
    out.push('"');
    if risky {
        out.push(' ');
    }
    for c in field.chars() {
        if c == '"' {
            out.push('"');
        }
        out.push(c);
    }
    out.push('"');
}

/// Writes rows of fields as text, with `\r\n` line endings as RFC 4180 says.
pub fn write(rows: &[Vec<String>], delimiter: char) -> String {
    let mut out = String::new();
    for r in rows {
        for (j, f) in r.iter().enumerate() {
            if j > 0 {
                out.push(delimiter);
            }
            write_field(&mut out, f, delimiter);
        }
        out.push_str("\r\n");
    }
    out
}

/// Writes numbers so they read back exactly (shortest round-trip form).
pub fn number_text(v: f64) -> String {
    format!("{v}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(t: &str) -> Vec<Vec<String>> {
        parse(t, ',').unwrap()
    }

    fn s(rows: &[&[&str]]) -> Vec<Vec<String>> {
        rows.iter()
            .map(|r| r.iter().map(|f| (*f).to_owned()).collect())
            .collect()
    }

    #[test]
    fn plain_rows() {
        assert_eq!(p("a,b\n1,2\n"), s(&[&["a", "b"], &["1", "2"]]));
    }

    #[test]
    fn line_endings() {
        let want = s(&[&["1", "2"], &["3", "4"]]);
        assert_eq!(p("1,2\r\n3,4\r\n"), want);
        assert_eq!(p("1,2\r3,4\r"), want);
        assert_eq!(p("1,2\n3,4"), want);
    }

    #[test]
    fn quotes_and_embedded_breaks() {
        assert_eq!(
            p("\"a,b\",\"say \"\"hi\"\"\",\"x\ny\"\n"),
            s(&[&["a,b", "say \"hi\"", "x\ny"]])
        );
    }

    #[test]
    fn blank_lines_skipped_but_empty_quoted_field_kept() {
        assert_eq!(p("1\n\n2\n"), s(&[&["1"], &["2"]]));
        assert_eq!(p("\"\"\n"), s(&[&[""]]));
        assert_eq!(p("1,\n"), s(&[&["1", ""]]));
    }

    #[test]
    fn byte_order_mark_dropped() {
        assert_eq!(p("\u{feff}a,b\n"), s(&[&["a", "b"]]));
    }

    #[test]
    fn unclosed_quote_is_an_error() {
        assert_eq!(
            parse("1,2\n\"abc", ','),
            Err(CsvError::UnclosedQuote { row: 2 })
        );
    }

    #[test]
    fn limits() {
        let wide = vec!["1"; MAX_COLS + 1].join(",");
        assert!(matches!(
            parse(&wide, ','),
            Err(CsvError::TooManyColumns { .. })
        ));
        let tall = "1\n".repeat(MAX_ROWS + 1);
        assert_eq!(parse(&tall, ','), Err(CsvError::TooManyRows));
        let long = "x".repeat(MAX_FIELD + 1);
        assert!(matches!(
            parse(&long, ','),
            Err(CsvError::FieldTooLong { .. })
        ));
        let huge = "a".repeat(MAX_BYTES + 1);
        assert_eq!(parse(&huge, ','), Err(CsvError::TooLarge));
    }

    #[test]
    fn delimiter_detection() {
        assert_eq!(detect_delimiter("a,b,c\n1,2,3\n"), ',');
        assert_eq!(detect_delimiter("a;b;c\n1;2;3\n"), ';');
        assert_eq!(detect_delimiter("a\tb\n1\t2\n"), '\t');
        assert_eq!(detect_delimiter("\"a,b\";c\n\"1,2\";3\n"), ';');
        assert_eq!(detect_delimiter("single\n"), ',');
        assert_eq!(detect_delimiter(""), ',');
    }

    #[test]
    fn numbers() {
        for (t, v) in [
            ("1", 1.0),
            (" -2.5 ", -2.5),
            ("+3", 3.0),
            (".5", 0.5),
            ("5.", 5.0),
            ("1e3", 1000.0),
            ("1.5E-2", 0.015),
        ] {
            assert_eq!(parse_number(t), Some(v), "{t}");
        }
        for t in [
            "", " ", "abc", "1,5", "1 000", "inf", "NaN", "0x10", "1e", "e5", "-", ".", "1e999",
            "1_0",
        ] {
            assert_eq!(parse_number(t), None, "{t}");
        }
    }

    #[test]
    fn grid_with_headers() {
        let g = to_grid(&s(&[&["x [m]", "t [s]"], &["1", "2"], &["3", "4.5"]])).unwrap();
        assert_eq!(g.headers.as_deref().map(<[String]>::len), Some(2));
        assert_eq!(g.rows, vec![vec![1.0, 2.0], vec![3.0, 4.5]]);
    }

    #[test]
    fn grid_without_headers() {
        let g = to_grid(&s(&[&["1", "2"], &["3", "4"]])).unwrap();
        assert!(g.headers.is_none());
        assert_eq!(g.rows.len(), 2);
    }

    #[test]
    fn a_lone_text_row_is_not_data() {
        assert_eq!(to_grid(&s(&[&["a", "b"]])), Err(GridError::Empty));
        assert_eq!(to_grid(&[]), Err(GridError::Empty));
    }

    #[test]
    fn grid_errors_name_the_place() {
        assert_eq!(
            to_grid(&s(&[&["1", "2"], &["3"]])),
            Err(GridError::Ragged {
                row: 2,
                found: 1,
                expected: 2
            })
        );
        let e = to_grid(&s(&[&["1", "2"], &["3", "x"]])).unwrap_err();
        assert_eq!(e.to_string(), "row 2, column 2: \"x\" is not a number");
        // a blank cell is not a number either
        assert!(matches!(
            to_grid(&s(&[&["1", ""]])),
            Err(GridError::NotANumber { col: 2, .. })
        ));
    }

    #[test]
    fn headers_split() {
        assert_eq!(split_header("force [kN]"), ("force".into(), "kN".into()));
        assert_eq!(split_header("  plain "), ("plain".into(), String::new()));
        assert_eq!(split_header("a [b] c"), ("a [b] c".into(), String::new()));
        assert_eq!(split_header("[m]"), (String::new(), "m".into()));
    }

    #[test]
    fn write_quotes_when_needed() {
        let out = write(&s(&[&["a,b", "say \"hi\"", "plain", " pad"]]), ',');
        assert_eq!(out, "\"a,b\",\"say \"\"hi\"\"\",plain,\" pad\"\r\n");
    }

    #[test]
    fn write_defuses_formulas_but_keeps_numbers() {
        let out = write(&s(&[&["=1+1", "-5", "+2.5", "@x", "-abc"]]), ',');
        assert_eq!(out, "\" =1+1\",-5,+2.5,\" @x\",\" -abc\"\r\n");
    }

    #[test]
    fn round_trip() {
        let rows = s(&[
            &["name", "note"],
            &["a,b", "line\nbreak"],
            &["q\"q", ""],
            &["", "x"],
        ]);
        for d in [',', ';', '\t'] {
            assert_eq!(parse(&write(&rows, d), d).unwrap(), rows, "{d:?}");
        }
    }

    #[test]
    fn numbers_round_trip_exactly() {
        for v in [
            0.1,
            1.0 / 3.0,
            1e-300,
            6.02214076e23,
            -0.0,
            123_456_789.123_456_79,
        ] {
            let back = parse_number(&number_text(v)).unwrap();
            assert_eq!(back.to_bits(), v.to_bits(), "{v}");
        }
    }

    #[test]
    fn hostile_text_never_panics() {
        // A cheap fuzz: every string over a small alphabet up to length 7.
        let alphabet = ['"', ',', '\n', '\r', 'a', '1', ' '];
        let mut stack = vec![String::new()];
        let mut seen = 0;
        while let Some(t) = stack.pop() {
            let _ = parse(&t, ',').map(|r| write(&r, ','));
            let _ = detect_delimiter(&t);
            seen += 1;
            if t.chars().count() < 7 {
                for c in alphabet {
                    let mut n = t.clone();
                    n.push(c);
                    stack.push(n);
                }
            }
        }
        assert!(seen > 100_000);
    }
}
