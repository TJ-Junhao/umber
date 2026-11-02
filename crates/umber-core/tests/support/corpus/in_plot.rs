//! Text at positions in the plot in every theme: a note in each corner with the matching
//! alignment, and an annotation whose text sits at a plot position with an arrow to a data point.

use umber_core::{
    Align, AnnotateOptions, AxisData, AxisValue, FontLibrary, InPlot, LineOptions, TextOptions,
};

use super::super::{Output, axes, emit_scene, figure, linear, themes};

/// Emits a figure with a note in each corner of the plot and one with an annotation pinned in
/// the plot.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        let mut ax = axes(&mut f);
        ax.title("Text in the corners of the plot")
            .xlim(0.0, 10.0)
            .unwrap()
            .ylim(0.0, 10.0)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 3.0, 6.0, 10.0]),
                &[2.0, 6.0, 4.0, 8.0],
                LineOptions::default(),
            )
            .unwrap();
        for (text, (x, y), align) in [
            ("top left", (0.02, 0.96), Align::Left),
            ("top right", (0.98, 0.96), Align::Right),
            ("bottom left", (0.02, 0.04), Align::Left),
            ("bottom right", (0.98, 0.04), Align::Right),
            ("centre", (0.5, 0.5), Align::Center),
        ] {
            let options = TextOptions {
                align,
                ..TextOptions::default()
            };
            ax.text(text, InPlot::new(x, y).unwrap(), options).unwrap();
        }
        emit_scene(
            &format!("in-plot-corners-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        let mut ax = axes(&mut f);
        ax.title("An annotation pinned in the plot")
            .xlim(0.0, 10.0)
            .unwrap()
            .ylim(0.0, 10.0)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 3.0, 6.0, 10.0]),
                &[2.0, 6.0, 4.0, 8.0],
                LineOptions::default(),
            )
            .unwrap();
        let at = (AxisValue::Number(3.0), AxisValue::Number(6.0));
        for (text, text_at, align, arrow) in [
            ("best so far\nat step 3", (0.78, 0.86), None, true),
            ("floor", (0.5, 0.1), None, false),
            (
                "top right,\nright-aligned",
                (0.98, 0.96),
                Some(Align::Right),
                true,
            ),
        ] {
            let options = AnnotateOptions {
                text_at: Some(InPlot::new(text_at.0, text_at.1).unwrap()),
                align,
                arrow,
                ..AnnotateOptions::default()
            };
            ax.annotate(text, at.clone(), options).unwrap();
        }
        emit_scene(
            &format!("in-plot-annotation-{name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }
}
