//! Commands (SPEC section 4.7): the table, the key bindings, and what is
//! available right now. Every user action goes through here (rule R5).

mod id;
pub mod keys;
mod registry;
mod symbols;

pub use id::CommandId;
pub use keys::BINDINGS;
pub use registry::{CommandContext, Registry};
pub use symbols::{greek_by_name, greek_symbols};
