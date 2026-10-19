//! Text and annotations at data positions in every theme: alignment, the eight sides of an
//! offset, arrows, and points on category and time axes.

use umber_core::{
    Align, AnnotateOptions, AxisData, AxisValue, BarOptions, FontLibrary, Instant, LineOptions,
    ScaleKind, ScatterOptions, SizeSpec, TextOptions,
};

use super::super::{Output, axes, emit_scene, figure, linear, themes};

fn number(v: f64) -> AxisValue {
    AxisValue::Number(v)
}

/// Emits text alignment, the eight offset directions, time-series annotations with and without
/// arrows, and an annotation at a category.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        let mut ax = axes(&mut f);
        ax.title("Text alignment at a point")
            .xlim(0.0, 10.0)
            .unwrap()
            .ylim(0.0, 10.0)
            .unwrap()
            .scatter(
                AxisData::Numbers(vec![2.0, 5.0, 8.0]),
                &[7.5, 5.0, 2.5],
                ScatterOptions::default(),
            )
            .unwrap();
        for (text, (x, y), align) in [
            ("LEFT: left edge", (2.0, 7.5), Align::Left),
            ("CENTER: centre", (5.0, 5.0), Align::Center),
            ("RIGHT: right edge", (8.0, 2.5), Align::Right),
        ] {
            let options = TextOptions {
                align,
                ..TextOptions::default()
            };
            ax.text(text, (number(x), number(y)), options).unwrap();
        }
        emit_scene(
            &format!("annotate-text-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        let mut ax = axes(&mut f);
        let points = [
            ("N", (1.0, 1.0), (0.0, 22.0)),
            ("NE", (2.0, 1.0), (18.0, 18.0)),
            ("E", (3.0, 1.0), (22.0, 0.0)),
            ("SE", (4.0, 1.0), (18.0, -18.0)),
            ("S", (1.0, 2.0), (0.0, -22.0)),
            ("SW", (2.0, 2.0), (-18.0, -18.0)),
            ("W", (3.0, 2.0), (-22.0, 0.0)),
            ("NW", (4.0, 2.0), (-18.0, 18.0)),
        ];
        let (xs, ys): (Vec<f64>, Vec<f64>) = points.iter().map(|(_, at, _)| *at).unzip();
        ax.title("Eight directions")
            .xlim(0.0, 5.0)
            .unwrap()
            .ylim(0.0, 3.0)
            .unwrap()
            .scatter(AxisData::Numbers(xs), &ys, ScatterOptions::default())
            .unwrap();
        for (text, (x, y), offset) in points {
            let options = AnnotateOptions {
                offset,
                ..AnnotateOptions::default()
            };
            ax.annotate(text, (number(x), number(y)), options).unwrap();
        }
        emit_scene(
            &format!("annotate-directions-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let day =
            |d: i64| Instant::from_micros(1_772_323_200_000_000 + d * 86_400_000_000).unwrap();
        let values = [
            21.0, 22.0, 24.0, 32.0, 25.0, 23.0, 15.0, 20.0, 23.0, 24.0, 27.0, 25.0,
        ];
        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Time, linear());
        let mut ax = axes(&mut f);
        ax.title("Peak and dip")
            .ylabel("Requests / s")
            .ylim(5.0, 40.0)
            .unwrap()
            .line(
                AxisData::Times((0..12).map(|d| Some(day(d))).collect()),
                &values,
                LineOptions::default(),
            )
            .unwrap()
            .annotate(
                "Peak",
                (AxisValue::Time(day(3)), number(32.0)),
                AnnotateOptions {
                    offset: (24.0, 18.0),
                    ..AnnotateOptions::default()
                },
            )
            .unwrap()
            .annotate(
                "Dip",
                (AxisValue::Time(day(6)), number(15.0)),
                AnnotateOptions {
                    offset: (18.0, -22.0),
                    arrow: false,
                    ..AnnotateOptions::default()
                },
            )
            .unwrap()
            .annotate(
                "Rise",
                (AxisValue::Time(day(9)), number(24.0)),
                AnnotateOptions {
                    offset: (-25.0, 25.0),
                    size: SizeSpec::times(1.2).unwrap(),
                    ..AnnotateOptions::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("annotate-arrows-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme, ScaleKind::Category, linear());
        let months = ["Jan", "Feb", "Mar", "Apr", "May"]
            .map(String::from)
            .to_vec();
        axes(&mut f)
            .title("Category coordinates")
            .ylabel("Orders")
            .ylim(0.0, 65.0)
            .unwrap()
            .bar(
                months,
                &[24.0, 31.0, 42.0, 37.0, 46.0],
                BarOptions::default(),
            )
            .unwrap()
            .annotate(
                "March launch",
                (AxisValue::Category("Mar".into()), number(42.0)),
                AnnotateOptions {
                    offset: (25.0, 24.0),
                    ..AnnotateOptions::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("annotate-category-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
}
