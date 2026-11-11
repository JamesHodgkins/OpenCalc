//! The Options window's commands, and taking new settings from it.

use super::dispatch::Effect;
use super::state::AppState;
use crate::commands::CommandId;
use crate::settings::Settings;
use crate::theme::{PaletteKey, Theme};

/// Run an Options command; `None` for any other command.
pub fn dispatch_options(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    let mut fx = Vec::new();
    match command {
        CommandId::AppOptions => {
            state.options_open = true;
            fx.push(Effect::RefreshFileType);
        }
        CommandId::AppOptionsClose => state.options_open = false,
        CommandId::ThemeNeutral => set_palette(state, &mut fx, PaletteKey::Neutral),
        CommandId::ThemeSlate => set_palette(state, &mut fx, PaletteKey::Slate),
        CommandId::FileTypeRegister => fx.push(Effect::SetFileType(true)),
        CommandId::FileTypeRemove => fx.push(Effect::SetFileType(false)),
        CommandId::FileClearRecent => state.recent.clear(),
        CommandId::AppOptionsDefaults => {
            set_palette(state, &mut fx, PaletteKey::Neutral);
            state.show_grid = true;
            state.snap = true;
            fx.extend(set_settings(state, Settings::default()));
        }
        _ => return None,
    }
    Some(fx)
}

/// Take new settings from the Options window. The grid applies to every open
/// worksheet; the defaults for new worksheets wait for the next one.
pub fn set_settings(state: &mut AppState, next: Settings) -> Vec<Effect> {
    let before = std::mem::replace(&mut state.settings, next);
    for sheet in &mut state.sheets {
        sheet.set_grid(next.grid);
    }
    if before.scale != next.scale || before.recovery != next.recovery {
        vec![Effect::ApplySettings]
    } else {
        Vec::new()
    }
}

fn set_palette(state: &mut AppState, fx: &mut Vec<Effect>, key: PaletteKey) {
    if state.palette != key {
        state.palette = key;
        state.theme = Theme::palette(key);
        fx.push(Effect::ApplyTheme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::GridSpacing;

    #[test]
    fn only_a_change_to_the_size_or_recovery_needs_the_app() {
        let mut s = AppState::new(PaletteKey::Neutral);
        let mut next = s.settings;
        next.grid = GridSpacing::Quarter;
        assert!(set_settings(&mut s, next).is_empty());
        assert_eq!(s.sheets[0].canvas.grid, GridSpacing::Quarter);
        next.scale = 110;
        assert_eq!(set_settings(&mut s, next), vec![Effect::ApplySettings]);
        next.recovery = 0;
        assert_eq!(set_settings(&mut s, next), vec![Effect::ApplySettings]);
    }
}
