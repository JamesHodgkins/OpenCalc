//! Format migrations: one function per version step, applied to the untyped JSON
//! before it is read into a `Worksheet`. A file never skips a step, so each
//! migration only has to know about two neighbouring versions.

use serde_json::Value;

use super::LoadError;

/// One step: turns the worksheet JSON of version `n` into that of version `n + 1`.
pub type Step = fn(Value) -> Result<Value, LoadError>;

/// The steps for released versions. `STEPS[n - 1]` goes from version `n` to
/// `n + 1`, so its length is `FORMAT_VERSION - 1`. Version 1 is the first, so
/// there are none yet.
pub const STEPS: &[Step] = &[];

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
    run(STEPS, from, crate::FORMAT_VERSION, value)
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
    fn there_is_a_step_for_every_released_version() {
        assert_eq!(STEPS.len() as u32 + 1, crate::FORMAT_VERSION);
    }
}
