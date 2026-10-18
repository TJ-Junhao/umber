//! Time values: instants as whole microseconds, their civil date and time, the calendar ticks
//! of a time range and the labels of those ticks.
//!
//! The calendar is the proleptic Gregorian one, without time zones or leap seconds, from
//! 0001-01-01 to 9999-12-31: the range of Python's `datetime`. Days convert to civil dates and
//! back with the days-from-civil algorithm.
//!
//! # Examples
//!
//! ```
//! use umber_core::Instant;
//!
//! let t = Instant::from_micros(1_772_517_600_000_000)?;
//! let c = t.civil();
//! assert_eq!((c.year, c.month, c.day, c.hour), (2026, 3, 3, 6));
//! # Ok::<(), umber_core::Error>(())
//! ```

use crate::{Error, Result};

/// Microseconds in a second.
const MICROS_PER_SECOND: i64 = 1_000_000;
/// Microseconds in a minute.
const MICROS_PER_MINUTE: i64 = 60 * MICROS_PER_SECOND;
/// Microseconds in an hour.
const MICROS_PER_HOUR: i64 = 60 * MICROS_PER_MINUTE;
/// Microseconds in a day.
const MICROS_PER_DAY: i64 = 24 * MICROS_PER_HOUR;

/// The English abbreviations of the months, January first.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// A point in time: whole microseconds since 1970-01-01T00:00, from [`Instant::MIN`] to
/// [`Instant::MAX`]. Instants carry no time zone. They order as time does.
///
/// # Examples
///
/// ```
/// use umber_core::Instant;
///
/// let epoch = Instant::from_micros(0)?;
/// assert!(Instant::MIN < epoch && epoch < Instant::MAX);
/// assert_eq!(epoch.civil().year, 1970);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(i64);

/// The civil date and time of an [`Instant`], as [`Instant::civil`] returns it.
///
/// # Examples
///
/// ```
/// use umber_core::Instant;
///
/// let c = Instant::MAX.civil();
/// assert_eq!((c.year, c.month, c.day), (9999, 12, 31));
/// assert_eq!((c.hour, c.minute, c.second, c.micro), (23, 59, 59, 999_999));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Civil {
    /// The year, from 1 to 9999.
    pub year: i32,
    /// The month, from 1 (January) to 12.
    pub month: u8,
    /// The day of the month, from 1.
    pub day: u8,
    /// The hour, from 0 to 23.
    pub hour: u8,
    /// The minute, from 0 to 59.
    pub minute: u8,
    /// The second, from 0 to 59.
    pub second: u8,
    /// The microsecond within the second, from 0 to 999,999.
    pub micro: u32,
}

impl Instant {
    /// The earliest instant: 0001-01-01T00:00.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Instant;
    ///
    /// assert_eq!(Instant::MIN.civil().year, 1);
    /// assert!(Instant::from_micros(Instant::MIN.micros() - 1).is_err());
    /// ```
    pub const MIN: Instant = Instant(days_from_civil(1, 1, 1) * MICROS_PER_DAY);

    /// The latest instant: 9999-12-31T23:59:59.999999.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Instant;
    ///
    /// assert_eq!(Instant::MAX.civil().year, 9999);
    /// assert!(Instant::from_micros(Instant::MAX.micros() + 1).is_err());
    /// ```
    pub const MAX: Instant = Instant(days_from_civil(10000, 1, 1) * MICROS_PER_DAY - 1);

    /// The instant `us` microseconds after 1970-01-01T00:00 (before it when negative).
    ///
    /// # Errors
    ///
    /// `Error::Data` (context `"Instant.from_micros()"`, parameter `us`) when `us` lies outside
    /// [`Instant::MIN`]..=[`Instant::MAX`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Instant;
    ///
    /// assert_eq!(Instant::from_micros(-1)?.civil().year, 1969);
    /// let err = Instant::from_micros(Instant::MAX.micros() + 1).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "Instant.from_micros(): us is 10000-01-01; times must lie between 0001-01-01 and 9999-12-31."
    /// );
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn from_micros(us: i64) -> Result<Instant> {
        if (Instant::MIN.0..=Instant::MAX.0).contains(&us) {
            Ok(Instant(us))
        } else {
            Err(range_error("Instant.from_micros()", "us", us).with_parameter("us"))
        }
    }

    /// The microseconds since 1970-01-01T00:00; negative before it.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Instant;
    ///
    /// assert_eq!(Instant::from_micros(1_500_000)?.micros(), 1_500_000);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn micros(self) -> i64 {
        self.0
    }

    /// The civil date and time of this instant.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Instant;
    ///
    /// let c = Instant::from_micros(951_827_696_789_012)?.civil();
    /// assert_eq!((c.year, c.month, c.day), (2000, 2, 29));
    /// assert_eq!((c.hour, c.minute, c.second, c.micro), (12, 34, 56, 789_012));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn civil(self) -> Civil {
        civil_of(self.0)
    }

    /// The instant nearest to `us` microseconds since 1970-01-01T00:00 that lies in the
    /// supported range: `us` itself, or [`Instant::MIN`] or [`Instant::MAX`] beyond them.
    pub(crate) fn clamped(us: i64) -> Instant {
        Instant(us.clamp(Instant::MIN.0, Instant::MAX.0))
    }

    /// This instant as messages write it: the ISO 8601 date, then the clock as far as it is not
    /// zero, such as `2026-03-03`, `2026-03-03T06:30`, `2026-03-03T06:30:15` or
    /// `2026-03-03T06:30:15.25`.
    pub(crate) fn label(self) -> String {
        let c = self.civil();
        let date = format!("{:04}-{:02}-{:02}", c.year, c.month, c.day);
        if (c.hour, c.minute, c.second, c.micro) == (0, 0, 0, 0) {
            return date;
        }
        let minutes = format!("{date}T{:02}:{:02}", c.hour, c.minute);
        if (c.second, c.micro) == (0, 0) {
            return minutes;
        }
        if c.micro == 0 {
            return format!("{minutes}:{:02}", c.second);
        }
        let fraction = format!("{:06}", c.micro);
        format!(
            "{minutes}:{:02}.{}",
            c.second,
            fraction.trim_end_matches('0')
        )
    }
}

/// The microseconds that stand for a missing time where times travel as whole microseconds:
/// the smallest `i64`, which is how NumPy stores `NaT`.
///
/// # Examples
///
/// ```
/// use umber_core::{MISSING_TIME, times_from_micros};
///
/// assert_eq!(times_from_micros("doc", "x", &[MISSING_TIME])?, [None]);
/// # Ok::<(), umber_core::Error>(())
/// ```
pub const MISSING_TIME: i64 = i64::MIN;

/// The times `micros`, each in microseconds since 1970-01-01T00:00, with [`MISSING_TIME`] for a
/// missing time, which becomes `None`, a gap. The values are named `name` in messages, such as
/// `x`.
///
/// # Errors
///
/// `Error::Data` under `context`, naming `name`, for the first value outside
/// [`Instant::MIN`]..=[`Instant::MAX`].
///
/// # Examples
///
/// ```
/// use umber_core::{Instant, MISSING_TIME, times_from_micros};
///
/// let times = times_from_micros("Axes.line()", "x", &[0, MISSING_TIME])?;
/// assert_eq!(times, [Some(Instant::from_micros(0)?), None]);
/// let late = Instant::MAX.micros() + 1;
/// let err = times_from_micros("Axes.line()", "x", &[0, late]).unwrap_err();
/// assert_eq!(
///     err.to_string(),
///     "Axes.line(): x[1] is 10000-01-01; times must lie between 0001-01-01 and 9999-12-31."
/// );
/// # Ok::<(), umber_core::Error>(())
/// ```
pub fn times_from_micros(
    context: &'static str,
    name: &'static str,
    micros: &[i64],
) -> Result<Vec<Option<Instant>>> {
    micros
        .iter()
        .enumerate()
        .map(|(i, &us)| {
            if us == MISSING_TIME {
                Ok(None)
            } else if (Instant::MIN.0..=Instant::MAX.0).contains(&us) {
                Ok(Some(Instant(us)))
            } else {
                Err(
                    Error::at_input(context, name, "", [(Some(i), range_message(us))])
                        .with_parameter(name),
                )
            }
        })
        .collect()
}

/// The time `us`, in microseconds since 1970-01-01T00:00, named `name` in messages, such as
/// `low`.
///
/// # Errors
///
/// `Error::Data` under `context`, naming `name`, when `us` lies outside
/// [`Instant::MIN`]..=[`Instant::MAX`]; [`MISSING_TIME`] does too.
///
/// # Examples
///
/// ```
/// use umber_core::{Instant, time_from_micros};
///
/// assert_eq!(time_from_micros("Axes.xlim()", "low", 0)?, Instant::from_micros(0)?);
/// let err = time_from_micros("Axes.xlim()", "low", Instant::MIN.micros() - 1).unwrap_err();
/// assert_eq!(
///     err.to_string(),
///     "Axes.xlim(): low is 0000-12-31; times must lie between 0001-01-01 and 9999-12-31."
/// );
/// # Ok::<(), umber_core::Error>(())
/// ```
pub fn time_from_micros(context: &'static str, name: &'static str, us: i64) -> Result<Instant> {
    if (Instant::MIN.0..=Instant::MAX.0).contains(&us) {
        Ok(Instant(us))
    } else {
        Err(range_error(context, name, us).with_parameter(name))
    }
}

/// The civil date and time of `us` microseconds since 1970-01-01T00:00, for any `us`: years
/// outside 1..=9999, down to 0 and below, are counted on in the proleptic calendar.
fn civil_of(us: i64) -> Civil {
    let (year, month, day) = civil_from_days(us.div_euclid(MICROS_PER_DAY));
    let clock = us.rem_euclid(MICROS_PER_DAY);
    // Every field below is in range by construction: the clock lies in 0..MICROS_PER_DAY, and a
    // year of i64 microseconds has at most six digits.
    Civil {
        year: year as i32,
        month: month as u8,
        day: day as u8,
        hour: (clock / MICROS_PER_HOUR) as u8,
        minute: (clock / MICROS_PER_MINUTE % 60) as u8,
        second: (clock / MICROS_PER_SECOND % 60) as u8,
        micro: (clock % MICROS_PER_SECOND) as u32,
    }
}

/// The days from 1970-01-01 to the civil date `year`-`month`-`day` (negative before it), for
/// `month` in 1..=12 and `day` in 1..=31.
pub(crate) const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    // Years start on March 1 here, so that the leap day ends the year.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = (month + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The civil date `(year, month, day)` of the day `days` after 1970-01-01; the inverse of
/// [`days_from_civil`].
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The `Error::Data` under `context` for a time `us` microseconds after 1970-01-01T00:00 that
/// lies outside the supported range: `"<what> is <date>; times must lie between 0001-01-01
/// and 9999-12-31."`, where `what` names the value, such as `x[3]`.
pub(crate) fn range_error(context: &'static str, what: &str, us: i64) -> Error {
    Error::data(context, format!("{what}{}", range_message(us)))
}

/// The explanation of a time outside the supported range, without its input position.
fn range_message(us: i64) -> String {
    let c = civil_of(us);
    let year = if c.year < 0 {
        format!("-{:04}", -i64::from(c.year))
    } else {
        format!("{:04}", c.year)
    };
    format!(
        " is {year}-{:02}-{:02}; times must lie between 0001-01-01 and 9999-12-31.",
        c.month, c.day
    )
}

/// The step between calendar ticks.
///
/// Steps longer than a day follow the calendar: years start on January 1 and are multiples of
/// the step; months start on the first of the month and count from January of year 0, so
/// 3-month steps fall on January, April, July and October. Day steps count whole days from
/// 0001-01-01, a Monday, so 7-day steps fall on Mondays. Shorter steps are multiples of their
/// unit within the day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TimeStep {
    /// This many years.
    Years(u32),
    /// This many months.
    Months(u8),
    /// This many days.
    Days(u8),
    /// This many hours.
    Hours(u8),
    /// This many minutes.
    Minutes(u8),
    /// This many seconds.
    Seconds(u8),
    /// This many microseconds, below one second.
    Sub(u32),
}

/// The steps automatic ticks choose from, the largest first.
const STEPS: [TimeStep; 48] = [
    TimeStep::Years(5000),
    TimeStep::Years(2000),
    TimeStep::Years(1000),
    TimeStep::Years(500),
    TimeStep::Years(200),
    TimeStep::Years(100),
    TimeStep::Years(50),
    TimeStep::Years(20),
    TimeStep::Years(10),
    TimeStep::Years(5),
    TimeStep::Years(2),
    TimeStep::Years(1),
    TimeStep::Months(6),
    TimeStep::Months(3),
    TimeStep::Months(1),
    TimeStep::Days(7),
    TimeStep::Days(2),
    TimeStep::Days(1),
    TimeStep::Hours(12),
    TimeStep::Hours(6),
    TimeStep::Hours(3),
    TimeStep::Hours(1),
    TimeStep::Minutes(30),
    TimeStep::Minutes(15),
    TimeStep::Minutes(5),
    TimeStep::Minutes(1),
    TimeStep::Seconds(30),
    TimeStep::Seconds(15),
    TimeStep::Seconds(5),
    TimeStep::Seconds(1),
    TimeStep::Sub(500_000),
    TimeStep::Sub(200_000),
    TimeStep::Sub(100_000),
    TimeStep::Sub(50_000),
    TimeStep::Sub(20_000),
    TimeStep::Sub(10_000),
    TimeStep::Sub(5_000),
    TimeStep::Sub(2_000),
    TimeStep::Sub(1_000),
    TimeStep::Sub(500),
    TimeStep::Sub(200),
    TimeStep::Sub(100),
    TimeStep::Sub(50),
    TimeStep::Sub(20),
    TimeStep::Sub(10),
    TimeStep::Sub(5),
    TimeStep::Sub(2),
    TimeStep::Sub(1),
];

/// The tick counts a step is chosen for first.
const WANTED_TICKS: std::ops::RangeInclusive<i64> = 5..=8;

/// The unit a step counts in. Units are numbered from 1970-01-01T00:00 (years by their number,
/// months as `year * 12 + month - 1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Micro,
}

impl Unit {
    /// The number of the unit that holds `us`.
    fn index(self, us: i64) -> i64 {
        let fixed = |length: i64| us.div_euclid(length);
        match self {
            Unit::Year => i64::from(civil_of(us).year),
            Unit::Month => {
                let c = civil_of(us);
                i64::from(c.year) * 12 + i64::from(c.month) - 1
            }
            Unit::Day => fixed(MICROS_PER_DAY),
            Unit::Hour => fixed(MICROS_PER_HOUR),
            Unit::Minute => fixed(MICROS_PER_MINUTE),
            Unit::Second => fixed(MICROS_PER_SECOND),
            Unit::Micro => us,
        }
    }

    /// The microseconds since 1970-01-01T00:00 at which unit number `index` starts.
    fn start(self, index: i64) -> i64 {
        match self {
            Unit::Year => days_from_civil(index, 1, 1) * MICROS_PER_DAY,
            Unit::Month => {
                days_from_civil(index.div_euclid(12), index.rem_euclid(12) + 1, 1) * MICROS_PER_DAY
            }
            Unit::Day => index * MICROS_PER_DAY,
            Unit::Hour => index * MICROS_PER_HOUR,
            Unit::Minute => index * MICROS_PER_MINUTE,
            Unit::Second => index * MICROS_PER_SECOND,
            Unit::Micro => index,
        }
    }
}

impl TimeStep {
    /// The grid of this step: its unit, how many units one step spans, and a unit number on the
    /// grid.
    fn grid(self) -> (Unit, i64, i64) {
        match self {
            TimeStep::Years(n) => (Unit::Year, i64::from(n), 0),
            TimeStep::Months(n) => (Unit::Month, i64::from(n), 0),
            TimeStep::Days(n) => (Unit::Day, i64::from(n), Instant::MIN.0 / MICROS_PER_DAY),
            TimeStep::Hours(n) => (Unit::Hour, i64::from(n), 0),
            TimeStep::Minutes(n) => (Unit::Minute, i64::from(n), 0),
            TimeStep::Seconds(n) => (Unit::Second, i64::from(n), 0),
            TimeStep::Sub(n) => (Unit::Micro, i64::from(n), 0),
        }
    }

    /// The number of the first unit on this step's grid at or after `low`, and how many grid
    /// points lie in `low..=high`.
    fn span(self, low: Instant, high: Instant) -> (i64, i64) {
        let (unit, every, origin) = self.grid();
        let mut first = unit.index(low.0);
        if unit.start(first) < low.0 {
            first += 1;
        }
        first += (origin - first).rem_euclid(every);
        let last = unit.index(high.0);
        let last = last - (last - origin).rem_euclid(every);
        let count = if last >= first {
            (last - first) / every + 1
        } else {
            0
        };
        (first, count)
    }

    /// The `count` instants of this step's grid from unit number `first` on.
    fn instants(self, first: i64, count: i64) -> Vec<Instant> {
        let (unit, every, _) = self.grid();
        (0..count)
            .map(|k| Instant(unit.start(first + k * every)))
            .collect()
    }

    /// The decimals a clock label shows for this step: as many as a sub-second step needs, else
    /// none.
    fn decimals(self) -> usize {
        match self {
            TimeStep::Sub(mut n) => {
                let mut decimals = 6;
                while n.is_multiple_of(10) && decimals > 1 {
                    n /= 10;
                    decimals -= 1;
                }
                decimals
            }
            _ => 0,
        }
    }
}

/// The automatic calendar ticks over `low..=high` (the ends may come in either order), with
/// their step.
///
/// The step is the largest of the step table that puts 5 to 8 ticks in the range. When none
/// does, it is the one whose count is closest to 6.5 among those that put at least 2 there,
/// the larger on a tie. Ticks are the step's grid points inside the range, never beyond it.
/// When no step puts two ticks in the range (`low` equals `high`), the step is one
/// microsecond, with the single tick at `low`.
pub(crate) fn time_ticks(low: Instant, high: Instant) -> (TimeStep, Vec<Instant>) {
    let (low, high) = (low.min(high), low.max(high));
    // The best step outside the wanted counts so far: its distance from 6.5, doubled.
    let mut nearest: Option<(i64, TimeStep, i64, i64)> = None;
    for step in STEPS {
        let (first, count) = step.span(low, high);
        if WANTED_TICKS.contains(&count) {
            return (step, step.instants(first, count));
        }
        let distance = (2 * count - 13).abs();
        if count >= 2 && nearest.is_none_or(|(best, ..)| distance < best) {
            nearest = Some((distance, step, first, count));
        }
    }
    let finest = TimeStep::Sub(1);
    let (step, first, count) = match nearest {
        Some((_, step, first, count)) => (step, first, count),
        None => {
            let (first, count) = finest.span(low, high);
            (finest, first, count)
        }
    };
    (step, step.instants(first, count))
}

/// The steps of the step table coarser than `step`, the nearest first, each with its ticks over
/// `low..=high` (the ends may come in either order); steps that put fewer than two ticks there
/// are left out. Where the labels of the automatic step crowd, an axis takes the first of these
/// whose labels keep apart.
pub(crate) fn coarser_ticks(
    step: TimeStep,
    low: Instant,
    high: Instant,
) -> impl Iterator<Item = (TimeStep, Vec<Instant>)> {
    let (low, high) = (low.min(high), low.max(high));
    let at = STEPS.iter().position(|&s| s == step).unwrap_or(STEPS.len());
    STEPS[..at].iter().rev().filter_map(move |&s| {
        let (first, count) = s.span(low, high);
        (count >= 2).then(|| (s, s.instants(first, count)))
    })
}

/// The step whose labels suit the instants `ticks` when they are not the grid of a step, as
/// explicit tick positions are: years when every one starts a year, months when every one
/// starts a month, days when every one is a midnight, then minutes and seconds, and below a
/// second as many decimals as the finest of them needs.
pub(crate) fn label_step(ticks: &[Instant]) -> TimeStep {
    let all = |test: &dyn Fn(Civil) -> bool| ticks.iter().all(|t| test(t.civil()));
    let midnight = |c: Civil| (c.hour, c.minute, c.second, c.micro) == (0, 0, 0, 0);
    if all(&|c| midnight(c) && (c.month, c.day) == (1, 1)) {
        TimeStep::Years(1)
    } else if all(&|c| midnight(c) && c.day == 1) {
        TimeStep::Months(1)
    } else if all(&midnight) {
        TimeStep::Days(1)
    } else if all(&|c| (c.second, c.micro) == (0, 0)) {
        TimeStep::Minutes(1)
    } else if all(&|c| c.micro == 0) {
        TimeStep::Seconds(1)
    } else {
        let mut every = 100_000;
        while every > 1 && !all(&|c| c.micro % every == 0) {
            every /= 10;
        }
        TimeStep::Sub(every)
    }
}

/// The concise labels of `ticks` on the grid of `step`: each shows the unit that changes, and
/// the first tick and every tick where the next larger unit changes also show that one. Year
/// steps write the year (`2026`); month steps the month (`Feb`, `Jan 2026`); day steps the day
/// (`3`, `Mar 3`); shorter steps the clock (`06:00`, `14:32:15`, `14:32:07.25`), with the date
/// (`Mar 3 06:00`).
pub(crate) fn concise_labels(ticks: &[Instant], step: TimeStep) -> Vec<String> {
    let mut previous: Option<Civil> = None;
    ticks
        .iter()
        .map(|&t| {
            let c = t.civil();
            let month = MONTHS[usize::from(c.month) - 1];
            let label = match step {
                TimeStep::Years(_) => format!("{:04}", c.year),
                TimeStep::Months(_) => {
                    if previous.is_none_or(|p| p.year != c.year) {
                        format!("{month} {:04}", c.year)
                    } else {
                        month.to_owned()
                    }
                }
                TimeStep::Days(_) => {
                    if previous.is_none_or(|p| (p.year, p.month) != (c.year, c.month)) {
                        format!("{month} {}", c.day)
                    } else {
                        c.day.to_string()
                    }
                }
                _ => {
                    let clock = clock(c, step);
                    let day = (c.year, c.month, c.day);
                    if previous.is_none_or(|p| (p.year, p.month, p.day) != day) {
                        format!("{month} {} {clock}", c.day)
                    } else {
                        clock
                    }
                }
            };
            previous = Some(c);
            label
        })
        .collect()
}

/// The labels of explicit ticks `ticks`, in axis order, labelled for `step` (see
/// [`label_step`]), by the concise rule applied to their own sequence: each label shows the
/// unit of the step, and also every larger unit that changes from the tick before it, years
/// included. The first label shows the next larger unit, and the year too when the ticks span
/// more than one year. So `Jan 2 2025, Jan 2 2026` for two ticks a year apart, and `Jan 2, 9,
/// Feb 2` within a year.
pub(crate) fn explicit_labels(ticks: &[Instant], step: TimeStep) -> Vec<String> {
    /// The calendar units a label can show, the largest first.
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Part {
        Clock,
        Day,
        Month,
        Year,
    }
    let level = match step {
        TimeStep::Years(_) => Part::Year,
        TimeStep::Months(_) => Part::Month,
        TimeStep::Days(_) => Part::Day,
        _ => Part::Clock,
    };
    // The largest unit in which `b` differs from `a`, if any.
    let changed = |a: Civil, b: Civil| {
        if a.year != b.year {
            Some(Part::Year)
        } else if a.month != b.month {
            Some(Part::Month)
        } else if a.day != b.day {
            Some(Part::Day)
        } else {
            None
        }
    };
    let civil: Vec<Civil> = ticks.iter().map(|t| t.civil()).collect();
    let next_larger = match level {
        Part::Clock => Part::Day,
        Part::Day => Part::Month,
        Part::Month | Part::Year => Part::Year,
    };
    let span = match (civil.first(), civil.last()) {
        (Some(&first), Some(&last)) => changed(first, last),
        _ => None,
    };
    civil
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let top = match i.checked_sub(1) {
                None => span.map_or(next_larger, |s| s.max(next_larger)),
                Some(j) => changed(civil[j], c).map_or(level, |p| p.max(level)),
            };
            let month = MONTHS[usize::from(c.month) - 1];
            let date = if top == Part::Year {
                format!("{month} {} {:04}", c.day, c.year)
            } else {
                format!("{month} {}", c.day)
            };
            match level {
                Part::Year => format!("{:04}", c.year),
                Part::Month if top == Part::Year => format!("{month} {:04}", c.year),
                Part::Month => month.to_owned(),
                Part::Day if top > Part::Day => date,
                Part::Day => c.day.to_string(),
                Part::Clock if top > Part::Clock => format!("{date} {}", clock(c, step)),
                Part::Clock => clock(c, step),
            }
        })
        .collect()
}

/// The ISO 8601 label of `t`, written down to the unit of `step`: `2026` for years, `2026-03`
/// for months, `2026-03-03` for days, `2026-03-03T06:00` for hours and minutes, and the
/// seconds, with the decimals a sub-second step needs, below that.
pub(crate) fn iso_label(t: Instant, step: TimeStep) -> String {
    let c = t.civil();
    let date = format!("{:04}-{:02}-{:02}", c.year, c.month, c.day);
    match step {
        TimeStep::Years(_) => format!("{:04}", c.year),
        TimeStep::Months(_) => format!("{:04}-{:02}", c.year, c.month),
        TimeStep::Days(_) => date,
        _ => format!("{date}T{}", clock(c, step)),
    }
}

/// The clock of `c` for a step below a day: `06:00` for hours and minutes, `06:00:15` for
/// seconds, and the seconds with the step's decimals (`06:00:15.25`) below that.
fn clock(c: Civil, step: TimeStep) -> String {
    let minutes = format!("{:02}:{:02}", c.hour, c.minute);
    match step {
        TimeStep::Seconds(_) => format!("{minutes}:{:02}", c.second),
        TimeStep::Sub(_) => {
            let decimals = step.decimals();
            let fraction = c.micro / 10u32.pow(6 - decimals as u32);
            format!("{minutes}:{:02}.{fraction:0decimals$}", c.second)
        }
        _ => minutes,
    }
}

#[cfg(test)]
mod tests;
