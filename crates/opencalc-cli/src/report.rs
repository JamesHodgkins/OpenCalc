//! The text `check` prints. Separate from `main` so it can be tested.

use std::io::Write;
use std::path::Path;

use opencalc_core::check::{check, Finding, Severity};
use opencalc_core::persist::{load_bytes, read_limited};

/// What checking a file found, as the process exit code.
pub const CLEAN: u8 = 0;
pub const PROBLEMS: u8 = 1;
pub const UNREADABLE: u8 = 2;

/// One finding as text: a header line, then the source with the failing part
/// marked underneath when it is known.
pub fn render(file: &str, f: &Finding) -> String {
    let kind = match f.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let mut text = format!(
        "{file}: region {} ({:.0}, {:.0}): {kind}: {}\n",
        f.region.0, f.at.0, f.at.1, f.message
    );
    if let Some(source) = &f.source {
        // One line, so the marker lines up whatever the source holds.
        let flat: String = source
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        text.push_str(&format!("    {flat}\n"));
        if let Some((start, end)) = f.range {
            let width = end.saturating_sub(start).max(1);
            text.push_str(&format!("    {}{}\n", " ".repeat(start), "^".repeat(width)));
        }
    }
    text
}

/// Check one file, printing findings to `out` and trouble reading it to `err`.
/// Returns the exit code for this file.
pub fn check_file(
    path: &Path,
    quiet: bool,
    out: &mut impl Write,
    err: &mut impl Write,
) -> std::io::Result<u8> {
    let name = path.display().to_string();
    let loaded = match read_limited(path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| load_bytes(&bytes).map_err(|e| e.to_string()))
    {
        Ok(loaded) => loaded,
        Err(reason) => {
            writeln!(err, "{name}: error: cannot open: {reason}")?;
            return Ok(UNREADABLE);
        }
    };
    let mut warnings = 0;
    for repair in &loaded.repairs {
        warnings += 1;
        writeln!(out, "{name}: warning: repaired on load: {repair}")?;
    }
    let findings = check(&loaded.worksheet);
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    warnings += findings.len() - errors;
    for f in &findings {
        write!(out, "{}", render(&name, f))?;
    }
    if !quiet {
        writeln!(
            out,
            "{name}: {} regions, {errors} error(s), {warnings} warning(s)",
            loaded.worksheet.regions().len()
        )?;
    }
    Ok(if errors + warnings == 0 {
        CLEAN
    } else {
        PROBLEMS
    })
}
