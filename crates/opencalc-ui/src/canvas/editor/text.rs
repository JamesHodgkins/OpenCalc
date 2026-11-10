//! The text under the math editor: a line of characters with a caret and a selection.
//! No egui here, so every rule is tested directly. Everything counts characters,
//! never bytes.

/// The most characters the editor will hold. The language's own limit is lower
/// (`MAX_SOURCE_CHARS`, which the parser reports); this one only stops a runaway paste.
pub const MAX_CHARS: usize = 8192;

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    chars: Vec<char>,
    /// Where typing goes (a character offset, `0..=len`).
    pub caret: usize,
    /// The other end of the selection; equal to `caret` when nothing is selected.
    pub anchor: usize,
    /// The text changed by typing since the caret last moved: the autocomplete
    /// list is only offered then.
    pub typing: bool,
    /// Which autocomplete entry is highlighted.
    pub pick: usize,
    /// The word whose autocomplete list was closed with Escape.
    pub dismissed: Option<String>,
    /// The x (in layout space) Up/Down aim for, kept across repeated vertical moves.
    pub goal_x: Option<f32>,
    /// The last insertion dropped characters because the editor is full.
    pub clipped: bool,
    /// An arrow press had nowhere left to go inside the expression: the direction
    /// (dx, dy) the caret is leaving by. The editor takes it and ends editing.
    pub leave: Option<(i8, i8)>,
}

impl Field {
    /// A field holding `source`, with the caret at the end.
    pub fn new(source: &str) -> Field {
        // Existing document content is authoritative. MAX_CHARS limits edits,
        // not what may be loaded into the transient field.
        let chars: Vec<char> = source.chars().collect();
        let end = chars.len();
        Field {
            chars,
            caret: end,
            anchor: end,
            typing: false,
            pick: 0,
            dismissed: None,
            goal_x: None,
            clipped: false,
            leave: None,
        }
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
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
        // Never drop document text when synchronizing after undo or an external
        // edit. The editor activation path handles sources above MAX_CHARS.
        self.chars = source.chars().collect();
        let len = self.chars.len();
        let keep = |at: usize| len.saturating_sub(old.saturating_sub(at));
        self.caret = keep(self.caret);
        self.anchor = keep(self.anchor);
    }

    /// Move the caret to `offset`; `extend` keeps the anchor, so the selection grows.
    pub fn set_caret(&mut self, offset: usize, extend: bool) {
        self.typing = false;
        self.goal_x = None;
        self.clipped = false;
        self.pick = 0;
        self.caret = offset.min(self.chars.len());
        if !extend {
            self.anchor = self.caret;
        }
    }

    pub fn select(&mut self, start: usize, end: usize) {
        self.typing = false;
        self.anchor = start.min(self.chars.len());
        self.caret = end.min(self.chars.len());
    }

    pub fn select_all(&mut self) {
        self.select(0, self.chars.len());
    }

    /// Whether this is a program (docs/PROGRAMMING.md): it has line breaks, or it
    /// ends with the word `program`, which the next Enter opens. Spaces and line
    /// breaks are text here, where in a formula the display does the spacing.
    pub fn in_program(&self) -> bool {
        let text = self.text();
        text.contains('\n') || ends_with_word(&text, "program")
    }

    /// The offset where the line holding `at` starts.
    fn line_start(&self, at: usize) -> usize {
        self.chars[..at.min(self.chars.len())]
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(0, |i| i + 1)
    }

    /// Whether only spaces lie between the start of the caret's line and the caret.
    pub fn in_line_indent(&self) -> bool {
        self.chars[self.line_start(self.caret)..self.caret]
            .iter()
            .all(|c| *c == ' ')
    }

    /// Enter in a program: a new line, indented as the line above, and four more
    /// after a line that opens a block.
    pub fn new_line(&mut self) {
        let start = self.line_start(self.caret);
        let line: String = self.chars[start..self.caret].iter().collect();
        let indent = line.chars().take_while(|c| *c == ' ').count();
        let body = line.trim_start();
        let opens = ["if ", "for ", "while ", "else", "try", "on error"]
            .iter()
            .any(|w| body.starts_with(w))
            || ends_with_word(body, "program");
        let extra = if opens { 4 } else { 0 };
        self.insert(&format!("\n{}", " ".repeat(indent + extra)));
    }

    /// Whether the caret's line holds nothing but spaces and the caret is at the
    /// very end: a blank line after the last one, which in a program means "done".
    pub fn at_blank_last_line(&self) -> bool {
        self.caret == self.chars.len()
            && self.chars[self.line_start(self.caret)..]
                .iter()
                .all(|c| *c == ' ')
            && self.line_start(self.caret) > 0
    }

    /// Drop the blank last line (and any blank lines before it).
    pub fn trim_end(&mut self) {
        while self.chars.last().is_some_and(|c| *c == ' ' || *c == '\n') {
            self.chars.pop();
        }
        self.caret = self.chars.len();
        self.anchor = self.caret;
    }

    /// Tab in a program, at the start of a line: four spaces. Shift-Tab takes up to
    /// four away from the start of the line.
    pub fn indent(&mut self, back: bool) {
        if !back {
            self.insert("    ");
            return;
        }
        let start = self.line_start(self.caret);
        let spaces = self.chars[start..]
            .iter()
            .take(4)
            .take_while(|c| **c == ' ')
            .count();
        self.chars.drain(start..start + spaces);
        self.caret = self.caret.saturating_sub(spaces).max(start);
        self.anchor = self.caret;
    }

    /// Replace the selection (or insert at the caret) with `s`. Control characters
    /// are dropped and line breaks become spaces (but stay line breaks in a
    /// program); the length limit is respected.
    pub fn insert(&mut self, s: &str) {
        let (a, b) = self.selection();
        let room = MAX_CHARS.saturating_sub(self.chars.len() - (b - a));
        let lines = self.in_program() || s.contains("program\n") || s.contains("program\r\n");
        let new: Vec<char> = s
            .chars()
            .filter(|c| !(lines && *c == '\r'))
            .flat_map(|c| match c {
                '\n' if lines => vec!['\n'],
                '\t' if lines => vec![' '; 4],
                '\n' | '\r' | '\t' => vec![' '],
                c => vec![c],
            })
            .filter(|c| *c == '\n' || !c.is_control())
            .collect();
        let wanted = new.len();
        let new: Vec<char> = new.into_iter().take(room).collect();
        let end = a + new.len();
        self.clipped = new.len() < wanted;
        self.goal_x = None;
        self.chars.splice(a..b, new);
        self.caret = end;
        self.anchor = end;
        self.typing = true;
        self.pick = 0;
    }

    /// Replace the characters `a..b` with `s`, leaving the caret after it.
    pub fn replace(&mut self, a: usize, b: usize, s: &str) {
        self.select(a, b);
        self.insert(s);
    }

    /// What typing `s` does. Mostly it inserts `s`, with three conveniences:
    /// a space after a backslash and a name turns it into the Greek letter
    /// (`greek` finds the letter for a name); `:` makes `:=`; and the `=` that
    /// finishes a `:=` the user typed by hand is not doubled.
    pub fn typed(&mut self, s: &str, greek: impl Fn(&str) -> Option<char>) {
        let before = |n: usize| self.caret.checked_sub(n).map(|i| self.chars[i]);
        let after = self.chars.get(self.caret).copied();
        let hand_typed_define = before(1) == Some('=') && before(2) == Some(':');
        let after_less = before(1) == Some('<');
        match s {
            // Spaces are not typed into math: the display does the spacing. A space
            // after a backslash name is the Greek-letter terminator. (In a program
            // they are text: `for i in 1..n` needs them.)
            " " => {
                if !self.has_selection() && !self.expand_greek(greek) && self.in_program() {
                    self.insert(" ");
                }
            }
            // `<-` is the assignment arrow of a program.
            "-" if after_less && self.in_program() && !self.has_selection() => {
                self.replace(self.caret - 1, self.caret, "\u{2190}");
            }
            ":" if after != Some('=') => self.insert(":="),
            "=" if !self.has_selection() && hand_typed_define => {}
            _ if self.in_program() => self.insert(s),
            _ => self.insert(&s.replace(' ', "")),
        }
    }

    /// Replace a backslash and name just before the caret by its Greek letter, if
    /// `greek` knows the name. Returns whether it did.
    pub fn expand_greek(&mut self, greek: impl Fn(&str) -> Option<char>) -> bool {
        if self.has_selection() {
            return false;
        }
        let end = self.caret;
        let mut start = end;
        while start > 0 && self.chars[start - 1].is_ascii_alphabetic() {
            start -= 1;
        }
        if start == end || start == 0 || self.chars[start - 1] != '\\' {
            return false;
        }
        let name: String = self.chars[start..end].iter().collect();
        let Some(letter) = greek(&name) else {
            return false;
        };
        self.replace(start - 1, end, &letter.to_string());
        true
    }

    /// Backspace: the selection, else the character before the caret.
    pub fn backspace(&mut self) {
        if !self.has_selection() && self.caret > 0 {
            self.anchor = self.caret - 1;
            // Between an empty pair of brackets, take both away together.
            let pair = (self.chars[self.caret - 1], self.chars.get(self.caret));
            if matches!(pair, ('(', Some(')')) | ('[', Some(']'))) {
                self.caret += 1;
            }
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

/// Whether `text` ends with `word` as a whole word.
fn ends_with_word(text: &str, word: &str) -> bool {
    let text = text.trim_end();
    text.strip_suffix(word)
        .is_some_and(|rest| !rest.chars().next_back().is_some_and(is_word))
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    #[test]
    fn paste_over_the_cap_is_flagged_and_a_caret_move_clears_it() {
        let mut f = Field::new("");
        f.insert(&"x".repeat(MAX_CHARS + 5));
        assert!(f.clipped);
        f.set_caret(0, false);
        assert!(!f.clipped);
        f.insert("y");
        assert!(f.clipped, "full again");
        f.select_all();
        f.insert("y");
        assert!(!f.clipped);
    }

    use super::*;

    #[test]
    fn a_program_keeps_its_spaces_and_lines_and_a_formula_does_not() {
        let mut formula = Field::new("");
        formula.typed("a", |_| None);
        formula.typed(" ", |_| None);
        formula.typed("b", |_| None);
        assert_eq!(formula.text(), "ab", "spaces are not typed into a formula");

        let mut p = Field::new("f := program");
        assert!(p.in_program(), "ending in `program` opens one");
        assert!(!Field::new("myprogram").in_program());
        p.new_line();
        assert_eq!(p.text(), "f := program\n    ", "a block opener indents");
        for c in "s <- 0".chars() {
            p.typed(&c.to_string(), |_| None);
        }
        assert_eq!(p.text(), "f := program\n    s \u{2190} 0");
        p.new_line();
        assert_eq!(
            p.text(),
            "f := program\n    s \u{2190} 0\n    ",
            "the indent carries on"
        );
        for c in "for i in 1..3".chars() {
            p.typed(&c.to_string(), |_| None);
        }
        p.new_line();
        assert!(
            p.text().ends_with("1..3\n        "),
            "a `for` indents the next line"
        );
    }

    #[test]
    fn a_blank_last_line_ends_a_program_and_tab_indents() {
        let mut p = Field::new("f := program\n    1 + 2");
        p.new_line();
        assert!(p.at_blank_last_line());
        p.trim_end();
        assert_eq!(p.text(), "f := program\n    1 + 2");
        assert!(!p.at_blank_last_line());

        let mut q = Field::new("f := program\n");
        assert!(q.in_line_indent());
        q.indent(false);
        assert_eq!(q.text(), "f := program\n    ");
        q.indent(true);
        assert_eq!(q.text(), "f := program\n");
        assert_eq!(q.caret, q.len());
    }

    #[test]
    fn pasting_lines_into_a_formula_joins_them_but_a_program_stays_lines() {
        let mut f = Field::new("");
        f.insert("a +\nb");
        assert_eq!(f.text(), "a + b");
        let mut p = Field::new("");
        p.insert("f := program\n\tx ← 1\n    x");
        assert_eq!(p.text(), "f := program\n    x ← 1\n    x");
    }

    fn greek(name: &str) -> Option<char> {
        match name {
            "alpha" => Some('\u{3b1}'),
            "Delta" => Some('\u{394}'),
            _ => None,
        }
    }

    #[test]
    fn backslash_name_then_space_becomes_the_greek_letter() {
        let mut f = Field::new("x + ");
        for c in ["\\", "a", "l", "p", "h", "a"] {
            f.typed(c, greek);
        }
        assert_eq!(f.text(), "x + \\alpha");
        f.typed(" ", greek);
        assert_eq!(
            (f.text().as_str(), f.caret),
            ("x + \u{3b1}", 5),
            "the space is used up"
        );
        // Case matters, and an unknown name keeps its text and gets the space.
        let mut g = Field::new("\\delta");
        g.typed(" ", greek);
        assert_eq!(g.text(), r"\delta", "an unknown name stays as typed");
        let mut h = Field::new("\\Delta");
        h.typed(" ", greek);
        assert_eq!(h.text(), "\u{394}");
        // A name with no backslash is left alone.
        let mut k = Field::new("alpha");
        k.typed(" ", greek);
        assert_eq!(k.text(), "alpha", "spaces are never typed into math");
        let mut m = Field::new("");
        m.typed("a b  c", greek);
        assert_eq!(m.text(), "abc", "spaces inside typed text are dropped");
    }

    #[test]
    fn a_colon_makes_a_define_and_the_hand_typed_equals_is_not_doubled() {
        let mut f = Field::new("x ");
        f.typed(":", greek);
        assert_eq!(f.text(), "x :=");
        f.typed("=", greek);
        assert_eq!(f.text(), "x :=", "the = that completes := is absorbed");
        f.typed(" ", greek);
        f.typed("1", greek);
        f.typed("=", greek);
        assert_eq!(f.text(), "x :=1=", "other equals signs are kept");
        // Typing : in front of an = does not double it either.
        let mut g = Field::new("x =");
        g.set_caret(2, false);
        g.typed(":", greek);
        assert_eq!(g.text(), "x :=");
    }

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

    #[test]
    fn backspace_inside_an_empty_pair_of_brackets_removes_both() {
        let mut f = Field::new("f()");
        f.set_caret(2, false);
        f.backspace();
        assert_eq!(f.text(), "f");
        let mut g = Field::new("[]");
        g.set_caret(1, false);
        g.backspace();
        assert_eq!(g.text(), "");
        let mut h = Field::new("(1)");
        h.set_caret(1, false);
        h.backspace();
        assert_eq!(
            h.text(),
            "1)",
            "a pair with something inside is not touched"
        );
    }

    #[test]
    fn goal_x_is_dropped_by_any_caret_move_or_edit() {
        let mut f = Field::new("abc");
        f.goal_x = Some(10.0);
        f.set_caret(1, false);
        assert_eq!(f.goal_x, None);
        f.goal_x = Some(10.0);
        f.insert("z");
        assert_eq!(f.goal_x, None);
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        #[derive(Clone, Debug)]
        enum Op {
            Insert(String),
            Typed(String),
            Backspace,
            Delete,
            Caret(usize, bool),
            Select(usize, usize),
            SelectAll,
            Sync(String),
        }

        fn op() -> impl Strategy<Value = Op> {
            // Any Unicode text, including controls, combining marks and astral chars.
            let s = || prop::string::string_regex("(?s).{0,12}").unwrap();
            prop_oneof![
                s().prop_map(Op::Insert),
                s().prop_map(Op::Typed),
                Just(Op::Backspace),
                Just(Op::Delete),
                (0..40usize, any::<bool>()).prop_map(|(a, b)| Op::Caret(a, b)),
                (0..40usize, 0..40usize).prop_map(|(a, b)| Op::Select(a, b)),
                Just(Op::SelectAll),
                s().prop_map(Op::Sync),
            ]
        }

        proptest! {
            #[test]
            fn caret_and_anchor_stay_in_range_and_the_selection_is_a_valid_slice(
                start in "(?s).{0,12}",
                ops in prop::collection::vec(op(), 0..40),
            ) {
                let mut f = Field::new(&start);
                for o in ops {
                    match o {
                        Op::Insert(s) => f.insert(&s),
                        Op::Typed(s) => f.typed(&s, |_| None),
                        Op::Backspace => f.backspace(),
                        Op::Delete => f.delete(),
                        Op::Caret(a, e) => f.set_caret(a, e),
                        Op::Select(a, b) => f.select(a, b),
                        Op::SelectAll => f.select_all(),
                        Op::Sync(s) => f.sync(&s),
                    }
                    prop_assert!(f.caret <= f.len());
                    prop_assert!(f.anchor <= f.len());
                    let (a, b) = f.selection();
                    prop_assert!(a <= b && b <= f.len());
                    let _ = f.selected_text();
                    prop_assert!(f.word_left(f.caret) <= f.caret);
                    prop_assert!(f.word_right(f.caret) >= f.caret);
                    prop_assert!(f.word_right(f.caret) <= f.len());
                }
            }

            #[test]
            fn insertion_never_exceeds_the_cap_and_replacing_with_the_old_text_restores_it(
                base in "[a-z +*/^()]{0,20}",
                pasted in "(?s).{0,30}",
                at in 0..25usize,
            ) {
                let mut f = Field::new(&base);
                let a = at.min(f.len());
                f.select(a, a);
                f.insert(&pasted);
                prop_assert!(f.len() <= MAX_CHARS.max(base.chars().count()));
                let inserted = f.caret - a;
                f.replace(a, a + inserted, "");
                prop_assert_eq!(f.text(), base);
            }
        }
    }
}
