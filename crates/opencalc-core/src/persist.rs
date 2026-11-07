//! Saving and loading worksheets as bytes. See `docs/PERSISTENCE.md`.
//!
//! This module turns a [`Worksheet`] into the bytes of an `.ocx` file and back. It
//! does no file or window work (rule R1): the app and the CLI own paths and
//! dialogs, and both load through [`load_bytes`], so `check` verifies exactly what
//! the app would open.
//!
//! Loading is untrusted input (rule R6). Every outcome is a [`Loaded`] worksheet,
//! possibly with a list of repairs, or a [`LoadError`] saying why not. Nothing
//! panics, hangs or allocates without bound.

mod container;
mod file;
mod migrate;
mod validate;

use serde::{Deserialize, Serialize};

use crate::document::{RegionId, Worksheet};
use crate::registry::RegistryError;
use crate::FORMAT_VERSION;

pub use container::{MAX_ENTRIES, MAX_FILE_BYTES, MAX_MANIFEST_BYTES, MAX_WORKSHEET_BYTES};
pub use file::{read_limited, write_atomic};
pub use migrate::{run as run_migrations, Step};
pub use validate::{Repair, MAX_COORD, MAX_CUSTOM_UNITS, MAX_EXTENT, MAX_REGIONS, MAX_TEXT_CHARS};

/// The file extension, without the dot.
pub const EXTENSION: &str = "ocx";

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("this is not an OpenCalc worksheet (not a valid archive: {0})")]
    NotAnArchive(String),
    #[error("the file is damaged: {0}")]
    Corrupt(String),
    #[error("the archive has an unsafe entry name '{0}'")]
    UnsafePath(String),
    #[error("the archive has '{0}' twice")]
    DuplicateEntry(String),
    #[error("the file has no {0}")]
    Missing(&'static str),
    #[error("{what} is larger than the limit of {limit} bytes")]
    TooLarge { what: &'static str, limit: u64 },
    #[error("too many {what} (at most {limit})")]
    TooMany { what: &'static str, limit: usize },
    #[error("{what} is longer than {limit} characters")]
    TooLong { what: &'static str, limit: usize },
    #[error("the manifest is not valid: {0}")]
    BadManifest(String),
    #[error("this worksheet was made by a newer OpenCalc (file format {found}; this version reads up to {supported})")]
    TooNew { found: u32, supported: u32 },
    #[error("the worksheet cannot be read: {0}")]
    BadWorksheet(String),
    #[error("upgrading the file from an older format failed: {0}")]
    Migration(String),
    #[error("two regions have the same number ({})", .0 .0)]
    DuplicateId(RegionId),
    #[error("a region number is too large")]
    IdOverflow,
    #[error("a custom unit is not valid: {0}")]
    Units(RegistryError),
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// The worksheet holds something a file could not represent faithfully. This
    /// is a bug in the program, not something the user did.
    #[error("the worksheet cannot be saved: {0}")]
    Invalid(String),
    #[error("the file could not be built: {0}")]
    Container(String),
}

/// A worksheet read from a file.
#[derive(Debug)]
pub struct Loaded {
    pub worksheet: Worksheet,
    /// Problems in the file that were fixed on loading. Show them once.
    pub repairs: Vec<Repair>,
    /// Which program wrote the file, as it says in the manifest.
    pub saved_by: String,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    format_version: u32,
    #[serde(default)]
    app: String,
}

/// The bytes of the file for `ws`. The same worksheet always gives the same bytes.
pub fn save_bytes(ws: &Worksheet) -> Result<Vec<u8>, SaveError> {
    // Anything the loader would repair or refuse is a fault in the worksheet.
    let mut copy = ws.clone();
    match validate::check(&mut copy) {
        Ok(repairs) if repairs.is_empty() => {}
        Ok(repairs) => return Err(SaveError::Invalid(repairs[0].to_string())),
        Err(e) => return Err(SaveError::Invalid(e.to_string())),
    }
    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        app: format!("opencalc {}", crate::version()),
    };
    let manifest =
        serde_json::to_vec(&manifest).map_err(|e| SaveError::Container(e.to_string()))?;
    let mut worksheet =
        serde_json::to_vec_pretty(ws).map_err(|e| SaveError::Container(e.to_string()))?;
    worksheet.push(b'\n');
    container::pack(&manifest, &worksheet).map_err(SaveError::Container)
}

/// Read a worksheet from the bytes of a file.
pub fn load_bytes(bytes: &[u8]) -> Result<Loaded, LoadError> {
    let (manifest, worksheet) = container::unpack(bytes)?;
    let manifest: Manifest =
        serde_json::from_slice(&manifest).map_err(|e| LoadError::BadManifest(e.to_string()))?;
    if manifest.format_version == 0 {
        return Err(LoadError::BadManifest("format_version is 0".into()));
    }
    if manifest.format_version > FORMAT_VERSION {
        return Err(LoadError::TooNew {
            found: manifest.format_version,
            supported: FORMAT_VERSION,
        });
    }
    let value: serde_json::Value =
        serde_json::from_slice(&worksheet).map_err(|e| LoadError::BadWorksheet(e.to_string()))?;
    let value = migrate::to_current(manifest.format_version, value)?;
    let mut worksheet: Worksheet =
        serde_json::from_value(value).map_err(|e| LoadError::BadWorksheet(e.to_string()))?;
    let repairs = validate::check(&mut worksheet)?;
    Ok(Loaded {
        worksheet,
        repairs,
        saved_by: manifest.app,
    })
}
