//! OpenCalc core: document model, parser, evaluator and units.
//!
//! This crate must not depend on any UI or windowing code. The UI and the
//! CLI both build on it, and it is the part tested most heavily.

pub mod document;
pub mod edit;
pub mod eval;
pub mod expr;
pub mod parser;
pub mod units;
pub mod worker;

/// Version of the on-disk document format. Increment with every breaking change
/// and add a migration for each step.
pub const FORMAT_VERSION: u32 = 1;

/// Crate version, for display in About dialogs and the CLI.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_version_is_one() {
        assert_eq!(FORMAT_VERSION, 1);
    }

    #[test]
    fn version_is_set() {
        assert!(!version().is_empty());
    }
}
