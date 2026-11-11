//! Format migrations: one function per version step, applied to the untyped JSON
//! before it is read into a `Worksheet`. A file never skips a step, so each
//! migration only has to know about two neighbouring versions.

use serde_json::Value;

use super::LoadError;

/// One step: turns the worksheet JSON of version `n` into that of version `n + 1`.
pub type Step = fn(Value) -> Result<Value, LoadError>;

/// The steps for released versions. `STEPS[n - 1]` goes from version `n` to
/// `n + 1`, so its length is `FORMAT_VERSION - 1`.
pub const STEPS: &[Step] = &[v1_to_v2, v2_to_v3];

/// Version 3 added shape regions and the draw order (`z`) of a region. Both are
/// optional with defaults, so a version 2 worksheet reads unchanged; the number
/// moves so an older program refuses the file instead of dropping the shapes.
fn v2_to_v3(value: Value) -> Result<Value, LoadError> {
    Ok(value)
}

/// Version 2 added plot options and series styles. Both are optional with
/// defaults, so a version 1 worksheet reads unchanged; the version number moves so
/// that a program that only knows version 1 refuses a version 2 file instead of
/// quietly dropping the plot settings and losing them on its next save.
fn v1_to_v2(value: Value) -> Result<Value, LoadError> {
    Ok(value)
}

/// Before tables and pictures had content they were saved as the bare strings
/// `"Table"` and `"Picture"`. Turn each into an empty one, so those files (and any version 2
/// file made while the format was still changing) keep loading.
fn fill_placeholders(value: &mut Value) {
    let Some(regions) = value.get_mut("regions").and_then(Value::as_array_mut) else {
        return;
    };
    for r in regions {
        if r.get("kind").and_then(Value::as_str) == Some("Picture") {
            r["kind"] = serde_json::json!({"Picture": {"name": "", "alt": "", "data": ""}});
        }
        if r.get("kind").and_then(Value::as_str) == Some("Table") {
            r["kind"] = serde_json::json!({"Table": {
                "name": "",
                "columns": [{}, {}],
                "rows": [["", ""], ["", ""], ["", ""]],
            }});
        }
    }
}

/// Bring `value` from `from` up to `to` using `steps`.
pub fn run(steps: &[Step], from: u32, to: u32, mut value: Value) -> Result<Value, LoadError> {
    for version in from..to {
        let step = steps
            .get(version as usize - 1)
            .ok_or_else(|| LoadError::Migration(format!("no step from format {version}")))?;
        value = step(value)?;
    }
    Ok(value)
}

/// Migrate a worksheet from format `from` to the current one.
pub fn to_current(from: u32, value: Value) -> Result<Value, LoadError> {
    let mut value = run(STEPS, from, crate::FORMAT_VERSION, value)?;
    fill_placeholders(&mut value);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rename_title(mut v: Value) -> Result<Value, LoadError> {
        let o = v
            .as_object_mut()
            .ok_or(LoadError::Migration("not an object".into()))?;
        if let Some(t) = o.remove("name") {
            o.insert("title".into(), t);
        }
        Ok(v)
    }

    fn add_flag(mut v: Value) -> Result<Value, LoadError> {
        v["flag"] = json!(true);
        Ok(v)
    }

    #[test]
    fn steps_run_in_order_one_version_at_a_time() {
        let out = run(&[rename_title, add_flag], 1, 3, json!({"name": "T"})).unwrap();
        assert_eq!(out, json!({"title": "T", "flag": true}));
        // Starting later skips the earlier steps.
        let out = run(&[rename_title, add_flag], 2, 3, json!({"name": "T"})).unwrap();
        assert_eq!(out, json!({"name": "T", "flag": true}));
    }

    #[test]
    fn the_current_version_needs_no_steps_and_a_missing_step_is_an_error() {
        assert_eq!(run(&[], 2, 2, json!(1)).unwrap(), json!(1));
        assert!(matches!(
            run(&[], 1, 2, json!(1)),
            Err(LoadError::Migration(_))
        ));
    }

    #[test]
    fn a_placeholder_table_becomes_an_empty_table() {
        let old = json!({"regions": [
            {"id": 1, "kind": "Table"},
            {"id": 2, "kind": "Picture"},
            {"id": 3, "kind": {"Text": {"text": "Table"}}},
        ]});
        let new = to_current(1, old).unwrap();
        assert!(new["regions"][0]["kind"]["Table"]["rows"].is_array());
        assert!(new["regions"][1]["kind"]["Picture"].is_object());
        assert_eq!(new["regions"][2]["kind"]["Text"]["text"], "Table");
        // already filled in: left alone
        assert_eq!(to_current(2, new.clone()).unwrap(), new);
    }

    #[test]
    fn there_is_a_step_for_every_released_version() {
        assert_eq!(STEPS.len() as u32 + 1, crate::FORMAT_VERSION);
    }
}
