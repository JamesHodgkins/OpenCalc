//! The worksheet canvas (spec section 4.2): one custom widget that owns pan,
//! zoom, painting, hit-testing, selection and drag.
//!
//! The pass reads the document and emits [`CanvasCmd`]s; the app applies them
//! after the frame, so the document is never mutated while it is being
//! iterated (rule R3). Selection, editing, drag and measured sizes are UI state
//! and live in [`CanvasUi`], not in the document.

mod abandon;
pub mod adapt;
pub mod align;
mod editor;
mod errors;
mod grid;
pub mod hit;
mod overlay;
pub(crate) mod page;
pub(crate) mod paint;
pub mod picture;
pub mod plot;
pub mod resize;
mod result;
pub(crate) mod separate;
mod shape;
mod table;
pub mod text_style;
pub(crate) mod unit_slot;
pub mod view;

use eframe::egui::{self, FontId, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use opencalc_core::document::{PageSetup, Point, Region, RegionId, ShapeKind, Size};
use opencalc_core::edit::Edit;
use opencalc_core::eval::Output;
use opencalc_core::registry::UnitSystem;

use crate::theme::{type_size, Theme};
use adapt::{point, rect_of, Sizes};
use hit::{click_selection, drag_positions, hit_test, in_marquee};
use view::snap;
pub use view::CanvasView;

/// Read-only inputs the canvas needs besides the document.
pub struct Env<'a> {
    /// The window theme: the desk, selection and overlays.
    pub theme: &'a Theme,
    /// The palette of what is drawn on a page (see `Theme::paper`).
    pub paper: &'a Theme,
    pub page: PageSetup,
    /// Header and footer text, and the title its `{title}` field shows.
    pub headers: &'a opencalc_core::document::HeaderFooter,
    pub title: &'a str,
    pub output: &'a Output,
    /// Manual calculation and the results are out of date: draw them dimmed.
    pub stale: bool,
    pub units: UnitSystem,
    pub show_grid: bool,
    pub snap: bool,
    /// A text box elsewhere in the window (the Functions search) holds the keyboard.
    pub text_box_focus: bool,
    /// Draft View: one continuous sheet, no page edges or margin box.
    pub draft: bool,
    /// Regions that hold matches for the open Find window.
    pub found: &'a crate::app::finding::Found,
}

/// What the canvas asks the app to do after the frame.
#[derive(Debug, PartialEq)]
pub enum CanvasCmd {
    /// A change to the document.
    Edit(Edit),
    /// A gesture ended (mouse released, in-place editing finished): the next
    /// edit starts a new undo step.
    Seal,
    /// Typing on the blank page: make a math region at the insertion point holding
    /// this text, and open it for editing.
    TypeAhead(String),
    /// A shape drawn with the drawing tool: make it with this box and select it.
    DrawShape {
        kind: ShapeKind,
        pos: Point,
        size: Size,
    },
}

enum Drag {
    Move {
        origins: Vec<(RegionId, Pos2)>,
        total: Vec2,
    },
    /// A handle of the selected region being pulled; `orig` is its box when the
    /// drag began (document space), so the result does not accumulate error.
    Resize {
        id: RegionId,
        handle: resize::Handle,
        orig: Rect,
    },
    /// A shape being drawn with the drawing tool.
    Draw { start: Pos2, current: Pos2 },
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
    /// Characters to select when the editor next opens (a template just inserted).
    pub preselect: Option<(usize, usize)>,
    /// The characters selected in a text region being edited, remembered after
    /// the editor closes so a style button applies to them: region and char range.
    pub text_sel: Option<(RegionId, usize, usize)>,
    /// What was being edited at the end of the last frame.
    last_editing: Option<RegionId>,
    /// Ask the in-place editor to take keyboard focus on its next frame.
    pub focus_edit: bool,
    /// The editor opens with the caret at the start rather than the end (the
    /// crosshair came in from the left or above).
    pub enter_at_start: bool,
    /// The math editor's text, caret and selection while a math region is edited.
    pub field: Option<editor::Field>,
    /// The unit being typed into the slot after a result.
    pub unit_edit: Option<unit_slot::UnitEdit>,
    /// The drawing tool: the next drag draws a shape of this kind (Escape or a
    /// finished shape puts it away).
    pub pen: Option<ShapeKind>,
    /// Where the next inserted region goes (document space).
    pub insert_at: Pos2,
    /// The snap grid (from the settings; `OpenSheet::set_grid` changes it).
    pub grid: crate::settings::GridSpacing,
    /// The last click was on blank page, so typing starts a math region there.
    pub armed: bool,
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
            preselect: None,
            text_sel: None,
            last_editing: None,
            focus_edit: false,
            enter_at_start: false,
            field: None,
            unit_edit: None,
            pen: None,
            insert_at: Pos2::new(40.0, 40.0),
            grid: Default::default(),
            // The initial insertion point is ready for type-to-create, just like
            // the point after moving the crosshair or clicking the blank page.
            armed: true,
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

    // The pages, and the grid inside their margins. Pages exist as far down as content reaches.
    let pages = page::page_count(&env.page, page::content_bottom(regions, &st.sizes));
    for screen in page::paint(&painter, rect, view, origin, &env.page, pages, env) {
        if env.show_grid {
            let on_page = painter.with_clip_rect(screen.intersect(rect));
            grid::grid(&on_page, screen, view, origin, env.paper, st.grid);
        }
    }

    for r in hit::back_to_front(regions) {
        let screen = view.rect_to_screen(rect_of(r, &st.sizes), origin);
        if !screen.intersects(rect.expand(100.0)) {
            continue;
        }
        let editing_here = st.editing == Some(r.id);
        let size = paint::region(ui, &painter, env, r, screen.min, view.zoom, editing_here);
        // While a region is edited, the editor sizes it from the form it is
        // typesetting (holes and all), which the strict parse measured here can
        // differ from; two writers made the frame lag behind the content.
        let editor_sizes_it = editing_here && st.field.is_some();
        if r.kind.is_auto_sized() && !editor_sizes_it {
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
            if st.selection.len() == 1 && resizable(r) {
                let inner = view.rect_to_screen(rect_of(r, &st.sizes), origin);
                for h in resize::Handle::ALL {
                    let square = Rect::from_center_size(h.point(inner), Vec2::splat(HANDLE_SIZE));
                    painter.rect_filled(square, 1.0, theme.base);
                    painter.rect_stroke(square, 1.0, outline, StrokeKind::Inside);
                }
            }
        }
    }

    overlay::marks(ui, &painter, view, origin, regions, st, theme);
    // Regions with a Find match: a light wash, stronger on the current one.
    for r in regions.iter().filter(|r| env.found.regions.contains(&r.id)) {
        let screen = view
            .rect_to_screen(rect_of(r, &st.sizes), origin)
            .expand(2.0);
        let strong = env.found.current == Some(r.id);
        let alpha = if strong { 90 } else { 45 };
        let wash = eframe::egui::Color32::from_rgba_unmultiplied(255, 200, 0, alpha);
        painter.rect_filled(screen, 2.0, wash);
        if strong {
            let edge = Stroke::new(1.5_f32, eframe::egui::Color32::from_rgb(230, 160, 0));
            painter.rect_stroke(screen, 2.0, edge, StrokeKind::Outside);
        }
    }

    let was_editing = st.editing.is_some();
    overlay::edit_overlay(ui, env, regions, view, origin, st, &mut cmds);
    if st.editing.is_some() {
        st.unit_edit = None;
    }
    unit_slot::edit(ui, env, regions, st, &mut cmds);
    if st.editing.is_none() {
        // (A key that just ended editing has already placed the crosshair.)
        if !was_editing {
            overlay::move_crosshair(ui, regions, st);
        }
        if st.editing.is_none() {
            overlay::crosshair(&painter, view, origin, st, env);
            overlay::type_to_create(ui, st, &mut cmds);
        }
    }
    let scene = Scene {
        env,
        regions,
        view,
        origin,
    };
    pointer(ui, &resp, &scene, st, &mut cmds);

    abandon::remove_if_empty(st, regions, &mut cmds);
    overlay::marquee(&painter, view, origin, st, theme);
    if let (Some(kind), Some(Drag::Draw { start, current })) = (st.pen, &st.drag) {
        let mods = ui.input(|i| i.modifiers);
        let grid = (env.snap && !mods.alt).then_some(st.grid.step());
        let (pos, size) = shape::drawn_box(
            kind,
            *start,
            *current,
            grid,
            mods.shift,
            mods.ctrl || mods.command,
        );
        let band = view.rect_to_screen(
            Rect::from_min_size(Pos2::new(pos.x, pos.y), Vec2::new(size.w, size.h)),
            origin,
        );
        shape::preview(&painter, kind, band, theme.accent);
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

/// Pixels from a handle's centre that still count as on it.
const HANDLE_REACH: f32 = 7.0;
/// Handle square, in pixels.
const HANDLE_SIZE: f32 = 8.0;

/// Whether the region has a size of its own to pull (typeset regions are sized by
/// their content) and is free to change.
fn resizable(r: &Region) -> bool {
    !r.locked && !r.kind.is_auto_sized()
}

/// The one selected region that shows resize handles, if there is one.
fn handled<'a>(regions: &'a [Region], st: &CanvasUi) -> Option<&'a Region> {
    let [id] = st.selection.as_slice() else {
        return None;
    };
    regions.iter().find(|r| r.id == *id && resizable(r))
}

/// The drawing tool: drag out a box for a shape. A click without a drag makes the
/// shape at its usual size with its corner at the click. Escape puts the tool away.
fn draw_with_pen(
    ui: &Ui,
    resp: &egui::Response,
    scene: &Scene,
    kind: ShapeKind,
    st: &mut CanvasUi,
    cmds: &mut Vec<CanvasCmd>,
) {
    let Scene {
        env, view, origin, ..
    } = *scene;
    let mods = ui.input(|i| i.modifiers);
    let grid = (env.snap && !mods.alt).then_some(st.grid.step());
    let (square, centre) = (mods.shift, mods.ctrl || mods.command);
    let to_doc = |p: Pos2| view.screen_to_doc(p, origin);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        st.pen = None;
        st.drag = None;
        return;
    }
    if resp.drag_started_by(PointerButton::Primary) {
        cmds.push(CanvasCmd::Seal);
        if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
            let start = to_doc(press);
            st.drag = Some(Drag::Draw {
                start,
                current: start,
            });
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let (Some(Drag::Draw { current, .. }), Some(p)) =
            (&mut st.drag, resp.interact_pointer_pos())
        {
            *current = to_doc(p);
        }
    }
    let finished = if resp.drag_stopped_by(PointerButton::Primary) {
        match st.drag.take() {
            Some(Drag::Draw { start, current }) => Some((start, current)),
            _ => None,
        }
    } else if resp.clicked() {
        resp.interact_pointer_pos().map(|p| (to_doc(p), to_doc(p)))
    } else {
        None
    };
    if let Some((start, end)) = finished {
        let (pos, size) = shape::drawn_box(kind, start, end, grid, square, centre);
        cmds.push(CanvasCmd::Seal);
        cmds.push(CanvasCmd::DrawShape { kind, pos, size });
        cmds.push(CanvasCmd::Seal);
        st.pen = None;
        st.drag = None;
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

    if let Some(kind) = st.pen {
        draw_with_pen(ui, resp, scene, kind, st, cmds);
        return;
    }
    let handle_under = |p: Pos2, st: &CanvasUi| {
        let r = handled(regions, st)?;
        let screen = view.rect_to_screen(rect_of(r, &st.sizes), origin);
        Some((
            r.id,
            resize::handle_at(screen, p, HANDLE_REACH)?,
            rect_of(r, &st.sizes),
        ))
    };
    if st.drag.is_none() && resp.hovered() {
        if let Some((_, h, _)) = ui
            .input(|i| i.pointer.hover_pos())
            .and_then(|p| handle_under(p, st))
        {
            ui.ctx().set_cursor_icon(h.cursor());
        }
    }
    if resp.drag_started_by(PointerButton::Primary) {
        cmds.push(CanvasCmd::Seal);
        if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
            let doc = to_doc(press);
            let grabbed = handle_under(press, st);
            match hit_test(regions, &st.sizes, doc) {
                _ if grabbed.is_some() => {
                    st.drag = grabbed.map(|(id, handle, orig)| Drag::Resize { id, handle, orig });
                }
                Some(id) => {
                    if !st.selection.contains(&id) {
                        st.selection = click_selection(&st.selection, Some(id), extend);
                    }
                    let origins: Vec<_> = regions
                        .iter()
                        .filter(|r| st.selection.contains(&r.id) && !r.locked)
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
                let grid = (env.snap && !mods.alt).then_some(st.grid.step());
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
            Some(Drag::Resize { id, handle, orig }) => {
                if let Some(p) = pos {
                    let grid = (env.snap && !mods.alt).then_some(st.grid.step());
                    let b = resize::resized(
                        *orig,
                        *handle,
                        to_doc(p),
                        grid,
                        mods.shift,
                        mods.ctrl || mods.command,
                    );
                    cmds.push(CanvasCmd::Edit(Edit::SetRegionBox {
                        id: *id,
                        pos: point(b.min),
                        size: Size::new(b.width(), b.height()),
                    }));
                }
            }
            Some(Drag::Draw { .. }) | None => {}
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
    // A click on the unit after a result opens it for typing.
    if resp.clicked() || resp.double_clicked() {
        if let Some(id) = unit_slot::hit(ui.ctx(), regions, pos) {
            cmds.push(CanvasCmd::Seal);
            st.selection = vec![id];
            st.editing = None;
            st.armed = false;
            st.unit_edit = unit_slot::begin(ui.ctx(), regions, id);
            return;
        }
    }
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
        st.armed = hit.is_none();
        if hit.is_none() {
            st.insert_at = snap(doc, st.grid.step());
        }
    }
}
