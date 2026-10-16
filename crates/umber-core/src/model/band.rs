//! Band marks: the options given to the band method and the stored band mark.

use std::fmt;
use std::sync::Arc;

use super::axis_data::SharedValues;
use crate::{ColorSpec, Instant};

/// Options of a band mark.
///
/// # Examples
///
/// ```
/// use umber_core::ColorSpec;
/// use umber_core::figure::BandOptions;
///
/// let options = BandOptions::default();
/// assert_eq!(options.color, ColorSpec::AUTO);
/// assert!(options.label.is_empty());
///
/// let named = BandOptions { label: "95% interval".into(), ..BandOptions::default() };
/// assert_ne!(named, options);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BandOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The fill color, drawn at the area opacity.
    pub color: ColorSpec,
}

impl Default for BandOptions {
    fn default() -> Self {
        BandOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
        }
    }
}

/// A band mark: the region between a lower and an upper curve over validated x values, and its
/// [`BandOptions`].
///
/// The x, low and high values are stored together and never change; clones share them. The x
/// values are numbers on a linear or log axis and times on a time axis
/// ([`AxesMut::band_time`](super::AxesMut::band_time)), in non-decreasing order, and at every
/// index where both are finite, low does not exceed high.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{BandOptions, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.band(&[0.0, 1.0, 2.0], &[1.0, 2.0, 1.5], &[3.0, 4.0, 3.5], BandOptions::default())?;
/// let Some(Mark::Band(band)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(band.x(), &[0.0, 1.0, 2.0]);
/// assert_eq!(band.low(), &[1.0, 2.0, 1.5]);
/// assert_eq!(band.high(), &[3.0, 4.0, 3.5]);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A band mark cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{BandMark, BandOptions, Mark};
///
/// let mark = Mark::Band(BandMark {
///     x: unreachable!(),
///     low: unreachable!(),
///     high: unreachable!(),
///     options: BandOptions::default(),
/// });
/// ```
#[derive(Clone, PartialEq)]
pub struct BandMark {
    x: SharedValues,
    low: Arc<[f64]>,
    high: Arc<[f64]>,
    options: BandOptions,
}

impl BandMark {
    /// A band over validated values of equal length.
    pub(super) fn new(
        x: SharedValues,
        low: Arc<[f64]>,
        high: Arc<[f64]>,
        options: BandOptions,
    ) -> BandMark {
        BandMark {
            x,
            low,
            high,
            options,
        }
    }

    /// The x values on a linear or log axis; empty on a time axis, whose x values
    /// [`BandMark::x_times`] gives.
    #[must_use]
    pub fn x(&self) -> &[f64] {
        self.x.numbers()
    }

    /// The x values on a time axis, `None` for a gap; empty on a linear or log axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use umber_core::figure::{BandOptions, Mark};
    /// use umber_core::{Figure, Instant, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Time, ScaleKind::Linear)?;
    /// let x = [Some(Instant::from_micros(0)?), None];
    /// axes.band_time(Arc::from(x), Arc::from([1.0, 2.0]), Arc::from([3.0, 4.0]), BandOptions::default())?;
    /// let Some(Mark::Band(band)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(band.x_times(), &x);
    /// assert!(band.x().is_empty());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_times(&self) -> &[Option<Instant>] {
        self.x.times()
    }

    /// The x values as stored: numbers or times.
    pub(crate) fn x_values(&self) -> &SharedValues {
        &self.x
    }

    /// The values of the lower curve, as many as the x values.
    #[must_use]
    pub fn low(&self) -> &[f64] {
        &self.low
    }

    /// The values of the upper curve, as many as the x values.
    #[must_use]
    pub fn high(&self) -> &[f64] {
        &self.high
    }

    /// Label and color.
    #[must_use]
    pub fn options(&self) -> &BandOptions {
        &self.options
    }
}

impl fmt::Debug for BandMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BandMark")
            .field("x", &self.x)
            .field("low", &self.low())
            .field("high", &self.high())
            .field("options", &self.options)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, Figure, LineOptions, Mark};
    use crate::{Error, LogBase, ScaleKind, Theme};

    const LIN: ScaleKind = ScaleKind::Linear;
    const NAN: f64 = f64::NAN;

    fn figure(x: ScaleKind, y: ScaleKind) -> Figure {
        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        figure.add_axes(x, y).unwrap();
        figure
    }

    /// The error of `band` called on axes of scale kinds `x` and `y`, after one line and one
    /// band: its variant name, text, parameter and hint. The figure is unchanged afterwards.
    #[track_caller]
    fn refused(
        (kind_x, kind_y): (ScaleKind, ScaleKind),
        (x, low, high): (&[f64], &[f64], &[f64]),
    ) -> (
        &'static str,
        String,
        Option<&'static str>,
        Option<&'static str>,
    ) {
        let mut f = figure(kind_x, kind_y);
        if kind_x.transform().is_some() {
            f.single_axes_mut()
                .line(AxisData::Numbers(vec![1.0]), &[1.0], LineOptions::default())
                .unwrap()
                .band(&[1.0], &[1.0], &[2.0], BandOptions::default())
                .unwrap();
        }
        let before = f.clone();
        let err = f
            .single_axes_mut()
            .band(x, low, high, BandOptions::default())
            .unwrap_err();
        let kind = match err {
            Error::Data { .. } => "data",
            Error::Usage { .. } => "usage",
            Error::Style { .. } => "style",
            _ => "other",
        };
        assert_eq!(f, before);
        (kind, err.to_string(), err.parameter(), err.hint())
    }

    #[test]
    fn band_unsorted_x_names_index() {
        let order = "band needs x in non-decreasing order.";
        let (kind, text, parameter, hint) =
            refused((LIN, LIN), (&[0.0, 3.0, 2.0], &[0.0; 3], &[1.0; 3]));
        assert_eq!(kind, "data");
        assert_eq!(
            text,
            format!("Axes.band(): x[2] is 2, below x[1] = 3; {order}")
        );
        assert_eq!(
            (parameter, hint),
            (Some("x"), Some("sort the points by x first."))
        );
        // A gap in x is skipped: the value is compared with the last one before the gap.
        let (_, text, ..) = refused((LIN, LIN), (&[0.0, NAN, -1.5], &[0.0; 3], &[1.0; 3]));
        assert_eq!(
            text,
            format!("Axes.band(): x[2] is -1.5, below x[0] = 0; {order}")
        );
        // The order is checked after the values and before low against high.
        let (_, text, ..) = refused((LIN, LIN), (&[1.0, 0.0], &[5.0; 2], &[1.0; 2]));
        assert!(text.starts_with("Axes.band(): x[1] is 0, below"), "{text}");
        let (_, text, ..) = refused(
            (LIN, LIN),
            (&[1.0, 0.0], &[0.0, f64::INFINITY], &[1.0, f64::INFINITY]),
        );
        assert!(text.starts_with("Axes.band(): low[1] is inf"), "{text}");
        let (_, text, parameter, hint) =
            refused((LIN, LIN), (&[1.0, 0.0], &[5.0; 2], &[1.0, f64::INFINITY]));
        assert_eq!(text, "Axes.band(): high[1] is inf; values must be finite.");
        assert_eq!(
            (parameter, hint),
            (Some("high"), Some("use NaN to leave a gap in the band."))
        );
        // Equal x is accepted.
        let mut f = figure(LIN, LIN);
        f.single_axes_mut()
            .band(
                &[0.0, 1.0, 1.0],
                &[0.0; 3],
                &[1.0; 3],
                BandOptions::default(),
            )
            .unwrap();
    }

    #[test]
    fn band_low_above_high_names_index() {
        let (kind, text, parameter, hint) = refused(
            (LIN, LIN),
            (&[0.0, 1.0, 2.0], &[0.0, 5.0, 1.0], &[1.0, 4.0, 2.0]),
        );
        assert_eq!(kind, "data");
        assert_eq!(
            text,
            "Axes.band(): low[1] is 5, above high[1] = 4; low must not exceed high."
        );
        assert_eq!(
            (parameter, hint),
            (
                Some("low"),
                Some("check that low and high are not swapped.")
            )
        );
        // The first index is named; a gap in either curve is not compared; equal is allowed.
        let (_, text, ..) = refused(
            (LIN, LIN),
            (&[0.0; 4], &[NAN, 3.0, 9.0, 8.0], &[0.0, NAN, 2.0, 1.0]),
        );
        assert_eq!(
            text,
            "Axes.band(): low[2] is 9, above high[2] = 2; low must not exceed high."
        );
        let mut f = figure(LIN, LIN);
        f.single_axes_mut()
            .band(
                &[0.0, 1.0],
                &[2.0, NAN],
                &[2.0, 1.0],
                BandOptions::default(),
            )
            .unwrap();
    }

    #[test]
    fn band_checks_lengths_values_and_axis_in_order() {
        let (kind, text, parameter, hint) = refused((LIN, LIN), (&[0.0, 1.0], &[1.0], &[2.0, 3.0]));
        assert_eq!(kind, "data");
        assert_eq!(
            text,
            "Axes.band(): x has 2 values but low has 1; x, low and high must have the same length."
        );
        assert_eq!((parameter, hint), (None, None));
        let (_, text, ..) = refused((LIN, LIN), (&[0.0, 1.0], &[1.0, 2.0], &[2.0]));
        assert_eq!(
            text,
            "Axes.band(): x has 2 values but high has 1; x, low and high must have the same length."
        );
        let gap = Some("use NaN to leave a gap in the band.");
        let (_, text, parameter, hint) = refused(
            (LIN, LIN),
            (&[0.0, f64::INFINITY], &[1.0, 2.0], &[2.0, 3.0]),
        );
        assert_eq!(text, "Axes.band(): x[1] is inf; values must be finite.");
        assert_eq!((parameter, hint), (Some("x"), gap));
        let (_, text, parameter, hint) = refused(
            (LIN, LIN),
            (&[0.0, 1.0], &[1.0, f64::NEG_INFINITY], &[2.0, 3.0]),
        );
        assert_eq!(text, "Axes.band(): low[1] is -inf; values must be finite.");
        assert_eq!((parameter, hint), (Some("low"), gap));
        let (_, text, parameter, hint) = refused(
            (LIN, LIN),
            (&[0.0, 1.0], &[1.0, 1.0], &[f64::INFINITY, 3.0]),
        );
        assert_eq!(text, "Axes.band(): high[0] is inf; values must be finite.");
        assert_eq!((parameter, hint), (Some("high"), gap));
        // Lengths come first, then x, then low, then high, then the order of the curves.
        let (_, text, ..) = refused((LIN, LIN), (&[f64::INFINITY], &[], &[]));
        assert!(text.starts_with("Axes.band(): x has 1 values"), "{text}");
        let (_, text, ..) = refused((LIN, LIN), (&[f64::INFINITY], &[f64::INFINITY], &[0.0]));
        assert!(text.starts_with("Axes.band(): x[0] is inf"), "{text}");
        let (_, text, ..) = refused(
            (LIN, LIN),
            (&[0.0, 1.0], &[5.0, f64::INFINITY], &[1.0, f64::INFINITY]),
        );
        assert!(text.starts_with("Axes.band(): low[1] is inf"), "{text}");
        let (_, text, ..) = refused(
            (LIN, LIN),
            (&[0.0, 1.0], &[5.0, 1.0], &[1.0, f64::INFINITY]),
        );
        assert!(text.starts_with("Axes.band(): high[1] is inf"), "{text}");
        // Log axes: x against the x scale, low and high against the y scale.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let (_, text, parameter, _) = refused((log, LIN), (&[1.0, -1.0], &[0.0; 2], &[1.0; 2]));
        assert!(
            text.starts_with("Axes.band(): x[1] is -1 on a log axis"),
            "{text}"
        );
        assert_eq!(parameter, Some("x"));
        let (_, text, parameter, _) = refused((LIN, log), (&[1.0, 2.0], &[1.0, 0.0], &[2.0; 2]));
        assert!(
            text.starts_with("Axes.band(): low[1] is 0 on a log axis"),
            "{text}"
        );
        assert_eq!(parameter, Some("low"));
    }

    #[test]
    fn band_refuses_a_category_x_axis_as_a_usage_error() {
        let (kind, text, parameter, hint) = refused(
            (ScaleKind::Category, LIN),
            (&[0.0, 1.0], &[0.0, 1.0], &[1.0, 2.0]),
        );
        assert_eq!(kind, "usage");
        assert_eq!(
            text,
            "Axes.band(): this is a category axis; bands along x apply to linear and log axes only."
        );
        assert_eq!(parameter, Some("x"));
        assert_eq!(
            hint,
            Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().")
        );
        // The axis kind is checked before any value.
        let (kind, ..) = refused((ScaleKind::Category, LIN), (&[0.0], &[], &[]));
        assert_eq!(kind, "usage");
    }

    #[test]
    fn band_accepts_empty_and_gapped_data_and_keeps_every_bit() {
        let gap = f64::from_bits(0xfff8_0000_0000_0001);
        let x: Arc<[f64]> = Arc::from([0.0, gap, -0.0, 2.0]);
        let low: Arc<[f64]> = Arc::from([1.0, 1.0, gap, 0.5]);
        let high: Arc<[f64]> = Arc::from([2.0, 3.0, 3.0, gap]);
        let options = BandOptions {
            label: "interval".into(),
            ..BandOptions::default()
        };
        let mut f = figure(LIN, LIN);
        f.single_axes_mut()
            .band(&[], &[], &[], BandOptions::default())
            .unwrap()
            .band(
                &[NAN, NAN],
                &[NAN, NAN],
                &[NAN, NAN],
                BandOptions::default(),
            )
            .unwrap()
            .band_shared(
                Arc::clone(&x),
                Arc::clone(&low),
                Arc::clone(&high),
                options.clone(),
            )
            .unwrap();
        let [_, _, Mark::Band(band)] = f.single_axes().marks() else {
            panic!("three bands expected")
        };
        let bits = |v: &[f64]| v.iter().map(|b| b.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(band.x()), bits(&x));
        assert_eq!(bits(band.low()), bits(&low));
        assert_eq!(bits(band.high()), bits(&high));
        assert_eq!(band.options(), &options);
        // The mark holds the caller's arrays, and clones read the same ones.
        assert!(std::ptr::eq(band.x(), &*x));
        assert!(std::ptr::eq(band.low(), &*low));
        assert!(std::ptr::eq(band.high(), &*high));
        let copy = f.clone();
        let [_, _, Mark::Band(cloned)] = copy.single_axes().marks() else {
            panic!("three bands expected")
        };
        assert!(std::ptr::eq(band.high(), cloned.high()));
    }

    #[test]
    fn a_failed_shared_band_keeps_no_reference_to_the_values() {
        let x: Arc<[f64]> = Arc::from([0.0, 1.0]);
        let low: Arc<[f64]> = Arc::from([3.0, 1.0]);
        let high: Arc<[f64]> = Arc::from([2.0, 2.0]);
        let mut f = figure(LIN, LIN);
        assert!(
            f.single_axes_mut()
                .band_shared(
                    Arc::clone(&x),
                    Arc::clone(&low),
                    Arc::clone(&high),
                    BandOptions::default()
                )
                .is_err()
        );
        assert_eq!(
            (
                Arc::strong_count(&x),
                Arc::strong_count(&low),
                Arc::strong_count(&high)
            ),
            (1, 1, 1)
        );
        assert!(f.single_axes().marks().is_empty());
    }
}
