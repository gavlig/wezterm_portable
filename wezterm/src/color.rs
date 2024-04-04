// forked from wezterm/term/src/color.rs git commit f4abf8fde
// MIT License

#[cfg(feature = "use_serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::result::Result;
pub use termwiz::color::{AnsiColor, ColorAttribute, RgbColor, SrgbaTuple};

#[derive(Clone, PartialEq)]
pub struct Palette256(pub [SrgbaTuple; 256]);

#[cfg(feature = "use_serde")]
impl Serialize for Palette256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.to_vec().serialize(serializer)
    }
}

#[cfg(feature = "use_serde")]
impl<'de> Deserialize<'de> for Palette256 {
    fn deserialize<D>(deserializer: D) -> Result<Palette256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Vec::<SrgbaTuple>::deserialize(deserializer)?;
        use std::convert::TryInto;
        Ok(Self(s.try_into().map_err(|_| {
            serde::de::Error::custom("Palette256 size mismatch")
        })?))
    }
}

impl std::iter::FromIterator<SrgbaTuple> for Palette256 {
    fn from_iter<I: IntoIterator<Item = SrgbaTuple>>(iter: I) -> Self {
        let mut colors = [SrgbaTuple::default(); 256];
        for (s, d) in iter.into_iter().zip(colors.iter_mut()) {
            *d = s;
        }
        Self(colors)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
pub struct ColorPalette {
    pub colors: Palette256,
    pub foreground: SrgbaTuple,
    pub background: SrgbaTuple,
    pub cursor_fg: SrgbaTuple,
    pub cursor_bg: SrgbaTuple,
    pub cursor_border: SrgbaTuple,
    pub selection_fg: SrgbaTuple,
    pub selection_bg: SrgbaTuple,
    pub scrollbar_thumb: SrgbaTuple,
    pub split: SrgbaTuple,
}

impl fmt::Debug for Palette256 {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        // If we wanted to dump all of the entries, we'd use this:
        // self.0[..].fmt(fmt)
        // However, we typically don't care about those and we're interested
        // in the Debug-ability of ColorPalette that embeds us.
        write!(fmt, "[suppressed]")
    }
}

impl ColorPalette {
    pub fn resolve_fg(&self, color: ColorAttribute, bold: bool) -> SrgbaTuple {
        match color {
            ColorAttribute::Default => self.foreground,
            ColorAttribute::PaletteIndex(idx) => {
                let shifted = if bold && idx < 8 { idx + 8} else { idx } as usize;
                self.colors.0[shifted]
            },
            ColorAttribute::TrueColorWithPaletteFallback(color, _)
            | ColorAttribute::TrueColorWithDefaultFallback(color) => color.into(),
        }
    }
    pub fn resolve_bg(&self, color: ColorAttribute) -> SrgbaTuple {
        match color {
            ColorAttribute::Default => self.background,
            ColorAttribute::PaletteIndex(idx) => self.colors.0[idx as usize],
            ColorAttribute::TrueColorWithPaletteFallback(color, _)
            | ColorAttribute::TrueColorWithDefaultFallback(color) => color.into(),
        }
    }
}

lazy_static::lazy_static! {
    static ref DEFAULT_PALETTE: ColorPalette = ColorPalette::compute_adventure_time();
}

impl Default for ColorPalette {
    /// Construct a default color palette
    fn default() -> ColorPalette {
        DEFAULT_PALETTE.clone()
    }
}

impl ColorPalette {
    pub fn default_ref() -> &'static ColorPalette {
        &DEFAULT_PALETTE
    }
}

impl ColorPalette {
    fn compute_default() -> Self {
        let mut colors = [SrgbaTuple::default(); 256];

        // The XTerm ansi color set
        let ansi: [SrgbaTuple; 16] = [
            // Black
            RgbColor::new_8bpc(0x00, 0x00, 0x00).into(),
            // Maroon
            RgbColor::new_8bpc(0xcc, 0x55, 0x55).into(),
            // Green
            RgbColor::new_8bpc(0x55, 0xcc, 0x55).into(),
            // Olive
            RgbColor::new_8bpc(0xcd, 0xcd, 0x55).into(),
            // Navy
            RgbColor::new_8bpc(0x54, 0x55, 0xcb).into(),
            // Purple
            RgbColor::new_8bpc(0xcc, 0x55, 0xcc).into(),
            // Teal
            RgbColor::new_8bpc(0x7a, 0xca, 0xca).into(),
            // Silver
            RgbColor::new_8bpc(0xcc, 0xcc, 0xcc).into(),
            // Grey
            RgbColor::new_8bpc(0x55, 0x55, 0x55).into(),
            // Red
            RgbColor::new_8bpc(0xff, 0x55, 0x55).into(),
            // Lime
            RgbColor::new_8bpc(0x55, 0xff, 0x55).into(),
            // Yellow
            RgbColor::new_8bpc(0xff, 0xff, 0x55).into(),
            // Blue
            RgbColor::new_8bpc(0x55, 0x55, 0xff).into(),
            // Fuchsia
            RgbColor::new_8bpc(0xff, 0x55, 0xff).into(),
            // Aqua
            RgbColor::new_8bpc(0x55, 0xff, 0xff).into(),
            // White
            RgbColor::new_8bpc(0xff, 0xff, 0xff).into(),
        ];

        colors[0..16].copy_from_slice(&ansi);

        // 216 color cube.
        // This isn't the perfect color cube, but it matches the values used
        // by xterm, which are slightly brighter.
        static RAMP6: [u8; 6] = [0, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
        for idx in 0..216 {
            let blue = RAMP6[idx % 6];
            let green = RAMP6[idx / 6 % 6];
            let red = RAMP6[idx / 6 / 6 % 6];

            colors[16 + idx] = RgbColor::new_8bpc(red, green, blue).into();
        }

        // 24 grey scales
        static GREYS: [u8; 24] = [
            0x08, 0x12, 0x1c, 0x26, 0x30, 0x3a, 0x44, 0x4e, 0x58, 0x62, 0x6c, 0x76, 0x80, 0x8a,
            0x94, 0x9e, 0xa8, 0xb2, /* Grey70 */
            0xbc, 0xc6, 0xd0, 0xda, 0xe4, 0xee,
        ];

        for idx in 0..24 {
            let grey = GREYS[idx];
            colors[232 + idx] = RgbColor::new_8bpc(grey, grey, grey).into();
        }

        let foreground = colors[249]; // Grey70
        let background = colors[AnsiColor::Black as usize];

        let cursor_bg = RgbColor::new_8bpc(0x52, 0xad, 0x70).into();
        let cursor_border = RgbColor::new_8bpc(0x52, 0xad, 0x70).into();
        let cursor_fg = colors[AnsiColor::Black as usize].into();

        let selection_fg = SrgbaTuple(0., 0., 0., 0.);
        let selection_bg = SrgbaTuple(0.5, 0.4, 0.6, 0.5);

        let scrollbar_thumb = RgbColor::new_8bpc(0x22, 0x22, 0x22).into();
        let split = RgbColor::new_8bpc(0x44, 0x44, 0x44).into();

        ColorPalette {
            colors: Palette256(colors),
            foreground,
            background,
            cursor_fg,
            cursor_bg,
            cursor_border,
            selection_fg,
            selection_bg,
            scrollbar_thumb,
            split,
        }
    }

    fn compute_adventure_time() -> Self {
        let mut colors = [SrgbaTuple::default(); 256];

        // The XTerm ansi color set
        let ansi: [SrgbaTuple; 16] = [
            RgbColor::new_8bpc(0x05, 0x04, 0x04).into(), // Black
            RgbColor::new_8bpc(0xbd, 0x00, 0x13).into(), // Maroon
            RgbColor::new_8bpc(0x4a, 0xb1, 0x18).into(), // Green
            RgbColor::new_8bpc(0xe7, 0x74, 0x1e).into(), // Olive

            RgbColor::new_8bpc(0x0f, 0x4a, 0xc6).into(), // Navy
            RgbColor::new_8bpc(0x66, 0x59, 0x93).into(), // Purple
            RgbColor::new_8bpc(0x70, 0xa5, 0x98).into(), // Teal
            RgbColor::new_8bpc(0xf8, 0xdc, 0xc0).into(), // Silver

            RgbColor::new_8bpc(0x4e, 0x7c, 0xbf).into(), // Grey
            RgbColor::new_8bpc(0xfc, 0x5f, 0x5a).into(), // Red
            RgbColor::new_8bpc(0x9e, 0xff, 0x6e).into(), // Lime
            RgbColor::new_8bpc(0xef, 0xc1, 0x1a).into(), // Yellow

            RgbColor::new_8bpc(0x19, 0x97, 0xc6).into(), // Blue
            RgbColor::new_8bpc(0x9b, 0x59, 0x53).into(), // Fuchsia
            RgbColor::new_8bpc(0xc8, 0xfa, 0xf4).into(), // Aqua
            RgbColor::new_8bpc(0xf6, 0xf5, 0xfb).into(), // White
        ];

        colors[0..16].copy_from_slice(&ansi);

        // 216 color cube.
        // This isn't the perfect color cube, but it matches the values used
        // by xterm, which are slightly brighter.
        static RAMP6: [u8; 6] = [0, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
        for idx in 0..216 {
            let blue = RAMP6[idx % 6];
            let green = RAMP6[idx / 6 % 6];
            let red = RAMP6[idx / 6 / 6 % 6];

            colors[16 + idx] = RgbColor::new_8bpc(red, green, blue).into();
        }

        // 24 grey scales
        static GREYS: [u8; 24] = [
            0x08, 0x12, 0x1c, 0x26, 0x30, 0x3a, 0x44, 0x4e, 0x58, 0x62, 0x6c, 0x76, 0x80, 0x8a,
            0x94, 0x9e, 0xa8, 0xb2, /* Grey70 */
            0xbc, 0xc6, 0xd0, 0xda, 0xe4, 0xee,
        ];

        for idx in 0..24 {
            let grey = GREYS[idx];
            colors[232 + idx] = RgbColor::new_8bpc(grey, grey, grey).into();
        }

        let foreground = RgbColor::new_8bpc(0xf8, 0xdc, 0xc0).into();
        let background = RgbColor::new_8bpc(0x1f, 0x1d, 0x45).into();

        let cursor_bg = RgbColor::new_8bpc(0xef, 0xbf, 0x38).into();
        let cursor_border = RgbColor::new_8bpc(0xef, 0xbf, 0x38).into();
        let cursor_fg = RgbColor::new_8bpc(0x08, 0x08, 0x0a).into();

        let selection_fg = RgbColor::new_8bpc(0x70, 0x6b, 0x4e).into();
        let selection_bg = RgbColor::new_8bpc(0xf3, 0xd9, 0xc4).into();

        let scrollbar_thumb = RgbColor::new_8bpc(0x22, 0x22, 0x22).into();
        let split = RgbColor::new_8bpc(0x44, 0x44, 0x44).into();

        ColorPalette {
            colors: Palette256(colors),
            foreground,
            background,
            cursor_fg,
            cursor_bg,
            cursor_border,
            selection_fg,
            selection_bg,
            scrollbar_thumb,
            split,
        }
    }
}
