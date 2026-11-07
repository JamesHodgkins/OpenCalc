//! Autocomplete: names to finish the word being typed, from the built-in
//! functions, the names defined in the worksheet and the units. Pure, so every rule
//! is tested without a window.

use opencalc_core::constants::CONSTANTS;
use opencalc_core::functions::FUNCTIONS;
use opencalc_core::units::builtin_symbols;

use super::Field;

/// Most entries offered at once.
pub const MAX_SHOWN: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Variable,
    Constant,
    Function,
    Unit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// What the list shows: a function's name or the name itself.
    pub label: String,
    /// What accepting it types: a function's name and an opening bracket, else the name.
    pub insert: String,
    /// A short explanation: a function's signature and summary.
    pub detail: String,
    pub kind: Kind,
}

/// The word being typed just before the caret, and where it starts. A number's
/// digits are not part of it (`2x` is the word `x`), and neither is a name after a
/// backslash (that is a Greek letter being entered).
pub fn word_before(field: &Field) -> Option<(usize, String)> {
    let chars = field.chars();
    let end = field.caret.min(chars.len());
    let mut start = end;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    while start < end && chars[start].is_ascii_digit() {
        start += 1;
    }
    if start == end || (start > 0 && chars[start - 1] == '\\') {
        return None;
    }
    Some((start, chars[start..end].iter().collect()))
}

/// Whether the word comes straight after a number (`5 m`, `2.5kN`), where a unit
/// is far more likely than a function.
pub fn after_number(field: &Field, start: usize) -> bool {
    let chars = field.chars();
    let before = chars[..start.min(chars.len())]
        .iter()
        .rev()
        .find(|c| **c != ' ');
    before.is_some_and(|c| c.is_ascii_digit() || *c == '.')
}

/// What to offer for `word`. A name that is exactly what was typed comes first, so
/// a variable you have already typed in full is still confirmed as defined (and `m`
/// is offered before `mm`); then defined names, functions and units. A unit comes
/// before the functions when `units_first` (the word follows a number).
pub fn candidates(word: &str, defined: &[String], units_first: bool) -> Vec<Candidate> {
    let lower = word.to_lowercase();
    let matches = |name: &str| name.to_lowercase().starts_with(&lower);
    let mut out: Vec<Candidate> = Vec::new();
    let mut push = |c: Candidate| {
        if !out.iter().any(|o| o.insert == c.insert) {
            out.push(c);
        }
    };
    let mut names: Vec<&String> = defined.iter().filter(|n| matches(n)).collect();
    names.sort_by_key(|n| (!n.starts_with(word), n.len(), (*n).clone()));
    for n in names {
        push(Candidate {
            label: n.clone(),
            insert: n.clone(),
            detail: "defined in this worksheet".to_owned(),
            kind: Kind::Variable,
        });
    }
    for c in CONSTANTS.iter().filter(|c| matches(c.name)) {
        push(Candidate {
            label: c.name.to_owned(),
            insert: c.name.to_owned(),
            detail: format!("constant \u{2014} {}", c.summary),
            kind: Kind::Constant,
        });
    }
    let mut functions: Vec<Candidate> = Vec::new();
    for f in FUNCTIONS.iter().filter(|f| matches(f.name)) {
        functions.push(Candidate {
            label: f.name.to_owned(),
            insert: format!("{}(", f.name),
            detail: format!("{}  \u{2014} {}", f.signature, f.summary),
            kind: Kind::Function,
        });
    }
    if matches("if") {
        functions.push(Candidate {
            label: "if".to_owned(),
            insert: "if(".to_owned(),
            detail: "if(condition, then, otherwise)  \u{2014} one of two values".to_owned(),
            kind: Kind::Function,
        });
    }
    let mut units: Vec<&str> = builtin_symbols().filter(|u| matches(u)).collect();
    units.sort_by_key(|u| (!u.starts_with(word), u.len(), (*u).to_owned()));
    let mut unit_list: Vec<Candidate> = Vec::new();
    for u in units {
        unit_list.push(Candidate {
            label: u.to_owned(),
            insert: u.to_owned(),
            detail: "unit".to_owned(),
            kind: Kind::Unit,
        });
    }
    let (first, second) = if units_first {
        (unit_list, functions)
    } else {
        (functions, unit_list)
    };
    for c in first.into_iter().chain(second) {
        push(c);
    }
    // Stable, so the order above is kept within each group.
    out.sort_by_key(|c| c.insert.trim_end_matches('(') != word);
    out.truncate(MAX_SHOWN);
    out
}

/// The list to show for the field right now: only while the user is typing a word,
/// and not after Escape closed it for that word.
pub fn for_field(field: &mut Field, defined: &[String]) -> (usize, Vec<Candidate>) {
    let Some((start, word)) = word_before(field) else {
        field.dismissed = None;
        return (0, Vec::new());
    };
    if field.dismissed.as_deref() != Some(word.as_str()) {
        field.dismissed = None;
    }
    if !field.typing || field.dismissed.is_some() || field.has_selection() {
        return (start, Vec::new());
    }
    let units_first = after_number(field, start);
    (start, candidates(&word, defined, units_first))
}

/// Whether an `=` typed now should be `:=`: the region holds nothing but one name,
/// the caret is after it, and nothing in the worksheet, the units or the constants
/// has that name. Evaluating a name that means nothing is only ever an error, so
/// the person almost certainly meant to define it. Anything else stays as typed.
pub fn defines_by_equals(field: &Field, defined: &[String]) -> bool {
    let chars = field.chars();
    if field.has_selection() || field.caret != chars.len() || chars.is_empty() {
        return false;
    }
    let name: String = chars.iter().collect();
    let is_name = (chars[0].is_alphabetic() || chars[0] == '_')
        && chars.iter().all(|c| c.is_alphanumeric() || *c == '_');
    if !is_name || defined.contains(&name) {
        return false;
    }
    // Units and constants are known to the evaluator; ask it.
    let id = opencalc_core::eval::RegionId(0);
    let known = opencalc_core::eval::evaluate(&[(id, opencalc_core::parser::parse(&name))]);
    matches!(
        known.values.get(&id),
        Some(Err(opencalc_core::eval::EvalError::Undefined(_)))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::parser::parse;

    fn field_at_end(text: &str) -> Field {
        Field::new(text)
    }

    #[test]
    fn the_word_is_what_is_before_the_caret_without_a_numbers_digits() {
        let w = |t: &str| word_before(&field_at_end(t));
        assert_eq!(w("x + sig"), Some((4, "sig".to_owned())));
        assert_eq!(w("2x"), Some((1, "x".to_owned())));
        assert_eq!(w("1.5e"), Some((3, "e".to_owned())));
        assert_eq!(w("F_x"), Some((0, "F_x".to_owned())));
        assert_eq!(w("a + "), None);
        assert_eq!(w("123"), None);
        assert_eq!(w(""), None);
        assert_eq!(w("\\alp"), None, "a Greek name is not completed");
        assert_eq!(w("\u{3b1}b"), Some((0, "\u{3b1}b".to_owned())));
    }

    #[test]
    fn functions_defined_names_and_units_are_offered_in_that_order() {
        let defined = vec!["sigma_y".to_owned(), "s1".to_owned()];
        let got = candidates("s", &defined, false);
        let labels: Vec<&str> = got.iter().map(|c| c.label.as_str()).collect();
        // `s` is itself a unit (seconds), so what was typed leads; then the
        // defined names, shortest first.
        assert_eq!(&labels[..3], ["s", "s1", "sigma_y"]);
        assert!(labels.contains(&"sin") || labels.contains(&"sqrt"));
        assert!(got.len() <= MAX_SHOWN);
        let kinds: Vec<Kind> = got.iter().map(|c| c.kind).collect();
        let first_function = kinds.iter().position(|k| *k == Kind::Function);
        let last_variable = kinds.iter().rposition(|k| *k == Kind::Variable);
        if let (Some(f), Some(v)) = (first_function, last_variable) {
            assert!(v < f);
        }
    }

    #[test]
    fn a_function_inserts_its_bracket_and_a_complete_name_is_not_offered_again() {
        let got = candidates("sqr", &[], false);
        assert_eq!(got.first().map(|c| c.insert.as_str()), Some("sqrt("));
        assert!(got[0].detail.contains("sqrt"));
        // The exact name of a function still offers it (to add the bracket).
        assert!(candidates("sqrt", &[], false)
            .iter()
            .any(|c| c.insert == "sqrt("));
        // Matching ignores case, so `kn` finds `kN`.
        assert!(candidates("kn", &[], false)
            .iter()
            .any(|c| c.insert == "kN"));
        assert!(candidates("zzzzz", &["a".to_owned()], false).is_empty());
    }

    #[test]
    fn nothing_offered_is_ever_something_that_would_not_parse() {
        let defined = vec!["F_x".to_owned(), "\u{3c3}".to_owned(), "d1".to_owned()];
        let mut seen = 0;
        for first in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_\u{3c3}F".chars() {
            for second in ["", "a", "e", "i", "o", "m", "N", "_", "1"] {
                for c in candidates(&format!("{first}{second}"), &defined, false) {
                    seen += 1;
                    let name = c.insert.trim_end_matches('(');
                    assert!(
                        name.chars()
                            .next()
                            .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
                            && name.chars().all(|ch| ch.is_alphanumeric() || ch == '_'),
                        "{c:?} is not a name"
                    );
                    let source = if c.kind == Kind::Function {
                        format!("{}1)", c.insert)
                    } else {
                        c.insert.clone()
                    };
                    assert!(
                        !matches!(parse(&source), opencalc_core::expr::Expr::Error(_)),
                        "{c:?} gives {source:?}, which does not parse"
                    );
                }
            }
        }
        assert!(seen > 100, "only {seen} candidates checked");
    }

    #[test]
    fn what_was_typed_in_full_is_offered_first_so_a_defined_name_is_confirmed() {
        // `b` is defined: it leads, ahead of units that merely start with b.
        let got = candidates("b", &["b".to_owned(), "beam".to_owned()], false);
        assert_eq!(got[0].insert, "b");
        assert_eq!(got[0].kind, Kind::Variable);
        assert!(got.iter().any(|c| c.insert == "beam"));
        // `m` is a unit and leads `mm`, `min` and the functions starting with m.
        let got = candidates("m", &[], false);
        assert_eq!((got[0].insert.as_str(), got[0].kind), ("m", Kind::Unit));
        assert!(got.iter().any(|c| c.insert == "mm"));
        // A defined name is not shadowed by a unit of the same name.
        let got = candidates("m", &["m".to_owned()], false);
        assert_eq!(got[0].kind, Kind::Variable);
        assert_eq!(got.iter().filter(|c| c.insert == "m").count(), 1);
    }

    #[test]
    fn constants_follow_defined_names_and_a_definition_takes_the_constants_place() {
        let got = candidates("p", &["pressure".to_owned()], false);
        let labels: Vec<&str> = got.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(&labels[..2], ["pressure", "pi"]);
        assert_eq!(got[1].kind, Kind::Constant);
        assert_eq!(candidates("g_", &[], false)[0].insert, "g_n");
        assert_eq!(candidates("\u{3c0}", &[], false)[0].insert, "\u{3c0}");
        let own = candidates("pi", &["pi".to_owned()], false);
        assert_eq!(own.iter().filter(|c| c.insert == "pi").count(), 1);
        assert_eq!(own[0].kind, Kind::Variable);
    }

    #[test]
    fn after_a_number_units_come_before_functions() {
        let kinds = |units_first| -> Vec<Kind> {
            candidates("m", &[], units_first)
                .iter()
                .skip(1)
                .map(|c| c.kind)
                .collect()
        };
        let plain = kinds(false);
        let after = kinds(true);
        assert_eq!(plain.first(), Some(&Kind::Function));
        assert_eq!(after.first(), Some(&Kind::Unit));
        assert!(after_number(&Field::new("5 m"), 2));
        assert!(after_number(&Field::new("2.5m"), 3));
        assert!(!after_number(&Field::new("a + m"), 4));
        assert!(!after_number(&Field::new("m"), 0));
    }

    #[test]
    fn the_list_shows_only_while_typing_and_stays_closed_after_escape() {
        let defined = vec!["force".to_owned()];
        let mut f = Field::new("");
        assert!(
            for_field(&mut f, &defined).1.is_empty(),
            "nothing typed yet"
        );
        f.typed("f", |_| None);
        f.typed("o", |_| None);
        let (start, list) = for_field(&mut f, &defined);
        assert_eq!(start, 0);
        assert!(list.iter().any(|c| c.insert == "force"));
        // Escape closes it for this word; more typing opens it again.
        f.dismissed = Some("fo".to_owned());
        assert!(for_field(&mut f, &defined).1.is_empty());
        f.typed("r", |_| None);
        assert!(!for_field(&mut f, &defined).1.is_empty());
        // Moving the caret hides it.
        f.set_caret(1, false);
        assert!(for_field(&mut f, &defined).1.is_empty());
    }
}
