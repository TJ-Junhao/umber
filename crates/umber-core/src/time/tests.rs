use proptest::prelude::*;

use super::*;

/// The instant at a civil date and time, built independently of [`Instant::civil`].
fn at(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8, micro: u32) -> Instant {
    let days = days_from_civil(i64::from(year), i64::from(month), i64::from(day));
    let clock = i64::from(hour) * MICROS_PER_HOUR
        + i64::from(minute) * MICROS_PER_MINUTE
        + i64::from(second) * MICROS_PER_SECOND
        + i64::from(micro);
    Instant::from_micros(days * MICROS_PER_DAY + clock).unwrap()
}

/// Midnight of a civil date.
fn date(year: i32, month: u8, day: u8) -> Instant {
    at(year, month, day, 0, 0, 0, 0)
}

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8, micro: u32) -> Civil {
    Civil {
        year,
        month,
        day,
        hour,
        minute,
        second,
        micro,
    }
}

/// The step and concise labels of the automatic ticks over `low..=high`.
fn concise(low: Instant, high: Instant) -> (TimeStep, Vec<String>) {
    let (step, ticks) = time_ticks(low, high);
    (step, concise_labels(&ticks, step))
}

/// The ISO labels of the automatic ticks over `low..=high`.
fn iso(low: Instant, high: Instant) -> Vec<String> {
    let (step, ticks) = time_ticks(low, high);
    ticks.iter().map(|&t| iso_label(t, step)).collect()
}

/// The day of the week of `t`, 0 for Monday.
fn weekday(t: Instant) -> i64 {
    // 1970-01-01 was a Thursday.
    (t.micros().div_euclid(MICROS_PER_DAY) + 3).rem_euclid(7)
}

/// `t` moved by `k` steps, with calendar arithmetic written independently of the tick
/// generator: years and months through the civil fields, the rest by their fixed length.
fn advance(t: Instant, step: TimeStep, k: i64) -> i64 {
    let c = t.civil();
    let fixed = |length: i64, n: i64| t.micros() + k * n * length;
    match step {
        TimeStep::Years(n) => {
            let year = i64::from(c.year) + k * i64::from(n);
            days_from_civil(year, 1, 1) * MICROS_PER_DAY
        }
        TimeStep::Months(n) => {
            let index = i64::from(c.year) * 12 + i64::from(c.month) - 1 + k * i64::from(n);
            days_from_civil(index.div_euclid(12), index.rem_euclid(12) + 1, 1) * MICROS_PER_DAY
        }
        TimeStep::Days(n) => fixed(MICROS_PER_DAY, i64::from(n)),
        TimeStep::Hours(n) => fixed(MICROS_PER_HOUR, i64::from(n)),
        TimeStep::Minutes(n) => fixed(MICROS_PER_MINUTE, i64::from(n)),
        TimeStep::Seconds(n) => fixed(MICROS_PER_SECOND, i64::from(n)),
        TimeStep::Sub(n) => fixed(1, i64::from(n)),
    }
}

/// Whether `t` lies on the grid of `step`: every field below the step's unit is zero, and the
/// unit itself is a multiple of the step (days count from 0001-01-01, a Monday).
fn aligned(t: Instant, step: TimeStep) -> bool {
    let c = t.civil();
    let clock = (c.hour, c.minute, c.second, c.micro);
    let midnight = clock == (0, 0, 0, 0);
    match step {
        TimeStep::Years(n) => (c.month, c.day) == (1, 1) && midnight && c.year % n as i32 == 0,
        TimeStep::Months(n) => {
            c.day == 1
                && midnight
                && (i64::from(c.year) * 12 + i64::from(c.month) - 1) % i64::from(n) == 0
        }
        TimeStep::Days(n) => {
            let since_min = (t.micros() - Instant::MIN.micros()) / MICROS_PER_DAY;
            midnight && since_min % i64::from(n) == 0
        }
        TimeStep::Hours(n) => {
            (c.minute, c.second, c.micro) == (0, 0, 0) && c.hour.is_multiple_of(n)
        }
        TimeStep::Minutes(n) => (c.second, c.micro) == (0, 0) && c.minute.is_multiple_of(n),
        TimeStep::Seconds(n) => c.micro == 0 && c.second.is_multiple_of(n),
        TimeStep::Sub(n) => c.micro.is_multiple_of(n),
    }
}

#[test]
fn instant_range_is_years_1_to_9999() {
    assert_eq!(Instant::MIN.micros(), -62_135_596_800_000_000);
    assert_eq!(Instant::MAX.micros(), 253_402_300_799_999_999);
    assert_eq!(Instant::MIN.civil(), civil(1, 1, 1, 0, 0, 0, 0));
    assert_eq!(
        Instant::MAX.civil(),
        civil(9999, 12, 31, 23, 59, 59, 999_999)
    );
    assert_eq!(
        Instant::from_micros(Instant::MIN.micros()).unwrap(),
        Instant::MIN
    );
    assert_eq!(
        Instant::from_micros(Instant::MAX.micros()).unwrap(),
        Instant::MAX
    );
    assert!(Instant::MIN < Instant::MAX);
}

#[test]
fn instants_outside_the_range_are_data_errors() {
    for (us, shown) in [
        (Instant::MAX.micros() + 1, "10000-01-01"),
        (Instant::MIN.micros() - 1, "0000-12-31"),
        (i64::MAX, "294247-01-10"),
        (i64::MIN, "-290308-12-21"),
    ] {
        let err = Instant::from_micros(us).unwrap_err();
        assert!(matches!(err, Error::Data { .. }), "{err:?}");
        assert_eq!(
            err.to_string(),
            format!(
                "Instant.from_micros(): us is {shown}; times must lie between 0001-01-01 and 9999-12-31."
            )
        );
        assert_eq!(err.parameter(), Some("us"));
    }
    // A caller names its own value, such as one element of a data argument.
    let err = range_error("Axes.line()", "x[3]", Instant::MAX.micros() + 1).with_parameter("x");
    assert_eq!(
        err.to_string(),
        "Axes.line(): x[3] is 10000-01-01; times must lie between 0001-01-01 and 9999-12-31."
    );
}

#[test]
fn civil_matches_known_dates() {
    assert_eq!(
        Instant::from_micros(0).unwrap().civil(),
        civil(1970, 1, 1, 0, 0, 0, 0)
    );
    assert_eq!(
        Instant::from_micros(-1).unwrap().civil(),
        civil(1969, 12, 31, 23, 59, 59, 999_999)
    );
    assert_eq!(
        Instant::from_micros(1_772_517_600_000_000).unwrap().civil(),
        civil(2026, 3, 3, 6, 0, 0, 0)
    );
    // A leap day, and a century year that is not a leap year.
    assert_eq!(
        Instant::from_micros(951_827_696_789_012).unwrap().civil(),
        civil(2000, 2, 29, 12, 34, 56, 789_012)
    );
    assert_eq!(
        Instant::from_micros(-2_203_891_200_000_000)
            .unwrap()
            .civil(),
        civil(1900, 3, 1, 0, 0, 0, 0)
    );
    assert_eq!(date(2026, 3, 3).micros(), 1_772_496_000_000_000);
}

proptest! {
    #[test]
    fn civil_round_trip_property(us in Instant::MIN.micros()..=Instant::MAX.micros()) {
        let t = Instant::from_micros(us).unwrap();
        let c = t.civil();
        prop_assert!((1..=9999).contains(&c.year));
        prop_assert!((1..=12).contains(&c.month));
        let leap = c.year % 4 == 0 && (c.year % 100 != 0 || c.year % 400 == 0);
        let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        prop_assert!(c.day >= 1 && c.day <= month_days[usize::from(c.month) - 1]);
        prop_assert!(c.hour < 24 && c.minute < 60 && c.second < 60 && c.micro < 1_000_000);
        let back = at(c.year, c.month, c.day, c.hour, c.minute, c.second, c.micro);
        prop_assert_eq!(back, t);
    }

    #[test]
    fn consecutive_days_follow_the_calendar(day in -719_162i64..2_932_896) {
        // Midnight of a day and of the next: the civil date steps by one day, rolling over
        // month and year ends and February 29 in leap years only.
        let today = Instant::from_micros(day * MICROS_PER_DAY).unwrap().civil();
        let next = Instant::from_micros((day + 1) * MICROS_PER_DAY).unwrap().civil();
        let leap = today.year % 4 == 0 && (today.year % 100 != 0 || today.year % 400 == 0);
        let last = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
            [usize::from(today.month) - 1];
        let expected = if today.day < last {
            (today.year, today.month, today.day + 1)
        } else if today.month < 12 {
            (today.year, today.month + 1, 1)
        } else {
            (today.year + 1, 1, 1)
        };
        prop_assert_eq!((next.year, next.month, next.day), expected);
    }

    #[test]
    fn ticks_across_spans_property(
        exponent in 3.0f64..16.2,
        place in 0.0f64..1.0,
    ) {
        // Spans from 1 ms to 500 years, placed anywhere in the supported range.
        let span = 10f64.powf(exponent).min(500.0 * 365.25 * 86_400e6) as i64;
        let room = Instant::MAX.micros() - Instant::MIN.micros() - span;
        let low = Instant::from_micros(Instant::MIN.micros() + (place * room as f64) as i64).unwrap();
        let high = Instant::from_micros(low.micros() + span).unwrap();
        let (step, ticks) = time_ticks(low, high);
        prop_assert!((2..=10).contains(&ticks.len()), "{} ticks at {:?} over {:?}", ticks.len(), step, (low, high));
        prop_assert!(ticks.windows(2).all(|w| w[0] < w[1]));
        prop_assert!(ticks.iter().all(|&t| low <= t && t <= high));
        prop_assert!(ticks.iter().all(|&t| aligned(t, step)), "{:?} {:?}", step, ticks);
        // Every grid point of the step inside the range is a tick.
        for w in ticks.windows(2) {
            prop_assert_eq!(advance(w[0], step, 1), w[1].micros());
        }
        prop_assert!(advance(ticks[0], step, -1) < low.micros());
        prop_assert!(advance(ticks[ticks.len() - 1], step, 1) > high.micros());
        // Neighbouring concise labels differ (a day number may come back in the next month,
        // after a tick that names the month), and ISO labels are all distinct.
        let labels = concise_labels(&ticks, step);
        prop_assert_eq!(labels.len(), ticks.len());
        prop_assert!(labels.windows(2).all(|w| w[0] != w[1]), "{:?}", labels);
        let isos: std::collections::BTreeSet<String> =
            ticks.iter().map(|&t| iso_label(t, step)).collect();
        prop_assert_eq!(isos.len(), ticks.len());
    }

    #[test]
    fn weekly_ticks_fall_on_mondays(day in -719_162i64..2_932_850, hour in 0i64..24) {
        // Six weeks give six or seven weekly ticks, inside the five to eight wanted.
        let low = Instant::from_micros(day * MICROS_PER_DAY + hour * MICROS_PER_HOUR).unwrap();
        let high = Instant::from_micros(low.micros() + 42 * MICROS_PER_DAY).unwrap();
        let (step, ticks) = time_ticks(low, high);
        prop_assert_eq!(step, TimeStep::Days(7));
        prop_assert!(ticks.iter().all(|&t| weekday(t) == 0 && t.civil().hour == 0));
    }
}

#[test]
fn weekly_ticks_on_known_mondays() {
    let (step, labels) = concise(date(2026, 1, 1), date(2026, 2, 12));
    assert_eq!(step, TimeStep::Days(7));
    assert_eq!(labels, ["Jan 5", "12", "19", "26", "Feb 2", "9"]);
    // 0001-01-01 was a Monday.
    assert_eq!(weekday(Instant::MIN), 0);
}

#[test]
fn step_is_the_largest_with_five_to_eight_ticks() {
    // Six hours of data: hourly ticks (six) win over three-hourly ones (two).
    let (step, ticks) = time_ticks(at(2026, 3, 3, 4, 40, 0, 0), at(2026, 3, 3, 10, 40, 0, 0));
    assert_eq!(step, TimeStep::Hours(1));
    assert_eq!(ticks.len(), 6);
    // Exactly eight ticks still count: the quarters from 2024-01-01 to 2025-10-01.
    let (step, ticks) = time_ticks(date(2024, 1, 1), date(2025, 10, 1));
    assert_eq!(step, TimeStep::Months(3));
    assert_eq!(ticks.len(), 8);
    // Ticks are inside the range, never snapped outward.
    assert_eq!(ticks[0], date(2024, 1, 1));
    assert_eq!(ticks[7], date(2025, 10, 1));
}

#[test]
fn without_five_to_eight_the_count_nearest_six_and_a_half_wins() {
    // Four months: monthly ticks (four) are nearer 6.5 than weekly ones (seventeen); the
    // quarterly step gives one tick and does not count.
    let (step, labels) = concise(date(2025, 11, 12), date(2026, 3, 12));
    assert_eq!(step, TimeStep::Months(1));
    assert_eq!(labels, ["Dec 2025", "Jan 2026", "Feb", "Mar"]);
    // Eight hours across midnight: nine hourly ticks are nearer than three 3-hourly ones.
    let (step, labels) = concise(at(2026, 3, 3, 20, 0, 0, 0), at(2026, 3, 4, 4, 0, 0, 0));
    assert_eq!(step, TimeStep::Hours(1));
    assert_eq!(
        labels,
        [
            "Mar 3 20:00",
            "21:00",
            "22:00",
            "23:00",
            "Mar 4 00:00",
            "01:00",
            "02:00",
            "03:00",
            "04:00"
        ]
    );
    // A tie goes to the larger step: four 2000-year ticks against nine 1000-year ones.
    let (step, labels) = concise(Instant::MIN, Instant::MAX);
    assert_eq!(step, TimeStep::Years(2000));
    assert_eq!(labels, ["2000", "4000", "6000", "8000"]);
}

#[test]
fn a_single_instant_gets_one_tick() {
    let t = at(2026, 3, 3, 6, 0, 0, 250);
    assert_eq!(time_ticks(t, t), (TimeStep::Sub(1), vec![t]));
    // The ends may come in either order.
    let (a, b) = (date(2026, 1, 1), date(2026, 2, 12));
    assert_eq!(time_ticks(b, a), time_ticks(a, b));
}

#[test]
fn extreme_years() {
    // The first year of the calendar: quarterly ticks from 0001-01-01.
    let first_year_end = Instant::from_micros(date(2, 1, 1).micros() - 1).unwrap();
    let (step, labels) = concise(Instant::MIN, first_year_end);
    assert_eq!(step, TimeStep::Months(3));
    assert_eq!(labels, ["Jan 0001", "Apr", "Jul", "Oct"]);
    assert_eq!(
        iso(Instant::MIN, first_year_end),
        ["0001-01", "0001-04", "0001-07", "0001-10"]
    );
    // The last year, up to its last microsecond.
    let (step, labels) = concise(date(9999, 1, 1), Instant::MAX);
    assert_eq!(step, TimeStep::Months(3));
    assert_eq!(labels, ["Jan 9999", "Apr", "Jul", "Oct"]);
    // The last second: fifths of a second up to 23:59:59.8.
    let last_second = Instant::from_micros(Instant::MAX.micros() - 1_000_000).unwrap();
    let (step, labels) = concise(last_second, Instant::MAX);
    assert_eq!(step, TimeStep::Sub(200_000));
    assert_eq!(
        labels,
        [
            "Dec 31 23:59:59.0",
            "23:59:59.2",
            "23:59:59.4",
            "23:59:59.6",
            "23:59:59.8"
        ]
    );
    assert_eq!(iso(last_second, Instant::MAX)[4], "9999-12-31T23:59:59.8");
    // The first day: every 6 hours from midnight to the next midnight.
    let (step, labels) = concise(Instant::MIN, date(1, 1, 2));
    assert_eq!(step, TimeStep::Hours(6));
    assert_eq!(
        labels,
        ["Jan 1 00:00", "06:00", "12:00", "18:00", "Jan 2 00:00"]
    );
    // Year ticks are multiples of the step; there is no year 0, so the first is 0010.
    let (step, labels) = concise(Instant::MIN, date(60, 1, 1));
    assert_eq!(step, TimeStep::Years(10));
    assert_eq!(labels, ["0010", "0020", "0030", "0040", "0050", "0060"]);
}

#[test]
fn concise_labels_examples() {
    // 90 seconds: every 15 seconds; the first tick carries the date.
    let (step, labels) = concise(at(2026, 3, 3, 14, 32, 7, 0), at(2026, 3, 3, 14, 33, 37, 0));
    assert_eq!(step, TimeStep::Seconds(15));
    assert_eq!(
        labels,
        [
            "Mar 3 14:32:15",
            "14:32:30",
            "14:32:45",
            "14:33:00",
            "14:33:15",
            "14:33:30"
        ]
    );
    // 6 hours: hourly.
    let (step, labels) = concise(at(2026, 3, 3, 4, 40, 0, 0), at(2026, 3, 3, 10, 40, 0, 0));
    assert_eq!(step, TimeStep::Hours(1));
    assert_eq!(
        labels,
        ["Mar 3 05:00", "06:00", "07:00", "08:00", "09:00", "10:00"]
    );
    // 10 days: every second day; the month shows on the first tick and where it changes.
    let (step, labels) = concise(at(2026, 2, 24, 8, 0, 0, 0), at(2026, 3, 6, 8, 0, 0, 0));
    assert_eq!(step, TimeStep::Days(2));
    assert_eq!(labels, ["Feb 26", "28", "Mar 2", "4", "6"]);
    // 4 months: monthly; the year shows on the first tick and where it changes.
    let (step, labels) = concise(date(2025, 11, 12), date(2026, 3, 12));
    assert_eq!(step, TimeStep::Months(1));
    assert_eq!(labels, ["Dec 2025", "Jan 2026", "Feb", "Mar"]);
    // 3 years: every 6 months.
    let (step, labels) = concise(date(2023, 5, 20), date(2026, 5, 20));
    assert_eq!(step, TimeStep::Months(6));
    assert_eq!(
        labels,
        ["Jul 2023", "Jan 2024", "Jul", "Jan 2025", "Jul", "Jan 2026"]
    );
    // 80 years: every 10 years.
    let (step, labels) = concise(date(1946, 4, 1), date(2026, 4, 1));
    assert_eq!(step, TimeStep::Years(10));
    assert_eq!(
        labels,
        [
            "1950", "1960", "1970", "1980", "1990", "2000", "2010", "2020"
        ]
    );
    // Below a second: the clock with as many decimals as the step needs.
    let (step, labels) = concise(
        at(2026, 3, 3, 14, 32, 7, 0),
        at(2026, 3, 3, 14, 32, 7, 900_000),
    );
    assert_eq!(step, TimeStep::Sub(200_000));
    assert_eq!(
        labels,
        [
            "Mar 3 14:32:07.0",
            "14:32:07.2",
            "14:32:07.4",
            "14:32:07.6",
            "14:32:07.8"
        ]
    );
    let (step, labels) = concise(at(2026, 3, 3, 14, 32, 7, 0), at(2026, 3, 3, 14, 32, 7, 30));
    assert_eq!(step, TimeStep::Sub(5));
    assert_eq!(labels[1], "14:32:07.000005");
}

#[test]
fn iso_labels_truncate_to_step() {
    let t = at(2026, 3, 3, 6, 5, 15, 0);
    let cases = [
        (TimeStep::Years(10), "2026"),
        (TimeStep::Months(1), "2026-03"),
        (TimeStep::Days(7), "2026-03-03"),
        (TimeStep::Hours(6), "2026-03-03T06:05"),
        (TimeStep::Minutes(5), "2026-03-03T06:05"),
        (TimeStep::Seconds(15), "2026-03-03T06:05:15"),
        (TimeStep::Sub(500_000), "2026-03-03T06:05:15.0"),
        (TimeStep::Sub(20_000), "2026-03-03T06:05:15.00"),
        (TimeStep::Sub(1), "2026-03-03T06:05:15.000000"),
    ];
    for (step, label) in cases {
        assert_eq!(iso_label(t, step), label, "{step:?}");
    }
    assert_eq!(iso_label(Instant::MIN, TimeStep::Days(1)), "0001-01-01");
    // Over a span, every label has the same form.
    assert_eq!(
        iso(date(2025, 11, 12), date(2026, 3, 12)),
        ["2025-12", "2026-01", "2026-02", "2026-03"]
    );
    assert_eq!(
        iso(at(2026, 3, 3, 4, 40, 0, 0), at(2026, 3, 3, 10, 40, 0, 0)),
        [
            "2026-03-03T05:00",
            "2026-03-03T06:00",
            "2026-03-03T07:00",
            "2026-03-03T08:00",
            "2026-03-03T09:00",
            "2026-03-03T10:00"
        ]
    );
}
