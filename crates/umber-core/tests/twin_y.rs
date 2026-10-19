//! Right axes retain numeric coordinates through every output backend.

#[path = "support/pdf_check.rs"]
mod pdf_check;

use umber_core::{AxisData, Figure, GridIndex, LineOptions, ScaleKind, SvgText, Theme};

fn figure() -> umber_core::Result<Figure> {
    let mut f = Figure::new(8.0, 5.0, Theme::pigment())?;
    let mut left = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    left.ylabel("Left units").line(
        AxisData::Numbers(vec![0.0, 1.0]),
        &[0.0, 10.0],
        LineOptions {
            label: "Left series".into(),
            ..Default::default()
        },
    )?;
    left.twin_y(ScaleKind::Linear)?.ylabel("Right units").line(
        AxisData::Numbers(vec![0.0, 2.0]),
        &[0.0, 1000.0],
        LineOptions {
            label: "Right series".into(),
            ..Default::default()
        },
    )?;
    Ok(f)
}

#[test]
fn twin_settings_share_x_and_keep_y_independent() {
    let mut f = figure().unwrap();
    let row = GridIndex::Auto;
    f.twin_axes_at_mut(row, row)
        .unwrap()
        .xlim(-2.0, 3.0)
        .unwrap()
        .ylim(0.0, 2000.0)
        .unwrap();
    let left = f.axes_at(row, row).unwrap();
    let right = f.twin_axes_at(row, row).unwrap();
    assert_eq!(left.x_limits(), right.x_limits());
    assert_eq!(left.y_limits(), None);
    assert_eq!(right.y_limits(), Some((0.0, 2000.0)));
    assert_eq!(f.panels().count(), 1);
    f.axes_at_mut(row, row).unwrap().xlim(-3.0, 4.0).unwrap();
    assert_eq!(
        f.twin_axes_at(row, row).unwrap().x_limits(),
        Some((-3.0, 4.0))
    );
}

#[test]
fn twin_lookup_errors_keep_their_context_and_details() {
    for (row, col, pie, parameter) in [
        (0, 0, true, "row"),
        (1, 0, false, "row"),
        (0, 1, false, "col"),
    ] {
        let mut f = Figure::new(8.0, 5.0, Theme::pigment()).unwrap();
        if pie {
            f.add_pie_axes().unwrap();
        } else {
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .twin_y(ScaleKind::Linear)
                .unwrap();
        }
        let (row, col) = (GridIndex::At(row), GridIndex::At(col));
        let original = f.axes_at(row, col).unwrap_err();
        for error in [
            f.twin_axes_at(row, col).unwrap_err(),
            f.twin_axes_at_mut(row, col).unwrap_err(),
        ] {
            assert!(error.to_string().starts_with("Figure.twin_axes_at():"));
            assert_eq!(error.parameter(), Some(parameter));
            assert_eq!(error.hint(), original.hint());
        }
    }
}

#[test]
fn twin_baseline_rejects_shown_without_changing_either_axes() {
    use umber_core::BaselineVisibility::{Auto, Hidden, Shown};

    let mut f = figure().unwrap();
    let row = GridIndex::Auto;
    f.axes_at_mut(row, row).unwrap().baseline(Shown).unwrap();
    for accepted in [Hidden, Auto] {
        let mut right = f.twin_axes_at_mut(row, row).unwrap();
        right.baseline(accepted).unwrap();
        let error = right.baseline(Shown).unwrap_err();
        assert!(error.to_string().starts_with("Axes.baseline():"));
        assert_eq!(error.parameter(), Some("shown"));
        assert_eq!(
            error.hint(),
            Some("the left axes draws the baseline; set it there.")
        );
        assert_eq!(right.baseline_visibility(), accepted);
        assert_eq!(f.axes_at(row, row).unwrap().baseline_visibility(), Shown);
    }
}

#[test]
fn twin_pdf_is_valid_and_matches_png() {
    let f = figure().unwrap();
    let pdf = f.to_pdf().unwrap();
    assert_eq!(pdf, f.to_pdf().unwrap());
    let dir = pdf_check::scratch("twin-y");
    let path = pdf_check::write(&dir, "axes.pdf", &pdf);
    pdf_check::assert_qpdf_clean(&path);
    let text = pdf_check::extracted_text(&path);
    for label in ["Left units", "Right units", "Left series", "Right series"] {
        assert!(text.contains(label), "{label}: {text}");
    }
    pdf_check::assert_matches_png(&path, &f.to_png(144).unwrap(), 144);
    assert!(f.to_svg(SvgText::Editable).unwrap().contains("Right units"));
    std::fs::remove_dir_all(dir).unwrap();
}
