//! Window furniture from spec section 4.1. Each panel reads application state
//! and pushes the commands the user raised; none changes state itself (rule R4).

pub mod about;
pub mod dialogs;
pub mod doc_tabs;
pub mod find;
pub mod functions;
pub mod home;
pub mod options;
pub mod page_setup;
pub mod plot_window;
pub mod status;
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
