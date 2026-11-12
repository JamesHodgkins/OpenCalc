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
mod chrome;
mod editor;
mod errors;
mod grid;
pub mod grip;
pub mod hit;
pub mod magnet;
mod overlay;
pub(crate) mod page;
pub(crate) mod paint;
pub mod picture;
pub mod plot;
mod pointer;
pub mod resize;
mod result;
pub(crate) mod separate;
mod shape;
mod table;
pub mod text_style;
pub(crate) mod unit_slot;
pub mod view;

use eframe::egui::{self, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use opencalc_core::document::{PageSetup, Point, Region, RegionId, ShapeKind, Size};
use opencalc_core::edit::Edit;
use opencalc_core::eval::Output;
use opencalc_core::registry::UnitSystem;

use crate::theme::{type_size, Theme};
use adapt::{rect_of, Sizes};
use pointer::drag::Drag;
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
        /// A line only: starts at the right edge, starts at the bottom edge.
        flip: (bool, bool),
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
    /// Lines to show where the drag in progress snapped to another region's edge.
    guides: Vec<magnet::Guide>,
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
            guides: Vec::new(),
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
        self.guides.clear();
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
    pointer::navigate(ui, &resp, origin, view);

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

    chrome::selection(&painter, env.paper, regions, view, origin, st);

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
    pointer::pointer(ui, &resp, &scene, st, &mut cmds);

    abandon::remove_if_empty(st, regions, &mut cmds);
    overlay::marquee(&painter, view, origin, st, env.paper);
    for g in &st.guides {
        let at = view.doc_to_screen(Pos2::new(g.at, g.at), origin);
        let (a, b) = if g.vertical {
            (Pos2::new(at.x, rect.top()), Pos2::new(at.x, rect.bottom()))
        } else {
            (Pos2::new(rect.left(), at.y), Pos2::new(rect.right(), at.y))
        };
        painter.line_segment(
            [a, b],
            Stroke::new(1.0_f32, env.paper.selection.gamma_multiply(0.8)),
        );
    }
    chrome::pen_preview(ui, &painter, env, env.paper, view, origin, st);
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
