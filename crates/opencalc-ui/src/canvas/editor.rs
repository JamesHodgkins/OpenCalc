//! The math editor (docs/EDITOR.md section 4.4): in-place editing of a math region
//! on its typeset form. It owns a caret and a selection (UI state, not document
//! state) and turns input into `SetMathSource` edits, so undo is unchanged.
//!
//! The text rules live in [`text::Field`]; this file reads input, asks the
//! `SourceLayout` where the caret goes, and draws.

mod text;

use eframe::egui::{
    self, Event, EventFilter, Id, Key, Modifiers, Pos2, Rect, Sense, Stroke, Ui, Vec2,
};
use opencalc_core::document::{Region, RegionKind};
use opencalc_core::edit::Edit;

pub use text::Field;

use super::paint::{MATH_SIZE, PAD};
use super::{CanvasCmd, CanvasUi, CanvasView, Env};
use crate::layout::SourceLayout;

/// Edit the math region `r` in place. Called every frame while it is being edited.
pub(super) fn math(
    ui: &mut Ui,
    env: &Env,
    r: &Region,
    view: &CanvasView,
    origin: Pos2,
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    let zoom = view.zoom;
    let (pad, size) = (PAD * zoom, MATH_SIZE * zoom);
    let paper = env.paper;
    let screen_min = view
        .rect_to_screen(super::adapt::rect_of(r, &st.sizes), origin)
        .min;
    let text_origin = screen_min + Vec2::splat(pad);
    let id = Id::new(("math-editor", r.id.0));

    let RegionKind::Math(m) = &r.kind else {
        return;
    };
    let source = m.source();
    let fresh = st.focus_edit || st.field.is_none();
    let field = st.field.get_or_insert_with(|| Field::new(source));
    if fresh {
        *field = Field::new(source);
    } else if field.text() != source {
        field.sync(source);
    }
    let layout_of = |ui: &Ui, text: &str| {
        ui.ctx()
            .fonts(|f| SourceLayout::new(f, paper.text, text, size))
    };
    let mut lay = layout_of(ui, &field.text());

    // The widget covers the region so clicks and drags reach the editor.
    let rect = Rect::from_min_size(
        screen_min,
        Vec2::new(
            (lay.layout.w + 2.0 * pad).max(60.0 * zoom),
            lay.layout.h + 2.0 * pad,
        ),
    );
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    if st.focus_edit {
        resp.request_focus();
        st.focus_edit = false;
    }
    ui.memory_mut(|mem| {
        mem.set_focus_lock_filter(
            id,
            EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: true,
            },
        );
    });
    ui.ctx().set_cursor_icon(egui::CursorIcon::Text);

    let mods = ui.input(|i| i.modifiers);
    let mut finished = false;
    // Pointer: press places the caret, dragging extends, double click picks a node.
    if let Some(pos) = resp.interact_pointer_pos() {
        let offset = lay.offset_at((pos - text_origin).to_pos2());
        if resp.double_clicked() {
            let (a, b) = lay.enclosing(offset);
            field.select(a, b);
        } else if ui.input(|i| i.pointer.primary_pressed()) {
            field.set_caret(offset, mods.shift);
            resp.request_focus();
        } else if resp.dragged() {
            field.set_caret(offset, true);
        }
    }

    if resp.has_focus() {
        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                Event::Text(s) | Event::Paste(s) => field.insert(&s),
                Event::Copy => copy(ui, field),
                Event::Cut => {
                    copy(ui, field);
                    field.insert("");
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => finished |= key_down(field, &lay, key, modifiers),
                _ => {}
            }
            // Later events in this frame (fast typing) see the text so far.
            lay = layout_of(ui, &field.text());
        }
    }
    if resp.lost_focus() {
        finished = true;
    }

    let text = field.text();
    if text != source {
        cmds.push(CanvasCmd::Edit(Edit::SetMathSource {
            id: r.id,
            source: text,
        }));
    }
    if finished {
        st.editing = None;
        st.field = None;
        cmds.push(CanvasCmd::Seal);
        return;
    }

    // Draw: selection, the typeset text, then the caret.
    let painter = ui.painter();
    let (a, b) = field.selection();
    for o in a..b {
        let (p, q) = (lay.caret_rect(o), lay.caret_rect(o + 1));
        if (p.center().y - q.center().y).abs() < p.height() * 0.5 && q.min.x > p.min.x {
            let band = Rect::from_min_max(
                text_origin + Vec2::new(p.min.x, p.min.y.min(q.min.y)),
                text_origin + Vec2::new(q.min.x, p.max.y.max(q.max.y)),
            );
            painter.rect_filled(band, 0.0, env.theme.accent.gamma_multiply(0.3));
        }
    }
    lay.layout.paint(painter, text_origin, paper.text);
    let caret = lay.caret_rect(field.caret);
    painter.line_segment(
        [
            text_origin + caret.min.to_vec2(),
            text_origin + caret.max.to_vec2(),
        ],
        Stroke::new(1.5_f32, env.theme.accent),
    );
    st.sizes.insert(
        r.id,
        Vec2::new(lay.layout.w + 2.0 * pad, lay.layout.h + 2.0 * pad) / zoom,
    );
}

fn copy(ui: &Ui, field: &Field) {
    if field.has_selection() {
        ui.ctx().copy_text(field.selected_text());
    }
}

/// One key press. Returns true when it ends editing.
fn key_down(field: &mut Field, lay: &SourceLayout, key: Key, mods: Modifiers) -> bool {
    let (shift, word) = (mods.shift, mods.command);
    let len = field.len();
    match key {
        Key::Enter | Key::Escape => return true,
        Key::Backspace => field.backspace(),
        Key::Delete => field.delete(),
        Key::A if mods.command => field.select_all(),
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
                field.set_caret(target, shift);
            }
        }
        Key::ArrowUp | Key::ArrowDown => {
            let c = lay.caret_rect(field.caret);
            let dy = c.height() * if key == Key::ArrowUp { -1.0 } else { 1.0 };
            let o = lay.offset_at(Pos2::new(c.min.x, c.center().y + dy));
            if o != field.caret {
                field.set_caret(o, shift);
            }
        }
        _ => {}
    }
    false
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
