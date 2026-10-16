//! Colormaps: a [`Cmap`] maps a position on `0..=1` to a color.
//!
//! A colormap holds stops spaced evenly from 0 to 1 and mixes neighbouring stops in Oklab, so a
//! custom colormap is perceptually even between its stops. The built-in sequential colormaps
//! are dense 256-stop tables of their published reference values.
//!
//! # Examples
//!
//! ```
//! use umber_core::{Cmap, Rgba};
//!
//! let cmap = Cmap::from_colors(&[Rgba::hex("#000000")?, Rgba::hex("#FFFFFF")?])?;
//! assert_eq!(cmap.at(0.0).to_hex(), "#000000");
//! assert_eq!(cmap.at(0.5).to_hex(), "#636363");
//! assert_eq!(cmap.reversed().at(0.0).to_hex(), "#FFFFFF");
//! # Ok::<(), umber_core::Error>(())
//! ```

mod tables;

use std::sync::Arc;

use crate::error::number;
use crate::{Error, Result, Rgba};

/// The fewest colors `Cmap::from_colors` accepts.
const MIN_COLORS: usize = 2;
/// The most colors `Cmap::from_colors` accepts.
const MAX_COLORS: usize = 16;

/// A color in Oklab: lightness, then the green-red and blue-yellow axes.
type Oklab = [f64; 3];

/// The blue, near-white and red stops of [`Cmap::blue_red`].
const BLUE_RED: [u32; 3] = [0x2B5C8A, 0xF4F1EC, 0xB23A2E];

/// An immutable colormap: stops evenly spaced on `0..=1`, interpolated in Oklab.
///
/// Equal colormaps have equal stops in the same order.
///
/// # Examples
///
/// ```
/// use umber_core::Cmap;
///
/// assert_eq!(Cmap::viridis().at(0.0).to_hex(), "#440154");
/// assert_eq!(Cmap::viridis().reversed().reversed(), Cmap::viridis());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Cmap {
    /// At least two stops.
    stops: Arc<[Oklab]>,
}

impl Cmap {
    /// The perceptually uniform sequential colormap from dark purple through teal to yellow.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Cmap;
    /// assert_eq!(Cmap::viridis().at(1.0).to_hex(), "#FDE725");
    /// ```
    #[must_use]
    pub fn viridis() -> Cmap {
        Cmap::from_hexes(&tables::VIRIDIS)
    }

    /// The sequential colormap from black through purple and orange to pale yellow.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Cmap;
    /// assert_eq!(Cmap::magma().at(0.0).to_hex(), "#000004");
    /// ```
    #[must_use]
    pub fn magma() -> Cmap {
        Cmap::from_hexes(&tables::MAGMA)
    }

    /// The sequential colormap from dark blue to yellow that stays distinguishable under the
    /// common forms of color blindness.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Cmap;
    /// assert_eq!(Cmap::cividis().at(0.0).to_hex(), "#00224E");
    /// ```
    #[must_use]
    pub fn cividis() -> Cmap {
        Cmap::from_hexes(&tables::CIVIDIS)
    }

    /// The diverging colormap from blue through near-white to red.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Cmap;
    /// assert_eq!(Cmap::blue_red().at(0.5).to_hex(), "#F4F1EC");
    /// ```
    #[must_use]
    pub fn blue_red() -> Cmap {
        Cmap::from_hexes(&BLUE_RED)
    }

    /// A colormap through `colors`, spaced evenly from 0 to 1.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Cmap.from_colors()"` and parameter `"colors"` unless
    /// there are 2 to 16 colors, or if a color is not opaque.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Cmap, Rgba};
    ///
    /// let cmap = Cmap::from_colors(&[Rgba::RED, Rgba::BLUE])?;
    /// assert_eq!(cmap.at(1.0), Rgba::BLUE);
    /// assert!(Cmap::from_colors(&[Rgba::RED]).is_err());
    /// assert!(Cmap::from_colors(&[Rgba::RED, Rgba::hex("#3366B880")?]).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn from_colors(colors: &[Rgba]) -> Result<Cmap> {
        if !(MIN_COLORS..=MAX_COLORS).contains(&colors.len()) {
            let count = colors.len();
            let noun = if count == 1 { "color" } else { "colors" };
            return Err(Error::style(
                "Cmap.from_colors()",
                format!(
                    "colors has {count} {noun}; a colormap needs {MIN_COLORS} to {MAX_COLORS} \
                     colors."
                ),
            )
            .with_parameter("colors")
            .with_hint("pass between 2 and 16 colors, e.g. [Color.BLUE, Color.RED]."));
        }
        if let Some((index, color)) = colors.iter().enumerate().find(|(_, c)| c.a != 255) {
            let alpha = (f64::from(color.a) / 255.0 * 1000.0).round() / 1000.0;
            return Err(Error::style(
                "Cmap.from_colors()",
                format!(
                    "colors[{index}] has alpha {}; colormap colors must be opaque.",
                    number(alpha)
                ),
            )
            .with_parameter("colors")
            .with_hint("use opaque colors; set transparency on the mark instead."));
        }
        Ok(Cmap {
            stops: colors.iter().copied().map(to_oklab).collect(),
        })
    }

    /// A colormap through the sRGB colors `hexes` (`0xRRGGBB`), which must number at least two.
    /// It is for the colormaps this crate defines; user colors go through
    /// [`from_colors`](Cmap::from_colors).
    pub(crate) fn from_hexes(hexes: &[u32]) -> Cmap {
        Cmap {
            stops: hexes
                .iter()
                .map(|&hex| {
                    let [_, r, g, b] = hex.to_be_bytes();
                    to_oklab(Rgba::opaque(r, g, b))
                })
                .collect(),
        }
    }

    /// This colormap read backwards.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Cmap;
    /// assert_eq!(Cmap::viridis().reversed().at(0.0), Cmap::viridis().at(1.0));
    /// ```
    #[must_use]
    pub fn reversed(&self) -> Cmap {
        Cmap {
            stops: self.stops.iter().rev().copied().collect(),
        }
    }

    /// The color at position `t` on `0..=1`. A position outside that range is clamped to the
    /// nearest end and a NaN is read as 0. The color is opaque.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Cmap, Rgba};
    ///
    /// let cmap = Cmap::from_colors(&[Rgba::RED, Rgba::BLUE])?;
    /// assert_eq!(cmap.at(-1.0), Rgba::RED);
    /// assert_eq!(cmap.at(2.0), Rgba::BLUE);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn at(&self, t: f64) -> Rgba {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        let last = self.stops.len() - 1;
        let position = t * last as f64;
        // `position` is at most `last`: the segment before the final stop owns `t = 1`.
        let segment = (position.floor() as usize).min(last - 1);
        let share = position - segment as f64;
        let (from, to) = (self.stops[segment], self.stops[segment + 1]);
        from_oklab(std::array::from_fn(|axis| {
            from[axis] + (to[axis] - from[axis]) * share
        }))
    }

    /// The stops as sRGB colors, in order.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Cmap, Rgba};
    ///
    /// let cmap = Cmap::from_colors(&[Rgba::RED, Rgba::BLUE])?;
    /// assert_eq!(cmap.colors(), [Rgba::RED, Rgba::BLUE]);
    /// assert_eq!(Cmap::viridis().colors().len(), 256);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn colors(&self) -> Vec<Rgba> {
        self.stops.iter().copied().map(from_oklab).collect()
    }

    /// This colormap as the Python expression that makes it, for messages: `Cmap.VIRIDIS` or
    /// `Cmap.VIRIDIS.reversed()` for a built-in, otherwise `Cmap.from_colors([...])` with its
    /// stops as `Color.hex()` calls.
    pub(crate) fn name(&self) -> String {
        let builtins = [
            ("VIRIDIS", Cmap::viridis()),
            ("MAGMA", Cmap::magma()),
            ("CIVIDIS", Cmap::cividis()),
            ("BLUE_RED", Cmap::blue_red()),
        ];
        for (name, builtin) in builtins {
            if *self == builtin {
                return format!("Cmap.{name}");
            }
            if *self == builtin.reversed() {
                return format!("Cmap.{name}.reversed()");
            }
        }
        let stops: Vec<String> = self
            .colors()
            .into_iter()
            .map(|color| format!("Color.hex(\"{}\")", color.to_hex()))
            .collect();
        format!("Cmap.from_colors([{}])", stops.join(", "))
    }
}

/// The linear-light value of an 8-bit sRGB channel.
fn decode(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The 8-bit sRGB channel of a linear-light value, clamped to the gamut.
fn encode(linear: f64) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let srgb = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round() as u8
}

/// The Oklab coordinates of the sRGB color `color`; its alpha is ignored.
fn to_oklab(color: Rgba) -> Oklab {
    let (r, g, b) = (decode(color.r), decode(color.g), decode(color.b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

/// The opaque sRGB color at the Oklab coordinates `lab`, clamped to the gamut.
fn from_oklab([lightness, a, b]: Oklab) -> Rgba {
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    Rgba::opaque(
        encode(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
        encode(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
        encode(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s),
    )
}
