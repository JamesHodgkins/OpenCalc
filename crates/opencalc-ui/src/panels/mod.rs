//! Window furniture from spec section 4.1. Each panel reads application state
//! and pushes the commands the user raised; none changes state itself (rule R4).

pub mod about;
pub mod app_menu;
pub mod check;
pub mod colour_picker;
pub mod dialogs;
pub mod doc_tabs;
pub mod find;
pub mod functions;
pub mod home;
pub mod label;
pub mod labels;
pub mod matrix;
pub mod options;
pub mod page_setup;
pub mod picture_window;
pub mod plot_view;
pub mod plot_window;
pub mod shape_controls;
pub mod shortcuts;
pub mod status;
pub mod table_window;
pub mod text_window;
pub mod title;
pub mod units;

use crate::app::AppState;
use crate::commands::{CommandId, Registry};

/// The read-only view of the app a panel draws from.
pub struct PanelCx<'a> {
    pub state: &'a AppState,
    pub registry: &'a Registry,
}

/// Where a panel puts the commands the user raised this frame.
pub type Commands = Vec<CommandId>;

/// Where a dialog first opens: `align` is the screen corner or centre it sits
/// against and `offset` how far in. Used with `.pivot(align)` and
/// `.default_pos(..)` rather than `.anchor(..)`, because an anchored window is
/// pinned in place and cannot be dragged.
pub fn default_pos(
    ctx: &eframe::egui::Context,
    align: eframe::egui::Align2,
    offset: [f32; 2],
) -> eframe::egui::Pos2 {
    let screen = ctx.screen_rect();
    let anchor = align.pos_in_rect(&screen);
    eframe::egui::Pos2::new(anchor.x + offset[0], anchor.y + offset[1])
}
