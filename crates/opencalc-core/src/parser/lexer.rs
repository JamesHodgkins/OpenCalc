//! Tokens: turning source text into symbols with the character range of each.

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tok {
    Num(f64),
    Ident(String),
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
    /// Text that is not a token (`parse_lenient` only): kept so the display can
    /// show it as typed.
    Bad(String),
}

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
            c if c.is_whitespace() => i += 1,
            '.' if chars.get(i + 1) == Some(&'.') => {
                toks.push(Tok::DotDot);
                i += 2;
            }
            '0'..='9' | '.' => {
                let start = i;
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
    Ok((toks, spans))
}

pub(super) fn describe(t: &Tok) -> String {
    match t {
        Tok::Num(v) => format!("number {v}"),
        Tok::Ident(n) => format!("'{n}'"),
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
