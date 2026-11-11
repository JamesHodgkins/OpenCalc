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

/// A lexing failure: the message and the character range it is about.
type LexError = (String, usize, usize);

/// With `lenient`, a character or number that is not valid becomes a `Tok::Bad`
/// instead of an error.
pub(super) fn tokenize(src: &str, lenient: bool) -> Result<Tokens, LexError> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (before, begin) = (toks.len(), i);
        let c = chars[i];
        if let Some((tok, len)) = symbol(&chars, i) {
            toks.push(tok);
            i += len;
        } else if c == '"' {
            i = text(&chars, i, lenient, &mut toks)?;
        } else if c == '\n' {
            if toks.last().is_some_and(|t| *t != Tok::Line) {
                toks.push(Tok::Line);
            }
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            i = number(&chars, i, lenient, &mut toks)?;
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            toks.push(Tok::Ident(chars[start..i].iter().collect()));
        } else if lenient {
            i = unknown(&chars, i, &mut toks);
        } else {
            return Err((format!("unexpected '{c}'"), i, i + 1));
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

/// The operator or bracket at `i`, with the number of characters it takes. Two-
/// character forms (`:=`, `==`, `..`) are tried before their first character.
fn symbol(chars: &[char], i: usize) -> Option<(Tok, usize)> {
    let next = chars.get(i + 1).copied();
    let one = |t: Tok| Some((t, 1));
    let two = |t: Tok| Some((t, 2));
    match (chars[i], next) {
        ('.', Some('.')) => two(Tok::DotDot),
        (':', Some('=')) => two(Tok::Define),
        ('=', Some('=')) => two(Tok::EqEq),
        ('!', Some('=')) => two(Tok::NotEq),
        ('<', Some('=')) => two(Tok::LessEq),
        ('>', Some('=')) => two(Tok::GreaterEq),
        ('\u{2190}', _) => one(Tok::Arrow),
        // The global definition sign. Every definition here is global already
        // (see docs/DOCUMENT_FEATURES.md), so it means the same as `:=`.
        ('\u{2261}', _) => one(Tok::Define),
        ('+', _) => one(Tok::Plus),
        ('-' | '\u{2212}', _) => one(Tok::Minus),
        ('*' | '\u{00b7}' | '\u{00d7}', _) => one(Tok::Star),
        ('/', _) => one(Tok::Slash),
        ('^', _) => one(Tok::Caret),
        ('(', _) => one(Tok::LParen),
        (')', _) => one(Tok::RParen),
        (',', _) => one(Tok::Comma),
        ('=', _) => one(Tok::Equals),
        ('\u{2260}', _) => one(Tok::NotEq),
        ('\u{2264}', _) => one(Tok::LessEq),
        ('<', _) => one(Tok::Less),
        ('\u{2265}', _) => one(Tok::GreaterEq),
        ('>', _) => one(Tok::Greater),
        ('[', _) => one(Tok::LBracket),
        (']', _) => one(Tok::RBracket),
        (';', _) => one(Tok::Semicolon),
        _ => None,
    }
}

/// Text in quotes starting at `i`. Returns where the next token starts.
fn text(chars: &[char], i: usize, lenient: bool, toks: &mut Vec<Tok>) -> Result<usize, LexError> {
    let start = i;
    let mut i = i + 1;
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
    Ok(i)
}

/// A number starting at `i`: decimal with an optional exponent, or `0x`, `0b`, `0o`.
/// Returns where the next token starts.
fn number(chars: &[char], i: usize, lenient: bool, toks: &mut Vec<Tok>) -> Result<usize, LexError> {
    let start = i;
    if let Some((radix, end)) = radix_literal(chars, i) {
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
        return Ok(end);
    }
    let mut i = i;
    while i < chars.len()
        && (chars[i].is_ascii_digit() || (chars[i] == '.' && chars.get(i + 1) != Some(&'.')))
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
    Ok(i)
}

/// Lenient mode only: a character that starts nothing. A Greek name being entered,
/// `\alpha`, is one piece, not a stray backslash followed by a name.
fn unknown(chars: &[char], i: usize, toks: &mut Vec<Tok>) -> usize {
    if chars[i] == '\\' {
        let mut end = i + 1;
        while end < chars.len() && chars[end].is_ascii_alphabetic() {
            end += 1;
        }
        toks.push(Tok::Bad(chars[i..end].iter().collect()));
        end
    } else {
        toks.push(Tok::Bad(chars[i].to_string()));
        i + 1
    }
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
