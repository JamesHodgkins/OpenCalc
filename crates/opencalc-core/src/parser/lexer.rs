//! Tokens: turning source text into symbols with the character range of each.

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tok {
    Num(f64),
    Ident(String),
    /// Text in quotes, without the quotes.
    Str(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
    Define,
    Equals,
    LBracket,
    RBracket,
    Semicolon,
    EqEq,
    NotEq,
    Less,
    Greater,
    LessEq,
    GreaterEq,
    DotDot,
    /// A line break (consecutive ones are one). Only programs use them.
    Line,
    /// `\u{2190}`: assignment to a local variable of a program.
    Arrow,
    /// Text that is not a token (`parse_lenient` only): kept so the display can
    /// show it as typed.
    Bad(String),
}

/// The longest text in quotes.
const MAX_TEXT_CHARS: usize = 256;

/// Tokens with the character range each came from.
pub(super) type Tokens = (Vec<Tok>, Vec<(usize, usize)>);

/// With `lenient`, a character or number that is not valid becomes a `Tok::Bad`
/// instead of an error.
pub(super) fn tokenize(src: &str, lenient: bool) -> Result<Tokens, (String, usize, usize)> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (before, begin) = (toks.len(), i);
        let c = chars[i];
        match c {
            '"' => {
                let start = i;
                i += 1;
                let mut text = String::new();
                while i < chars.len() && chars[i] != '"' && chars[i] != '\n' {
                    text.push(chars[i]);
                    i += 1;
                }
                let closed = chars.get(i) == Some(&'"');
                if closed {
                    i += 1;
                }
                if closed && text.chars().count() <= MAX_TEXT_CHARS {
                    toks.push(Tok::Str(text));
                } else if lenient {
                    toks.push(Tok::Bad(chars[start..i].iter().collect()));
                } else if closed {
                    return Err((
                        format!("text is longer than {MAX_TEXT_CHARS} characters"),
                        start,
                        i,
                    ));
                } else {
                    return Err(("text is missing its closing quote".into(), start, i));
                }
            }
            '\n' => {
                if toks.last().is_some_and(|t| *t != Tok::Line) {
                    toks.push(Tok::Line);
                }
                i += 1;
            }
            '\u{2190}' => {
                toks.push(Tok::Arrow);
                i += 1;
            }
            c if c.is_whitespace() => i += 1,
            '.' if chars.get(i + 1) == Some(&'.') => {
                toks.push(Tok::DotDot);
                i += 2;
            }
            '0'..='9' | '.' => {
                let start = i;
                if let Some((radix, end)) = radix_literal(&chars, i) {
                    i = end;
                    let digits: String = chars[start + 2..end].iter().collect();
                    // Exact only up to 2^53, the integers an f64 holds without rounding.
                    match u64::from_str_radix(&digits, radix) {
                        Ok(v) if v <= (1u64 << 53) => toks.push(Tok::Num(v as f64)),
                        _ if lenient => toks.push(Tok::Bad(chars[start..end].iter().collect())),
                        _ => {
                            return Err((
                                "number too large to hold exactly (most 2^53)".to_string(),
                                start,
                                end,
                            ))
                        }
                    }
                    spans.push((begin, i));
                    continue;
                }
                while i < chars.len()
                    && (chars[i].is_ascii_digit()
                        || (chars[i] == '.' && chars.get(i + 1) != Some(&'.')))
                {
                    i += 1;
                }
                // Exponent: only when `e` is followed by a digit or a signed digit.
                if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                    let mut j = i + 1;
                    if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                        j += 1;
                    }
                    if j < chars.len() && chars[j].is_ascii_digit() {
                        while j < chars.len() && chars[j].is_ascii_digit() {
                            j += 1;
                        }
                        i = j;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                match text.parse::<f64>() {
                    Ok(v) => toks.push(Tok::Num(v)),
                    Err(_) if lenient => toks.push(Tok::Bad(text)),
                    Err(_) => return Err((format!("bad number '{text}'"), start, i)),
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                toks.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            // The global definition sign. Every definition here is global already
            // (see docs/DOCUMENT_FEATURES.md), so it means the same as `:=`.
            '\u{2261}' => {
                toks.push(Tok::Define);
                i += 1;
            }
            ':' if chars.get(i + 1) == Some(&'=') => {
                toks.push(Tok::Define);
                i += 2;
            }
            '+' => {
                toks.push(Tok::Plus);
                i += 1;
            }
            '-' | '\u{2212}' => {
                toks.push(Tok::Minus);
                i += 1;
            }
            '*' | '\u{00b7}' | '\u{00d7}' => {
                toks.push(Tok::Star);
                i += 1;
            }
            '/' => {
                toks.push(Tok::Slash);
                i += 1;
            }
            '^' => {
                toks.push(Tok::Caret);
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            '=' if chars.get(i + 1) == Some(&'=') => {
                toks.push(Tok::EqEq);
                i += 2;
            }
            '=' => {
                toks.push(Tok::Equals);
                i += 1;
            }
            '!' if chars.get(i + 1) == Some(&'=') => {
                toks.push(Tok::NotEq);
                i += 2;
            }
            '\u{2260}' => {
                toks.push(Tok::NotEq);
                i += 1;
            }
            '<' if chars.get(i + 1) == Some(&'=') => {
                toks.push(Tok::LessEq);
                i += 2;
            }
            '\u{2264}' => {
                toks.push(Tok::LessEq);
                i += 1;
            }
            '<' => {
                toks.push(Tok::Less);
                i += 1;
            }
            '>' if chars.get(i + 1) == Some(&'=') => {
                toks.push(Tok::GreaterEq);
                i += 2;
            }
            '\u{2265}' => {
                toks.push(Tok::GreaterEq);
                i += 1;
            }
            '>' => {
                toks.push(Tok::Greater);
                i += 1;
            }
            '[' => {
                toks.push(Tok::LBracket);
                i += 1;
            }
            ']' => {
                toks.push(Tok::RBracket);
                i += 1;
            }
            ';' => {
                toks.push(Tok::Semicolon);
                i += 1;
            }
            // A Greek name being entered, `\alpha`, is one piece, not a stray
            // backslash followed by a name.
            '\\' if lenient => {
                let start = i;
                i += 1;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                toks.push(Tok::Bad(chars[start..i].iter().collect()));
            }
            other if lenient => {
                toks.push(Tok::Bad(other.to_string()));
                i += 1;
            }
            other => return Err((format!("unexpected '{other}'"), i, i + 1)),
        }
        if toks.len() > before {
            spans.push((begin, i));
        }
    }
    // A line break at the end ends nothing.
    while !lenient && toks.last() == Some(&Tok::Line) {
        toks.pop();
        spans.pop();
    }
    Ok((toks, spans))
}

/// `0x1F`, `0b101`, `0o17`: the radix and the end of the literal, when the text at
/// `i` is one. It needs at least one digit of the radix, and the digits must not
/// run straight into a letter or digit that is not one (`0b12` is an error, not
/// `0b1` times `2`), so those fall through to the decimal reader and fail there.
fn radix_literal(chars: &[char], i: usize) -> Option<(u32, usize)> {
    if chars[i] != '0' {
        return None;
    }
    let radix = match chars.get(i + 1)? {
        'x' | 'X' => 16,
        'b' | 'B' => 2,
        'o' | 'O' => 8,
        _ => return None,
    };
    let mut end = i + 2;
    while end < chars.len() && chars[end].is_digit(radix) {
        end += 1;
    }
    if end == i + 2 || chars.get(end).is_some_and(|c| c.is_alphanumeric()) {
        return None;
    }
    Some((radix, end))
}

pub(super) fn describe(t: &Tok) -> String {
    match t {
        Tok::Num(v) => format!("number {v}"),
        Tok::Ident(n) => format!("'{n}'"),
        Tok::Str(s) => format!("text \"{s}\""),
        Tok::Line => "line break".into(),
        Tok::Arrow => "'\u{2190}'".into(),
        Tok::Plus => "'+'".into(),
        Tok::Minus => "'-'".into(),
        Tok::Star => "'*'".into(),
        Tok::Slash => "'/'".into(),
        Tok::Caret => "'^'".into(),
        Tok::LParen => "'('".into(),
        Tok::RParen => "')'".into(),
        Tok::Comma => "','".into(),
        Tok::Define => "':='".into(),
        Tok::Equals => "'='".into(),
        Tok::LBracket => "'['".into(),
        Tok::RBracket => "']'".into(),
        Tok::Semicolon => "';'".into(),
        Tok::EqEq => "'=='".into(),
        Tok::NotEq => "'!='".into(),
        Tok::Less => "'<'".into(),
        Tok::Greater => "'>'".into(),
        Tok::LessEq => "'<='".into(),
        Tok::GreaterEq => "'>='".into(),
        Tok::DotDot => "'..'".into(),
        Tok::Bad(text) => format!("'{text}'"),
    }
}
