//! The math editor (docs/EDITOR.md section 4.4): in-place editing of a math region
//! on its typeset form. It owns a caret and a selection (UI state, not document
//! state) and turns input into `SetMathSource` edits, so undo is unchanged.
//!
//! The text rules live in [`text::Field`] and the autocomplete rules in
//! [`complete`]; this file reads input, asks the `SourceLayout` where the caret
//! goes, and draws.

mod blink;
mod complete;
mod greek;
mod keys;
mod popup;
mod text;
use keys::{accept, copy, grow, key_down};

use eframe::egui::{
    self, Align2, Event, EventFilter, FontId, Id, Key, Pos2, Rect, Sense, Stroke, Ui, Vec2,
};
use opencalc_core::document::{Region, RegionKind};
use opencalc_core::edit::Edit;

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
    let source_len = source.chars().count();
    if source_len > text::MAX_CHARS {
        refuse_long(ui, st, cmds, env, text_origin, zoom, source_len);
        return;
    }
    prepare_field(st, source);
    let Some(field) = st.field.as_mut() else {
        return;
    };
    let defined = complete::defined_names(cx.regions);
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

    pointer_input(ui, &resp, field, &lay, text_origin);
    let mut finished = false;
    if resp.has_focus() {
        finished |= key_events(ui, field, &mut lay, &defined, &layout_of);
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
    let leaving = field.leave.take();
    finished |= leaving.is_some();

    let text = field.text();
    if text != source {
        cmds.push(CanvasCmd::Edit(Edit::SetMathSource {
            id: r.id,
            source: text,
        }));
    }
    if finished {
        if !pressed_elsewhere && ui.input(|i| i.key_pressed(Key::Enter)) {
            super::overlay::next_input_below(st, r);
        } else if let Some(dir) = leaving {
            super::overlay::leave_toward(st, r, dir);
            // Let the next arrow press move the crosshair rather than be held here.
            ui.memory_mut(|m| m.surrender_focus(id));
        }
        st.editing = None;
        st.field = None;
        cmds.push(CanvasCmd::Seal);
        return;
    }

    let caret = paint(ui, env, field, &lay, text_origin, id, zoom);
    st.sizes.insert(
        r.id,
        Vec2::new(lay.layout.w + 2.0 * pad, lay.layout.h + 2.0 * pad) / zoom,
    );
    let below = text_origin + Vec2::new(caret.min.x, caret.max.y + 4.0);
    autocomplete(ui, env, field, &defined, popup_id, below, &resp);
}

/// A source too long to edit: keep it whole in the worksheet and say so. This editor
/// has no windowed-buffer model, so a truncated `Field` is never made active.
fn refuse_long(
    ui: &Ui,
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
    env: &Env,
    at: Pos2,
    zoom: f32,
    len: usize,
) {
    st.field = None;
    st.focus_edit = false;
    ui.painter().text(
        at,
        Align2::LEFT_TOP,
        format!(
            "Cannot edit: expression has {len} characters (limit {}).",
            text::MAX_CHARS
        ),
        FontId::proportional(12.0 * zoom),
        env.theme.error,
    );
    if ui.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter)) {
        st.editing = None;
        cmds.push(CanvasCmd::Seal);
    }
}

/// Make `st.field` the editing state for `source`: a new one when editing has just
/// started (with any selection or caret position asked for), else brought in step
/// with the source.
fn prepare_field(st: &mut CanvasUi, source: &str) {
    let fresh = st.focus_edit || st.field.is_none();
    let field = st.field.get_or_insert_with(|| Field::new(source));
    if fresh {
        *field = Field::new(source);
        if let Some((a, b)) = st.preselect.take() {
            field.select(a, b);
        }
        if std::mem::take(&mut st.enter_at_start) {
            field.set_caret(0, false);
        }
    } else if field.text() != source {
        field.sync(source);
    }
}

/// Pointer: press places the caret, dragging extends, double click picks a node.
fn pointer_input(
    ui: &Ui,
    resp: &egui::Response,
    field: &mut Field,
    lay: &SourceLayout,
    text_origin: Pos2,
) {
    let mods = ui.input(|i| i.modifiers);
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
}

/// Keyboard, text, paste, copy and cut for this frame. Gives back whether one of
/// them ended editing. The layout is redone after each event, so later events in
/// the frame (fast typing) see the text so far.
fn key_events(
    ui: &Ui,
    field: &mut Field,
    lay: &mut SourceLayout,
    defined: &[String],
    layout_of: &impl Fn(&Ui, &str) -> SourceLayout,
) -> bool {
    let mut finished = false;
    let events = ui.input(|i| i.events.clone());
    for event in events {
        let (start, list) = complete::for_field(field, defined);
        match event {
            // In a program a space is text; see `Field::typed`.
            Event::Text(s) if s == " " && field.in_program() && !field.has_selection() => {
                field.typed(&s, greek_by_name);
            }
            Event::Text(s) if s == " " => {
                // Space finishes a Greek name, else grows the selection.
                if field.has_selection() || !field.expand_greek(greek_by_name) {
                    grow(field, lay);
                }
            }
            Event::Text(s) if s == "=" && complete::defines_by_equals(field, defined) => {
                field.insert(":=");
            }
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
            } => finished |= key_down(field, lay, key, modifiers, start, &list),
            _ => {}
        }
        *lay = layout_of(ui, &field.text());
    }
    finished
}

/// Draw the selection, the typeset text and the caret. Gives back the caret's box.
fn paint(
    ui: &Ui,
    env: &Env,
    field: &Field,
    lay: &SourceLayout,
    text_origin: Pos2,
    id: Id,
    zoom: f32,
) -> Rect {
    let painter = ui.painter();
    let (a, b) = field.selection();
    for band in lay.bands(a, b) {
        let band = band.translate(text_origin.to_vec2());
        painter.rect_filled(band, 0.0, env.theme.accent.gamma_multiply(0.3));
    }
    lay.layout.paint(painter, text_origin, env.paper.text);
    let caret = lay.caret_rect(field.caret);
    if blink::caret_shown(ui, id) {
        let line = [
            text_origin + caret.min.to_vec2(),
            text_origin + caret.max.to_vec2(),
        ];
        painter.line_segment(line, Stroke::new(1.5_f32, env.theme.accent));
    }
    if field.clipped {
        super::overlay::clip_notice(painter, text_origin, lay.layout.h, zoom, env);
    }
    caret
}

/// The autocomplete list, under the caret.
fn autocomplete(
    ui: &Ui,
    env: &Env,
    field: &mut Field,
    defined: &[String],
    popup_id: Id,
    at: Pos2,
    resp: &egui::Response,
) {
    let (start, list) = complete::for_field(field, defined);
    if list.is_empty() {
        ui.data_mut(|d| d.remove::<Rect>(popup_id));
        return;
    }
    let (rect, clicked) = popup::show(ui, popup_id, at, &list, field.pick, env);
    ui.data_mut(|d| d.insert_temp(popup_id, rect));
    if let Some(i) = clicked {
        accept(field, start, &list[i]);
        resp.request_focus();
    }
}
