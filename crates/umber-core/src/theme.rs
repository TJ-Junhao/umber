//! Themes: the visual tokens that style a figure.
//!
//! A [`Theme`] is immutable. The three built-in themes are [`Theme::pigment`] (the default),
//! [`Theme::graphite`] and [`Theme::midnight`]. Customize one with the chainable `with_*`
//! methods, each of which validates its argument and returns a new theme.
//!
//! # Examples
//!
//! ```
//! use umber_core::Theme;
//!
//! let theme = Theme::pigment().with_line_width(2.5)?.with_font_scale(1.2)?;
//! assert_eq!(theme.line_width(), 2.5);
//! assert_eq!(Theme::default(), Theme::pigment());
//! # Ok::<(), umber_core::Error>(())
//! ```

use crate::error::number;
use crate::{Error, Font, Result, Rgba, SizeSpec};

/// The largest accepted `font_scale`.
const MAX_FONT_SCALE: f64 = 10.0;

/// The most colors a palette may hold.
const MAX_PALETTE: usize = 16;

/// The widest stroke drawn, whatever the theme says.
const MAX_STROKE_WIDTH: f32 = 1.0e4;
/// The largest marker diameter drawn, whatever the theme says.
const MAX_MARKER_SIZE: f32 = 1.0e4;

/// `v` if it is finite and greater than 0, capped at `max`; 0 otherwise.
fn drawn_size(v: f32, max: f32) -> f32 {
    if v.is_finite() && v > 0.0 {
        v.min(max)
    } else {
        0.0
    }
}

/// A stroke width as drawn: finite and within `0..=MAX_STROKE_WIDTH`. Gridlines, the baseline
/// and lines are stroked at this width of their token.
pub(crate) fn drawn_stroke_width(width: f32) -> f32 {
    drawn_size(width, MAX_STROKE_WIDTH)
}

/// Gridline dash style.
///
/// # Examples
///
/// ```
/// use umber_core::Dash;
///
/// assert_ne!(Dash::Solid, Dash::Dotted);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dash {
    /// A continuous line.
    Solid,
    /// Short dashes.
    Dashed,
    /// Dots.
    Dotted,
}

/// Marker shape for scatter points.
///
/// # Examples
///
/// ```
/// use umber_core::Marker;
///
/// assert_ne!(Marker::Circle, Marker::Square);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Marker {
    /// A filled circle.
    Circle,
    /// A filled square.
    Square,
    /// A filled triangle.
    Triangle,
    /// A filled diamond.
    Diamond,
    /// A cross.
    Cross,
}

/// How legends are drawn.
///
/// A theme sets the style for every axes; [`Axes::legend`](crate::Axes::legend) overrides it
/// for one axes.
///
/// # Examples
///
/// ```
/// use umber_core::LegendStyle;
///
/// assert_ne!(LegendStyle::Header, LegendStyle::Chips);
/// assert_ne!(LegendStyle::Header, LegendStyle::None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LegendStyle {
    /// Entries in a row above the plot.
    Header,
    /// Labels placed directly at the end of each series.
    Direct,
    /// Entries as chips.
    Chips,
    /// No legend: no entry row and no direct labels.
    None,
}

/// How area and histogram fills are painted. It never decides whether something is filled:
/// lines are never filled, and the fill under a line is the explicit
/// [`Axes::area`](crate::Axes::area) mark.
///
/// # Examples
///
/// ```
/// use umber_core::{FillStyle, Theme};
///
/// assert_eq!(Theme::pigment().fill_style(), FillStyle::Solid);
/// assert_eq!(Theme::midnight().fill_style(), FillStyle::Gradient);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FillStyle {
    /// A flat fill: areas at 0.32 of the series color's opacity, histogram bins in the series
    /// color.
    Solid,
    /// A vertical gradient that fades out toward the baseline: areas from 0.32 of the series
    /// color's opacity at the line, histogram bins from 0.95 at the tallest bin, with a solid
    /// top edge on each bin.
    Gradient,
}

/// Which gridlines are drawn.
///
/// # Examples
///
/// ```
/// use umber_core::GridLines;
///
/// assert_ne!(GridLines::Horizontal, GridLines::None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GridLines {
    /// Lines at y ticks only.
    Horizontal,
    /// Lines at x ticks only.
    Vertical,
    /// Lines at both.
    Both,
    /// No gridlines.
    None,
}

/// A complete set of visual tokens.
///
/// Sizes are in points. Fields are private; read them through the accessors and derive
/// variants with the `with_*` methods.
///
/// # Examples
///
/// ```
/// use umber_core::Theme;
///
/// let base = Theme::midnight();
/// let thick = base.with_line_width(3.0)?;
/// assert_eq!(base.line_width(), 1.9);
/// assert_eq!(thick.line_width(), 3.0);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    background: Rgba,
    text_color: Rgba,
    muted_color: Rgba,
    grid_color: Rgba,
    grid_lines: GridLines,
    grid_dash: Dash,
    grid_width: f32,
    baseline_color: Rgba,
    baseline_width: f32,
    title_font: Font,
    body_font: Font,
    title_size: f32,
    body_size: f32,
    font_scale: f32,
    legend_style: LegendStyle,
    fill_style: FillStyle,
    line_width: f32,
    marker_size: f32,
    marker_alpha: f32,
    bar_corner_radius: f32,
    hist_gap: f32,
    palette: Vec<Rgba>,
}

/// An opaque color from `0xRRGGBB`.
const fn rgb(hex: u32) -> Rgba {
    Rgba::opaque((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn palette_of(hexes: [u32; 8]) -> Vec<Rgba> {
    hexes.into_iter().map(rgb).collect()
}

/// The range a numeric token must fall in.
#[derive(Clone, Copy)]
enum Bound {
    /// Greater than 0.
    Positive,
    /// Greater than 0 and at most [`MAX_FONT_SCALE`].
    FontScale,
    /// At least 0.
    NonNegative,
    /// Within 0.0..=1.0.
    Unit,
}

/// Validate a numeric token and narrow it to `f32`.
fn check_number(context: &'static str, name: &str, value: f64, bound: Bound) -> Result<f32> {
    let narrowed = value as f32;
    let (ok, required) = match bound {
        Bound::Positive => (narrowed > 0.0, "a finite number greater than 0"),
        Bound::FontScale => (
            narrowed > 0.0 && value <= MAX_FONT_SCALE,
            "a finite number greater than 0 and at most 10",
        ),
        Bound::NonNegative => (value >= 0.0, "a finite number of at least 0"),
        Bound::Unit => (
            (0.0..=1.0).contains(&value),
            "a finite number from 0.0 to 1.0",
        ),
    };
    if ok && value.is_finite() && narrowed.is_finite() {
        Ok(narrowed)
    } else {
        Err(Error::style(
            context,
            format!(
                "{name} is {value}; {name} must be {required}.",
                value = number(value)
            ),
        )
        .with_parameter("value"))
    }
}

impl Theme {
    /// The default theme: warm paper, serif titles, dotted horizontal grid.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Theme;
    ///
    /// assert_eq!(Theme::pigment().background().to_hex(), "#FAF6F0");
    /// ```
    #[must_use]
    pub fn pigment() -> Theme {
        Theme {
            background: rgb(0xFAF6F0),
            text_color: rgb(0x2B211B),
            muted_color: rgb(0x8C8076),
            grid_color: rgb(0xDDD3C7),
            grid_lines: GridLines::Horizontal,
            grid_dash: Dash::Dotted,
            grid_width: 0.75,
            baseline_color: rgb(0x2B211B),
            baseline_width: 1.3,
            title_font: Font::SourceSerif4,
            body_font: Font::IbmPlexSans,
            title_size: 15.0,
            body_size: 8.5,
            font_scale: 1.0,
            legend_style: LegendStyle::Header,
            fill_style: FillStyle::Solid,
            line_width: 2.0,
            marker_size: 6.0,
            marker_alpha: 0.85,
            bar_corner_radius: 0.0,
            hist_gap: 0.0,
            palette: palette_of([
                0xC4553A, 0x2E6B70, 0xE2A33B, 0x4F6FA3, 0x86A37A, 0x8B5279, 0x6E4C38, 0xA69F94,
            ]),
        }
    }

    /// A clean white theme with a navy-led palette.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Theme;
    ///
    /// assert_eq!(Theme::graphite().background().to_hex(), "#FFFFFF");
    /// ```
    #[must_use]
    pub fn graphite() -> Theme {
        Theme {
            background: rgb(0xFFFFFF),
            text_color: rgb(0x111111),
            muted_color: rgb(0x737373),
            grid_color: rgb(0xEBEBEB),
            grid_lines: GridLines::Horizontal,
            grid_dash: Dash::Solid,
            grid_width: 0.75,
            baseline_color: rgb(0x111111),
            baseline_width: 1.0,
            title_font: Font::Geist,
            body_font: Font::Geist,
            title_size: 15.0,
            body_size: 8.5,
            font_scale: 1.0,
            legend_style: LegendStyle::Direct,
            fill_style: FillStyle::Solid,
            line_width: 1.8,
            marker_size: 5.6,
            marker_alpha: 0.90,
            bar_corner_radius: 0.0,
            hist_gap: 1.2,
            palette: palette_of([
                0x233F86, 0xE4572E, 0xF2A900, 0x17A398, 0x9B5DE5, 0x5BA4E6, 0xD9658F, 0xA3A3A3,
            ]),
        }
    }

    /// A dark theme with a monospaced body font and gradient fills.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Theme;
    ///
    /// assert_eq!(Theme::midnight().background().to_hex(), "#121419");
    /// ```
    #[must_use]
    pub fn midnight() -> Theme {
        Theme {
            background: rgb(0x121419),
            text_color: rgb(0xE7E9EF),
            muted_color: rgb(0x868D9C),
            grid_color: rgb(0x272B34),
            grid_lines: GridLines::Horizontal,
            grid_dash: Dash::Dashed,
            grid_width: 0.75,
            baseline_color: rgb(0x3C4250),
            baseline_width: 1.0,
            title_font: Font::Inter,
            body_font: Font::JetBrainsMono,
            title_size: 14.5,
            body_size: 8.5,
            font_scale: 1.0,
            legend_style: LegendStyle::Chips,
            fill_style: FillStyle::Gradient,
            line_width: 1.9,
            marker_size: 6.2,
            marker_alpha: 0.90,
            bar_corner_radius: 3.5,
            hist_gap: 1.5,
            palette: palette_of([
                0x75AEF5, 0xDF7F78, 0x96E7A7, 0xE0AE57, 0xC9A3F5, 0x3ACED6, 0xF5A06F, 0x9DA5B1,
            ]),
        }
    }

    /// The series palette, in assignment order.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Theme;
    ///
    /// assert_eq!(Theme::pigment().palette().len(), 8);
    /// assert_eq!(Theme::pigment().palette()[0].to_hex(), "#C4553A");
    /// ```
    #[must_use]
    pub fn palette(&self) -> &[Rgba] {
        &self.palette
    }

    /// The page background color.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().background().to_hex(), "#FAF6F0");
    /// ```
    #[must_use]
    pub fn background(&self) -> Rgba {
        self.background
    }

    /// The primary text color.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().text_color().to_hex(), "#2B211B");
    /// ```
    #[must_use]
    pub fn text_color(&self) -> Rgba {
        self.text_color
    }

    /// The color of tick labels and other secondary text.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().muted_color().to_hex(), "#8C8076");
    /// ```
    #[must_use]
    pub fn muted_color(&self) -> Rgba {
        self.muted_color
    }

    /// The gridline color.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().grid_color().to_hex(), "#DDD3C7");
    /// ```
    #[must_use]
    pub fn grid_color(&self) -> Rgba {
        self.grid_color
    }

    /// Which gridlines are drawn.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().grid_lines(), GridLines::Horizontal);
    /// ```
    #[must_use]
    pub fn grid_lines(&self) -> GridLines {
        self.grid_lines
    }

    /// The gridline dash style.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().grid_dash(), Dash::Dotted);
    /// ```
    #[must_use]
    pub fn grid_dash(&self) -> Dash {
        self.grid_dash
    }

    /// The gridline width in points.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().grid_width(), 0.75);
    /// ```
    #[must_use]
    pub fn grid_width(&self) -> f32 {
        self.grid_width
    }

    /// The color of the axis baseline.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::midnight().baseline_color().to_hex(), "#3C4250");
    /// ```
    #[must_use]
    pub fn baseline_color(&self) -> Rgba {
        self.baseline_color
    }

    /// The width of the axis baseline in points.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().baseline_width(), 1.3);
    /// ```
    #[must_use]
    pub fn baseline_width(&self) -> f32 {
        self.baseline_width
    }

    /// The font of titles, drawn semibold.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().title_font(), Font::SourceSerif4);
    /// ```
    #[must_use]
    pub fn title_font(&self) -> Font {
        self.title_font
    }

    /// The font of tick labels, axis labels and other body text.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::midnight().body_font(), Font::JetBrainsMono);
    /// ```
    #[must_use]
    pub fn body_font(&self) -> Font {
        self.body_font
    }

    /// The title size in points, before `font_scale` is applied.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::midnight().title_size(), 14.5);
    /// ```
    #[must_use]
    pub fn title_size(&self) -> f32 {
        self.title_size
    }

    /// The body text size in points, before `font_scale` is applied.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().body_size(), 8.5);
    /// ```
    #[must_use]
    pub fn body_size(&self) -> f32 {
        self.body_size
    }

    /// The factor that multiplies `title_size` and `body_size`; 1.0 by default.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().font_scale(), 1.0);
    /// ```
    #[must_use]
    pub fn font_scale(&self) -> f32 {
        self.font_scale
    }

    /// How legends are drawn.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::graphite().legend_style(), LegendStyle::Direct);
    /// ```
    #[must_use]
    pub fn legend_style(&self) -> LegendStyle {
        self.legend_style
    }

    /// How area and histogram fills are painted.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::midnight().fill_style(), FillStyle::Gradient);
    /// ```
    #[must_use]
    pub fn fill_style(&self) -> FillStyle {
        self.fill_style
    }

    /// The line width in points.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::pigment().line_width(), 2.0);
    /// ```
    #[must_use]
    pub fn line_width(&self) -> f32 {
        self.line_width
    }

    /// The marker diameter in points.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::midnight().marker_size(), 6.2);
    /// ```
    #[must_use]
    pub fn marker_size(&self) -> f32 {
        self.marker_size
    }

    /// The stroke width, as drawn, of a line mark (or the line of an area) that asks for
    /// `width`, and of its legend swatch: `width` resolved against the line width (see
    /// [`SizeSpec`]), finite and within `0..=MAX_STROKE_WIDTH`.
    pub(crate) fn drawn_line_width(&self, width: SizeSpec) -> f32 {
        drawn_stroke_width(width.resolve(self.line_width))
    }

    /// The marker diameter, as drawn, of a scatter mark that asks for `size`, and of its legend
    /// dot: `size` resolved against the marker size (see [`SizeSpec`]), finite and within
    /// `0..=MAX_MARKER_SIZE`. The end dots of direct labels take [`SizeSpec::Auto`].
    pub(crate) fn drawn_marker_size(&self, size: SizeSpec) -> f32 {
        drawn_size(size.resolve(self.marker_size), MAX_MARKER_SIZE)
    }

    /// The marker opacity, from 0.0 to 1.0.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::graphite().marker_alpha(), 0.9);
    /// ```
    #[must_use]
    pub fn marker_alpha(&self) -> f32 {
        self.marker_alpha
    }

    /// The bar corner radius in points.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::midnight().bar_corner_radius(), 3.5);
    /// ```
    #[must_use]
    pub fn bar_corner_radius(&self) -> f32 {
        self.bar_corner_radius
    }

    /// The gap between histogram bars in points.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme};
    ///
    /// assert_eq!(Theme::graphite().hist_gap(), 1.2);
    /// ```
    #[must_use]
    pub fn hist_gap(&self) -> f32 {
        self.hist_gap
    }

    /// Replace the palette.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Theme.with_palette()"` unless it holds 1 to 16 colors.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_palette(&[Rgba::RED, Rgba::BLUE])?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_palette(&self, colors: &[Rgba]) -> Result<Theme> {
        if colors.is_empty() || colors.len() > MAX_PALETTE {
            return Err(Error::style(
                "Theme.with_palette()",
                format!(
                    "palette has {} colors; a palette needs 1 to {MAX_PALETTE} colors.",
                    colors.len()
                ),
            )
            .with_parameter("colors"));
        }
        Ok(Theme {
            palette: colors.to_vec(),
            ..self.clone()
        })
    }

    /// Replace the background color.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_background(Rgba::WHITE)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_background(&self, value: Rgba) -> Result<Theme> {
        Ok(Theme {
            background: value,
            ..self.clone()
        })
    }

    /// Replace the text color.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_text_color(Rgba::WHITE)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_text_color(&self, value: Rgba) -> Result<Theme> {
        Ok(Theme {
            text_color: value,
            ..self.clone()
        })
    }

    /// Replace the muted text color.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_muted_color(Rgba::GRAY)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_muted_color(&self, value: Rgba) -> Result<Theme> {
        Ok(Theme {
            muted_color: value,
            ..self.clone()
        })
    }

    /// Replace the gridline color.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_grid_color(Rgba::GRAY)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_grid_color(&self, value: Rgba) -> Result<Theme> {
        Ok(Theme {
            grid_color: value,
            ..self.clone()
        })
    }

    /// Replace the baseline color.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_baseline_color(Rgba::GRAY)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_baseline_color(&self, value: Rgba) -> Result<Theme> {
        Ok(Theme {
            baseline_color: value,
            ..self.clone()
        })
    }

    /// Replace which gridlines are drawn.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_grid_lines(GridLines::Both)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_grid_lines(&self, value: GridLines) -> Result<Theme> {
        Ok(Theme {
            grid_lines: value,
            ..self.clone()
        })
    }

    /// Replace the gridline dash style.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_grid_dash(Dash::Solid)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_grid_dash(&self, value: Dash) -> Result<Theme> {
        Ok(Theme {
            grid_dash: value,
            ..self.clone()
        })
    }

    /// Replace the title font.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_title_font(Font::Inter)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_title_font(&self, value: Font) -> Result<Theme> {
        Ok(Theme {
            title_font: value,
            ..self.clone()
        })
    }

    /// Replace the body font.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_body_font(Font::Inter)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_body_font(&self, value: Font) -> Result<Theme> {
        Ok(Theme {
            body_font: value,
            ..self.clone()
        })
    }

    /// Replace the font scale.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Theme.with_font_scale()"` unless the value is finite, greater than 0
    /// and at most 10.0 (the cap keeps derived font sizes finite). Values that underflow to 0 when
    /// narrowed to `f32` are rejected.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_font_scale(1.2)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_font_scale(&self, value: f64) -> Result<Theme> {
        let value = check_number(
            "Theme.with_font_scale()",
            "font_scale",
            value,
            Bound::FontScale,
        )?;
        Ok(Theme {
            font_scale: value,
            ..self.clone()
        })
    }

    /// Replace the line width.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Theme.with_line_width()"` unless the value is finite and greater than 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_line_width(2.5)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_line_width(&self, value: f64) -> Result<Theme> {
        let value = check_number(
            "Theme.with_line_width()",
            "line_width",
            value,
            Bound::Positive,
        )?;
        Ok(Theme {
            line_width: value,
            ..self.clone()
        })
    }

    /// Replace the marker diameter.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Theme.with_marker_size()"` unless the value is finite and greater than 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_marker_size(7.0)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_marker_size(&self, value: f64) -> Result<Theme> {
        let value = check_number(
            "Theme.with_marker_size()",
            "marker_size",
            value,
            Bound::Positive,
        )?;
        Ok(Theme {
            marker_size: value,
            ..self.clone()
        })
    }

    /// Replace the marker opacity.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Theme.with_marker_alpha()"` unless the value is finite and within 0.0..=1.0.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_marker_alpha(0.5)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_marker_alpha(&self, value: f64) -> Result<Theme> {
        let value = check_number(
            "Theme.with_marker_alpha()",
            "marker_alpha",
            value,
            Bound::Unit,
        )?;
        Ok(Theme {
            marker_alpha: value,
            ..self.clone()
        })
    }

    /// Replace the baseline width.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Theme.with_baseline_width()"` unless the value is finite and at least 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_baseline_width(1.5)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_baseline_width(&self, value: f64) -> Result<Theme> {
        let value = check_number(
            "Theme.with_baseline_width()",
            "baseline_width",
            value,
            Bound::NonNegative,
        )?;
        Ok(Theme {
            baseline_width: value,
            ..self.clone()
        })
    }

    /// Replace the bar corner radius.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Theme.with_bar_corner_radius()"` unless the value is finite and at least 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_bar_corner_radius(2.0)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_bar_corner_radius(&self, value: f64) -> Result<Theme> {
        let value = check_number(
            "Theme.with_bar_corner_radius()",
            "bar_corner_radius",
            value,
            Bound::NonNegative,
        )?;
        Ok(Theme {
            bar_corner_radius: value,
            ..self.clone()
        })
    }

    /// Replace the histogram bar gap.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Theme.with_hist_gap()"` unless the value is finite and at least 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_hist_gap(1.0)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_hist_gap(&self, value: f64) -> Result<Theme> {
        let value = check_number(
            "Theme.with_hist_gap()",
            "hist_gap",
            value,
            Bound::NonNegative,
        )?;
        Ok(Theme {
            hist_gap: value,
            ..self.clone()
        })
    }

    /// Replace the legend style.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_legend_style(LegendStyle::Chips)?;
    /// # let _ = theme;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_legend_style(&self, value: LegendStyle) -> Result<Theme> {
        Ok(Theme {
            legend_style: value,
            ..self.clone()
        })
    }

    /// Replace the fill style of areas and histograms.
    ///
    /// Returns a new theme; `self` is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Dash, Font, FillStyle, GridLines, LegendStyle, Theme, Rgba};
    ///
    /// let theme = Theme::pigment().with_fill_style(FillStyle::Gradient)?;
    /// assert_eq!(theme.fill_style(), FillStyle::Gradient);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_fill_style(&self, value: FillStyle) -> Result<Theme> {
        Ok(Theme {
            fill_style: value,
            ..self.clone()
        })
    }
}

impl Default for Theme {
    fn default() -> Theme {
        Theme::pigment()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hexes(theme: &Theme) -> Vec<String> {
        theme.palette().iter().map(|c| c.to_hex()).collect()
    }

    #[test]
    fn extreme_values_print_in_scientific_form() {
        let err = Theme::pigment().with_line_width(1e-50).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Theme.with_line_width(): line_width is 1e-50; line_width must be a finite number greater than 0."
        );
    }

    #[test]
    fn built_in_tokens_match_design_values() {
        let p = Theme::pigment();
        assert_eq!(p.background().to_hex(), "#FAF6F0");
        assert_eq!(p.text_color().to_hex(), "#2B211B");
        assert_eq!(p.muted_color().to_hex(), "#8C8076");
        assert_eq!(p.grid_color().to_hex(), "#DDD3C7");
        assert_eq!(
            (p.grid_lines(), p.grid_dash()),
            (GridLines::Horizontal, Dash::Dotted)
        );
        assert_eq!(p.grid_width(), 0.75);
        assert_eq!(p.baseline_color().to_hex(), "#2B211B");
        assert_eq!(p.baseline_width(), 1.3);
        assert_eq!(
            (p.title_font(), p.body_font()),
            (Font::SourceSerif4, Font::IbmPlexSans)
        );
        assert_eq!(
            (p.title_size(), p.body_size(), p.font_scale()),
            (15.0, 8.5, 1.0)
        );
        assert_eq!(
            (p.legend_style(), p.fill_style()),
            (LegendStyle::Header, FillStyle::Solid)
        );
        assert_eq!(p.line_width(), 2.0);
        assert_eq!((p.marker_size(), p.marker_alpha()), (6.0, 0.85));
        assert_eq!((p.bar_corner_radius(), p.hist_gap()), (0.0, 0.0));
        assert_eq!(
            hexes(&p),
            [
                "#C4553A", "#2E6B70", "#E2A33B", "#4F6FA3", "#86A37A", "#8B5279", "#6E4C38",
                "#A69F94"
            ]
        );

        let g = Theme::graphite();
        assert_eq!(g.background().to_hex(), "#FFFFFF");
        assert_eq!(g.text_color().to_hex(), "#111111");
        assert_eq!(g.muted_color().to_hex(), "#737373");
        assert_eq!(g.grid_color().to_hex(), "#EBEBEB");
        assert_eq!(
            (g.grid_lines(), g.grid_dash()),
            (GridLines::Horizontal, Dash::Solid)
        );
        assert_eq!(g.grid_width(), 0.75);
        assert_eq!(g.baseline_color().to_hex(), "#111111");
        assert_eq!(g.baseline_width(), 1.0);
        assert_eq!((g.title_font(), g.body_font()), (Font::Geist, Font::Geist));
        assert_eq!(
            (g.title_size(), g.body_size(), g.font_scale()),
            (15.0, 8.5, 1.0)
        );
        assert_eq!(
            (g.legend_style(), g.fill_style()),
            (LegendStyle::Direct, FillStyle::Solid)
        );
        assert_eq!(g.line_width(), 1.8);
        assert_eq!((g.marker_size(), g.marker_alpha()), (5.6, 0.90));
        assert_eq!((g.bar_corner_radius(), g.hist_gap()), (0.0, 1.2));
        assert_eq!(
            hexes(&g),
            [
                "#233F86", "#E4572E", "#F2A900", "#17A398", "#9B5DE5", "#5BA4E6", "#D9658F",
                "#A3A3A3"
            ]
        );

        let m = Theme::midnight();
        assert_eq!(m.background().to_hex(), "#121419");
        assert_eq!(m.text_color().to_hex(), "#E7E9EF");
        assert_eq!(m.muted_color().to_hex(), "#868D9C");
        assert_eq!(m.grid_color().to_hex(), "#272B34");
        assert_eq!(
            (m.grid_lines(), m.grid_dash()),
            (GridLines::Horizontal, Dash::Dashed)
        );
        assert_eq!(m.grid_width(), 0.75);
        assert_eq!(m.baseline_color().to_hex(), "#3C4250");
        assert_eq!(m.baseline_width(), 1.0);
        assert_eq!(
            (m.title_font(), m.body_font()),
            (Font::Inter, Font::JetBrainsMono)
        );
        assert_eq!(
            (m.title_size(), m.body_size(), m.font_scale()),
            (14.5, 8.5, 1.0)
        );
        assert_eq!(
            (m.legend_style(), m.fill_style()),
            (LegendStyle::Chips, FillStyle::Gradient)
        );
        assert_eq!(m.line_width(), 1.9);
        assert_eq!((m.marker_size(), m.marker_alpha()), (6.2, 0.90));
        assert_eq!((m.bar_corner_radius(), m.hist_gap()), (3.5, 1.5));
        assert_eq!(
            hexes(&m),
            [
                "#75AEF5", "#DF7F78", "#96E7A7", "#E0AE57", "#C9A3F5", "#3ACED6", "#F5A06F",
                "#9DA5B1"
            ]
        );

        assert_eq!(Theme::default(), Theme::pigment());
    }

    #[test]
    fn with_methods_validate() {
        let t = Theme::pigment();
        let bad = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

        // Colors, enums and fonts are always valid; check they apply.
        assert_eq!(
            t.with_background(Rgba::BLACK).unwrap().background(),
            Rgba::BLACK
        );
        assert_eq!(
            t.with_text_color(Rgba::RED).unwrap().text_color(),
            Rgba::RED
        );
        assert_eq!(
            t.with_muted_color(Rgba::RED).unwrap().muted_color(),
            Rgba::RED
        );
        assert_eq!(
            t.with_grid_color(Rgba::RED).unwrap().grid_color(),
            Rgba::RED
        );
        assert_eq!(
            t.with_baseline_color(Rgba::RED).unwrap().baseline_color(),
            Rgba::RED
        );
        assert_eq!(
            t.with_grid_lines(GridLines::Both).unwrap().grid_lines(),
            GridLines::Both
        );
        assert_eq!(
            t.with_grid_dash(Dash::Solid).unwrap().grid_dash(),
            Dash::Solid
        );
        assert_eq!(
            t.with_title_font(Font::Inter).unwrap().title_font(),
            Font::Inter
        );
        assert_eq!(
            t.with_body_font(Font::Inter).unwrap().body_font(),
            Font::Inter
        );
        assert_eq!(
            t.with_legend_style(LegendStyle::Chips)
                .unwrap()
                .legend_style(),
            LegendStyle::Chips
        );
        assert_eq!(
            t.with_fill_style(FillStyle::Gradient).unwrap().fill_style(),
            FillStyle::Gradient
        );

        // Palette.
        let two = [Rgba::RED, Rgba::BLUE];
        assert_eq!(t.with_palette(&two).unwrap().palette(), two);
        assert_eq!(
            t.with_palette(&[Rgba::RED; 16]).unwrap().palette().len(),
            16
        );
        let err = t.with_palette(&[]).unwrap_err();
        assert!(
            err.to_string().starts_with("Theme.with_palette(): "),
            "{err}"
        );
        assert!(t.with_palette(&[Rgba::RED; 17]).is_err());

        // Positive numbers.
        let tiny = [1e-50, 1e-100, f64::MIN_POSITIVE, f64::from_bits(1)];
        for v in bad.iter().chain(&[0.0, -1.0, 1e300]).chain(&tiny) {
            assert!(t.with_font_scale(*v).is_err(), "font_scale {v}");
            assert!(t.with_line_width(*v).is_err(), "line_width {v}");
            assert!(t.with_marker_size(*v).is_err(), "marker_size {v}");
        }
        let smallest = f64::from(f32::from_bits(1));
        assert_eq!(
            t.with_font_scale(smallest).unwrap().font_scale(),
            f32::from_bits(1)
        );
        assert_eq!(
            t.with_line_width(smallest).unwrap().line_width(),
            f32::from_bits(1)
        );
        assert_eq!(
            t.with_marker_size(smallest).unwrap().marker_size(),
            f32::from_bits(1)
        );
        assert_eq!(t.with_font_scale(10.0).unwrap().font_scale(), 10.0);
        let err = t.with_font_scale(10.000001).unwrap_err();
        assert!(err.to_string().contains("at most 10"), "{err}");
        assert_eq!(t.with_font_scale(1.2).unwrap().font_scale(), 1.2);
        assert_eq!(t.with_line_width(2.5).unwrap().line_width(), 2.5);
        assert_eq!(t.with_marker_size(7.0).unwrap().marker_size(), 7.0);

        // Non-negative numbers: zero is fine.
        for v in bad.iter().chain(&[-0.5, 1e300]) {
            assert!(t.with_baseline_width(*v).is_err(), "baseline_width {v}");
            assert!(
                t.with_bar_corner_radius(*v).is_err(),
                "bar_corner_radius {v}"
            );
            assert!(t.with_hist_gap(*v).is_err(), "hist_gap {v}");
        }
        assert_eq!(t.with_baseline_width(0.0).unwrap().baseline_width(), 0.0);
        assert_eq!(
            t.with_bar_corner_radius(2.0).unwrap().bar_corner_radius(),
            2.0
        );
        assert_eq!(t.with_hist_gap(0.0).unwrap().hist_gap(), 0.0);

        // Alpha: 0 and 1 are fine.
        for v in bad.iter().chain(&[-0.1, 1.1]) {
            assert!(t.with_marker_alpha(*v).is_err(), "marker_alpha {v}");
        }
        assert_eq!(t.with_marker_alpha(0.0).unwrap().marker_alpha(), 0.0);
        assert_eq!(t.with_marker_alpha(1.0).unwrap().marker_alpha(), 1.0);

        // Message format.
        let err = t.with_line_width(f64::NAN).unwrap_err();
        assert!(matches!(
            &err,
            Error::Style {
                context: "Theme.with_line_width()",
                ..
            }
        ));
        assert!(err.to_string().contains("must be"), "{err}");
    }

    #[test]
    fn with_methods_do_not_mutate_the_original() {
        let original = Theme::graphite();
        let before = original.clone();
        let changed = original
            .with_line_width(4.0)
            .unwrap()
            .with_palette(&[Rgba::RED])
            .unwrap()
            .with_background(Rgba::BLACK)
            .unwrap();
        assert_eq!(original, before);
        assert_ne!(changed, before);
        assert_eq!(changed.palette(), [Rgba::RED]);
    }

    // ---- CVD safety -----------------------------------------------------------------

    type Mat = [[f64; 3]; 3];

    // Machado, Oliveira & Fernandes 2009, severity 1.0.
    const PROTANOPIA: Mat = [
        [0.152286, 1.052583, -0.204868],
        [0.114503, 0.786281, 0.099216],
        [-0.003882, -0.048116, 1.051998],
    ];
    const DEUTERANOPIA: Mat = [
        [0.367322, 0.860646, -0.227968],
        [0.280085, 0.672501, 0.047413],
        [-0.011820, 0.042940, 0.968881],
    ];

    fn srgb_to_linear(c: u8) -> f64 {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn linear_rgb(c: Rgba) -> [f64; 3] {
        [
            srgb_to_linear(c.r),
            srgb_to_linear(c.g),
            srgb_to_linear(c.b),
        ]
    }

    fn simulate(m: &Mat, rgb: [f64; 3]) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (o, row) in out.iter_mut().zip(m) {
            *o = (row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2]).clamp(0.0, 1.0);
        }
        out
    }

    /// Linear sRGB to CIE Lab (D65).
    fn lab(rgb: [f64; 3]) -> [f64; 3] {
        let [r, g, b] = rgb;
        let x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
        let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
        let z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b;
        let f = |t: f64| {
            if t > 216.0 / 24389.0 {
                t.cbrt()
            } else {
                (24389.0 / 27.0 * t + 16.0) / 116.0
            }
        };
        let (fx, fy, fz) = (f(x / 0.95047), f(y), f(z / 1.08883));
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }

    /// CIEDE2000 (Sharma, Wu & Dalal 2005).
    fn delta_e2000(a: [f64; 3], b: [f64; 3]) -> f64 {
        use std::f64::consts::PI;
        let (l1, a1, b1) = (a[0], a[1], a[2]);
        let (l2, a2, b2) = (b[0], b[1], b[2]);
        let c1 = a1.hypot(b1);
        let c2 = a2.hypot(b2);
        let cbar7 = ((c1 + c2) / 2.0).powi(7);
        let g = 0.5 * (1.0 - (cbar7 / (cbar7 + 25f64.powi(7))).sqrt());
        let (ap1, ap2) = ((1.0 + g) * a1, (1.0 + g) * a2);
        let (cp1, cp2) = (ap1.hypot(b1), ap2.hypot(b2));
        let hue = |b: f64, ap: f64| {
            if b == 0.0 && ap == 0.0 {
                0.0
            } else {
                let h = b.atan2(ap).to_degrees();
                if h < 0.0 { h + 360.0 } else { h }
            }
        };
        let (hp1, hp2) = (hue(b1, ap1), hue(b2, ap2));
        let dl = l2 - l1;
        let dc = cp2 - cp1;
        let dh_deg = if cp1 * cp2 == 0.0 {
            0.0
        } else if (hp2 - hp1).abs() <= 180.0 {
            hp2 - hp1
        } else if hp2 - hp1 > 180.0 {
            hp2 - hp1 - 360.0
        } else {
            hp2 - hp1 + 360.0
        };
        let dh = 2.0 * (cp1 * cp2).sqrt() * (dh_deg.to_radians() / 2.0).sin();
        let lbar = (l1 + l2) / 2.0;
        let cbar = (cp1 + cp2) / 2.0;
        let hbar = if cp1 * cp2 == 0.0 {
            hp1 + hp2
        } else if (hp1 - hp2).abs() <= 180.0 {
            (hp1 + hp2) / 2.0
        } else if hp1 + hp2 < 360.0 {
            (hp1 + hp2 + 360.0) / 2.0
        } else {
            (hp1 + hp2 - 360.0) / 2.0
        };
        let t = 1.0 - 0.17 * (hbar - 30.0).to_radians().cos()
            + 0.24 * (2.0 * hbar).to_radians().cos()
            + 0.32 * (3.0 * hbar + 6.0).to_radians().cos()
            - 0.20 * (4.0 * hbar - 63.0).to_radians().cos();
        let dtheta = 30.0 * (-((hbar - 275.0) / 25.0).powi(2)).exp();
        let cbar7 = cbar.powi(7);
        let rc = 2.0 * (cbar7 / (cbar7 + 25f64.powi(7))).sqrt();
        let sl = 1.0 + 0.015 * (lbar - 50.0).powi(2) / (20.0 + (lbar - 50.0).powi(2)).sqrt();
        let sc = 1.0 + 0.045 * cbar;
        let sh = 1.0 + 0.015 * cbar * t;
        let rt = -(2.0 * dtheta * (PI / 180.0)).sin() * rc;
        ((dl / sl).powi(2) + (dc / sc).powi(2) + (dh / sh).powi(2) + rt * (dc / sc) * (dh / sh))
            .sqrt()
    }

    /// Minimum pairwise ΔE2000 of the first four palette colors under a simulation.
    fn min_delta_e(theme: &Theme, matrix: &Mat) -> f64 {
        let labs: Vec<[f64; 3]> = theme
            .palette()
            .iter()
            .take(4)
            .map(|c| lab(simulate(matrix, linear_rgb(*c))))
            .collect();
        let mut min = f64::INFINITY;
        for (i, a) in labs.iter().enumerate() {
            for b in &labs[i + 1..] {
                min = min.min(delta_e2000(*a, *b));
            }
        }
        min
    }

    #[test]
    fn delta_e2000_matches_published_pairs() {
        // Sharma et al. 2005 test data, pairs 1 and 2.
        let d = delta_e2000([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485]);
        assert!((d - 2.0425).abs() < 1e-4, "{d}");
        let d = delta_e2000([50.0, 3.1571, -77.2803], [50.0, 0.0, -82.7485]);
        assert!((d - 2.8615).abs() < 1e-4, "{d}");
    }

    #[test]
    fn palettes_are_cvd_safe() {
        // (theme, deuteranopia reference, protanopia reference); None = only the overall
        // minimum is known.
        let cases = [
            ("pigment", Theme::pigment(), Some(12.1), Some(14.2)),
            ("graphite", Theme::graphite(), Some(14.4), Some(22.1)),
            ("midnight", Theme::midnight(), None, None),
        ];
        let mut midnight_overall = f64::INFINITY;
        for (name, theme, deut_ref, prot_ref) in cases {
            let deut = min_delta_e(&theme, &DEUTERANOPIA);
            let prot = min_delta_e(&theme, &PROTANOPIA);
            eprintln!("{name}: deuteranopia {deut:.2}, protanopia {prot:.2}");
            assert!(deut >= 10.0, "{name} deuteranopia min ΔE {deut:.2}");
            assert!(prot >= 10.0, "{name} protanopia min ΔE {prot:.2}");
            if let Some(r) = deut_ref {
                assert!(
                    (deut - r).abs() <= 0.2,
                    "{name} deuteranopia {deut:.2} vs {r}"
                );
            }
            if let Some(r) = prot_ref {
                assert!(
                    (prot - r).abs() <= 0.2,
                    "{name} protanopia {prot:.2} vs {r}"
                );
            }
            if name == "midnight" {
                midnight_overall = deut.min(prot);
            }
        }
        assert!(
            (midnight_overall - 12.4).abs() <= 0.2,
            "midnight overall {midnight_overall:.2}"
        );
    }
}
