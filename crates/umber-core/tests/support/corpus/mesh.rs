//! Numeric cells and sparse two-dimensional counts in each theme.

use super::super::{Output, axes, emit_scene, figure, linear, themes};
use umber_core::{FontLibrary, HexbinOptions, Hist2dOptions, LogBase, MeshOptions, Transform};

/// Emits edge-spanning cells, rectangular counts and regular hexagonal counts.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        axes(&mut f)
            .mesh(
                &[-2.0, 0.0, 1.0, 4.0],
                &[0.0, 1.0, 3.0],
                &[-2.0, f64::NAN, 4.0, 1.0, 0.0, 2.0],
                MeshOptions {
                    center: Some(0.0),
                    ..MeshOptions::default()
                },
            )
            .unwrap()
            .title("Cell values");
        emit_scene(&format!("mesh-{name}"), &f.to_scene(lib).unwrap(), sink);

        let x: Vec<_> = (0..180)
            .map(|i| (i as f64 * 0.37).sin() + (i % 9) as f64 / 7.0)
            .collect();
        let y: Vec<_> = (0..180)
            .map(|i| (i as f64 * 0.17).cos() + (i % 7) as f64 / 5.0)
            .collect();
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        axes(&mut f)
            .hist2d(
                &x,
                &y,
                Hist2dOptions {
                    bins: (12, 9),
                    scale: Transform::Log(LogBase::TEN),
                    ..Hist2dOptions::default()
                },
            )
            .unwrap()
            .title("Rectangular counts");
        emit_scene(&format!("hist2d-{name}"), &f.to_scene(lib).unwrap(), sink);

        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        axes(&mut f)
            .hexbin(
                &x,
                &y,
                HexbinOptions {
                    gridsize: 12,
                    ..HexbinOptions::default()
                },
            )
            .unwrap()
            .title("Hexagonal counts");
        emit_scene(&format!("hexbin-{name}"), &f.to_scene(lib).unwrap(), sink);
    }
}
