//! The text caret blinks, as carets do, but stays solid while you are typing or
//! moving it, so it is never lost mid-edit.

use std::time::Duration;

use eframe::egui::{Event, Id, Ui};

/// How long the caret is shown, and then hidden, in seconds.
const HALF_PERIOD: f64 = 0.53;

/// Whether the caret shows `idle` seconds after the last key or click.
fn shown_after(idle: f64) -> bool {
    idle.max(0.0) % (2.0 * HALF_PERIOD) < HALF_PERIOD
}

/// How long until the caret next switches, `idle` seconds after the last key.
fn next_switch(idle: f64) -> f64 {
    HALF_PERIOD - idle.max(0.0) % HALF_PERIOD
}

/// Whether to draw the caret of the editor `id` this frame. Also asks for a frame
/// when it will next change, so it blinks without the rest of the app redrawing.
pub fn caret_shown(ui: &Ui, id: Id) -> bool {
    let now = ui.input(|i| i.time);
    let active = ui.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                Event::Text(_)
                    | Event::Paste(_)
                    | Event::Cut
                    | Event::Key { pressed: true, .. }
                    | Event::PointerButton { pressed: true, .. }
            )
        })
    });
    let key = id.with("last-input");
    let last = if active {
        now
    } else {
        ui.data(|d| d.get_temp::<f64>(key)).unwrap_or(now)
    };
    ui.data_mut(|d| d.insert_temp(key, last));
    let idle = now - last;
    ui.ctx()
        .request_repaint_after(Duration::from_secs_f64(next_switch(idle).max(0.02)));
    shown_after(idle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_caret_is_on_just_after_input_then_off_then_on_again() {
        assert!(shown_after(0.0));
        assert!(shown_after(0.5));
        assert!(!shown_after(0.6));
        assert!(!shown_after(1.0));
        assert!(shown_after(1.1));
        assert!(
            shown_after(-1.0),
            "a clock that went back is not a reason to hide it"
        );
    }

    #[test]
    fn the_next_wake_up_is_when_the_caret_changes() {
        assert!((next_switch(0.0) - HALF_PERIOD).abs() < 1e-9);
        assert!((next_switch(0.5) - 0.03).abs() < 1e-9);
        for idle in [0.0, 0.2, 0.53, 0.8, 1.05, 3.3] {
            let wait = next_switch(idle);
            assert!(wait > 0.0 && wait <= HALF_PERIOD);
            assert_ne!(
                shown_after(idle + wait - 1e-6),
                shown_after(idle + wait + 1e-6)
            );
        }
    }
}
