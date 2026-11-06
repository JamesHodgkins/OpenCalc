//! The worksheet canvas (spec section 4.2): one custom widget that owns pan,
//! zoom, painting, hit-testing, selection and drag.
//!
//! The pass reads the document and emits [`DocCmd`]s; the app applies them
//! after the frame, so the document is never mutated while it is being
//! iterated. Selection, editing and drag are UI state and live in [`CanvasUi`].

pub mod hit;
mod paint;
pub mod view;

use eframe::egui::{
    self, FontId, Id, Key, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
};
use opencalc_core::eval::{Output, RegionId};

use crate::sheet::{Region, RegionKind};
use crate::theme::{type_size, Theme};
use hit::{click_selection, drag_positions, hit_test, in_marquee};
pub use view::CanvasView;
use view::{snap, GRID};

/// Read-only inputs the canvas needs besides the document.
pub struct Env<'a> {
    pub theme: &'a Theme,
    pub output: &'a Output,
    pub show_grid: bool,
    pub snap: bool,
}

/// Document edits requested by the canvas.
#[derive(Debug, PartialEq)]
pub enum DocCmd {
    Move(Vec<(RegionId, Pos2)>),
    /// A measured content size for an auto-sized region.
    Resize(RegionId, Vec2),
    SetSource(RegionId, String),
    SetText(RegionId, String),
}

enum Drag {
    Move {
        origins: Vec<(RegionId, Pos2)>,
        total: Vec2,
    },
    Marquee {
        start: Pos2,
        current: Pos2,
        /// Selection to keep when the marquee extends rather than replaces it.
        base: Vec<RegionId>,
    },
}

/// Ephemeral per-sheet UI state. Not part of the document.
pub struct CanvasUi {
    pub selection: Vec<RegionId>,
    pub editing: Option<RegionId>,
    /// Ask the in-place editor to take keyboard focus on its next frame.
    pub focus_edit: bool,
    /// Where the next inserted region goes (document space).
    pub insert_at: Pos2,
    drag: Option<Drag>,
}

impl Default for CanvasUi {
    fn default() -> Self {
        CanvasUi {
            selection: Vec::new(),
            editing: None,
            focus_edit: false,
            insert_at: Pos2::new(40.0, 40.0),
            drag: None,
        }
    }
}

pub struct CanvasOut {
    pub cmds: Vec<DocCmd>,
    /// Screen rectangle the canvas occupied.
    pub viewport: Rect,
}

pub fn show(
    ui: &mut Ui,
    env: &Env,
    regions: &[Region],
    view: &mut CanvasView,
    st: &mut CanvasUi,
) -> CanvasOut {
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let origin = rect.min;
    let painter = ui.painter_at(rect);
    let theme = env.theme;
    let mut cmds = Vec::new();

    navigate(ui, &resp, origin, view);
    if env.show_grid {
        paint::grid(&painter, rect, view, origin, theme);
    }

    for r in regions {
        let screen = view.rect_to_screen(r.rect(), origin);
        if !screen.intersects(rect.expand(100.0)) {
            continue;
        }
        let editing_here = st.editing == Some(r.id);
        let size = paint::region(ui, &painter, env, r, screen.min, view.zoom, editing_here);
        if r.kind.is_auto_sized() && (size - r.size).abs().max_elem() > 0.5 {
            cmds.push(DocCmd::Resize(r.id, size));
        }
    }

    let outline = Stroke::new(1.5_f32, theme.accent);
    for id in &st.selection {
        if let Some(r) = regions.iter().find(|r| r.id == *id) {
            let screen = view.rect_to_screen(r.rect(), origin).expand(2.0);
            painter.rect_stroke(screen, 2.0, outline, StrokeKind::Outside);
        }
    }

    edit_overlay(ui, regions, view, origin, st, &mut cmds);
    pointer(ui, &resp, env, regions, view, origin, st, &mut cmds);

    if let Some(Drag::Marquee { start, current, .. }) = &st.drag {
        let m = view.rect_to_screen(Rect::from_two_pos(*start, *current), origin);
        painter.rect_filled(m, 0.0, theme.accent.gamma_multiply(0.18));
        painter.rect_stroke(
            m,
            0.0,
            Stroke::new(1.0_f32, theme.accent),
            StrokeKind::Inside,
        );
    }
    painter.text(
        rect.min + Vec2::new(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        format!("{:.0}%", view.zoom * 100.0),
        FontId::proportional(type_size::SMALL),
        theme.text_weak,
    );
    CanvasOut {
        cmds,
        viewport: rect,
    }
}

/// Wheel pans, Ctrl+wheel (or pinch) zooms about the pointer, middle-drag pans.
fn navigate(ui: &Ui, resp: &egui::Response, origin: Pos2, view: &mut CanvasView) {
    if resp.hovered() {
        let zoom = ui.input(|i| i.zoom_delta());
        if (zoom - 1.0).abs() > f32::EPSILON {
            if let Some(p) = ui.input(|i| i.pointer.hover_pos()) {
                view.zoom_about(p, origin, zoom);
            }
        } else {
            view.offset += ui.input(|i| i.smooth_scroll_delta);
        }
    }
    if resp.dragged_by(PointerButton::Middle) {
        view.offset += resp.drag_delta();
    }
}

#[allow(clippy::too_many_arguments)]
fn pointer(
    ui: &Ui,
    resp: &egui::Response,
    env: &Env,
    regions: &[Region],
    view: &CanvasView,
    origin: Pos2,
    st: &mut CanvasUi,
    cmds: &mut Vec<DocCmd>,
) {
    let mods = ui.input(|i| i.modifiers);
    let extend = mods.shift || mods.command;
    let to_doc = |p: Pos2| view.screen_to_doc(p, origin);

    if resp.drag_started_by(PointerButton::Primary) {
        if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
            let doc = to_doc(press);
            match hit_test(regions, doc) {
                Some(id) => {
                    if !st.selection.contains(&id) {
                        st.selection = click_selection(&st.selection, Some(id), extend);
                    }
                    let origins: Vec<_> = regions
                        .iter()
                        .filter(|r| st.selection.contains(&r.id))
                        .map(|r| (r.id, r.pos))
                        .collect();
                    st.drag = Some(Drag::Move {
                        origins,
                        total: Vec2::ZERO,
                    });
                }
                None => {
                    let base = if extend {
                        st.selection.clone()
                    } else {
                        Vec::new()
                    };
                    st.selection = base.clone();
                    st.drag = Some(Drag::Marquee {
                        start: doc,
                        current: doc,
                        base,
                    });
                }
            }
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        let delta = resp.drag_delta() / view.zoom;
        let pos = resp.interact_pointer_pos();
        match &mut st.drag {
            Some(Drag::Move { origins, total }) => {
                *total += delta;
                let grid = (env.snap && !mods.alt).then_some(GRID);
                cmds.push(DocCmd::Move(drag_positions(origins, *total, grid)));
            }
            Some(Drag::Marquee { current, .. }) => {
                if let Some(p) = pos {
                    *current = to_doc(p);
                }
            }
            None => {}
        }
    }
    if resp.drag_stopped_by(PointerButton::Primary) {
        if let Some(Drag::Marquee {
            start,
            current,
            base,
        }) = st.drag.take()
        {
            let mut sel = base;
            for id in in_marquee(regions, Rect::from_two_pos(start, current)) {
                if !sel.contains(&id) {
                    sel.push(id);
                }
            }
            st.selection = sel;
        }
        st.drag = None;
    }

    let Some(pos) = resp.interact_pointer_pos() else {
        return;
    };
    if resp.double_clicked() {
        if let Some(id) = hit_test(regions, to_doc(pos)) {
            if regions.iter().any(|r| r.id == id && r.kind.is_editable()) {
                st.selection = vec![id];
                st.editing = Some(id);
                st.focus_edit = true;
            }
        }
    } else if resp.clicked() {
        let doc = to_doc(pos);
        let hit = hit_test(regions, doc);
        st.selection = click_selection(&st.selection, hit, extend);
        if hit != st.editing {
            st.editing = None;
        }
        if hit.is_none() {
            st.insert_at = snap(doc, GRID);
        }
    }
}

/// The in-place editor for the region being edited.
fn edit_overlay(
    ui: &mut Ui,
    regions: &[Region],
    view: &CanvasView,
    origin: Pos2,
    st: &mut CanvasUi,
    cmds: &mut Vec<DocCmd>,
) {
    let Some(id) = st.editing else { return };
    let Some(r) = regions.iter().find(|r| r.id == id) else {
        st.editing = None;
        return;
    };
    let screen = view.rect_to_screen(r.rect(), origin);
    let zoom = view.zoom;
    let widget_id = Id::new(("region", id.0));
    let width = screen.width().max(220.0 * zoom);
    let (resp, change) = match &r.kind {
        RegionKind::Math { source, .. } => {
            let mut buf = source.clone();
            let rect = Rect::from_min_size(screen.min, Vec2::new(width, 28.0 * zoom));
            let edit = TextEdit::singleline(&mut buf)
                .id(widget_id)
                .font(FontId::monospace(14.0 * zoom))
                .desired_width(width);
            let resp = ui.put(rect, edit);
            let change = resp.changed().then_some(DocCmd::SetSource(id, buf));
            (resp, change)
        }
        RegionKind::Text { text } => {
            let mut buf = text.clone();
            let rect = Rect::from_min_size(
                screen.min,
                Vec2::new(r.size.x * zoom, screen.height().max(40.0 * zoom)),
            );
            let edit = TextEdit::multiline(&mut buf)
                .id(widget_id)
                .font(FontId::proportional(14.0 * zoom))
                .desired_width(rect.width());
            let resp = ui.put(rect, edit);
            let change = resp.changed().then_some(DocCmd::SetText(id, buf));
            (resp, change)
        }
        _ => {
            st.editing = None;
            return;
        }
    };
    cmds.extend(change);
    if st.focus_edit {
        resp.request_focus();
        st.focus_edit = false;
    } else if resp.lost_focus() || ui.input(|i| i.key_pressed(Key::Escape)) {
        st.editing = None;
    }
}
