//! The text under the math editor: a line of characters with a caret and a selection.
//! No egui here, so every rule is tested directly. Everything counts characters,
//! never bytes.

/// The most characters the editor will hold. The language's own limit is lower
/// (`MAX_SOURCE_CHARS`, which the parser reports); this one only stops a runaway paste.
pub const MAX_CHARS: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    chars: Vec<char>,
    /// Where typing goes (a character offset, `0..=len`).
    pub caret: usize,
    /// The other end of the selection; equal to `caret` when nothing is selected.
    pub anchor: usize,
}

impl Field {
    /// A field holding `source`, with the caret at the end.
    pub fn new(source: &str) -> Field {
        let chars: Vec<char> = source.chars().take(MAX_CHARS).collect();
        let end = chars.len();
        Field {
            chars,
            caret: end,
            anchor: end,
        }
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    /// The selection as `(start, end)`, start first.
    pub fn selection(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    pub fn selected_text(&self) -> String {
        let (a, b) = self.selection();
        self.chars[a..b].iter().collect()
    }

    /// The document changed under the editor (undo, a function inserted from the
    /// Functions window): take the new text. The caret keeps its distance from the
    /// end, so text added at the end leaves it after the addition.
    pub fn sync(&mut self, source: &str) {
        let old = self.chars.len();
        self.chars = source.chars().take(MAX_CHARS).collect();
        let len = self.chars.len();
        let keep = |at: usize| len.saturating_sub(old.saturating_sub(at));
        self.caret = keep(self.caret);
        self.anchor = keep(self.anchor);
    }

    /// Move the caret to `offset`; `extend` keeps the anchor, so the selection grows.
    pub fn set_caret(&mut self, offset: usize, extend: bool) {
        self.caret = offset.min(self.chars.len());
        if !extend {
            self.anchor = self.caret;
        }
    }

    pub fn select(&mut self, start: usize, end: usize) {
        self.anchor = start.min(self.chars.len());
        self.caret = end.min(self.chars.len());
    }

    pub fn select_all(&mut self) {
        self.select(0, self.chars.len());
    }

    /// Replace the selection (or insert at the caret) with `s`. Control characters
    /// are dropped and line breaks become spaces; the length limit is respected.
    pub fn insert(&mut self, s: &str) {
        let (a, b) = self.selection();
        let room = MAX_CHARS.saturating_sub(self.chars.len() - (b - a));
        let new: Vec<char> = s
            .chars()
            .map(|c| {
                if c == '\n' || c == '\r' || c == '\t' {
                    ' '
                } else {
                    c
                }
            })
            .filter(|c| !c.is_control())
            .take(room)
            .collect();
        let end = a + new.len();
        self.chars.splice(a..b, new);
        self.caret = end;
        self.anchor = end;
    }

    /// Backspace: the selection, else the character before the caret.
    pub fn backspace(&mut self) {
        if !self.has_selection() && self.caret > 0 {
            self.anchor = self.caret - 1;
        }
        self.insert("");
    }

    /// Delete: the selection, else the character after the caret.
    pub fn delete(&mut self) {
        if !self.has_selection() && self.caret < self.chars.len() {
            self.anchor = self.caret + 1;
        }
        self.insert("");
    }

    /// The start of the word before `from`: white space is skipped, then a run of
    /// letters, digits and underscores, or a single other character.
    pub fn word_left(&self, from: usize) -> usize {
        let mut i = from.min(self.chars.len());
        while i > 0 && self.chars[i - 1].is_whitespace() {
            i -= 1;
        }
        if i > 0 && is_word(self.chars[i - 1]) {
            while i > 0 && is_word(self.chars[i - 1]) {
                i -= 1;
            }
        } else {
            i = i.saturating_sub(1);
        }
        i
    }

    /// The end of the word after `from`.
    pub fn word_right(&self, from: usize) -> usize {
        let n = self.chars.len();
        let mut i = from.min(n);
        while i < n && self.chars[i].is_whitespace() {
            i += 1;
        }
        if i < n && is_word(self.chars[i]) {
            while i < n && is_word(self.chars[i]) {
                i += 1;
            }
        } else if i < n {
            i += 1;
        }
        i
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_inserts_at_the_caret_and_moves_it() {
        let mut f = Field::new("ac");
        f.set_caret(1, false);
        f.insert("b");
        assert_eq!((f.text().as_str(), f.caret, f.anchor), ("abc", 2, 2));
    }

    #[test]
    fn typing_over_a_selection_replaces_it() {
        let mut f = Field::new("hello world");
        f.select(0, 5);
        f.insert("bye");
        assert_eq!((f.text().as_str(), f.caret), ("bye world", 3));
    }

    #[test]
    fn backspace_and_delete_remove_one_character_or_the_selection() {
        let mut f = Field::new("abcd");
        f.set_caret(2, false);
        f.backspace();
        assert_eq!((f.text().as_str(), f.caret), ("acd", 1));
        f.delete();
        assert_eq!((f.text().as_str(), f.caret), ("ad", 1));
        f.select(0, 2);
        f.backspace();
        assert_eq!((f.text().as_str(), f.caret), ("", 0));
        // At the ends there is nothing to remove and nothing breaks.
        f.backspace();
        f.delete();
        assert_eq!(f.text(), "");
    }

    #[test]
    fn everything_counts_characters_not_bytes() {
        let mut f = Field::new("\u{3b1}\u{3b2}\u{3b3}");
        assert_eq!(f.len(), 3);
        f.set_caret(1, false);
        f.insert("\u{20ac}");
        assert_eq!(
            (f.text().as_str(), f.caret),
            ("\u{3b1}\u{20ac}\u{3b2}\u{3b3}", 2)
        );
        f.backspace();
        f.backspace();
        assert_eq!(f.text(), "\u{3b2}\u{3b3}");
    }

    #[test]
    fn control_characters_and_line_breaks_never_get_in() {
        let mut f = Field::new("");
        f.insert("a\nb\u{7}c\td");
        assert_eq!(f.text(), "a bc d");
    }

    #[test]
    fn a_runaway_paste_stops_at_the_limit() {
        let mut f = Field::new("");
        f.insert(&"x".repeat(MAX_CHARS * 2));
        assert_eq!(f.len(), MAX_CHARS);
        f.insert("y");
        assert_eq!(f.len(), MAX_CHARS, "no room left");
        f.select(0, 10);
        f.insert("yy");
        assert_eq!(f.len(), MAX_CHARS - 8, "replacing a selection frees room");
    }

    #[test]
    fn the_selection_is_ordered_whichever_way_it_was_made() {
        let mut f = Field::new("abcdef");
        f.set_caret(4, false);
        f.set_caret(1, true);
        assert_eq!(f.selection(), (1, 4));
        assert_eq!(f.selected_text(), "bcd");
        f.set_caret(5, false);
        assert!(!f.has_selection());
    }

    #[test]
    fn words_are_names_numbers_or_single_symbols() {
        let f = Field::new("sigma_1 := F * 2.5");
        assert_eq!(f.word_right(0), 7);
        assert_eq!(f.word_right(7), 9, "a symbol is a word of one character");
        assert_eq!(f.word_right(9), 10);
        assert_eq!(f.word_right(10), 12);
        assert_eq!(f.word_left(7), 0);
        assert_eq!(f.word_left(12), 11);
        assert_eq!(f.word_left(0), 0);
        assert_eq!(f.word_right(f.len()), f.len());
    }

    #[test]
    fn syncing_to_a_shorter_document_keeps_the_caret_in_range() {
        let mut f = Field::new("abcdef");
        f.sync("ab");
        assert_eq!((f.text().as_str(), f.caret, f.anchor), ("ab", 2, 2));
        // Text added at the end leaves the caret after it; in the middle, before it.
        f.sync("ab+cd");
        assert_eq!(f.caret, 5);
        f.set_caret(1, false);
        f.sync("ab+cde");
        assert_eq!(f.caret, 2, "the caret keeps its distance from the end");
    }
}
