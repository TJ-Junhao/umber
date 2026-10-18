//! Event rows on numeric, logarithmic and time axes in every theme.

use umber_core::{AxisData, EventsOptions, FontLibrary, Instant, ScaleKind, SizeSpec};

use super::super::{Output, axes, emit_scene, figure, linear, log, themes};

/// Emits event rows, including an empty row and missing positions.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (kind, x) in [
            ("numeric", linear()),
            ("log", log(10.0)),
            ("time", ScaleKind::Time),
        ] {
            let mut f = figure(6.0, 4.0, theme.clone(), x, ScaleKind::Category);
            let positions = [vec![1.0, 3.0, f64::NAN, 7.0], vec![], vec![2.0, 5.0, 9.0]]
                .into_iter()
                .map(|group| {
                    if x == ScaleKind::Time {
                        AxisData::Times(
                            group
                                .into_iter()
                                .map(|v| {
                                    v.is_finite().then(|| {
                                        Instant::from_micros((v * 86_400_000_000.0) as i64).unwrap()
                                    })
                                })
                                .collect(),
                        )
                    } else {
                        AxisData::Numbers(group)
                    }
                })
                .collect();
            axes(&mut f)
                .title("Arrival times")
                .events(
                    ["North", "Empty", "South"].map(String::from).to_vec(),
                    positions,
                    EventsOptions {
                        length: 0.8,
                        width: SizeSpec::pt(1.5).unwrap(),
                        ..Default::default()
                    },
                )
                .unwrap();
            emit_scene(
                &format!("events-{kind}-{name}"),
                &f.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
}
