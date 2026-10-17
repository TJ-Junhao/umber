//! Explicit baseline visibility on lines and bars, and the category-axis error.

use super::super::{Output, axes, emit_scene, figure, linear, numbers, record};
use std::fmt::Write as _;
use umber_core::{BaselineVisibility, FontLibrary, ScaleKind, Theme};

/// Emits the two visibility overrides and records the call-time usage error.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let mut line = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    axes(&mut line)
        .line(
            numbers(vec![0.0, 1.0, 2.0]),
            &[1.0, -1.0, 2.0],
            Default::default(),
        )
        .unwrap()
        .baseline(BaselineVisibility::Shown)
        .unwrap();
    emit_scene("baseline-line-shown", &line.to_scene(lib).unwrap(), sink);

    let mut bar = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    axes(&mut bar)
        .bar(
            vec!["A".into(), "B".into(), "C".into()],
            &[1.0, -1.0, 2.0],
            Default::default(),
        )
        .unwrap()
        .baseline(BaselineVisibility::Hidden)
        .unwrap();
    emit_scene("baseline-bar-hidden", &bar.to_scene(lib).unwrap(), sink);

    let mut f = figure(
        6.0,
        4.0,
        Theme::pigment(),
        ScaleKind::Category,
        ScaleKind::Category,
    );
    let mut out = String::new();
    for (label, setting) in [
        ("Auto", BaselineVisibility::Auto),
        ("Hidden", BaselineVisibility::Hidden),
    ] {
        axes(&mut f).baseline(setting).unwrap();
        let before = f.clone();
        let err = axes(&mut f)
            .baseline(BaselineVisibility::Shown)
            .unwrap_err();
        assert_eq!(f, before);
        let _ = writeln!(
            out,
            "shown after {label}: {}\nparameter={:?} hint={:?}\nstate unchanged=true",
            record::error(&err),
            err.parameter(),
            err.hint(),
        );
    }
    sink(Output {
        kind: "errors",
        file: "baseline.txt".into(),
        bytes: out.into_bytes(),
    });
}
