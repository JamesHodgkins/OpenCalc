//! The math editor (docs/EDITOR.md section 4.4): in-place editing of a math region
//! on its typeset form. It owns a caret and a selection (UI state, not document
//! state) and turns input into `SetMathSource` edits, so undo is unchanged.
//!
//! The text rules live in [`text::Field`] and the autocomplete rules in
//! [`complete`]; this file reads input, asks the `SourceLayout` where the caret
//! goes, and draws.

mod complete;
mod text;

use eframe::egui::{
    self, Event, EventFilter, Id, Key, Modifiers, Order, Pos2, Rect, RichText, Sense, Stroke, Ui,
    Vec2,
};
use opencalc_core::document::{Region, RegionKind};
use opencalc_core::edit::Edit;
use opencalc_core::expr::{AssignKind, Expr};

use complete::Candidate;
pub use text::Field;

use super::paint::{MATH_SIZE, PAD};
use super::{CanvasCmd, CanvasUi, CanvasView, Env};
use crate::commands::greek_by_name;
use crate::layout::SourceLayout;

/// What the editor reads besides the region itself.
pub(super) struct Ctx<'a> {
    pub env: &'a Env<'a>,
    pub regions: &'a [Region],
    pub view: &'a CanvasView,
    pub origin: Pos2,
}

/// The names the worksheet defines (variables and functions), for autocomplete.
fn defined_names(regions: &[Region]) -> Vec<String> {
    let mut out = Vec::new();
    for r in regions {
        if let RegionKind::Math(m) = &r.kind {
            match m.expr() {
                Expr::Assign {
                    kind: AssignKind::Define,
                    target,
                    ..
                } => out.push(target.clone()),
                Expr::FunctionDef { name, .. } => out.push(name.clone()),
                _ => {}
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Edit the math region `r` in place. Called every frame while it is being edited.
pub(super) fn math(
    ui: &mut Ui,
    cx: &Ctx,
    r: &Region,
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    let RegionKind::Math(m) = &r.kind else {
        return;
    };
    let (env, view) = (cx.env, cx.view);
    let zoom = view.zoom;
    let (pad, size) = (PAD * zoom, MATH_SIZE * zoom);
    let paper = env.paper;
    let screen_min = view
        .rect_to_screen(super::adapt::rect_of(r, &st.sizes), cx.origin)
        .min;
    let text_origin = screen_min + Vec2::splat(pad);
    let id = Id::new(("math-editor", r.id.0));
    let popup_id = id.with("popup");

    let source = m.source();
    let fresh = st.focus_edit || st.field.is_none();
    let field = st.field.get_or_insert_with(|| Field::new(source));
    if fresh {
        *field = Field::new(source);
    } else if field.text() != source {
        field.sync(source);
    }
    let defined = defined_names(cx.regions);
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
            let (start, list) = complete::for_field(field, &defined);
            match event {
                Event::Text(s) => field.typed(&s, greek_by_name),
                Event::Paste(s) => field.insert(&s),
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
                } => finished |= key_down(field, &lay, key, modifiers, start, &list),
                _ => {}
            }
            // Later events in this frame (fast typing) see the text so far.
            lay = layout_of(ui, &field.text());
        }
    }
    // A press on the autocomplete list takes focus for a moment; that is not the
    // end of editing.
    let on_popup = ui.data(|d| d.get_temp::<Rect>(popup_id)).is_some_and(|p| {
        ui.input(|i| i.pointer.interact_pos())
            .is_some_and(|pos| p.contains(pos))
    });
    // Clicking the ribbon or a window does not end editing: the editor takes the
    // keyboard back unless another text box (the Functions search) holds it. A
    // press on the canvas outside the region, and Enter or Escape, end it.
    // (A press this frame is left alone: the widget pressed may be a text box.)
    if !resp.has_focus() && !env.text_box_focus && !ui.input(|i| i.pointer.any_pressed()) {
        resp.request_focus();
    }
    // (The pointer is read first: asking egui which layer is under it while inside
    // `ui.input` would lock the context twice.)
    let press = ui.input(|i| {
        i.pointer
            .primary_pressed()
            .then(|| i.pointer.interact_pos())
            .flatten()
    });
    let pressed_elsewhere = press.is_some_and(|p| {
        ui.clip_rect().contains(p)
            && ui.ctx().layer_id_at(p) == Some(ui.layer_id())
            && !rect.contains(p)
            && !on_popup
    });
    finished |= pressed_elsewhere;

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
    for band in lay.bands(a, b) {
        let band = band.translate(text_origin.to_vec2());
        painter.rect_filled(band, 0.0, env.theme.accent.gamma_multiply(0.3));
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

    // The autocomplete list, under the caret.
    let (start, list) = complete::for_field(field, &defined);
    if list.is_empty() {
        ui.data_mut(|d| d.remove::<Rect>(popup_id));
        return;
    }
    let at = text_origin + Vec2::new(caret.min.x, caret.max.y + 4.0);
    let (rect, clicked) = popup(ui, popup_id, at, &list, field.pick, env);
    ui.data_mut(|d| d.insert_temp(popup_id, rect));
    if let Some(i) = clicked {
        accept(field, start, &list[i]);
        resp.request_focus();
    }
}

fn copy(ui: &Ui, field: &Field) {
    if field.has_selection() {
        ui.ctx().copy_text(field.selected_text());
    }
}

/// Type the chosen entry in place of the word before the caret.
fn accept(field: &mut Field, start: usize, c: &Candidate) {
    field.replace(start, field.caret, &c.insert);
    // What was just typed is a whole name; do not offer to complete it again.
    field.typing = false;
}

/// One key press. Returns true when it ends editing. `list` is the autocomplete
/// list showing (for the word that starts at `start`), if any.
fn key_down(
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

/// Draw the list at `at`. Returns its rectangle and the entry clicked, if any.
fn popup(
    ui: &Ui,
    id: Id,
    at: Pos2,
    list: &[Candidate],
    pick: usize,
    env: &Env,
) -> (Rect, Option<usize>) {
    let mut clicked = None;
    let area = egui::Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(at)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .fill(env.theme.surface)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for (i, c) in list.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let selected = i == pick.min(list.len() - 1);
                            let label = RichText::new(&c.label).monospace();
                            if ui.selectable_label(selected, label).clicked() {
                                clicked = Some(i);
                            }
                            ui.label(RichText::new(&c.detail).small().color(env.theme.text_weak));
                        });
                    }
                });
        });
    (area.response.rect, clicked)
}
