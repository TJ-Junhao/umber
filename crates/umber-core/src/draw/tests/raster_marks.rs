//! Scatters, lines and tripcolors drawn as one image in vector output
//! ([`MarkRender::Raster`]): which items of the marks group go into an [`Item::Raster`], and
//! that its rectangle is the marks clip.

use std::sync::Arc;

use super::{mark_group, nums};
use crate::model::{AxisData, Figure, LineOptions, ScatterOptions};
use crate::render::render_png;
use crate::scene::{Item, Rect};
use crate::{
    ErrorRange, FontLibrary, MarkRender, Marker, PointLabels, ScaleKind, ScatterGroup, SizeSpec,
    SvgText, Theme, Triangles, TripcolorOptions,
};

/// The marks group of `figure`'s scene: its clip and items.
fn marks_of(figure: &Figure) -> (Rect, Vec<Item>) {
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    let (clip, items) = mark_group(&scene);
    (clip, items.to_vec())
}

/// The kind of each item, `raster` with the kinds of its items.
fn kinds(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .map(|item| match item {
            Item::Fill { .. } => "fill".to_owned(),
            Item::Stroke { .. } => "stroke".to_owned(),
            Item::Markers { .. } => "markers".to_owned(),
            Item::Text(_) | Item::RotatedText { .. } => "text".to_owned(),
            Item::Image { .. } => "image".to_owned(),
            Item::Group { .. } => "group".to_owned(),
            Item::Raster { items, .. } => format!("raster[{}]", kinds(items).join(" ")),
        })
        .collect()
}

/// A figure of one linear axes holding what `draw` adds.
fn figure(draw: impl FnOnce(&mut crate::AxesMut<'_>)) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    draw(&mut axes);
    f
}

fn scatter(render: MarkRender, connect: bool, bars: bool) -> ScatterOptions {
    ScatterOptions {
        connect,
        y_error: bars.then(|| ErrorRange::Numbers {
            low: Arc::from([0.5, 1.5, 2.0]),
            high: Arc::from([1.5, 2.5, 3.5]),
        }),
        render,
        ..ScatterOptions::default()
    }
}

#[test]
fn a_raster_line_is_one_raster_item_covering_the_marks_clip() {
    let f = figure(|a| {
        let options = LineOptions {
            render: MarkRender::Raster,
            ..LineOptions::default()
        };
        a.line(nums(&[0.0, 1.0, 2.0]), &[1.0, 3.0, 2.0], options)
            .unwrap();
    });
    let (clip, items) = marks_of(&f);
    assert_eq!(kinds(&items), ["raster[stroke]"]);
    let Item::Raster { rect, .. } = &items[0] else {
        unreachable!()
    };
    assert_eq!(*rect, clip);
}

#[test]
fn vector_marks_draw_no_raster_item() {
    let f = figure(|a| {
        a.line(nums(&[0.0, 1.0]), &[1.0, 2.0], LineOptions::default())
            .unwrap()
            .scatter(
                nums(&[0.0, 1.0, 2.0]),
                &[1.0, 2.0, 3.0],
                scatter(MarkRender::Vector, true, true),
            )
            .unwrap();
    });
    let (_, items) = marks_of(&f);
    assert!(kinds(&items).iter().all(|kind| !kind.starts_with("raster")));
}

#[test]
fn a_scatter_keeps_its_error_bars_out_of_its_images() {
    let points = |options: ScatterOptions| {
        let f = figure(|a| {
            a.scatter(nums(&[0.0, 1.0, 2.0]), &[1.0, 2.0, 3.0], options)
                .unwrap();
        });
        kinds(&marks_of(&f).1)
    };
    // The markers alone: one image of the outline and fill layers.
    assert_eq!(
        points(scatter(MarkRender::Raster, false, false)),
        ["raster[markers markers]"]
    );
    // With a joining line and no bars, the line and the markers are one image.
    assert_eq!(
        points(scatter(MarkRender::Raster, true, false)),
        ["raster[stroke markers markers]"]
    );
    // The bars stay vector shapes, over the line and under the markers.
    let both = points(scatter(MarkRender::Raster, true, true));
    assert_eq!(both.first().map(String::as_str), Some("raster[stroke]"));
    assert_eq!(
        both.last().map(String::as_str),
        Some("raster[markers markers]")
    );
    assert!(both[1..both.len() - 1].iter().all(|kind| kind == "stroke"));
    let bars = points(scatter(MarkRender::Raster, false, true));
    assert!(bars[..bars.len() - 1].iter().all(|kind| kind == "stroke"));
    assert_eq!(
        bars.last().map(String::as_str),
        Some("raster[markers markers]")
    );
}

#[test]
fn the_groups_of_a_grouped_raster_scatter_or_line_share_one_image() {
    let group = |name: &str, y: f64| {
        ScatterGroup::new(
            name,
            AxisData::Numbers(vec![0.0, 1.0]),
            AxisData::Numbers(vec![y, y + 1.0]),
        )
    };
    let f = figure(|a| {
        a.grouped_scatter(
            vec![group("North", 1.0), group("South", 3.0)],
            Marker::Circle,
            SizeSpec::Auto,
            PointLabels::default(),
            false,
            MarkRender::Raster,
        )
        .unwrap();
    });
    assert_eq!(
        kinds(&marks_of(&f).1),
        ["raster[markers markers markers markers]"]
    );
    let lines = figure(|a| {
        let xy = |y: f64| (nums(&[0.0, 1.0]), AxisData::Numbers(vec![y, y + 1.0]));
        let ((x1, y1), (x2, y2)) = (xy(1.0), xy(3.0));
        a.grouped_line(
            vec![("North".into(), x1, y1), ("South".into(), x2, y2)],
            crate::Dash::Solid,
            SizeSpec::Auto,
            MarkRender::Raster,
        )
        .unwrap();
    });
    assert_eq!(kinds(&marks_of(&lines).1), ["raster[stroke stroke]"]);
}

#[test]
fn a_raster_tripcolor_is_one_raster_item_of_its_triangles() {
    let f = figure(|a| {
        let (x, y) = ([0.0, 1.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
        let options = TripcolorOptions {
            render: MarkRender::Raster,
            ..TripcolorOptions::default()
        };
        a.tripcolor(
            &x,
            &y,
            Triangles::Given(&[[0, 1, 2], [0, 2, 3]]),
            &[0.0, 1.0, 2.0, 3.0],
            options,
        )
        .unwrap();
    });
    let (_, items) = marks_of(&f);
    let [Item::Raster { items: inner, .. }] = items.as_slice() else {
        panic!("{:?}", kinds(&items));
    };
    assert!(
        inner
            .iter()
            .all(|item| matches!(item, Item::Fill { .. } | Item::Stroke { .. })),
        "{:?}",
        kinds(inner)
    );
}

#[test]
fn a_raster_mark_that_draws_nothing_adds_no_image() {
    let f = figure(|a| {
        let options = LineOptions {
            render: MarkRender::Raster,
            ..LineOptions::default()
        };
        a.line(nums(&[0.0, 1.0]), &[0.0, 1.0], LineOptions::default())
            .unwrap()
            .line(nums(&[f64::NAN]), &[f64::NAN], options)
            .unwrap();
    });
    assert_eq!(kinds(&marks_of(&f).1), ["stroke"]);
}

fn raster_line(a: &mut crate::AxesMut<'_>, y: f64) {
    let options = LineOptions {
        render: MarkRender::Raster,
        ..LineOptions::default()
    };
    a.line(nums(&[0.0, 1.0, 2.0]), &[y, y + 1.0, y], options)
        .unwrap();
}

#[test]
fn consecutive_raster_lines_are_one_image_of_their_items_in_order() {
    let f = figure(|a| {
        raster_line(a, 0.0);
        raster_line(a, 1.0);
        raster_line(a, 2.0);
    });
    let (clip, items) = marks_of(&f);
    assert_eq!(kinds(&items), ["raster[stroke stroke stroke]"]);
    let Item::Raster { rect, items: inner } = &items[0] else {
        unreachable!()
    };
    assert_eq!(*rect, clip);
    assert!(inner.iter().all(|item| matches!(item, Item::Stroke { .. })));
    // Strokes of different lines differ, so the order of the calls is the order of the items.
    assert!(inner[0] != inner[1] && inner[1] != inner[2]);
}

#[test]
fn a_vector_item_between_raster_marks_keeps_them_two_images() {
    let f = figure(|a| {
        raster_line(a, 0.0);
        a.line(nums(&[0.0, 1.0]), &[1.0, 2.0], LineOptions::default())
            .unwrap();
        raster_line(a, 2.0);
        raster_line(a, 3.0);
    });
    assert_eq!(
        kinds(&marks_of(&f).1),
        ["raster[stroke]", "stroke", "raster[stroke stroke]"]
    );
    // A reference line is drawn after the data marks, so it never splits them.
    let g = figure(|a| {
        raster_line(a, 0.0);
        a.hline(1.0, crate::RuleOptions::default()).unwrap();
        raster_line(a, 2.0);
    });
    assert_eq!(kinds(&marks_of(&g).1), ["raster[stroke stroke]", "stroke"]);
}

#[test]
fn a_raster_scatter_after_a_raster_line_shares_its_image() {
    let f = figure(|a| {
        raster_line(a, 0.0);
        a.scatter(
            nums(&[0.0, 1.0, 2.0]),
            &[1.0, 2.0, 3.0],
            scatter(MarkRender::Raster, false, false),
        )
        .unwrap();
    });
    assert_eq!(kinds(&marks_of(&f).1), ["raster[stroke markers markers]"]);
}

#[test]
fn the_error_bars_of_a_raster_scatter_split_the_images_around_them() {
    let f = figure(|a| {
        raster_line(a, 0.0);
        a.scatter(
            nums(&[0.0, 1.0, 2.0]),
            &[1.0, 2.0, 3.0],
            scatter(MarkRender::Raster, false, true),
        )
        .unwrap();
        raster_line(a, 2.0);
    });
    let kinds = kinds(&marks_of(&f).1);
    assert_eq!(kinds.first().map(String::as_str), Some("raster[stroke]"));
    assert_eq!(
        kinds.last().map(String::as_str),
        Some("raster[markers markers stroke]")
    );
}

#[test]
fn two_axes_never_share_an_image() {
    let grid = crate::Grid::new(1, 2).unwrap();
    let mut f = Figure::with_grid(8.0, 4.0, Theme::pigment(), grid).unwrap();
    for col in 0..2 {
        let at = crate::Placement {
            col: crate::GridIndex::At(col),
            ..crate::Placement::default()
        };
        let mut axes = f
            .add_axes_at(at, ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        raster_line(&mut axes, 0.0);
        raster_line(&mut axes, 1.0);
    }
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let images: Vec<usize> = scene
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Group {
                clip: Some(_),
                items,
            } => Some(
                items
                    .iter()
                    .filter(|i| matches!(i, Item::Raster { .. }))
                    .count(),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(images, [1, 1]);
}

#[test]
fn merging_raster_marks_leaves_png_output_as_vector_marks_draw_it() {
    let draw = |render: MarkRender| {
        figure(|a| {
            for y in 0..4 {
                let options = LineOptions {
                    render,
                    ..LineOptions::default()
                };
                let y = f64::from(y);
                a.line(nums(&[0.0, 1.0, 2.0]), &[y, y + 1.0, y], options)
                    .unwrap();
            }
        })
    };
    for dpi in [72, 200] {
        let png = |render| {
            render_png(&draw(render).to_scene(FontLibrary::global()).unwrap(), dpi).unwrap()
        };
        assert_eq!(png(MarkRender::Raster), png(MarkRender::Vector), "{dpi}");
    }
}

#[test]
fn two_hundred_raster_lines_are_one_image_in_pdf_and_svg() {
    let f = figure(|a| {
        for y in 0..200 {
            raster_line(a, f64::from(y));
        }
    });
    let svg = f.to_svg(SvgText::Outlines, 100).unwrap();
    assert_eq!(svg.matches("<image ").count(), 1);
    let pdf = f.to_pdf(100).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert_eq!(
        text.matches("/Subtype /Image").count() - text.matches("/SMask ").count(),
        1
    );
}
