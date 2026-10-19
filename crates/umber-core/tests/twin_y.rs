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
