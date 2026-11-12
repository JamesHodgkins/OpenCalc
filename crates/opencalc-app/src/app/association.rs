//! Making `.ocalc` files open in OpenCalc when they are double-clicked, and show the
//! worksheet icon.
//!
//! On Windows this is a few keys under the current user's `Software\Classes` (so no
//! administrator rights are needed, and nothing outside the user's own settings is
//! touched). They are written and removed with the system's own `reg` tool. Other
//! systems register file types through their installer or desktop files, so the
//! option is not offered there.

use std::path::{Path, PathBuf};
use std::process::Command;

use opencalc_core::persist::EXTENSION;

/// The name of the file type, as the registry knows it.
const PROG_ID: &str = "OpenCalc.Worksheet";
/// Where per-user file types live.
pub const USER_CLASSES: &str = r"HKCU\Software\Classes";

/// Whether this system's file type can be set from here, and whether it is.
pub trait FileType {
    fn supported(&self) -> bool;
    /// `.ocalc` files open in this copy of the program.
    fn registered(&self) -> bool;
    /// Register (`true`) or remove the registration.
    fn set(&mut self, on: bool) -> Result<(), String>;
}

/// The keys to write, as (key under the classes root, value): each key's default
/// value. `exe` is the program; resource 2 inside it is the worksheet icon.
pub fn entries(exe: &Path) -> Vec<(String, String)> {
    let exe = exe.display();
    vec![
        (format!(".{EXTENSION}"), PROG_ID.to_owned()),
        (PROG_ID.to_owned(), "OpenCalc worksheet".to_owned()),
        (format!(r"{PROG_ID}\DefaultIcon"), format!("\"{exe}\",-2")),
        (
            format!(r"{PROG_ID}\shell\open\command"),
            format!("\"{exe}\" \"%1\""),
        ),
    ]
}

/// The system's file types, through `reg`, under `root`.
pub struct NativeFileType {
    root: String,
    exe: Option<PathBuf>,
}

impl NativeFileType {
    pub fn new() -> NativeFileType {
        NativeFileType::under(USER_CLASSES, std::env::current_exe().ok())
    }

    /// Keys under another root, for trying it out without touching real settings.
    pub fn under(root: &str, exe: Option<PathBuf>) -> NativeFileType {
        NativeFileType {
            root: root.to_owned(),
            exe,
        }
    }

    fn key(&self, name: &str) -> String {
        format!(r"{}\{name}", self.root)
    }

    fn reg(args: &[&str]) -> Result<String, String> {
        let out = Command::new("reg")
            .args(args)
            .output()
            .map_err(|e| format!("could not run reg: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    }

    /// A key's default value, if it has one.
    fn default_of(&self, name: &str) -> Option<String> {
        let text = Self::reg(&["query", &self.key(name), "/ve"]).ok()?;
        // "    (Default)    REG_SZ    value"
        let line = text.lines().find(|l| l.contains("REG_SZ"))?;
        Some(line.split_once("REG_SZ")?.1.trim().to_owned())
    }
}

impl Default for NativeFileType {
    fn default() -> Self {
        NativeFileType::new()
    }
}

impl FileType for NativeFileType {
    fn supported(&self) -> bool {
        cfg!(windows) && self.exe.is_some()
    }

    fn registered(&self) -> bool {
        let Some(exe) = &self.exe else { return false };
        self.default_of(&format!(".{EXTENSION}")).as_deref() == Some(PROG_ID)
            && self
                .default_of(&format!(r"{PROG_ID}\shell\open\command"))
                .is_some_and(|c| c.contains(&exe.display().to_string()))
    }

    fn set(&mut self, on: bool) -> Result<(), String> {
        let exe = self
            .exe
            .clone()
            .ok_or("the program's own path is unknown")?;
        if on {
            for (name, value) in entries(&exe) {
                Self::reg(&["add", &self.key(&name), "/ve", "/d", &value, "/f"])?;
            }
            // Ask Explorer to look again at its icons; it carries on if that fails.
            let _ = Command::new("ie4uinit").arg("-show").status();
            return Ok(());
        }
        // Leave `.ocalc` alone if another program has taken it since.
        let dot = format!(".{EXTENSION}");
        if self.default_of(&dot).as_deref() == Some(PROG_ID) {
            Self::reg(&["delete", &self.key(&dot), "/f"])?;
        }
        if self.default_of(PROG_ID).is_some() {
            Self::reg(&["delete", &self.key(PROG_ID), "/f"])?;
        }
        Ok(())
    }
}

/// A file type that lives in memory, for tests.
#[derive(Default)]
pub struct MemoryFileType {
    pub on: bool,
    pub fail: bool,
}

impl FileType for MemoryFileType {
    fn supported(&self) -> bool {
        true
    }

    fn registered(&self) -> bool {
        self.on
    }

    fn set(&mut self, on: bool) -> Result<(), String> {
        if self.fail {
            return Err("access denied".to_owned());
        }
        self.on = on;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_name_the_type_its_icon_and_how_to_open_a_file() {
        let e = entries(Path::new(r"C:\Apps\OpenCalc\opencalc.exe"));
        let value = |key: &str| e.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        assert_eq!(value(".ocalc"), Some("OpenCalc.Worksheet"));
        assert_eq!(
            value(r"OpenCalc.Worksheet\DefaultIcon"),
            Some(r#""C:\Apps\OpenCalc\opencalc.exe",-2"#)
        );
        assert_eq!(
            value(r"OpenCalc.Worksheet\shell\open\command"),
            Some(r#""C:\Apps\OpenCalc\opencalc.exe" "%1""#)
        );
    }

    /// The real tool, against a scratch key, never the real file types.
    #[cfg(windows)]
    #[test]
    fn registering_and_removing_work_against_a_scratch_key() {
        let root = format!(r"HKCU\Software\OpenCalcTest{}\Classes", std::process::id());
        let exe = PathBuf::from(r"C:\Test Folder\opencalc.exe");
        let mut native = NativeFileType::under(&root, Some(exe));
        assert!(native.supported());
        assert!(!native.registered());
        native.set(true).unwrap();
        assert!(native.registered());
        assert_eq!(
            native
                .default_of(r"OpenCalc.Worksheet\shell\open\command")
                .as_deref(),
            Some(r#""C:\Test Folder\opencalc.exe" "%1""#),
            "a path with a space survives"
        );
        // Another program's claim on the extension is not removed.
        NativeFileType::reg(&["add", &native.key(".ocalc"), "/ve", "/d", "Other.App", "/f"])
            .unwrap();
        native.set(false).unwrap();
        assert_eq!(native.default_of(".ocalc").as_deref(), Some("Other.App"));
        assert!(native.default_of("OpenCalc.Worksheet").is_none());
        assert!(!native.registered());
        let scratch = root.rsplit_once(r"\Classes").unwrap().0.to_owned();
        let _ = NativeFileType::reg(&["delete", &scratch, "/f"]);
    }
}
