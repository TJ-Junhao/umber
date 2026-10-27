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

use crate::cmap::{Oklab, to_oklab};
use crate::{Rgba, Theme};

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
