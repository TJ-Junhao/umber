//! Colors: concrete sRGB values and color specifications that resolve against a theme palette.
//!
//! # Examples
//!
//! ```
//! use umber_core::{Alpha, ColorSpec, Rgba};
//!
//! let palette = [Rgba::RED, Rgba::BLUE];
//! let faded = ColorSpec::AUTO.with_alpha(Alpha::new(0.5)?);
//! let color = faded.resolve(&palette, 1);
//! assert_eq!(color.to_hex(), "#3366B880");
//! # Ok::<(), umber_core::Error>(())
//! ```

use crate::{Error, Result};

/// A concrete sRGB color with 8-bit channels and straight (non-premultiplied) alpha.
///
/// # Examples
///
/// ```
/// use umber_core::Rgba;
/// let terracotta = Rgba::hex("#C4553A")?;
/// assert_eq!((terracotta.r, terracotta.a), (0xC4, 255));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel; 255 is opaque.
    pub a: u8,
}

const HEX_HINT: &str = "use #RRGGBB or #RRGGBBAA.";

impl Rgba {
    /// `#1A1A1A`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::BLACK.to_hex(), "#1A1A1A");
    /// ```
    pub const BLACK: Rgba = Rgba::opaque(0x1A, 0x1A, 0x1A);

    /// `#FFFFFF`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::WHITE.to_hex(), "#FFFFFF");
    /// ```
    pub const WHITE: Rgba = Rgba::opaque(0xFF, 0xFF, 0xFF);

    /// `#8C8C8C`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::GRAY.to_hex(), "#8C8C8C");
    /// ```
    pub const GRAY: Rgba = Rgba::opaque(0x8C, 0x8C, 0x8C);

    /// `#D64933`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::RED.to_hex(), "#D64933");
    /// ```
    pub const RED: Rgba = Rgba::opaque(0xD6, 0x49, 0x33);

    /// `#E8853D`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::ORANGE.to_hex(), "#E8853D");
    /// ```
    pub const ORANGE: Rgba = Rgba::opaque(0xE8, 0x85, 0x3D);

    /// `#E9B949`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::YELLOW.to_hex(), "#E9B949");
    /// ```
    pub const YELLOW: Rgba = Rgba::opaque(0xE9, 0xB9, 0x49);

    /// `#4E9A5E`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::GREEN.to_hex(), "#4E9A5E");
    /// ```
    pub const GREEN: Rgba = Rgba::opaque(0x4E, 0x9A, 0x5E);

    /// `#2E8B8B`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::TEAL.to_hex(), "#2E8B8B");
    /// ```
    pub const TEAL: Rgba = Rgba::opaque(0x2E, 0x8B, 0x8B);

    /// `#3366B8`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::BLUE.to_hex(), "#3366B8");
    /// ```
    pub const BLUE: Rgba = Rgba::opaque(0x33, 0x66, 0xB8);

    /// `#7D5BA6`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::PURPLE.to_hex(), "#7D5BA6");
    /// ```
    pub const PURPLE: Rgba = Rgba::opaque(0x7D, 0x5B, 0xA6);

    /// `#D46A92`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::PINK.to_hex(), "#D46A92");
    /// ```
    pub const PINK: Rgba = Rgba::opaque(0xD4, 0x6A, 0x92);

    /// `#7A5236`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::BROWN.to_hex(), "#7A5236");
    /// ```
    pub const BROWN: Rgba = Rgba::opaque(0x7A, 0x52, 0x36);
    /// Build an opaque color from 8-bit channels.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::opaque(1, 2, 3).a, 255);
    /// ```
    pub const fn opaque(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r, g, b, a: 255 }
    }

    /// Parse `#RRGGBB` or `#RRGGBBAA` (hex digits in either case).
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Color.hex()"` for any other input.
    ///
    /// # Examples
    ///
    /// ```
    /// assert!(umber_core::Rgba::hex("#c4553a").is_ok());
    /// assert!(umber_core::Rgba::hex("#c45").is_err());
    /// ```
    pub fn hex(code: &str) -> Result<Rgba> {
        let bad = || Error::Style {
            context: "Color.hex()",
            message: format!("{code:?} is not a hex color; {HEX_HINT}"),
        };
        let digits = code.strip_prefix('#').ok_or_else(bad)?;
        if !(digits.len() == 6 || digits.len() == 8)
            || !digits.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(bad());
        }
        let byte = |i: usize| -> Result<u8> {
            let pair = digits.get(i..i + 2).ok_or_else(bad)?;
            u8::from_str_radix(pair, 16).map_err(|_| bad())
        };
        let a = if digits.len() == 8 { byte(6)? } else { 255 };
        Ok(Rgba {
            r: byte(0)?,
            g: byte(2)?,
            b: byte(4)?,
            a,
        })
    }

    /// Build an opaque color from integer channels in 0..=255.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Color.rgb()"` if any channel is out of range.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::from_ints(255, 0, 0)?.r, 255);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn from_ints(r: i64, g: i64, b: i64) -> Result<Rgba> {
        let channel = |name: &str, v: i64| -> Result<u8> {
            u8::try_from(v).map_err(|_| Error::Style {
                context: "Color.rgb()",
                message: format!("{name} is {v}; each channel must be between 0 and 255."),
            })
        };
        Ok(Rgba::opaque(
            channel("r", r)?,
            channel("g", g)?,
            channel("b", b)?,
        ))
    }

    /// Return this color with its alpha replaced (not multiplied) by `alpha`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Alpha, Rgba};
    /// assert_eq!(Rgba::RED.with_alpha(Alpha::new(0.0)?).a, 0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_alpha(self, alpha: Alpha) -> Rgba {
        Rgba {
            a: alpha.to_u8(),
            ..self
        }
    }

    /// Format as `#RRGGBB` when opaque, otherwise `#RRGGBBAA`, using uppercase digits.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Rgba::opaque(255, 0, 16).to_hex(), "#FF0010");
    /// ```
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }
}

/// Opacity from 0.0 (fully transparent) to 1.0 (opaque). Always finite and in range.
///
/// # Examples
///
/// ```
/// assert_eq!(umber_core::Alpha::new(0.25)?.get(), 0.25);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alpha(f32);

impl Alpha {
    /// Fully opaque.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Alpha::OPAQUE.to_u8(), 255);
    /// ```
    pub const OPAQUE: Alpha = Alpha(1.0);

    /// Validate an opacity value.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Color.alpha()"` unless `0.0 <= a <= 1.0`.
    ///
    /// # Examples
    ///
    /// ```
    /// assert!(umber_core::Alpha::new(1.5).is_err());
    /// ```
    pub fn new(a: f64) -> Result<Alpha> {
        if (0.0..=1.0).contains(&a) {
            Ok(Alpha(a as f32))
        } else {
            Err(Error::Style {
                context: "Color.alpha()",
                message: format!("alpha is {a}; alpha must be between 0.0 and 1.0."),
            })
        }
    }

    /// The opacity as a float.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Alpha::OPAQUE.get(), 1.0);
    /// ```
    pub fn get(self) -> f32 {
        self.0
    }

    /// The opacity as an 8-bit channel value, rounded to nearest.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Alpha::new(0.5)?.to_u8(), 128);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn to_u8(self) -> u8 {
        (self.0 * 255.0).round().clamp(0.0, 255.0) as u8
    }
}

/// The color a mark asks for. Palette-based variants are resolved at render time, against
/// the theme of the figure the mark ends up in.
///
/// # Examples
///
/// ```
/// use umber_core::{ColorSpec, Rgba};
/// let palette = [Rgba::RED, Rgba::BLUE];
/// assert_eq!(ColorSpec::AUTO.resolve(&palette, 1), Rgba::BLUE);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorSpec {
    /// The next palette color, in series order within the axes; carries an alpha override.
    Auto(Alpha),
    /// A fixed palette slot (wraps around the palette length).
    Palette {
        /// Zero-based palette index.
        index: usize,
        /// Opacity applied to the palette color.
        alpha: Alpha,
    },
    /// A concrete color that ignores the theme.
    Fixed(Rgba),
}

impl ColorSpec {
    /// The next palette color, opaque.
    ///
    /// # Examples
    ///
    /// ```
    /// assert!(matches!(umber_core::ColorSpec::AUTO, umber_core::ColorSpec::Auto(_)));
    /// ```
    pub const AUTO: ColorSpec = ColorSpec::Auto(Alpha::OPAQUE);

    /// Replace the opacity of this spec.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Alpha, ColorSpec, Rgba};
    /// let c = ColorSpec::Fixed(Rgba::RED).with_alpha(Alpha::new(0.0)?);
    /// assert_eq!(c.resolve(&[], 0).a, 0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_alpha(self, alpha: Alpha) -> ColorSpec {
        match self {
            ColorSpec::Auto(_) => ColorSpec::Auto(alpha),
            ColorSpec::Palette { index, .. } => ColorSpec::Palette { index, alpha },
            ColorSpec::Fixed(rgba) => ColorSpec::Fixed(rgba.with_alpha(alpha)),
        }
    }

    /// Turn this spec into a concrete color. `auto_index` is the series' position among the
    /// axes' `Auto` marks. An empty palette resolves to [`Rgba::BLACK`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ColorSpec, Rgba};
    /// assert_eq!(ColorSpec::AUTO.resolve(&[Rgba::RED], 7), Rgba::RED);
    /// ```
    pub fn resolve(self, palette: &[Rgba], auto_index: usize) -> Rgba {
        let pick = |i: usize| {
            palette
                .get(i.checked_rem(palette.len()).unwrap_or(0))
                .copied()
                .unwrap_or(Rgba::BLACK)
        };
        match self {
            ColorSpec::Auto(alpha) => pick(auto_index).with_alpha(alpha),
            ColorSpec::Palette { index, alpha } => pick(index).with_alpha(alpha),
            ColorSpec::Fixed(rgba) => rgba,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_constants_match_spec() {
        let expected = [
            (Rgba::BLACK, "#1A1A1A"),
            (Rgba::WHITE, "#FFFFFF"),
            (Rgba::GRAY, "#8C8C8C"),
            (Rgba::RED, "#D64933"),
            (Rgba::ORANGE, "#E8853D"),
            (Rgba::YELLOW, "#E9B949"),
            (Rgba::GREEN, "#4E9A5E"),
            (Rgba::TEAL, "#2E8B8B"),
            (Rgba::BLUE, "#3366B8"),
            (Rgba::PURPLE, "#7D5BA6"),
            (Rgba::PINK, "#D46A92"),
            (Rgba::BROWN, "#7A5236"),
        ];
        for (color, hex) in expected {
            assert_eq!(color.to_hex(), hex);
        }
    }

    #[test]
    fn hex_parses_both_forms_case_insensitively() {
        assert_eq!(
            Rgba::hex("#c4553a").unwrap(),
            Rgba {
                r: 0xC4,
                g: 0x55,
                b: 0x3A,
                a: 255
            }
        );
        assert_eq!(
            Rgba::hex("#C4553A80").unwrap(),
            Rgba {
                r: 0xC4,
                g: 0x55,
                b: 0x3A,
                a: 0x80
            }
        );
    }

    #[test]
    fn hex_rejects_malformed_input() {
        for bad in [
            "",
            "#",
            "C4553A",
            "#C45",
            "#C4553",
            "#C4553A8",
            "#GG553A",
            " #C4553A",
            "#C4553A ",
            "#ÄÄ553A",
        ] {
            let err = Rgba::hex(bad).unwrap_err();
            assert!(
                matches!(
                    err,
                    Error::Style {
                        context: "Color.hex()",
                        ..
                    }
                ),
                "{bad:?}"
            );
            assert!(
                err.to_string().ends_with("; use #RRGGBB or #RRGGBBAA."),
                "{err}"
            );
        }
    }

    #[test]
    fn to_hex_round_trips_and_omits_opaque_alpha() {
        for s in ["#000000", "#FFFFFF", "#12AB9F", "#12AB9F00", "#12AB9F7F"] {
            assert_eq!(Rgba::hex(s).unwrap().to_hex(), s);
        }
    }

    #[test]
    fn from_ints_validates_range() {
        assert_eq!(
            Rgba::from_ints(0, 128, 255).unwrap(),
            Rgba::opaque(0, 128, 255)
        );
        for (r, g, b) in [(-1, 0, 0), (0, 256, 0), (0, 0, 1000)] {
            let err = Rgba::from_ints(r, g, b).unwrap_err();
            assert!(matches!(
                err,
                Error::Style {
                    context: "Color.rgb()",
                    ..
                }
            ));
        }
    }

    #[test]
    fn alpha_validates_and_converts() {
        assert_eq!(Alpha::new(0.0).unwrap().to_u8(), 0);
        assert_eq!(Alpha::new(1.0).unwrap().to_u8(), 255);
        assert_eq!(Alpha::new(0.5).unwrap().to_u8(), 128);
        for bad in [-0.01, 1.01, f64::NAN, f64::INFINITY] {
            let err = Alpha::new(bad).unwrap_err();
            assert!(matches!(
                err,
                Error::Style {
                    context: "Color.alpha()",
                    ..
                }
            ));
        }
    }

    #[test]
    fn with_alpha_replaces_rather_than_multiplies() {
        let half = Alpha::new(0.5).unwrap();
        let c = Rgba::hex("#11223340").unwrap().with_alpha(half);
        assert_eq!(c.a, 128);
    }

    #[test]
    fn resolve_auto_and_palette_wrap_around() {
        let pal = [Rgba::RED, Rgba::GREEN, Rgba::BLUE];
        assert_eq!(ColorSpec::AUTO.resolve(&pal, 0), Rgba::RED);
        assert_eq!(ColorSpec::AUTO.resolve(&pal, 4), Rgba::GREEN);
        let p = ColorSpec::Palette {
            index: 5,
            alpha: Alpha::OPAQUE,
        };
        assert_eq!(p.resolve(&pal, 0), Rgba::BLUE);
    }

    #[test]
    fn resolve_applies_spec_alpha_to_palette_colors() {
        let pal = [Rgba::RED];
        let half = Alpha::new(0.5).unwrap();
        assert_eq!(ColorSpec::AUTO.with_alpha(half).resolve(&pal, 0).a, 128);
        assert_eq!(
            ColorSpec::Fixed(Rgba::BLUE)
                .with_alpha(half)
                .resolve(&pal, 0)
                .a,
            128
        );
    }

    #[test]
    fn resolve_with_empty_palette_falls_back_to_black() {
        assert_eq!(ColorSpec::AUTO.resolve(&[], 3), Rgba::BLACK);
    }

    #[test]
    fn alpha_boundaries_are_checked_on_the_f64_input() {
        for bad in [-f64::from_bits(1), f64::from_bits(1.0_f64.to_bits() + 1)] {
            let err = Alpha::new(bad).unwrap_err();
            assert!(
                matches!(
                    err,
                    Error::Style {
                        context: "Color.alpha()",
                        ..
                    }
                ),
                "{bad:e}"
            );
        }
        for ok in [-0.0, 0.0, 1.0, 0.999999] {
            assert!(Alpha::new(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn palette_with_alpha_replaces_palette_alpha_and_keeps_index() {
        let translucent_red = Rgba::hex("#FF000080").unwrap();
        let pal = [Rgba::BLUE, translucent_red, Rgba::GREEN];
        let half = Alpha::new(0.5).unwrap();
        let spec = ColorSpec::Palette {
            index: 1,
            alpha: Alpha::OPAQUE,
        }
        .with_alpha(half);
        assert_eq!(
            spec,
            ColorSpec::Palette {
                index: 1,
                alpha: half
            }
        );
        assert_eq!(
            spec.resolve(&pal, 0),
            Rgba {
                r: 0xFF,
                g: 0,
                b: 0,
                a: 128
            }
        );
        // Opaque alpha also replaces the palette color's own alpha.
        let opaque = ColorSpec::Palette {
            index: 1,
            alpha: Alpha::OPAQUE,
        };
        assert_eq!(opaque.resolve(&pal, 0), Rgba::opaque(0xFF, 0, 0));
        // Index wraps around and is not taken from auto_index.
        let wrapped = ColorSpec::Palette {
            index: 4,
            alpha: half,
        };
        assert_eq!(
            wrapped.resolve(&pal, 2),
            Rgba {
                a: 128,
                ..translucent_red
            }
        );
    }

    #[test]
    fn fixed_ignores_palette_and_auto_index() {
        let half = Alpha::new(0.5).unwrap();
        let spec = ColorSpec::Fixed(Rgba::BLUE).with_alpha(half);
        let want = Rgba {
            a: 128,
            ..Rgba::BLUE
        };
        for pal in [&[][..], &[Rgba::RED][..]] {
            for auto_index in [0, 5] {
                assert_eq!(spec.resolve(pal, auto_index), want);
            }
        }
    }
}
