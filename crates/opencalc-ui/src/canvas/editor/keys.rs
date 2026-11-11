//! Keys and clipboard in the math editor: what each key does to the field, accepting an
//! autocomplete candidate, and growing the selection.

use eframe::egui::{Key, Modifiers, Pos2, Ui};

use super::complete::{self, Candidate};
use super::Field;
use crate::commands::greek_by_name;
use crate::layout::SourceLayout;

pub(super) fn copy(ui: &Ui, field: &Field) {
    if field.has_selection() {
        ui.ctx().copy_text(field.selected_text());
    }
}

/// Whether the choice is exactly the word already typed.
fn is_complete(field: &Field, c: &Candidate) -> bool {
    complete::word_before(field).is_some_and(|(_, w)| c.insert == w)
}

/// Type the chosen entry in place of the word before the caret.
pub(super) fn accept(field: &mut Field, start: usize, c: &Candidate) {
    field.replace(start, field.caret, &c.insert);
    // What was just typed is a whole name; do not offer to complete it again.
    field.typing = false;
}

/// One key press. Returns true when it ends editing. `list` is the autocomplete
/// list showing (for the word that starts at `start`), if any.
pub(super) fn key_down(
    field: &mut Field,
    lay: &SourceLayout,
    key: Key,
    mods: Modifiers,
    start: usize,
    list: &[Candidate],
) -> bool {
    let (shift, word) = (mods.shift, mods.command);
    let len = field.len();
    let listing = !list.is_empty();
    match key {
        // In a program Enter is always a new line (or, on a blank last line, done),
        // even with the completion list open: Tab accepts a completion there, and
        // at the start of a line it indents.
        Key::Enter if field.in_program() && field.at_blank_last_line() => {
            field.trim_end();
            return true;
        }
        Key::Enter if field.in_program() => field.new_line(),
        Key::Tab if field.in_program() && (shift || (field.in_line_indent() && !listing)) => {
            field.indent(shift);
        }
        // A name typed in full that is the top choice: Enter is just "done".
        Key::Enter if listing && is_complete(field, &list[field.pick.min(list.len() - 1)]) => {
            return true;
        }
        Key::Enter | Key::Tab if listing => {
            accept(field, start, &list[field.pick.min(list.len() - 1)]);
        }
        Key::Tab => {
            field.expand_greek(greek_by_name);
        }
        Key::Escape if listing => {
            field.dismissed = complete::word_before(field).map(|(_, w)| w);
        }
        Key::Enter | Key::Escape => return true,
        Key::ArrowDown if listing => field.pick = (field.pick + 1) % list.len(),
        Key::ArrowUp if listing => field.pick = (field.pick + list.len() - 1) % list.len(),
        Key::Backspace => field.backspace(),
        Key::Delete => field.delete(),
        Key::A if mods.command => field.select_all(),
        Key::G if mods.command => _ = field.swap_greek(),
        Key::Space if mods.command => grow(field, lay),
        Key::Home => field.set_caret(0, shift),
        Key::End => field.set_caret(len, shift),
        Key::ArrowLeft | Key::ArrowRight => {
            let left = key == Key::ArrowLeft;
            let (a, b) = field.selection();
            if field.has_selection() && !shift && !word {
                field.set_caret(if left { a } else { b }, false);
            } else {
                let target = match (word, left) {
                    (true, true) => field.word_left(field.caret),
                    (true, false) => field.word_right(field.caret),
                    (false, _) => step(lay, field.caret, if left { -1 } else { 1 }, len),
                };
                if target == field.caret && !shift {
                    field.leave = Some((if left { -1 } else { 1 }, 0));
                }
                field.set_caret(target, shift);
            }
        }
        Key::ArrowUp | Key::ArrowDown => {
            let c = lay.caret_rect(field.caret);
            let dy = c.height() * if key == Key::ArrowUp { -1.0 } else { 1.0 };
            let x = field.goal_x.unwrap_or(c.min.x);
            let o = lay.offset_at(Pos2::new(x, c.center().y + dy));
            if o != field.caret {
                field.set_caret(o, shift);
            } else if !shift {
                field.leave = Some((0, if key == Key::ArrowUp { -1 } else { 1 }));
            }
            field.goal_x = Some(x);
        }
        _ => {}
    }
    false
}

/// Space: select the next larger piece of the expression around the selection.
pub(super) fn grow(field: &mut Field, lay: &SourceLayout) {
    let (a, b) = field.selection();
    let (s, e) = lay.grow(a, b);
    field.select(s, e);
}

/// One character left or right, skipping characters that draw nothing (the caret
/// would not appear to move).
fn step(lay: &SourceLayout, from: usize, dir: i64, len: usize) -> usize {
    let here = lay.caret_rect(from);
    let mut o = from;
    loop {
        let next = o as i64 + dir;
        if next < 0 || next as usize > len {
            return o;
        }
        o = next as usize;
        let c = lay.caret_rect(o);
        if (c.min.x - here.min.x).abs() > 0.01 || (c.center().y - here.center().y).abs() > 0.01 {
            return o;
        }
    }
}
