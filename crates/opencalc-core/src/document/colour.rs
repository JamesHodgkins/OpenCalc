//! Colours for drawn things (shapes): a palette colour with a shade, an exact
//! RGBA value, or "automatic".
//!
//! A palette colour is saved as its place in the palette, not as RGB, so the
//! palette can be tuned later and every worksheet follows. An exact colour is
//! saved as the four numbers the user chose. Core holds no UI types: it works the
//! RGBA out ([`Colour::resolve`]) and the window program paints it.

use serde::{Deserialize, Serialize};

/// Number of palette colours (the columns of the colour picker).
pub const THEME_SLOTS: u8 = 10;
/// Number of shades of each palette colour: 0 is the colour itself, 1 to 3 are
/// lighter and 4 to 5 are darker (the rows of the colour picker).
pub const THEME_SHADES: u8 = 6;

/// The palette: white, black, light grey, slate, blue, orange, grey, amber, sky
/// blue, green.
pub const THEME: [[u8; 3]; THEME_SLOTS as usize] = [
    [0xff, 0xff, 0xff],
    [0x00, 0x00, 0x00],
    [0xe4, 0xe7, 0xeb],
    [0x3d, 0x4f, 0x6b],
    [0x2f, 0x6b, 0xd6],
    [0xe8, 0x7f, 0x34],
    [0x9a, 0x9d, 0xa3],
    [0xf2, 0xb2, 0x0a],
    [0x4f, 0x9b, 0xdc],
    [0x55, 0xa8, 0x44],
];

/// The fixed row of everyday colours under the palette.
pub const STANDARD: [[u8; 3]; 10] = [
    [0xa6, 0x00, 0x00],
    [0xf0, 0x1c, 0x1c],
    [0xff, 0xb3, 0x00],
    [0xff, 0xef, 0x00],
    [0x96, 0xd6, 0x4a],
    [0x00, 0xa9, 0x58],
    [0x00, 0xb0, 0xea],
    [0x00, 0x6f, 0xbf],
    [0x0b, 0x24, 0x6b],
    [0x78, 0x2f, 0xa8],
];

/// How far each shade moves a palette colour toward white (positive) or black
/// (negative), in percent. Most colours go lighter three times then darker
/// twice. White and light grey only go darker and black only goes lighter, so a
/// column never repeats the same colour.
const SHADE_MIX: [[i16; THEME_SHADES as usize]; THEME_SLOTS as usize] = [
    [0, -5, -15, -25, -35, -50],
    [0, 50, 35, 25, 15, 5],
    [0, -10, -25, -50, -75, -90],
    [0, 80, 60, 40, -25, -50],
    [0, 80, 60, 40, -25, -50],
    [0, 80, 60, 40, -25, -50],
    [0, 80, 60, 40, -25, -50],
    [0, 80, 60, 40, -25, -50],
    [0, 80, 60, 40, -25, -50],
    [0, 80, 60, 40, -25, -50],
];

/// How far a shade of a palette colour moves toward white (positive) or black
/// (negative), in percent; 0 for the colour itself or a place that is not there.
pub fn shade_mix(slot: u8, shade: u8) -> i16 {
    SHADE_MIX
        .get(slot as usize)
        .and_then(|row| row.get(shade as usize))
        .copied()
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Colour {
    /// The program's choice for this use (the text colour for a line, a soft
    /// tint for a fill).
    #[default]
    Auto,
    /// A palette colour and one of its shades.
    Theme { slot: u8, shade: u8 },
    /// Exactly this colour: red, green, blue, alpha (255 is opaque).
    Rgba([u8; 4]),
}

impl Colour {
    pub const fn theme(slot: u8, shade: u8) -> Colour {
        Colour::Theme { slot, shade }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Colour {
        Colour::Rgba([r, g, b, 255])
    }

    /// Whether this names a real palette place (an exact colour always does).
    pub fn is_valid(self) -> bool {
        match self {
            Colour::Theme { slot, shade } => slot < THEME_SLOTS && shade < THEME_SHADES,
            _ => true,
        }
    }

    /// The same colour, or automatic if it named a palette place that is not there.
    pub fn repaired(self) -> Colour {
        if self.is_valid() {
            self
        } else {
            Colour::Auto
        }
    }

    /// The RGBA to paint. `auto` is what automatic means where this is used.
    pub fn resolve(self, auto: [u8; 4]) -> [u8; 4] {
        match self.repaired() {
            Colour::Auto => auto,
            Colour::Rgba(c) => c,
            Colour::Theme { slot, shade } => {
                let base = THEME[slot as usize];
                let mix = shade_mix(slot, shade);
                let (target, k) = if mix >= 0 { (255, mix) } else { (0, -mix) };
                let blend = |c: u8| {
                    let c = i32::from(c);
                    (c + (target - c) * i32::from(k) / 100) as u8
                };
                [blend(base[0]), blend(base[1]), blend(base[2]), 255]
            }
        }
    }

    /// The same colour with its alpha changed (an exact colour only; a palette
    /// colour becomes the exact colour it resolves to).
    pub fn with_alpha(self, alpha: u8) -> Colour {
        let [r, g, b, _] = self.resolve([0, 0, 0, 255]);
        Colour::Rgba([r, g, b, alpha])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTO: [u8; 4] = [1, 2, 3, 4];

    #[test]
    fn automatic_takes_what_the_caller_says() {
        assert_eq!(Colour::Auto.resolve(AUTO), AUTO);
        assert_eq!(Colour::default(), Colour::Auto);
    }

    #[test]
    fn a_palette_colour_at_shade_zero_is_the_palette_entry() {
        let [r, g, b] = THEME[4];
        assert_eq!(Colour::theme(4, 0).resolve(AUTO), [r, g, b, 255]);
    }

    #[test]
    fn lighter_shades_move_toward_white_and_darker_toward_black() {
        let base = Colour::theme(4, 0).resolve(AUTO);
        let light = Colour::theme(4, 1).resolve(AUTO);
        let dark = Colour::theme(4, 5).resolve(AUTO);
        for i in 0..3 {
            assert!(light[i] >= base[i] && dark[i] <= base[i]);
        }
        assert_eq!(Colour::theme(0, 5).resolve(AUTO), [128, 128, 128, 255]);
    }

    #[test]
    fn no_column_repeats_a_colour() {
        for slot in 0..THEME_SLOTS {
            let mut seen = Vec::new();
            for shade in 0..THEME_SHADES {
                let c = Colour::theme(slot, shade).resolve(AUTO);
                assert!(!seen.contains(&c), "slot {slot} shade {shade} repeats");
                seen.push(c);
            }
        }
    }

    #[test]
    fn an_exact_colour_keeps_its_alpha() {
        assert_eq!(Colour::Rgba([9, 8, 7, 6]).resolve(AUTO), [9, 8, 7, 6]);
        assert_eq!(
            Colour::theme(1, 0).with_alpha(128),
            Colour::Rgba([0, 0, 0, 128])
        );
    }

    #[test]
    fn a_palette_place_that_is_not_there_is_automatic() {
        for bad in [
            Colour::theme(THEME_SLOTS, 0),
            Colour::theme(0, THEME_SHADES),
        ] {
            assert!(!bad.is_valid());
            assert_eq!(bad.repaired(), Colour::Auto);
            assert_eq!(bad.resolve(AUTO), AUTO);
        }
    }

    #[test]
    fn it_round_trips_through_json() {
        for c in [
            Colour::Auto,
            Colour::theme(3, 2),
            Colour::Rgba([1, 2, 3, 4]),
        ] {
            let text = serde_json::to_string(&c).unwrap();
            assert_eq!(serde_json::from_str::<Colour>(&text).unwrap(), c);
        }
    }
}
