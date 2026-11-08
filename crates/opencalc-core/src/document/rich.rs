//! Rich text: styled stretches inside a text region.
//!
//! A text region keeps its text as one string and, beside it, a short list of
//! **runs**: each run says how many characters it covers and how they look (bold,
//! italic, underline, colour). The runs start at the first character and follow on
//! one after another; characters past the last run are plain. A region with no runs
//! is exactly the region that existed before rich text, so old files read unchanged.
//! The region's own `FontStyle` (bold or italic for all of it) still applies, and a
//! run adds to it.

use serde::{Deserialize, Serialize};

/// How many text colours there are to choose from.
pub const TEXT_COLOURS: u8 = 8;

/// How a stretch of text looks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStyle {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    /// An index into the palette of `TEXT_COLOURS`; none is the normal text colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<u8>,
}

impl RunStyle {
    pub fn is_plain(&self) -> bool {
        *self == RunStyle::default()
    }
}

/// `len` characters in one style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub len: u32,
    #[serde(flatten)]
    pub style: RunStyle,
}

/// One style per character, for working on a stretch by position.
fn expand(runs: &[Run], chars: usize) -> Vec<RunStyle> {
    let mut out = Vec::with_capacity(chars);
    for run in runs {
        for _ in 0..run.len {
            if out.len() == chars {
                return out;
            }
            out.push(run.style);
        }
    }
    out.resize(chars, RunStyle::default());
    out
}

/// The shortest list of runs for these per-character styles.
fn compress(styles: &[RunStyle]) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for s in styles {
        match out.last_mut() {
            Some(last) if last.style == *s => last.len += 1,
            _ => out.push(Run { len: 1, style: *s }),
        }
    }
    while out.last().is_some_and(|r| r.style.is_plain()) {
        out.pop();
    }
    out
}

fn clean(style: RunStyle) -> RunStyle {
    RunStyle {
        colour: style.colour.filter(|c| *c < TEXT_COLOURS),
        ..style
    }
}

/// Runs made valid for a text of `chars` characters: cut at the end of the text,
/// empty runs and unknown colours dropped, neighbours in the same style joined, and
/// plain stretches at the end removed.
pub fn normalise(runs: &[Run], chars: usize) -> Vec<Run> {
    let styles: Vec<RunStyle> = expand(runs, chars).into_iter().map(clean).collect();
    compress(&styles)
}

/// The runs after `old` was changed to `new`: what lies before the change and after
/// it keeps its style, and new characters take the style of the character just
/// before them (or just after, at the start), as typing in a word word-processor does.
pub fn adjust(runs: &[Run], old: &str, new: &str) -> Vec<Run> {
    if old == new {
        return runs.to_vec();
    }
    let (o, n): (Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect());
    let prefix = o.iter().zip(&n).take_while(|(a, b)| a == b).count();
    let max_suffix = o.len().min(n.len()) - prefix;
    let suffix = o
        .iter()
        .rev()
        .zip(n.iter().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    let styles = expand(runs, o.len());
    let inherit = if prefix > 0 {
        styles[prefix - 1]
    } else {
        styles.get(prefix).copied().unwrap_or_default()
    };
    let mut next = Vec::with_capacity(n.len());
    next.extend_from_slice(&styles[..prefix]);
    next.resize(n.len() - suffix, inherit);
    next.extend_from_slice(&styles[o.len() - suffix..]);
    compress(&next)
}

/// Runs with `change` applied to the characters `start..end` of a text of `chars`
/// characters.
pub fn restyle(
    runs: &[Run],
    chars: usize,
    start: usize,
    end: usize,
    change: impl Fn(&mut RunStyle),
) -> Vec<Run> {
    let mut styles = expand(runs, chars);
    let end = end.min(chars);
    for s in styles.iter_mut().take(end).skip(start.min(end)) {
        change(s);
        *s = clean(*s);
    }
    compress(&styles)
}

/// Whether every character in `start..end` satisfies `has`. False for an empty range.
pub fn all_have(
    runs: &[Run],
    chars: usize,
    start: usize,
    end: usize,
    has: impl Fn(&RunStyle) -> bool,
) -> bool {
    let styles = expand(runs, chars);
    let end = end.min(chars);
    start < end && styles[start..end].iter().all(has)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bold() -> RunStyle {
        RunStyle {
            bold: true,
            ..RunStyle::default()
        }
    }

    fn run(len: u32, style: RunStyle) -> Run {
        Run { len, style }
    }

    #[test]
    fn restyling_splits_and_joins() {
        let r = restyle(&[], 10, 2, 5, |s| s.bold = true);
        assert_eq!(r, [run(2, RunStyle::default()), run(3, bold())]);
        // making the rest bold joins the neighbours; the plain tail is dropped
        let r = restyle(&r, 10, 0, 2, |s| s.bold = true);
        assert_eq!(r, [run(5, bold())]);
        let r = restyle(&r, 10, 0, 5, |s| s.bold = false);
        assert_eq!(r, []);
        // out-of-range ends are clamped
        assert_eq!(restyle(&[], 4, 2, 99, |s| s.italic = true).len(), 2);
        assert_eq!(restyle(&[], 4, 3, 1, |s| s.italic = true), []);
    }

    #[test]
    fn normalising_cuts_to_the_text_and_cleans_up() {
        let messy = [
            run(0, bold()),
            run(2, bold()),
            run(2, bold()),
            run(
                3,
                RunStyle {
                    colour: Some(200),
                    ..RunStyle::default()
                },
            ),
            run(50, bold()),
        ];
        assert_eq!(normalise(&messy, 5), [run(4, bold())]);
        assert_eq!(normalise(&messy, 0), []);
    }

    #[test]
    fn typing_inherits_the_style_before_it() {
        // "ab" bold then "cd" plain; type X after "ab"
        let runs = [run(2, bold())];
        let after = adjust(&runs, "abcd", "abXcd");
        assert_eq!(after, [run(3, bold())]);
        // at the very start, take the style of the character after
        assert_eq!(adjust(&runs, "abcd", "Xabcd"), [run(3, bold())]);
        // appended after plain text stays plain
        assert_eq!(adjust(&runs, "abcd", "abcdX"), [run(2, bold())]);
    }

    #[test]
    fn deleting_and_replacing_keep_the_rest_in_place() {
        let runs = [
            run(2, RunStyle::default()),
            run(3, bold()),
            run(2, RunStyle::default()),
        ];
        // delete "cd" from "abcdefg" -> "abefg": the bold part loses two characters
        assert_eq!(
            adjust(&runs, "abcdefg", "abefg"),
            [run(2, RunStyle::default()), run(1, bold())]
        );
        // replace everything
        assert_eq!(adjust(&runs, "abcdefg", "xyz"), []);
        // empty to something, something to empty
        assert_eq!(adjust(&[], "", "hi"), []);
        assert_eq!(adjust(&runs, "abcdefg", ""), []);
    }

    #[test]
    fn adjusting_always_leaves_runs_that_fit_the_new_text() {
        let alphabet = ['a', 'b', ' '];
        let mut texts = vec![String::new()];
        for _ in 0..5 {
            let mut next = texts.clone();
            for t in &texts {
                for c in alphabet {
                    let mut s = t.clone();
                    s.push(c);
                    next.push(s);
                }
            }
            next.sort();
            next.dedup();
            texts = next;
        }
        let runs = [run(3, bold()), run(1, RunStyle::default()), run(2, bold())];
        for old in texts.iter().step_by(7) {
            for new in texts.iter().step_by(11) {
                if old == new {
                    continue; // unchanged text leaves the runs as they were
                }
                let r = adjust(&runs, old, new);
                let total: u32 = r.iter().map(|x| x.len).sum();
                assert!(total as usize <= new.chars().count(), "{old:?} -> {new:?}");
                assert!(r.last().is_none_or(|l| !l.style.is_plain()));
            }
        }
    }

    #[test]
    fn a_range_is_checked_for_a_style() {
        let runs = [run(4, bold())];
        assert!(all_have(&runs, 8, 0, 4, |s| s.bold));
        assert!(!all_have(&runs, 8, 2, 6, |s| s.bold));
        assert!(!all_have(&runs, 8, 3, 3, |s| s.bold));
    }

    #[test]
    fn saved_form_is_small_and_round_trips() {
        let runs = vec![run(
            3,
            RunStyle {
                bold: true,
                colour: Some(2),
                ..RunStyle::default()
            },
        )];
        let json = serde_json::to_string(&runs).unwrap();
        assert_eq!(json, r#"[{"len":3,"bold":true,"colour":2}]"#);
        assert_eq!(serde_json::from_str::<Vec<Run>>(&json).unwrap(), runs);
    }
}
