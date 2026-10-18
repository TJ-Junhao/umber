//! Event ticks arranged in category rows.

use std::collections::HashMap;
use std::sync::Arc;

use super::AxisData;
use super::validation::check_event_positions;
use crate::{ColorSpec, Error, Result, ScaleKind, SizeSpec};

/// Options of an event mark.
///
/// # Examples
///
/// ```
/// use umber_core::EventsOptions;
/// assert_eq!(EventsOptions::default().length, 0.8);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct EventsOptions {
    /// The tick length as a fraction of the row band, above zero and at most one.
    pub length: f64,
    /// The tick stroke width; automatic uses the theme's line width.
    pub width: SizeSpec,
    /// The color of all ticks in this mark.
    pub color: ColorSpec,
}

impl Default for EventsOptions {
    fn default() -> Self {
        Self {
            length: 0.8,
            width: SizeSpec::Auto,
            color: ColorSpec::AUTO,
        }
    }
}

/// Validated event rows and their positions, shared by clones.
///
/// # Examples
///
/// ```
/// use umber_core::{AxisData, EventsOptions, Figure, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Category)?;
/// ax.events(vec!["A".into()], vec![AxisData::Numbers(vec![1.0])], EventsOptions::default())?;
/// let Mark::Events(mark) = &ax.marks()[0] else { unreachable!() };
/// assert_eq!(mark.rows(), ["A"]);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct EventsMark {
    rows: Arc<[String]>,
    positions: Arc<[AxisData]>,
    options: EventsOptions,
}

impl EventsMark {
    /// Validates every row before storing anything.
    pub(super) fn new(
        x: ScaleKind,
        y: ScaleKind,
        rows: Vec<String>,
        positions: Vec<AxisData>,
        options: EventsOptions,
    ) -> Result<Self> {
        const CONTEXT: &str = "Axes.events()";
        if y != ScaleKind::Category || x == ScaleKind::Category {
            return Err(Error::usage(CONTEXT, "events need a numeric or time x axis and a category y axis; choose those axis scales.")
                .with_parameter("rows")
                .with_hint("pass y=Scale.category() with a linear, log or time x axis."));
        }
        if !options.length.is_finite() || options.length <= 0.0 || options.length > 1.0 {
            return Err(Error::style(
                CONTEXT,
                format!(
                    "length is {}; it must be finite, above 0 and at most 1.",
                    crate::error::number(options.length)
                ),
            )
            .with_parameter("length")
            .with_hint("pass a fraction of the row height, such as 0.8."));
        }
        if rows.len() != positions.len() {
            return Err(Error::data(
                CONTEXT,
                format!(
                    "positions has {} groups; rows has {}.",
                    positions.len(),
                    rows.len()
                ),
            )
            .with_parameter("positions")
            .with_hint("pass one group per row, using an empty group when there are no events."));
        }
        let mut first = HashMap::with_capacity(rows.len());
        for (i, row) in rows.iter().enumerate() {
            if let Some(j) = first.insert(row, i) {
                return Err(Error::data(
                    CONTEXT,
                    format!("rows[{i}] repeats {row:?} from rows[{j}]; each row must appear once."),
                )
                .with_parameter("rows")
                .with_hint("combine the events of repeated rows."));
            }
        }
        for (i, group) in positions.iter().enumerate() {
            check_event_positions(x, i, group)?;
        }
        Ok(Self {
            rows: rows.into(),
            positions: positions.into(),
            options,
        })
    }

    /// The row names in input order, including empty rows.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{AxisData, EventsOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Category)?;
    /// # ax.events(vec!["A".into()], vec![AxisData::Numbers(vec![])], EventsOptions::default())?;
    /// # let Mark::Events(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.rows(), ["A"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn rows(&self) -> &[String] {
        &self.rows
    }

    /// One group of numeric or time positions per row, including gaps as given.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{AxisData, EventsOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Category)?;
    /// # ax.events(vec!["A".into()], vec![AxisData::Numbers(vec![2.0])], EventsOptions::default())?;
    /// # let Mark::Events(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.positions()[0], AxisData::Numbers(vec![2.0]));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn positions(&self) -> &[AxisData] {
        &self.positions
    }

    /// The tick length, width and color as given.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{EventsOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Category)?;
    /// # ax.events(vec![], vec![], EventsOptions::default())?;
    /// # let Mark::Events(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.options().length, 0.8);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &EventsOptions {
        &self.options
    }
}
