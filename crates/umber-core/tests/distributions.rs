//! Histogram variants, ECDFs and KDE lines through the public API: options, stored values,
//! errors, and that a failed call leaves the figure unchanged.

#![allow(clippy::unwrap_used)]

use umber_core::{
    Axes, AxesMut, AxisData, Bandwidth, Bins, ColorSpec, Dash, EcdfOptions, Error, Figure,
    GridIndex, GroupedHistOptions, GroupedKdeOptions, HistOptions, HistStat, HistStyle, Instant,
    KdeOptions, LogBase, Mark, ScaleKind, Series, SizeSpec, Theme,
};

const LIN: ScaleKind = ScaleKind::Linear;
const LOG: ScaleKind = ScaleKind::Log { base: LogBase::TEN };

fn figure(x: ScaleKind, y: ScaleKind) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure.add_axes(x, y).unwrap();
    figure
}

/// The one axes of `figure`, for changing.
fn axes(figure: &mut Figure) -> AxesMut<'_> {
    figure
        .axes_at_mut(GridIndex::Auto, GridIndex::Auto)
        .unwrap()
}

/// The one axes of `figure`.
fn panel(figure: &Figure) -> &Axes {
    figure.axes_at(GridIndex::Auto, GridIndex::Auto).unwrap()
}

fn series(label: &str, values: &[f64]) -> Series {
    Series {
        label: label.into(),
        values: values.to_vec(),
        color: ColorSpec::AUTO,
    }
}

/// The error of `call` on a figure of `x` and `y` axes that already holds one line, with the
/// figure checked to be unchanged afterwards.
#[track_caller]
fn refused(
    (x, y): (ScaleKind, ScaleKind),
    call: impl FnOnce(&mut Figure) -> umber_core::Result<()>,
) -> Error {
    let mut f = figure(x, y);
    axes(&mut f).title("Kept");
    let before = f.clone();
    let err = call(&mut f).unwrap_err();
    assert_eq!(f, before);
    err
}

#[track_caller]
fn assert_error(err: &Error, display: &str, parameter: Option<&str>, hint: Option<&str>) {
    assert_eq!(err.to_string(), display);
    assert_eq!(err.parameter(), parameter);
    assert_eq!(err.hint(), hint);
}

#[test]
fn hist_options_default_to_the_plain_histogram() {
    let options = HistOptions::default();
    assert_eq!(options.bins, Bins::Count(30));
    assert_eq!(options.style, HistStyle::Bars);
    assert!(!options.cumulative);
    assert_eq!(options.stat, HistStat::Count);
    let grouped = GroupedHistOptions::default();
    assert_eq!(
        (
            grouped.bins,
            grouped.style,
            grouped.cumulative,
            grouped.stat
        ),
        (Bins::Count(30), HistStyle::Step, false, HistStat::Count)
    );
}

#[test]
fn hist_keeps_its_edges_and_options() {
    let mut f = figure(LIN, LIN);
    let options = HistOptions {
        bins: Bins::Edges(vec![0.0, 1.0, 4.0]),
        style: HistStyle::Step,
        cumulative: true,
        stat: HistStat::Fraction,
        label: "Sizes".into(),
        color: ColorSpec::AUTO,
    };
    axes(&mut f)
        .hist(&[0.5, 1.0, 4.0], options.clone())
        .unwrap();
    let [Mark::Hist(hist)] = panel(&f).marks() else {
        panic!("expected one histogram");
    };
    assert_eq!(hist.options(), options);
    assert_eq!(hist.values(), [0.5, 1.0, 4.0]);
}

#[test]
fn values_outside_edges_rejected() {
    let edges = HistOptions {
        bins: Bins::Edges(vec![0.0, 2.5, 10.0]),
        ..HistOptions::default()
    };
    let err = refused((LIN, LIN), |f| {
        axes(f)
            .hist(&[-1.0, 0.0, f64::NAN, 10.0, 10.5, 99.0], edges.clone())
            .map(drop)
    });
    assert!(matches!(err, Error::Data { .. }), "{err:?}");
    assert_error(
        &err,
        "Axes.hist(): values has 3 finite values outside the bin edges, which span 0 to 10; \
         every finite value must lie within the edges.",
        Some("values"),
        Some("widen the edges or filter the values first."),
    );
}

#[test]
fn bad_edges_are_data_errors_naming_bins() {
    let cases: [(Vec<f64>, &str, &str); 5] = [
        (
            vec![1.0],
            "Axes.hist(): bins has 1 edge; bin edges must number at least 2.",
            "pass at least two increasing edges, or a number of bins.",
        ),
        (
            vec![0.0, 1.0, 1.0],
            "Axes.hist(): bins[2] is 1, not above bins[1], 1; bin edges must be strictly \
             increasing.",
            "sort the edges and remove repeated ones.",
        ),
        (
            vec![0.0, f64::NAN],
            "Axes.hist(): bins[1] is NaN; bin edges must be finite.",
            "pass at least two increasing edges, or a number of bins.",
        ),
        (
            vec![f64::NEG_INFINITY, 0.0],
            "Axes.hist(): bins[0] is -inf; bin edges must be finite.",
            "pass at least two increasing edges, or a number of bins.",
        ),
        (
            vec![1.0, 1.0 + 1e-14],
            "Axes.hist(): bins span 1 to 1.00000000000001, which is narrower than a linear axis \
             can show; bin edges must span more.",
            "widen the edges.",
        ),
    ];
    for (edges, display, hint) in cases {
        let options = HistOptions {
            bins: Bins::Edges(edges),
            ..HistOptions::default()
        };
        let err = refused((LIN, LIN), |f| axes(f).hist(&[1.0], options).map(drop));
        assert!(matches!(err, Error::Data { .. }), "{err:?}");
        assert_error(&err, display, Some("bins"), Some(hint));
    }
}

#[test]
fn cumulative_density_is_a_usage_error() {
    for edges in [Bins::Count(4), Bins::Edges(vec![0.0, 1.0])] {
        let options = HistOptions {
            bins: edges,
            cumulative: true,
            stat: HistStat::Density,
            ..HistOptions::default()
        };
        let err = refused((LIN, LIN), |f| axes(f).hist(&[0.5], options).map(drop));
        assert!(matches!(err, Error::Usage { .. }), "{err:?}");
        assert_error(
            &err,
            "Axes.hist(): cumulative is True with Stat.DENSITY; a cumulative histogram shows \
             counts or fractions.",
            Some("cumulative"),
            Some("use Stat.FRACTION, or ecdf()."),
        );
    }
}

#[test]
fn grouped_hist_shares_bins_and_refuses_bars() {
    let mut f = figure(LIN, LIN);
    let groups = vec![
        series("north", &[1.0, 2.0]),
        series("south", &[2.5, f64::NAN]),
    ];
    axes(&mut f)
        .grouped_hist(groups.clone(), GroupedHistOptions::default())
        .unwrap();
    let [Mark::GroupedHist(mark)] = panel(&f).marks() else {
        panic!("expected one grouped histogram");
    };
    let labels: Vec<&str> = mark.series().map(|s| s.label()).collect();
    assert_eq!(labels, ["north", "south"]);
    assert_eq!(mark.series().nth(1).unwrap().values()[0], 2.5);
    assert_eq!(mark.options(), GroupedHistOptions::default());

    let bars = GroupedHistOptions {
        style: HistStyle::Bars,
        ..GroupedHistOptions::default()
    };
    let err = refused((LIN, LIN), |f| {
        axes(f).grouped_hist(groups.clone(), bars).map(drop)
    });
    assert!(matches!(err, Error::Usage { .. }), "{err:?}");
    assert_error(
        &err,
        "Axes.hist(): style is Hist.BARS with groups; bars of several groups would hide each \
         other.",
        Some("style"),
        Some("use Hist.STACKED or Hist.STEP."),
    );
    let err = refused((LIN, LIN), |f| {
        axes(f)
            .grouped_hist(Vec::new(), GroupedHistOptions::default())
            .map(drop)
    });
    assert_error(
        &err,
        "Axes.hist(): series is empty; pass at least one series.",
        Some("series"),
        None,
    );
    let err = refused((LIN, LIN), |f| {
        axes(f)
            .grouped_hist(
                vec![series("a", &[1.0]), series("b", &[f64::INFINITY])],
                GroupedHistOptions::default(),
            )
            .map(drop)
    });
    assert_error(
        &err,
        "Axes.hist(): series[1].values[0] is inf; values must be finite.",
        Some("values"),
        Some("a histogram ignores NaN, so use it to leave a value out."),
    );
}

#[test]
fn ecdf_keeps_its_values_and_options() {
    let mut f = figure(LIN, LIN);
    let options = EcdfOptions {
        complementary: true,
        label: "Load".into(),
        color: ColorSpec::AUTO,
        dash: Dash::Dashed,
        width: SizeSpec::pt(2.0).unwrap(),
    };
    let values = AxisData::Numbers(vec![3.0, f64::NAN, 1.0]);
    axes(&mut f).ecdf(values.clone(), options.clone()).unwrap();
    let [Mark::Ecdf(ecdf)] = panel(&f).marks() else {
        panic!("expected one ECDF");
    };
    assert_eq!(ecdf.options(), &options);
    assert!(matches!(ecdf.values(), AxisData::Numbers(v) if v.len() == 3 && v[2] == 1.0));
    assert!(!EcdfOptions::default().complementary);
}

#[test]
fn ecdf_takes_times_on_a_time_axis() {
    let mut f = figure(ScaleKind::Time, LIN);
    let day = |d: i64| Some(Instant::from_micros(d * 86_400_000_000).unwrap());
    axes(&mut f)
        .ecdf(
            AxisData::Times(vec![day(3), None, day(1)]),
            EcdfOptions::default(),
        )
        .unwrap();
    assert_eq!(panel(&f).marks().len(), 1);
    let err = refused((LIN, LIN), |f| {
        axes(f)
            .ecdf(AxisData::Times(vec![day(1)]), EcdfOptions::default())
            .map(drop)
    });
    assert!(matches!(err, Error::Data { .. }), "{err:?}");
}

#[test]
fn ecdf_errors() {
    let err = refused((LIN, LIN), |f| {
        axes(f)
            .ecdf(AxisData::Numbers(vec![f64::NAN]), EcdfOptions::default())
            .map(drop)
    });
    assert_error(
        &err,
        "Axes.ecdf(): values has no finite value; an ECDF needs at least one.",
        Some("values"),
        Some("pass at least one value that is not NaN."),
    );
    let err = refused((LIN, LOG), |f| {
        axes(f)
            .ecdf(AxisData::Numbers(vec![1.0]), EcdfOptions::default())
            .map(drop)
    });
    assert!(matches!(err, Error::Usage { .. }), "{err:?}");
    assert_eq!(err.parameter(), Some("y"));
    let err = refused((LOG, LIN), |f| {
        axes(f)
            .ecdf(AxisData::Numbers(vec![1.0, -2.0]), EcdfOptions::default())
            .map(drop)
    });
    assert_error(
        &err,
        "Axes.ecdf(): values[1] is -2 on a log axis; values must be positive.",
        Some("values"),
        Some("use a linear axis, or remove values that are not positive."),
    );
    let err = refused((LIN, LIN), |f| {
        axes(f)
            .ecdf(
                AxisData::Categories(vec!["a".into()]),
                EcdfOptions::default(),
            )
            .map(drop)
    });
    assert!(matches!(err, Error::Data { .. }), "{err:?}");
}

#[test]
fn kde_keeps_its_values_and_reports_its_bandwidth() {
    let mut f = figure(LIN, LIN);
    let values = [1.0, 2.0, 4.0, f64::NAN];
    axes(&mut f)
        .kde(&values, KdeOptions::default())
        .unwrap()
        .kde(
            &values,
            KdeOptions {
                bandwidth: Bandwidth::Fixed(0.25),
                ..KdeOptions::default()
            },
        )
        .unwrap();
    let [Mark::Kde(scott), Mark::Kde(fixed)] = panel(&f).marks() else {
        panic!("expected two KDE lines");
    };
    // Scott's rule over the finite values: the sample standard deviation times n^(-1/5).
    let n = 3.0_f64;
    let mean = 7.0_f64 / 3.0;
    let sd = (((1.0 - mean).powi(2) + (2.0 - mean).powi(2) + (4.0 - mean).powi(2)) / 2.0).sqrt();
    let expected = sd * n.powf(-0.2);
    assert!((scott.bandwidth() / expected - 1.0).abs() < 1e-12);
    assert_eq!(fixed.bandwidth(), 0.25);
    assert_eq!(scott.values().len(), 4);
    assert_eq!(KdeOptions::default().bandwidth, Bandwidth::Scott);
}

#[test]
fn kde_log_axis_rejected() {
    let err = refused((LOG, LIN), |f| {
        axes(f).kde(&[1.0, 2.0], KdeOptions::default()).map(drop)
    });
    assert!(matches!(err, Error::Usage { .. }), "{err:?}");
    assert_error(
        &err,
        "Axes.kde(): the x axis is a log axis, which density curves do not support; use a \
         linear x axis.",
        None,
        Some("pass x=Scale.linear() to Figure.add_axes()."),
    );
    let err = refused((ScaleKind::Time, LIN), |f| {
        axes(f).kde(&[1.0, 2.0], KdeOptions::default()).map(drop)
    });
    assert!(matches!(err, Error::Usage { .. }), "{err:?}");
    let err = refused((LIN, LOG), |f| {
        axes(f).kde(&[1.0, 2.0], KdeOptions::default()).map(drop)
    });
    assert!(matches!(err, Error::Usage { .. }), "{err:?}");
    assert_eq!(err.parameter(), Some("y"));
}

#[test]
fn kde_errors() {
    let err = refused((LIN, LIN), |f| {
        axes(f).kde(&[2.0, 2.0], KdeOptions::default()).map(drop)
    });
    assert_error(
        &err,
        "Axes.kde(): values has 1 distinct finite value; the Scott bandwidth needs at least 2.",
        Some("values"),
        Some("pass bandwidth= in data units."),
    );
    for (h, shown) in [(0.0, "0"), (-1.0, "-1"), (f64::INFINITY, "inf")] {
        let options = KdeOptions {
            bandwidth: Bandwidth::Fixed(h),
            ..KdeOptions::default()
        };
        let err = refused((LIN, LIN), |f| axes(f).kde(&[1.0, 2.0], options).map(drop));
        assert!(matches!(err, Error::Style { .. }), "{err:?}");
        assert_error(
            &err,
            &format!(
                "Axes.kde(): bandwidth is {shown}; bandwidth must be a finite number greater \
                 than 0."
            ),
            Some("bandwidth"),
            Some("pass a bandwidth in data units greater than 0, or ub.AUTO."),
        );
    }
    let narrow = KdeOptions {
        bandwidth: Bandwidth::Fixed(1e-305),
        ..KdeOptions::default()
    };
    let err = refused((LIN, LIN), |f| axes(f).kde(&[0.0], narrow).map(drop));
    assert!(matches!(err, Error::Data { .. }), "{err:?}");
    assert_eq!(err.parameter(), Some("bandwidth"));
}

#[test]
fn grouped_kde_adds_one_line_per_series_or_none() {
    let mut f = figure(LIN, LIN);
    let groups = vec![series("a", &[1.0, 2.0, 3.0]), series("b", &[5.0, 7.0])];
    axes(&mut f)
        .grouped_kde(groups, GroupedKdeOptions::default())
        .unwrap();
    let marks = panel(&f).marks();
    assert_eq!(marks.len(), 2);
    let [Mark::Kde(a), Mark::Kde(b)] = marks else {
        panic!("expected two KDE lines");
    };
    assert_eq!(
        (a.options().label.as_str(), b.options().label.as_str()),
        ("a", "b")
    );
    // The second series has one distinct value too few for Scott's rule: nothing is added.
    let err = refused((LIN, LIN), |f| {
        axes(f)
            .grouped_kde(
                vec![series("a", &[1.0, 2.0]), series("b", &[3.0])],
                GroupedKdeOptions::default(),
            )
            .map(drop)
    });
    assert_error(
        &err,
        "Axes.kde(): series[1].values has 1 distinct finite value; the Scott bandwidth needs at \
         least 2.",
        Some("values"),
        Some("pass bandwidth= in data units."),
    );
}
