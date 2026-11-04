//! Contrast with the background for color-mapped marks whose colors can come close to it.
//!
//! A colormap may pass close to the theme background (a centered diverging colormap's midpoint
//! does in every theme), and a mark drawn in such a color barely shows. In Oklab, let `d` be the
//! Euclidean distance from the mark's color, taken opaque, to the background, and
//! `w = clamp(1 − d / 0.25, 0, 1)` its weight ([`CONTRAST_DISTANCE`]). A mark with `w > 0` moves
//! its color's Oklab lightness away from the background, down when the background's lightness
//! is at least 0.5 and up otherwise, a and b unchanged. Color-mapped scatter points and bubbles
//! take the color moved by the full 0.25 ([`CONTRAST_LIGHTNESS`]), mixed with the background by
//! `w`, for their outline ([`Contrast::full`]); color-mapped contour lines are drawn in their
//! color moved by `w × 0.25` ([`Contrast::weighted`]).
//!
//! Text is judged by the WCAG 2 contrast ratio instead ([`relative_luminance`]): the values
//! written on cells and bars take black or white by it, and the emphasis color of each preset
//! theme reaches 4.5:1 against its background.

use crate::cmap::{Oklab, to_oklab};
use crate::{Rgba, Theme};

/// The WCAG 2 relative luminance of `color`'s sRGB channels, from 0 (black) to 1 (white).
pub(super) fn relative_luminance(color: Rgba) -> f64 {
    let linear = |channel: u8| {
        let c = f64::from(channel) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

/// The WCAG 2 contrast ratio of the opaque colors `a` and `b`, from 1 (equal luminance) to 21
/// (black on white): the lighter luminance plus 0.05 over the darker plus 0.05.
#[cfg(test)]
pub(super) fn contrast_ratio(a: Rgba, b: Rgba) -> f64 {
    let (a, b) = (relative_luminance(a), relative_luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The Oklab distance from the background at which a color-mapped mark needs no contrast.
const CONTRAST_DISTANCE: f64 = 0.25;
/// The largest Oklab lightness shift of a contrasting color away from the background.
const CONTRAST_LIGHTNESS: f64 = 0.25;

/// The theme background, with its Oklab conversion shared by every color compared with it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Background {
    /// The background color.
    color: Rgba,
    /// Its Oklab coordinates.
    lab: Oklab,
}

impl Background {
    /// The background of `theme`.
    pub(super) fn of(theme: &Theme) -> Self {
        let color = theme.background();
        Self {
            color,
            lab: to_oklab(color),
        }
    }

    /// The background color.
    pub(super) fn color(self) -> Rgba {
        self.color
    }

    /// The background's Oklab coordinates.
    pub(super) fn lab(self) -> Oklab {
        self.lab
    }

    /// The contrast `color`, taken opaque, needs against the background: `None` when it lies at
    /// least [`CONTRAST_DISTANCE`] from it in Oklab (`w = 0`).
    pub(super) fn contrast(self, color: Rgba) -> Option<Contrast> {
        let lab = to_oklab(color);
        let distance = lab
            .iter()
            .zip(self.lab)
            .map(|(color, background)| (color - background).powi(2))
            .sum::<f64>()
            .sqrt();
        let weight = (1.0 - distance / CONTRAST_DISTANCE).clamp(0.0, 1.0);
        if weight == 0.0 {
            return None;
        }
        let shift = if self.lab[0] >= 0.5 {
            -CONTRAST_LIGHTNESS
        } else {
            CONTRAST_LIGHTNESS
        };
        Some(Contrast {
            weight,
            color: lab,
            shift,
        })
    }
}

/// How a color close to the background moves away from it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Contrast {
    /// `w = clamp(1 − d / 0.25, 0, 1)`, in `(0, 1]`: how close the color is to the background.
    pub(super) weight: f64,
    /// The color in Oklab.
    color: Oklab,
    /// The lightness shift away from the background: [`CONTRAST_LIGHTNESS`], negative on a
    /// light background.
    shift: f64,
}

impl Contrast {
    /// The color with its lightness moved by the full shift, before any clamp to the sRGB gamut.
    pub(super) fn full(self) -> Oklab {
        let [lightness, a, b] = self.color;
        [lightness + self.shift, a, b]
    }

    /// The color with its lightness moved by `w` times the shift and kept within `0..=1`,
    /// before any clamp to the sRGB gamut.
    pub(super) fn weighted(self) -> Oklab {
        let [lightness, a, b] = self.color;
        [(lightness + self.weight * self.shift).clamp(0.0, 1.0), a, b]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Oklab distance between two colors, taken opaque.
    fn distance(a: Rgba, b: Rgba) -> f64 {
        to_oklab(a)
            .iter()
            .zip(to_oklab(b))
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    #[test]
    fn contrast_ratio_spans_one_to_twenty_one() {
        assert!((contrast_ratio(Rgba::BLACK, Rgba::BLACK) - 1.0).abs() < 1e-12);
        let white = Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        let black = Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };
        assert!((contrast_ratio(black, white) - 21.0).abs() < 1e-9);
        assert_eq!(contrast_ratio(black, white), contrast_ratio(white, black));
    }

    #[test]
    fn emphasis_colors_read_as_text_and_stand_apart() {
        // Contrast ratios and Oklab distances of the presets, rounded down: the emphasis color
        // colors text, so it needs 4.5:1 against the background, and it must not be mistaken for
        // a cell of the sequential colormap or for one of the first palette colors.
        for (theme, least_ratio) in [
            (Theme::pigment(), 5.4),
            (Theme::graphite(), 5.3),
            (Theme::midnight(), 6.8),
        ] {
            let emphasis = theme.emphasis_color();
            let ratio = contrast_ratio(emphasis, theme.background());
            assert!(
                ratio >= 4.5 && ratio >= least_ratio,
                "{emphasis:?}: {ratio}"
            );
            let cmap = theme.cmap();
            let nearest_cell = (0..=256)
                .map(|i| distance(emphasis, cmap.at(f64::from(i) / 256.0)))
                .fold(f64::INFINITY, f64::min);
            assert!(nearest_cell > 0.2, "{emphasis:?}: {nearest_cell}");
            let nearest_series = theme.palette()[..4]
                .iter()
                .map(|&color| distance(emphasis, color))
                .fold(f64::INFINITY, f64::min);
            assert!(nearest_series > 0.07, "{emphasis:?}: {nearest_series}");
        }
    }
}
