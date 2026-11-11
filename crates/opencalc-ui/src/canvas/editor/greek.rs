//! Ctrl+G: turn the letter before the caret (or each letter selected) into its
//! Greek counterpart, and a Greek letter back into the Latin one. The pairs follow
//! a common habit in math editors: `p` is pi, `q` is theta, `f` is phi, `c` is chi.

use super::Field;

/// Latin letter, Greek lower case, Greek upper case. A Latin letter with no Greek
/// partner that is not just a lookalike (j, v) is left out.
const PAIRS: &[(char, char, char)] = &[
    ('a', '\u{3b1}', '\u{391}'),
    ('b', '\u{3b2}', '\u{392}'),
    ('c', '\u{3c7}', '\u{3a7}'),
    ('d', '\u{3b4}', '\u{394}'),
    ('e', '\u{3b5}', '\u{395}'),
    ('f', '\u{3c6}', '\u{3a6}'),
    ('g', '\u{3b3}', '\u{393}'),
    ('h', '\u{3b7}', '\u{397}'),
    ('i', '\u{3b9}', '\u{399}'),
    ('k', '\u{3ba}', '\u{39a}'),
    ('l', '\u{3bb}', '\u{39b}'),
    ('m', '\u{3bc}', '\u{39c}'),
    ('n', '\u{3bd}', '\u{39d}'),
    ('o', '\u{3bf}', '\u{39f}'),
    ('p', '\u{3c0}', '\u{3a0}'),
    ('q', '\u{3b8}', '\u{398}'),
    ('r', '\u{3c1}', '\u{3a1}'),
    ('s', '\u{3c3}', '\u{3a3}'),
    ('t', '\u{3c4}', '\u{3a4}'),
    ('u', '\u{3c5}', '\u{3a5}'),
    ('w', '\u{3c9}', '\u{3a9}'),
    ('x', '\u{3be}', '\u{39e}'),
    ('y', '\u{3c8}', '\u{3a8}'),
    ('z', '\u{3b6}', '\u{396}'),
];

/// The other alphabet's letter for `c`, if it has one.
pub fn swapped(c: char) -> Option<char> {
    for (latin, lower, upper) in PAIRS {
        if c == *latin {
            return Some(*lower);
        }
        if c == latin.to_ascii_uppercase() {
            return Some(*upper);
        }
        if c == *lower {
            return Some(*latin);
        }
        if c == *upper {
            return Some(latin.to_ascii_uppercase());
        }
    }
    None
}

impl Field {
    /// Swap the letters selected, or the one before the caret, between Latin and
    /// Greek. Returns whether anything changed.
    pub fn swap_greek(&mut self) -> bool {
        let (a, b) = if self.has_selection() {
            self.selection()
        } else if self.caret > 0 {
            (self.caret - 1, self.caret)
        } else {
            return false;
        };
        let old: Vec<char> = self.chars()[a..b].to_vec();
        let new: String = old.iter().map(|c| swapped(*c).unwrap_or(*c)).collect();
        if new.chars().eq(old.iter().copied()) {
            return false;
        }
        let had_selection = self.has_selection();
        self.replace(a, b, &new);
        if had_selection {
            self.select(a, a + new.chars().count());
        } else {
            self.typing = false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_pair_swaps_both_ways_in_both_cases() {
        for (latin, lower, upper) in PAIRS {
            assert_eq!(swapped(*latin), Some(*lower));
            assert_eq!(swapped(*lower), Some(*latin));
            assert_eq!(swapped(latin.to_ascii_uppercase()), Some(*upper));
            assert_eq!(swapped(*upper), Some(latin.to_ascii_uppercase()));
        }
        assert_eq!(swapped('j'), None);
        assert_eq!(swapped('7'), None);
        assert_eq!(swapped('+'), None);
    }

    #[test]
    fn ctrl_g_acts_on_the_letter_before_the_caret_or_the_selection() {
        let mut f = Field::new("2p");
        assert!(f.swap_greek());
        assert_eq!(f.text(), "2\u{3c0}");
        assert!(f.swap_greek(), "pressed again it goes back");
        assert_eq!(f.text(), "2p");
        let mut f = Field::new("ab+1");
        f.select(0, 3);
        assert!(f.swap_greek());
        assert_eq!(f.text(), "\u{3b1}\u{3b2}+1");
        assert!(f.has_selection(), "the selection is kept");
        let mut f = Field::new("5");
        assert!(!f.swap_greek(), "a digit has no Greek form");
        assert!(!Field::new("").swap_greek());
        let mut f = Field::new("D");
        f.swap_greek();
        assert_eq!(f.text(), "\u{394}");
    }
}
