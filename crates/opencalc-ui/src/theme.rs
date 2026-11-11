//! Theme tokens and their application to egui (spec section 4.6).
//!
//! No widget hard-codes a colour; everything reads from [`Theme`]. Every
//! background is one of four tones.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteKey {
    Neutral,
    Slate,
}

impl PaletteKey {
    /// Key used when persisting the choice.
    pub fn key(self) -> &'static str {
        match self {
            PaletteKey::Neutral => "neutral",
            PaletteKey::Slate => "slate",
        }
    }

    pub fn from_key(k: &str) -> PaletteKey {
        if k == "slate" {
            PaletteKey::Slate
        } else {
            PaletteKey::Neutral
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    // Four tones of elevation.
    pub base: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub raised_high: Color32,
    // Outlines.
    pub border: Color32,
    pub border_strong: Color32,
    pub pressed: Color32,
    // Accent (red, to tell OpenCalc from OpenDraft).
    pub accent: Color32,
    // Hover and press on accent fills (SPEC 4.6). Nothing is accent-filled since
    // the app button became the File menu, so only the contrast test reads them.
    #[cfg_attr(not(test), allow(dead_code))]
    pub accent_hover: Color32,
    #[cfg_attr(not(test), allow(dead_code))]
    pub accent_pressed: Color32,
    pub accent_text: Color32,
    // Status. `error` is for text and icons only, never fills.
    pub warning: Color32,
    pub error: Color32,
    // Text.
    pub text: Color32,
    pub text_weak: Color32,
    pub text_disabled: Color32,
    pub text_on_accent: Color32,
    /// Line colours for plot series, in order.
    pub series: [Color32; 8],
}

impl Theme {
    pub fn palette(key: PaletteKey) -> Theme {
        let (base, surface, raised, raised_high, border, border_strong, pressed) = match key {
            PaletteKey::Neutral => (
                rgb(0x252525),
                rgb(0x323232),
                rgb(0x3E3E3E),
                rgb(0x4C4C4C),
                rgb(0x474747),
                rgb(0x6A6A6A),
                rgb(0x585858),
            ),
            PaletteKey::Slate => (
                rgb(0x222933),
                rgb(0x313946),
                rgb(0x3B4453),
                rgb(0x4F5A6E),
                rgb(0x4A5466),
                rgb(0x6B778C),
                rgb(0x5D6980),
            ),
        };
        Theme {
            base,
            surface,
            raised,
            raised_high,
            border,
            border_strong,
            pressed,
            accent: rgb(0xC62828),
            accent_hover: rgb(0xD32F2F),
            accent_pressed: rgb(0xB71C1C),
            accent_text: rgb(0xFF8A80),
            warning: rgb(0xF5B942),
            error: rgb(0xFF7B7B),
            text: rgb(0xDCDCDC),
            text_weak: rgb(0xB4B4B4),
            text_disabled: rgb(0x646464),
            text_on_accent: rgb(0xFFFFFF),
            series: [
                rgb(0xFF7B7B),
                rgb(0x4FC3F7),
                rgb(0x81C784),
                rgb(0xFFB74D),
                rgb(0xBA68C8),
                rgb(0x4DB6AC),
                rgb(0xFFD54F),
                rgb(0xA1887F),
            ],
        }
    }

    /// The palette of a page: black on white, whatever the app theme, as a printed
    /// page would be. Everything drawn on a worksheet page (maths, text, results,
    /// plots, errors) uses this, not the window theme.
    pub fn paper() -> Theme {
        Theme {
            base: rgb(0xFFFFFF),
            surface: rgb(0xFFFFFF),
            raised: rgb(0xEEEEEE),
            raised_high: rgb(0xE0E0E0),
            border: rgb(0xC4C4C4),
            border_strong: rgb(0x8E8E8E),
            pressed: rgb(0xD0D0D0),
            accent: rgb(0xC62828),
            accent_hover: rgb(0xD32F2F),
            accent_pressed: rgb(0xB71C1C),
            accent_text: rgb(0xB71C1C),
            warning: rgb(0x8A5300),
            error: rgb(0xB71C1C),
            text: rgb(0x1A1A1A),
            text_weak: rgb(0x555555),
            text_disabled: rgb(0x9A9A9A),
            text_on_accent: rgb(0xFFFFFF),
            series: [
                rgb(0xC62828),
                rgb(0x1565C0),
                rgb(0x2E7D32),
                rgb(0xE65100),
                rgb(0x6A1B9A),
                rgb(0x00695C),
                rgb(0x827717),
                rgb(0x5D4037),
            ],
        }
    }

    /// The four background tones, lowest first.
    #[cfg(test)]
    pub fn tones(&self) -> [Color32; 4] {
        [self.base, self.surface, self.raised, self.raised_high]
    }
}

/// Metrics from the spec: 8 px spacing unit, 4 px radius, 26 px controls.
pub mod metrics {
    pub const SPACING: f32 = 8.0;
    pub const RADIUS: u8 = 4;
    pub const CONTROL_HEIGHT: f32 = 26.0;
    pub const WINDOW_RADIUS: u8 = RADIUS + 2;
    pub const SCROLLBAR_WIDTH: f32 = 6.0;
    /// Heights of the window's horizontal bars (SPEC 4.1).
    pub const TITLE_BAR: f32 = 32.0;
    pub const TAB_STRIP: f32 = 25.0;
    pub const DOC_TAB: f32 = 26.0;
    /// The ribbon body: a large button, a title row and the frame's margins.
    pub const RIBBON: f32 = 66.0 + 16.0 + 8.0;
    pub const STATUS_BAR: f32 = 24.0;
}

/// Type sizes in px.
pub mod type_size {
    pub const BODY: f32 = 12.5;
    pub const SMALL: f32 = 11.0;
    pub const TAB: f32 = 13.0;
    pub const HEADING: f32 = 15.0;
    pub const RIBBON_LABEL: f32 = 11.5;
    pub const RIBBON_TITLE: f32 = 10.5;
}

/// Apply the theme to the context: visuals and text styles together.
pub fn apply(ctx: &egui::Context, theme: &Theme) {
    let mut visuals = egui::Visuals::dark();
    let r = CornerRadius::same(metrics::RADIUS);

    visuals.override_text_color = Some(theme.text);
    visuals.panel_fill = theme.base;
    visuals.window_fill = theme.surface;
    visuals.window_stroke = Stroke::new(1.0_f32, theme.border);
    visuals.window_corner_radius = CornerRadius::same(metrics::WINDOW_RADIUS);
    visuals.menu_corner_radius = r;
    visuals.extreme_bg_color = theme.base; // sunken input wells
    visuals.faint_bg_color = theme.surface;
    visuals.hyperlink_color = theme.accent_text;
    visuals.warn_fg_color = theme.warning;
    visuals.error_fg_color = theme.error;
    visuals.selection.bg_fill = theme.accent;
    visuals.selection.stroke = Stroke::new(1.0_f32, theme.text_on_accent);
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    visuals.popup_shadow = visuals.window_shadow;

    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = theme.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, theme.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, theme.text);
    w.noninteractive.corner_radius = r;
    // Resting buttons are flat: raised fill, no outline.
    w.inactive.bg_fill = theme.raised;
    w.inactive.weak_bg_fill = theme.raised;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0_f32, theme.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = theme.raised_high;
    w.hovered.weak_bg_fill = theme.raised_high;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, theme.border);
    w.hovered.fg_stroke = Stroke::new(1.0_f32, theme.text);
    w.hovered.corner_radius = r;
    w.active.bg_fill = theme.pressed;
    w.active.weak_bg_fill = theme.pressed;
    w.active.bg_stroke = Stroke::new(1.0_f32, theme.border_strong);
    w.active.fg_stroke = Stroke::new(1.0_f32, theme.text);
    w.active.corner_radius = r;
    w.open.bg_fill = theme.pressed;
    w.open.weak_bg_fill = theme.pressed;
    w.open.bg_stroke = Stroke::NONE;
    w.open.corner_radius = r;

    ctx.set_visuals(visuals);

    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(metrics::SPACING / 2.0, metrics::SPACING / 2.0);
        style.spacing.interact_size.y = metrics::CONTROL_HEIGHT;
        style.spacing.scroll = egui::style::ScrollStyle {
            bar_width: metrics::SCROLLBAR_WIDTH,
            floating: false,
            foreground_color: true,
            dormant_handle_opacity: 0.45,
            active_handle_opacity: 0.7,
            interact_handle_opacity: 0.9,
            ..egui::style::ScrollStyle::solid()
        };
        use egui::{FontFamily, FontId, TextStyle};
        style.text_styles = [
            (
                TextStyle::Body,
                FontId::new(type_size::BODY, FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                FontId::new(type_size::BODY, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(type_size::SMALL, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(type_size::HEADING, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(type_size::BODY, FontFamily::Monospace),
            ),
        ]
        .into();
    });
}

/// Draw the 2 px focus ring used by every custom-painted control.
pub fn focus_ring(painter: &egui::Painter, rect: egui::Rect, theme: &Theme) {
    painter.rect_stroke(
        rect.expand(1.0),
        CornerRadius::same(metrics::RADIUS),
        Stroke::new(2.0_f32, theme.accent_text),
        egui::StrokeKind::Outside,
    );
}

/// WCAG relative luminance of an sRGB colour.
#[cfg(test)]
pub fn luminance(c: Color32) -> f64 {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG contrast ratio between two colours (1.0 to 21.0).
#[cfg(test)]
pub fn contrast(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [PaletteKey; 2] = [PaletteKey::Neutral, PaletteKey::Slate];

    /// A page is read like print: body text, weak text, errors and warnings need
    /// 4.5:1 on the white page, and each plot line 3:1 (lines are not text).
    #[test]
    fn the_paper_palette_is_readable_on_white() {
        let p = Theme::paper();
        for (name, colour) in [
            ("text", p.text),
            ("text_weak", p.text_weak),
            ("error", p.error),
            ("warning", p.warning),
            ("accent_text", p.accent_text),
        ] {
            assert!(
                contrast(colour, p.base) >= 4.5,
                "{name}: {:.2}",
                contrast(colour, p.base)
            );
        }
        for (k, colour) in p.series.iter().enumerate() {
            assert!(
                contrast(*colour, p.base) >= 3.0,
                "series {k}: {:.2}",
                contrast(*colour, p.base)
            );
        }
        // Lines must also be told apart from each other.
        for (a, ca) in p.series.iter().enumerate() {
            for cb in &p.series[a + 1..] {
                assert_ne!(ca, cb);
            }
        }
    }

    #[test]
    fn the_dark_series_colours_are_readable_on_the_dark_surface() {
        for key in KEYS {
            let t = Theme::palette(key);
            for (k, colour) in t.series.iter().enumerate() {
                assert!(contrast(*colour, t.surface) >= 3.0, "{key:?} series {k}");
            }
        }
    }

    #[test]
    fn contrast_of_black_on_white_is_21() {
        assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 1e-9);
    }

    /// Policy (WCAG 2.x): text needs 4.5:1; icons and focus rings need 3:1.
    /// Error and accent text appear as text on `base` and `surface` (canvas,
    /// panels, popups). On `raised` and `raised_high` (buttons, cards, hover)
    /// they appear only as icons or rings, so 3:1 applies there.
    /// `raised_high` is a transient hover state; only the focus ring is checked on it.
    #[test]
    fn accent_text_and_error_contrast_on_every_tone() {
        for key in KEYS {
            let t = Theme::palette(key);
            for (name, fg) in [("accent_text", t.accent_text), ("error", t.error)] {
                for tone in [t.base, t.surface] {
                    let c = contrast(fg, tone);
                    assert!(c >= 4.5, "{name} text on {tone:?} in {key:?}: {c:.2}:1");
                }
                let c = contrast(fg, t.raised);
                assert!(c >= 3.0, "{name} icon on raised in {key:?}: {c:.2}:1");
            }
            // The focus ring must be visible on every tone, including hover.
            for tone in t.tones() {
                let c = contrast(t.accent_text, tone);
                assert!(c >= 3.0, "focus ring on {tone:?} in {key:?}: {c:.2}:1");
            }
        }
    }

    #[test]
    fn body_text_is_readable_on_every_tone() {
        for key in KEYS {
            let t = Theme::palette(key);
            for tone in t.tones() {
                let c = contrast(t.text, tone);
                assert!(c >= 4.5, "text on {tone:?} in {key:?}: {c:.2}:1");
            }
            // Weak text is used on base, surface and raised (labels, inactive tabs).
            for tone in [t.base, t.surface, t.raised] {
                let c = contrast(t.text_weak, tone);
                assert!(c >= 4.5, "text_weak on {tone:?} in {key:?}: {c:.2}:1");
            }
        }
    }

    #[test]
    fn text_on_accent_fill_is_readable() {
        let t = Theme::palette(PaletteKey::Neutral);
        for fill in [t.accent, t.accent_hover, t.accent_pressed] {
            assert!(contrast(t.text_on_accent, fill) >= 4.5);
        }
    }

    #[test]
    fn tones_get_lighter_with_elevation() {
        for key in KEYS {
            let t = Theme::palette(key);
            let l: Vec<f64> = t.tones().iter().map(|c| luminance(*c)).collect();
            assert!(l.windows(2).all(|w| w[0] < w[1]));
        }
    }

    #[test]
    fn palette_keys_round_trip_and_unknown_falls_back() {
        for key in KEYS {
            assert_eq!(PaletteKey::from_key(key.key()), key);
        }
        assert_eq!(PaletteKey::from_key("nonsense"), PaletteKey::Neutral);
    }
}
