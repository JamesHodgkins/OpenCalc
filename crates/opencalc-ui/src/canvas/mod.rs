//! The worksheet canvas (spec section 4.2): one custom widget that owns pan,
//! zoom, painting, hit-testing, selection and drag.
//!
//! The pass reads the document and emits [`CanvasCmd`]s; the app applies them
//! after the frame, so the document is never mutated while it is being
//! iterated (rule R3). Selection, editing, drag and measured sizes are UI state
//! and live in [`CanvasUi`], not in the document.

pub mod adapt;
pub mod hit;
mod overlay;
mod page;
mod paint;
mod plot;
mod result;
pub mod view;

use eframe::egui::{self, FontId, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use opencalc_core::document::{PageSetup, Region, RegionId};
use opencalc_core::edit::Edit;
use opencalc_core::eval::Output;
use opencalc_core::registry::UnitSystem;

use crate::theme::{type_size, Theme};
use adapt::{point, rect_of, Sizes};
use hit::{click_selection, drag_positions, hit_test, in_marquee};
pub use view::CanvasView;
use view::{snap, GRID};

/// Read-only inputs the canvas needs besides the document.
pub struct Env<'a> {
    /// The window theme: the desk, selection and overlays.
    pub theme: &'a Theme,
    /// The palette of what is drawn on a page (see `Theme::paper`).
    pub paper: &'a Theme,
    pub page: PageSetup,
    pub output: &'a Output,
    pub units: UnitSystem,
    pub show_grid: bool,
    pub snap: bool,
    /// Draft View: one continuous sheet, no page edges or margin box.
    pub draft: bool,
}

/// What the canvas asks the app to do after the frame.
#[derive(Debug, PartialEq)]
pub enum CanvasCmd {
    /// A change to the document.
    Edit(Edit),
    /// A gesture ended (mouse released, in-place editing finished): the next
    /// edit starts a new undo step.
    Seal,
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
    /// Content sizes measured by the last paint.
    pub sizes: Sizes,
    /// Fit the page width into the viewport on the next frame (set when a
    /// worksheet is first shown).
    pub fit_pending: bool,
    drag: Option<Drag>,
}

impl Default for CanvasUi {
    fn default() -> Self {
        CanvasUi {
            selection: Vec::new(),
            editing: None,
            focus_edit: false,
            insert_at: Pos2::new(40.0, 40.0),
            sizes: Sizes::new(),
            fit_pending: true,
            drag: None,
        }
    }
}

impl CanvasUi {
    /// Forget anything that refers to regions that no longer exist (after undo,
    /// redo or delete).
    pub fn retain_existing(&mut self, regions: &[Region]) {
        let exists = |id: &RegionId| regions.iter().any(|r| r.id == *id);
        self.selection.retain(exists);
        if self.editing.is_some_and(|id| !exists(&id)) {
            self.editing = None;
        }
        self.sizes.retain(|id, _| exists(id));
    }

    /// Drop any drag in progress (after undo or redo changed the document under it).
    pub fn cancel_gesture(&mut self) {
        self.drag = None;
    }
}

pub struct CanvasOut {
    pub cmds: Vec<CanvasCmd>,
    /// Screen rectangle the canvas occupied.
    pub viewport: Rect,
    /// The page in view (counting from 1) and the number of pages.
    pub page: (usize, usize),
}

/// Everything the pointer logic reads, bundled so functions take one argument.
struct Scene<'a> {
    env: &'a Env<'a>,
    regions: &'a [Region],
    view: &'a CanvasView,
    origin: Pos2,
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

    if st.fit_pending && rect.width() > 1.0 {
        *view = page::fit_width(&env.page, rect.size());
        st.fit_pending = false;
    }
    navigate(ui, &resp, origin, view);

    // The pages, and the grid on them. Pages exist as far down as content reaches.
    let pages = page::page_count(&env.page, page::content_bottom(regions, &st.sizes));
    for screen in page::paint(&painter, rect, view, origin, &env.page, pages, env) {
        if env.show_grid {
            let on_page = painter.with_clip_rect(screen.intersect(rect));
            paint::grid(&on_page, screen, view, origin, env.paper);
        }
    }

    for r in regions {
        let screen = view.rect_to_screen(rect_of(r, &st.sizes), origin);
        if !screen.intersects(rect.expand(100.0)) {
            continue;
        }
        let editing_here = st.editing == Some(r.id);
        let size = paint::region(ui, &painter, env, r, screen.min, view.zoom, editing_here);
        if r.kind.is_auto_sized() {
            st.sizes.insert(r.id, size);
        }
    }

    let outline = Stroke::new(1.5_f32, theme.accent);
    for id in &st.selection {
        if let Some(r) = regions.iter().find(|r| r.id == *id) {
            let screen = view
                .rect_to_screen(rect_of(r, &st.sizes), origin)
                .expand(2.0);
            painter.rect_stroke(screen, 2.0, outline, StrokeKind::Outside);
        }
    }

    overlay::edit_overlay(ui, regions, view, origin, st, &mut cmds);
    let scene = Scene {
        env,
        regions,
        view,
        origin,
    };
    pointer(ui, &resp, &scene, st, &mut cmds);

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
    let middle = view.screen_to_doc(rect.center(), origin).y;
    CanvasOut {
        cmds,
        viewport: rect,
        page: (page::page_at(&env.page, middle, pages) + 1, pages),
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

fn pointer(
    ui: &Ui,
    resp: &egui::Response,
    scene: &Scene,
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    let Scene {
        env,
        regions,
        view,
        origin,
    } = *scene;
    let mods = ui.input(|i| i.modifiers);
    let extend = mods.shift || mods.command;
    let to_doc = |p: Pos2| view.screen_to_doc(p, origin);

    if resp.drag_started_by(PointerButton::Primary) {
        cmds.push(CanvasCmd::Seal);
        if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
            let doc = to_doc(press);
            match hit_test(regions, &st.sizes, doc) {
                Some(id) => {
                    if !st.selection.contains(&id) {
                        st.selection = click_selection(&st.selection, Some(id), extend);
                    }
                    let origins: Vec<_> = regions
                        .iter()
                        .filter(|r| st.selection.contains(&r.id))
                        .map(|r| (r.id, adapt::pos(r.pos)))
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
                let moves = drag_positions(origins, *total, grid)
                    .into_iter()
                    .map(|(id, p)| (id, point(p)))
                    .collect();
                cmds.push(CanvasCmd::Edit(Edit::MoveRegions { moves }));
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
        cmds.push(CanvasCmd::Seal);
        if let Some(Drag::Marquee {
            start,
            current,
            base,
        }) = st.drag.take()
        {
            let mut sel = base;
            for id in in_marquee(regions, &st.sizes, Rect::from_two_pos(start, current)) {
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
        cmds.push(CanvasCmd::Seal);
        if let Some(id) = hit_test(regions, &st.sizes, to_doc(pos)) {
            if regions.iter().any(|r| r.id == id && r.kind.is_editable()) {
                st.selection = vec![id];
                st.editing = Some(id);
                st.focus_edit = true;
            }
        }
    } else if resp.clicked() {
        cmds.push(CanvasCmd::Seal);
        let doc = to_doc(pos);
        let hit = hit_test(regions, &st.sizes, doc);
        st.selection = click_selection(&st.selection, hit, extend);
        if hit != st.editing {
            st.editing = None;
        }
        if hit.is_none() {
            st.insert_at = snap(doc, GRID);
        }
    }
}
