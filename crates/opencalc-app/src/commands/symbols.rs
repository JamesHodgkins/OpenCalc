//! The Greek letters the ribbon flyout offers, and their names.

/// Names of the symbols the Greek flyout offers.
const SYMBOLS: [(char, &str); 16] = [
    ('\u{03b1}', "alpha"),
    ('\u{03b2}', "beta"),
    ('\u{03b3}', "gamma"),
    ('\u{03b4}', "delta"),
    ('\u{03b5}', "epsilon"),
    ('\u{03b8}', "theta"),
    ('\u{03bb}', "lambda"),
    ('\u{03bc}', "mu"),
    ('\u{03c0}', "pi"),
    ('\u{03c1}', "rho"),
    ('\u{03c3}', "sigma"),
    ('\u{03c4}', "tau"),
    ('\u{03c6}', "phi"),
    ('\u{03c9}', "omega"),
    ('\u{0394}', "Delta"),
    ('\u{03a9}', "Omega"),
];

/// The Greek letter for a name such as `alpha` or `Delta` (case matters).
pub fn greek_by_name(name: &str) -> Option<char> {
    SYMBOLS.iter().find(|(_, n)| *n == name).map(|(c, _)| *c)
}

/// The characters of the Greek flyout, in display order.
pub fn greek_symbols() -> impl Iterator<Item = char> {
    SYMBOLS.iter().map(|(c, _)| *c)
}

pub(super) fn symbol_name(c: char) -> &'static str {
    SYMBOLS
        .iter()
        .find(|(s, _)| *s == c)
        .map_or("Symbol", |(_, n)| n)
}
