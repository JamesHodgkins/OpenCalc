//! Commands (SPEC section 4.7): the table, the key bindings, and what is
//! available right now. Every user action goes through here (rule R5).

mod id;
mod keys;
mod registry;

pub use id::{greek_by_name, greek_symbols, CommandId};
pub use keys::BINDINGS;
pub use registry::{CommandContext, Registry};
