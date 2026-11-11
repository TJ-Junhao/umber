//! Scatters, lines and tripcolors drawn as one image in SVG and PDF output
//! ([`MarkRender::Raster`]): PNG output does not change, the image takes the mark's place and
//! clip, is transparent where the mark draws nothing, follows `raster_dpi`, and looks as the
//! vector mark does when a PDF reader draws both at the raster resolution.
//!
//! Some tests run PDF readers; see `support/pdf_check.rs` for when they are skipped.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use umber_core::figure::{AxisData, BandOptions, LineOptions, ScatterOptions};
use umber_core::render::{render_pdf, render_png, render_svg};
use umber_core::{
    ColorBy, ColorSpec, ColorbarOptions, ErrorRange, Figure, FontLibrary, Item, MarkRender, Place,
    PointColor, PointLabels, Rect, ScaleKind, Scene, SvgText, Theme, TriValues, Triangles,
    TripcolorOptions, pdf_pages,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;
use pdf_check::image::{DecodedPng, decode_rgba8};
use pdf_check::*;

const UNDER: umber_core::Rgba = umber_core::Rgba {
    r: 255,
    g: 0,
    b: 0,
    a: 255,
};
const OVER: umber_core::Rgba = umber_core::Rgba {
    r: 0,
    g: 0,
    b: 255,
    a: 255,
};

/// `n` points spread over `0..10` by `0..10`, the same every time.
fn points(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    (0..n).map(|_| (10.0 * next(), 10.0 * next())).unzip()
}

fn numbers(values: &[f64]) -> AxisData {
    AxisData::Numbers(values.to_vec())
}

/// A figure whose axes hold a red line under the mark that `draw` adds and a blue line over
/// it, with limits 0 to 10 on both axes.
fn layered(theme: Theme, draw: impl FnOnce(&mut umber_core::AxesMut<'_>)) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    axes.xlim(0.0, 10.0).unwrap().ylim(0.0, 10.0).unwrap();
    let line = |label: &str, color| LineOptions {
        label: label.into(),
        color: ColorSpec::Fixed(color),
        ..LineOptions::default()
    };
    axes.line(numbers(&[0.0, 10.0]), &[1.0, 9.0], line("Under", UNDER))
        .unwrap();
    draw(&mut axes);
    axes.line(numbers(&[0.0, 10.0]), &[9.0, 1.0], line("Over", OVER))
        .unwrap();
    f
}

/// A scatter of `n` points between two lines, labelled, drawn as `render` says.
fn scatter_figure(render: MarkRender, n: usize) -> Figure {
    layered(Theme::pigment(), |a| {
        let (x, y) = points(n);
        let options = ScatterOptions {
            label: "Points".into(),
            render,
            ..ScatterOptions::default()
        };
        a.scatter(AxisData::Numbers(x), &y, options).unwrap();
    })
}

/// A scatter with its own line, error bars and value labels, drawn as `render` says.
fn dressed_scatter(render: MarkRender) -> Figure {
    layered(Theme::graphite(), |a| {
        let x = [1.0, 3.0, 5.0, 7.0, 9.0];
        let options = ScatterOptions {
            label: "Folds".into(),
            labels: PointLabels {
                annotate: true,
                place: Place::Right,
                ..PointLabels::default()
            },
            y_error: Some(ErrorRange::Numbers {
                low: Arc::from([2.0, 3.0, 4.5, 4.0, 6.0]),
                high: Arc::from([3.0, 4.5, 5.5, 6.5, 7.0]),
            }),
            connect: true,
            render,
            ..ScatterOptions::default()
        };
        a.scatter(numbers(&x), &[2.5, 4.0, 5.0, 5.5, 6.5], options)
            .unwrap();
    })
}

/// A long noisy line drawn as `render` says.
fn line_figure(render: MarkRender) -> Figure {
    layered(Theme::midnight(), |a| {
        let (_, y) = points(4000);
        let x: Vec<f64> = (0..y.len()).map(|i| i as f64 / 400.0).collect();
        let options = LineOptions {
            label: "Noise".into(),
            render,
            ..LineOptions::default()
        };
        a.line(AxisData::Numbers(x), &y, options).unwrap();
    })
}

/// A 6 by 6 grid of points, triangulated, with the values of the triangles around the middle
/// point NaN when `holes`.
fn tripcolor_figure(render: MarkRender, holes: bool) -> Figure {
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for j in 0..6 {
        for i in 0..6 {
            x.push(f64::from(i) * 2.0);
            y.push(f64::from(j) * 2.0);
        }
    }
    let mut triangles = Vec::new();
    let mut values = Vec::new();
    for j in 0..5_i64 {
        for i in 0..5_i64 {
            let corner = j * 6 + i;
            triangles.push([corner, corner + 1, corner + 7]);
            triangles.push([corner, corner + 7, corner + 6]);
            let hole = holes && (i, j) == (2, 2);
            values.push(if hole { f64::NAN } else { (i + j) as f64 });
            values.push(if hole { f64::NAN } else { (i * j) as f64 });
        }
    }
    let mut f = Figure::new(5.0, 5.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .tripcolor(
            &x,
            &y,
            Triangles::Given(&triangles),
            &values,
            TripcolorOptions {
                on: TriValues::Triangles,
                render,
                ..TripcolorOptions::default()
            },
        )
        .unwrap();
    f
}

/// A color-mapped scatter with a colorbar, drawn as `render` says.
fn color_by_figure(render: MarkRender) -> Figure {
    let (x, y) = points(3000);
    let values: Vec<f64> = x.iter().zip(&y).map(|(a, b)| a * b).collect();
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(
            AxisData::Numbers(x),
            &y,
            ScatterOptions {
                color: PointColor::By(ColorBy::new(values)),
                render,
                ..ScatterOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions {
            label: "Product".into(),
            ..ColorbarOptions::default()
        })
        .unwrap();
    f
}

/// Every figure of these tests with a raster mark, by name, with the same figure drawn as
/// vector shapes.
fn pairs() -> Vec<(&'static str, Figure, Figure)> {
    vec![
        (
            "scatter",
            scatter_figure(MarkRender::Raster, 2000),
            scatter_figure(MarkRender::Vector, 2000),
        ),
        (
            "dressed scatter",
            dressed_scatter(MarkRender::Raster),
            dressed_scatter(MarkRender::Vector),
        ),
        (
            "line",
            line_figure(MarkRender::Raster),
            line_figure(MarkRender::Vector),
        ),
        (
            "tripcolor",
            tripcolor_figure(MarkRender::Raster, true),
            tripcolor_figure(MarkRender::Vector, true),
        ),
        (
            "color by",
            color_by_figure(MarkRender::Raster),
            color_by_figure(MarkRender::Vector),
        ),
    ]
}

fn scene(figure: &Figure) -> Scene {
    figure.to_scene(FontLibrary::global()).unwrap()
}

/// The `<image>` elements of `svg`, as text.
fn svg_images(svg: &str) -> Vec<&str> {
    svg.match_indices("<image ")
        .map(|(at, _)| {
            let end = svg[at..].find("/>").unwrap();
            &svg[at..at + end + 2]
        })
        .collect()
}

/// The number attribute `name` of an SVG element.
fn attribute(element: &str, name: &str) -> f64 {
    let at = element.find(&format!(" {name}=\"")).unwrap() + name.len() + 3;
    element[at..at + element[at..].find('"').unwrap()]
        .parse()
        .unwrap()
}

/// Standard base64, as data URIs carry it.
fn unbase64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => panic!("not base64: {c}"),
    };
    let bytes: Vec<u8> = text.bytes().filter(|&c| c != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let v: Vec<u32> = chunk.iter().map(|&c| u32::from(value(c))).collect();
        let n = v
            .iter()
            .enumerate()
            .fold(0, |n, (i, &d)| n | d << (18 - 6 * i));
        for i in 0..chunk.len() - 1 {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    out
}

/// The PNG an SVG `<image>` holds, decoded.
fn image_pixels(element: &str) -> DecodedPng {
    let data = element
        .split("href=\"data:image/png;base64,")
        .nth(1)
        .unwrap();
    decode_rgba8(&unbase64(&data[..data.find('"').unwrap()])).unwrap()
}

/// The clip of the marks group of `scene`: its first clipped group.
fn marks_clip(scene: &Scene) -> Rect {
    scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group {
                clip: Some(clip), ..
            } => Some(*clip),
            _ => None,
        })
        .unwrap()
}

/// The image XObjects of `pdf` that are not soft masks.
fn pdf_images(pdf: &[u8]) -> usize {
    let text = String::from_utf8_lossy(pdf);
    text.matches("/Subtype /Image").count() - text.matches("/SMask ").count()
}

#[test]
fn png_output_is_the_same_for_vector_and_raster() {
    for (name, raster, vector) in pairs() {
        let (raster, vector) = (scene(&raster), scene(&vector));
        assert_ne!(
            raster, vector,
            "{name}: the raster scene holds a raster item"
        );
        for dpi in [72, 200] {
            assert_eq!(
                render_png(&raster, dpi).unwrap(),
                render_png(&vector, dpi).unwrap(),
                "{name} at {dpi} dpi"
            );
        }
    }
}

#[test]
fn svg_and_pdf_draw_a_raster_mark_as_one_image() {
    for (name, raster, vector) in pairs() {
        let svg = raster.to_svg(SvgText::Outlines, 300).unwrap();
        let images = svg_images(&svg);
        // The dressed scatter's line and markers are two images, around its error bars.
        let expected = if name == "dressed scatter" { 2 } else { 1 };
        assert_eq!(images.len(), expected, "{name}");
        assert!(
            images
                .iter()
                .all(|image| !image.contains("image-rendering") && !image.contains("class=")),
            "{name}: an image of marks is scaled as viewers scale images"
        );
        assert!(!svg.contains("<style>"), "{name}");
        let pdf = raster.to_pdf(300).unwrap();
        assert_eq!(pdf_images(&pdf), expected, "{name}");
        assert_eq!(
            String::from_utf8_lossy(&pdf).matches("/SMask ").count(),
            expected
        );
        assert!(String::from_utf8_lossy(&pdf).contains("/Interpolate true"));
        // Drawn as vector shapes, the figure has no image at all.
        assert!(svg_images(&vector.to_svg(SvgText::Outlines, 300).unwrap()).is_empty());
        assert_eq!(pdf_images(&vector.to_pdf(300).unwrap()), 0, "{name}");
    }
}

#[test]
fn a_raster_mark_keeps_its_place_between_the_marks_under_and_over_it() {
    let figure = scatter_figure(MarkRender::Raster, 500);
    let svg = figure.to_svg(SvgText::Outlines, 300).unwrap();
    let under = svg.find("stroke=\"#FF0000\"").unwrap();
    let image = svg.find("<image ").unwrap();
    let over = svg.find("stroke=\"#0000FF\"").unwrap();
    assert!(under < image && image < over, "{under} {image} {over}");
    let pdf = figure.to_pdf(300).unwrap();
    let content = page_content(&pdf);
    let under = content.find("1 0 0 RG").unwrap();
    let image = content.find(" Do\n").unwrap();
    let over = content.find("0 0 1 RG").unwrap();
    assert!(under < image && image < over, "{under} {image} {over}");
}

#[test]
fn the_image_covers_the_plot_on_a_quarter_inch_grid_and_is_clipped_to_it() {
    for (name, raster, _) in pairs() {
        let scene = scene(&raster);
        let clip = marks_clip(&scene);
        // At 300 dpi a quarter inch is 75 pixels; at 150 dpi it is not whole, and half an inch,
        // 75 pixels, is used.
        for (dpi, step) in [(150, 36.0), (300, 18.0)] {
            let svg = raster.to_svg(SvgText::Outlines, dpi).unwrap();
            let images = svg_images(&svg);
            let pixel = 72.0 / dpi as f64;
            for image in images {
                // Each edge lies on the grid or on the page's edge, within one step of the clip.
                let (x, y) = (attribute(image, "x"), attribute(image, "y"));
                let (w, h) = (attribute(image, "width"), attribute(image, "height"));
                let (left, top) = (f64::from(clip.x), f64::from(clip.y));
                let (right, bottom) = (left + f64::from(clip.width), top + f64::from(clip.height));
                let on_grid = |v: f64, page: f32| {
                    ((v / step).round() * step - v).abs() < 0.002
                        || (v - f64::from(page)).abs() < pixel + 0.002
                };
                let near = |outer: f64, inner: f64| outer - 0.002 <= inner && inner < outer + step;
                assert!(
                    on_grid(x, 0.0) && on_grid(y, 0.0),
                    "{name} at {dpi}: {image:.120}"
                );
                assert!(
                    on_grid(x + w, scene.width) && on_grid(y + h, scene.height),
                    "{name} at {dpi}: {image:.120}"
                );
                assert!(
                    near(x, left) && near(y, top) && near(right, x + w) && near(bottom, y + h),
                    "{name} at {dpi}: {image:.120}"
                );
                let png = image_pixels(image);
                assert!(((w / pixel) - f64::from(png.width)).abs() < 0.01, "{name}");
                assert!(((h / pixel) - f64::from(png.height)).abs() < 0.01, "{name}");
                // The image is inside the group that clips the marks.
                let before = &svg[..svg.find(image).unwrap()];
                let group = before.rfind("<g ").unwrap();
                assert!(
                    before[group..].starts_with("<g clip-path=\"url(#c"),
                    "{name}"
                );
                assert!(!before[group..].contains("</g>"), "{name}");
            }
        }
        // In PDF, the image is drawn under the clip of the marks.
        let content = page_content(&raster.to_pdf(300).unwrap());
        let image = content.find(" Do\n").unwrap();
        let clip_at = content[..image].rfind(" re\nW\nn\n").unwrap();
        // Every item between is closed, except the image's own state.
        let ops: Vec<&str> = content[clip_at..image].lines().collect();
        let count = |op: &str| ops.iter().filter(|line| **line == op).count();
        assert_eq!(count("q"), count("Q") + 1, "{name}");
    }
}

#[test]
fn pixels_where_the_mark_draws_nothing_are_transparent() {
    let svg = scatter_figure(MarkRender::Raster, 300)
        .to_svg(SvgText::Outlines, 300)
        .unwrap();
    let png = image_pixels(svg_images(&svg)[0]);
    let alphas: Vec<u8> = png.rgba.chunks_exact(4).map(|p| p[3]).collect();
    let clear = alphas.iter().filter(|&&a| a == 0).count();
    let opaque = alphas.iter().filter(|&&a| a == 255).count();
    assert!(clear > alphas.len() / 2, "{clear} of {}", alphas.len());
    assert!(opaque > 0 && opaque < alphas.len());
    // The corners hold no point.
    assert_eq!(png.at(0, 0)[3], 0);
}

#[test]
fn tripcolor_nan_triangles_stay_transparent_in_the_image() {
    let figure = tripcolor_figure(MarkRender::Raster, true);
    let clip = marks_clip(&scene(&figure));
    let svg = figure.to_svg(SvgText::Outlines, 300).unwrap();
    let image = svg_images(&svg)[0];
    let png = image_pixels(image);
    // Data 0..10 fill the plot; the blank square is 4..6 on both axes, its middle at 5, 5.
    let pixel = 72.0 / 300.0;
    let at = |fx: f64, fy: f64| {
        let x = f64::from(clip.x) + fx * f64::from(clip.width) - attribute(image, "x");
        let y = f64::from(clip.y) + fy * f64::from(clip.height) - attribute(image, "y");
        png.at((x / pixel) as u32, (y / pixel) as u32)
    };
    assert_eq!(at(0.5, 0.5)[3], 0, "the middle of the blank triangles");
    assert_eq!(at(0.15, 0.15)[3], 255, "a colored triangle");
    assert_eq!(at(0.85, 0.6)[3], 255, "a colored triangle");
}

#[test]
fn the_legend_swatch_stays_vector() {
    let raster = scatter_figure(MarkRender::Raster, 2000)
        .to_svg(SvgText::Outlines, 300)
        .unwrap();
    let vector = scatter_figure(MarkRender::Vector, 2000)
        .to_svg(SvgText::Outlines, 300)
        .unwrap();
    // The points are in the image; drawn as shapes, each is an outline and a fill.
    let uses = |svg: &str| svg.matches("<use ").count();
    assert_eq!((uses(&raster), uses(&vector)), (0, 2 * 2000));
    // The legend, the last group, is the same vector shapes, a filled marker shape for the
    // points, with the mark drawn either way.
    let legend = |svg: &str| svg[svg.rfind("\n<g>\n").unwrap()..].to_owned();
    assert_eq!(legend(&raster), legend(&vector));
    assert!(legend(&raster).contains("<path d=") && !legend(&raster).contains("<image"));
    assert!(raster.rfind("\n<g>\n").unwrap() > raster.find("<image ").unwrap());
}

#[test]
fn a_color_mapped_scatter_keeps_its_colorbar_vector() {
    let raster = color_by_figure(MarkRender::Raster);
    let svg = raster.to_svg(SvgText::Outlines, 300).unwrap();
    assert_eq!(svg_images(&svg).len(), 1);
    assert!(
        svg.contains("<linearGradient"),
        "the colorbar is a gradient"
    );
    let vector = color_by_figure(MarkRender::Vector)
        .to_svg(SvgText::Outlines, 300)
        .unwrap();
    let gradients = |svg: &str| svg.matches("<linearGradient").count();
    assert_eq!(gradients(&svg), gradients(&vector));
}

#[test]
fn raster_dpi_sets_the_size_of_the_image() {
    let figure = scatter_figure(MarkRender::Raster, 500);
    let clip = marks_clip(&scene(&figure));
    for dpi in [72, 150, 300, 600] {
        let svg = figure.to_svg(SvgText::Outlines, dpi).unwrap();
        let png = image_pixels(svg_images(&svg)[0]);
        let s = dpi as f64 / 72.0;
        // The clip in pixels, widened to the grid of a quarter inch, or half an inch where a
        // quarter is not whole, and cut to the 432 by 288 pt page.
        let step = if dpi % 4 == 0 { dpi / 4 } else { dpi / 2 } as f64;
        let down = |v: f32| ((f64::from(v) * s).floor() / step).floor() * step;
        let up =
            |v: f32, page: f64| (((f64::from(v) * s).ceil() / step).ceil() * step).min(page * s);
        let columns = up(clip.x + clip.width, 432.0) - down(clip.x);
        let rows = up(clip.y + clip.height, 288.0) - down(clip.y);
        assert_eq!(
            (f64::from(png.width), f64::from(png.height)),
            (columns, rows)
        );
        let pdf = String::from_utf8_lossy(&figure.to_pdf(dpi).unwrap()).into_owned();
        assert!(pdf.contains(&format!("/Width {columns}\n")), "{dpi}");
        assert!(pdf.contains(&format!("/Height {rows}\n")), "{dpi}");
    }
}

#[test]
fn raster_dpi_changes_nothing_without_a_raster_mark() {
    for figure in [
        scatter_figure(MarkRender::Vector, 500),
        line_figure(MarkRender::Vector),
        tripcolor_figure(MarkRender::Vector, true),
    ] {
        let scene = scene(&figure);
        for text in [SvgText::Outlines, SvgText::Editable] {
            let svg = render_svg(&scene, text, 300).unwrap();
            for dpi in [10, 72, 2400] {
                assert_eq!(render_svg(&scene, text, dpi).unwrap(), svg);
                assert_eq!(figure.to_svg(text, i64::from(dpi)).unwrap(), svg);
            }
        }
        let pdf = render_pdf(&scene, 300).unwrap();
        for dpi in [10, 72, 2400] {
            assert_eq!(render_pdf(&scene, dpi).unwrap(), pdf);
            assert_eq!(figure.to_pdf(i64::from(dpi)).unwrap(), pdf);
            assert_eq!(pdf_pages(&[&figure], i64::from(dpi)).unwrap(), pdf);
        }
    }
}

#[test]
fn raster_dpi_out_of_range_is_refused() {
    let figure = scatter_figure(MarkRender::Vector, 10);
    for dpi in [9, 2401, 0, -300, i64::MAX] {
        let message = format!("raster_dpi {dpi} is out of range; use a value from 10 to 2400.");
        for (context, err) in [
            ("Figure.to_svg()", figure.to_svg(SvgText::Outlines, dpi)),
            ("Figure.to_pdf()", figure.to_pdf(dpi).map(|_| String::new())),
            (
                "to_pdf()",
                pdf_pages(&[&figure], dpi).map(|_| String::new()),
            ),
        ] {
            let err = err.unwrap_err();
            assert!(matches!(err, umber_core::Error::Style { .. }), "{err:?}");
            assert_eq!(err.to_string(), format!("{context}: {message}"));
            assert_eq!(err.parameter(), Some("raster_dpi"));
        }
    }
    let scene = scene(&figure);
    assert!(render_svg(&scene, SvgText::Outlines, 9).is_err());
    assert!(render_pdf(&scene, 2401).is_err());
    assert!(umber_core::render::render_pdf_pages(&[&scene], 5).is_err());
}

#[test]
fn an_image_larger_than_a_png_may_be_is_refused() {
    // The plot of a 6 by 4 in figure at 2,400 dpi is far over 2^26 pixels.
    let figure = scatter_figure(MarkRender::Raster, 10);
    for (context, err) in [
        (
            "Figure.to_svg()",
            figure.to_svg(SvgText::Outlines, 2400).unwrap_err(),
        ),
        ("Figure.to_pdf()", figure.to_pdf(2400).unwrap_err()),
        ("to_pdf()", pdf_pages(&[&figure], 2400).unwrap_err()),
    ] {
        let text = err.to_string();
        assert!(
            text.starts_with(&format!("{context}: ")) && text.ends_with(", over 67108864."),
            "{text}"
        );
        assert!(
            text.contains("a mark drawn as an image would have "),
            "{text}"
        );
        assert_eq!(err.parameter(), Some("raster_dpi"));
    }
    let err = pdf_pages(&[&scatter_figure(MarkRender::Vector, 10), &figure], 2400).unwrap_err();
    assert!(
        err.to_string()
            .starts_with("to_pdf(): figures[1]: a mark drawn"),
        "{err}"
    );
    // A lower raster_dpi draws it.
    assert!(figure.to_pdf(600).is_ok());
}

#[test]
fn a_document_of_several_pages_draws_each_raster_mark_on_its_page() {
    let raster = scatter_figure(MarkRender::Raster, 500);
    let vector = line_figure(MarkRender::Vector);
    let pdf = pdf_pages(&[&raster, &vector, &raster], 150).unwrap();
    assert_eq!(pdf_images(&pdf), 2);
    let text = String::from_utf8_lossy(&pdf);
    let size = String::from_utf8_lossy(&raster.to_pdf(150).unwrap()).into_owned();
    let width = after(&size, "/Width ")
        .split('\n')
        .next()
        .unwrap()
        .to_owned();
    assert_eq!(
        text.matches(&format!("/Width {width}\n")).count(),
        4,
        "images and masks"
    );
    // The resolution applies to every page.
    let finer = pdf_pages(&[&raster, &vector, &raster], 300).unwrap();
    assert!(!String::from_utf8_lossy(&finer).contains(&format!("/Width {width}\n")));
    if have("qpdf") {
        let dir = scratch("raster-pages");
        assert_qpdf_clean(&write(&dir, "pages.pdf", &pdf));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

/// The largest mean channel difference allowed between a PDF reader's drawing of a figure with
/// a raster mark and of the same figure with the mark as vector shapes, both at the raster
/// resolution: the two rasterizers antialias edges differently, as they do for every shape. The
/// largest measured is 0.82 (MuPDF, a scatter of 5,000 points over a band), with no pixel over
/// 64 apart.
const MAX_RASTER_MEAN: f64 = 1.0;
/// The largest share of pixels with a channel more than 64 apart between the two.
const MAX_RASTER_FAR: f64 = 0.001;
/// The largest mean channel difference allowed between Poppler's drawing of a figure with a
/// raster mark and its PNG at the raster resolution, where the image is drawn pixel for pixel
/// and only the vector shapes around it differ. The largest measured is 0.29, with 0.05% of
/// pixels over 64 apart.
const MAX_IMAGE_MEAN: f64 = 0.5;
/// The largest share of pixels with a channel more than 64 apart between the two.
const MAX_IMAGE_FAR: f64 = 0.002;

#[test]
fn at_the_raster_resolution_a_raster_mark_looks_as_the_vector_mark() {
    if !have("pdftoppm") {
        return;
    }
    let dir = scratch("raster-marks-match");
    let mut cases = pairs();
    // A band under a dense scatter shows through the transparent pixels of its image.
    cases.push((
        "scatter over a band",
        banded(MarkRender::Raster),
        banded(MarkRender::Vector),
    ));
    for (name, raster, vector) in cases {
        let file = name.replace(' ', "-");
        let raster_pdf = write(
            &dir,
            &format!("{file}-raster.pdf"),
            &raster.to_pdf(300).unwrap(),
        );
        let vector_pdf = write(
            &dir,
            &format!("{file}-vector.pdf"),
            &vector.to_pdf(300).unwrap(),
        );
        assert_readers_parse(&raster_pdf);
        let png = png_raster(&raster.to_png(300).unwrap());
        let raster_page = poppler_raster_for_png(&raster_pdf, 300, &png);
        let vector_page = poppler_raster_for_png(&vector_pdf, 300, &png);
        let image = raster_difference(&raster_page, &png);
        let shapes = raster_difference(&raster_page, &vector_page);
        let baseline = raster_difference(&vector_page, &png);
        eprintln!(
            "{name} at 300 dpi, Poppler: raster PDF against vector PDF {:.4} / {:.5}, against \
             PNG {:.4} / {:.5}; vector PDF against PNG {:.4} / {:.5}",
            shapes.0, shapes.1, image.0, image.1, baseline.0, baseline.1
        );
        assert!(
            shapes.0 <= MAX_RASTER_MEAN && shapes.1 <= MAX_RASTER_FAR,
            "{name}: raster against vector PDF {shapes:?}; files at {}",
            dir.display()
        );
        assert!(
            image.0 <= MAX_IMAGE_MEAN && image.1 <= MAX_IMAGE_FAR,
            "{name}: raster PDF against PNG {image:?}; files at {}",
            dir.display()
        );
        if have("mutool") {
            let raster_page = mupdf_raster_for_png(&raster_pdf, 300, &png);
            let vector_page = mupdf_raster_for_png(&vector_pdf, 300, &png);
            let shapes = raster_difference(&raster_page, &vector_page);
            eprintln!(
                "{name} at 300 dpi, MuPDF: raster PDF against vector PDF {:.4} / {:.5}",
                shapes.0, shapes.1
            );
            assert!(
                shapes.0 <= MAX_RASTER_MEAN && shapes.1 <= MAX_RASTER_FAR,
                "{name}: MuPDF, raster against vector PDF {shapes:?}"
            );
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// A dense scatter, drawn as `render` says, over a vector band.
fn banded(render: MarkRender) -> Figure {
    layered(Theme::pigment(), |a| {
        a.band(
            &[0.0, 10.0],
            &[3.0, 3.0],
            &[7.0, 7.0],
            BandOptions {
                label: "Band".into(),
                ..BandOptions::default()
            },
        )
        .unwrap();
        let (x, y) = points(5000);
        let options = ScatterOptions {
            render,
            ..ScatterOptions::default()
        };
        a.scatter(AxisData::Numbers(x), &y, options).unwrap();
    })
}

/// A scene holding a red square drawn as an image inside `depth` groups.
fn nested_raster(depth: usize) -> Scene {
    let mut square = umber_core::Path::new();
    square.move_to(umber_core::Point { x: 10.0, y: 10.0 });
    square.line_to(umber_core::Point { x: 30.0, y: 10.0 });
    square.line_to(umber_core::Point { x: 30.0, y: 30.0 });
    square.close();
    let mut item = Item::Raster {
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 40.0,
        },
        items: vec![Item::Fill {
            path: square,
            paint: umber_core::Paint::Solid(UNDER),
            rule: umber_core::FillRule::NonZero,
        }],
    };
    for _ in 0..depth {
        item = Item::Group {
            clip: None,
            items: vec![item],
        };
    }
    let mut scene = Scene::new(40.0, 40.0, OVER);
    scene.push(item);
    scene
}

#[test]
fn an_image_of_marks_counts_as_a_group_in_the_nesting_depth() {
    let empty = Scene::new(40.0, 40.0, OVER);
    let deepest = umber_core::scene::MAX_GROUP_DEPTH;
    // Inside the deepest groups drawn, the image is a group too deep, as in PNG output.
    let hidden = nested_raster(deepest);
    assert_eq!(
        render_png(&hidden, 72).unwrap(),
        render_png(&empty, 72).unwrap()
    );
    assert!(svg_images(&render_svg(&hidden, SvgText::Outlines, 72).unwrap()).is_empty());
    assert_eq!(pdf_images(&render_pdf(&hidden, 72).unwrap()), 0);
    let shown = nested_raster(deepest - 1);
    assert_ne!(
        render_png(&shown, 72).unwrap(),
        render_png(&empty, 72).unwrap()
    );
    assert_eq!(
        svg_images(&render_svg(&shown, SvgText::Outlines, 72).unwrap()).len(),
        1
    );
    assert_eq!(pdf_images(&render_pdf(&shown, 72).unwrap()), 1);
}

const WHITE: umber_core::Rgba = umber_core::Rgba {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};

/// A rectangle path from `(left, top)` to `(right, bottom)`.
fn box_path(left: f32, top: f32, right: f32, bottom: f32) -> umber_core::Path {
    let mut path = umber_core::Path::new();
    path.move_to(umber_core::Point { x: left, y: top });
    path.line_to(umber_core::Point { x: right, y: top });
    path.line_to(umber_core::Point {
        x: right,
        y: bottom,
    });
    path.line_to(umber_core::Point { x: left, y: bottom });
    path.close();
    path
}

fn solid(path: umber_core::Path, color: umber_core::Rgba) -> Item {
    Item::Fill {
        path,
        paint: umber_core::Paint::Solid(color),
        rule: umber_core::FillRule::NonZero,
    }
}

/// The page of `pdf` drawn by Poppler and, when it is installed, by MuPDF at `dpi`, each with the
/// reader's name; `png` sets the size, as the readers round the page up.
fn readers(
    dir: &std::path::Path,
    name: &str,
    pdf: &[u8],
    dpi: u32,
    png: &Raster,
) -> Vec<(&'static str, Raster)> {
    let path = write(dir, &format!("{name}.pdf"), pdf);
    let mut pages = vec![("Poppler", poppler_raster_for_png(&path, dpi, png))];
    if have("mutool") {
        pages.push(("MuPDF", mupdf_raster_for_png(&path, dpi, png)));
    }
    pages
}

#[test]
fn readers_that_rescale_the_image_draw_no_dark_halo() {
    if !have("pdftoppm") {
        return;
    }
    let dir = scratch("raster-halo");
    // Shapes of one color with antialiased edges at fractions of a pixel, drawn as an image over
    // a white page: every pixel any reader draws is between the color and white.
    for (color, check) in [
        (
            WHITE,
            (|p: [u8; 3]| p.iter().all(|&c| c >= 250)) as fn([u8; 3]) -> bool,
        ),
        (UNDER, |p: [u8; 3]| p[0] >= 250 && p[1].abs_diff(p[2]) <= 4),
    ] {
        let mut scene = Scene::new(144.0, 72.0, WHITE);
        scene.push(Item::Raster {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 144.0,
                height: 72.0,
            },
            items: vec![
                solid(box_path(10.13, 10.37, 70.61, 40.29), color),
                solid(box_path(80.4, 20.2, 80.9, 60.7), color),
                solid(box_path(100.05, 5.55, 130.95, 6.15), color),
            ],
        });
        let pdf = render_pdf(&scene, 300).unwrap();
        for dpi in [72, 144, 97, 450] {
            let png = png_raster(&render_png(&scene, dpi).unwrap());
            for (reader, page) in readers(&dir, "halo", &pdf, dpi, &png) {
                let (width, height) = page.size();
                let dark = (0..height)
                    .flat_map(|y| (0..width).map(move |x| (x, y)))
                    .filter(|&(x, y)| !check(page.at(x, y)))
                    .count();
                assert_eq!(dark, 0, "{reader} at {dpi} dpi, color {color:?}");
            }
        }
        // The SVG image holds the same padded colors, for browsers, which smooth it too.
        let svg = render_svg(&scene, SvgText::Outlines, 300).unwrap();
        let png = image_pixels(svg_images(&svg)[0]);
        assert!(
            png.rgba
                .chunks_exact(4)
                .all(|p| p[..3] == [color.r, color.g, color.b])
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn at_a_low_raster_dpi_the_image_meets_the_vector_shapes_beside_it() {
    if !have("pdftoppm") {
        return;
    }
    let dir = scratch("raster-low-dpi");
    // Red vector shapes up to x = 72 pt and a red image from there, at 10 dpi, whose pixel edge
    // lies on 72 pt: drawn at 720 dpi, no light column opens between them.
    let mut scene = Scene::new(144.0, 72.0, WHITE);
    scene.push(solid(box_path(0.0, 0.0, 72.0, 72.0), UNDER));
    scene.push(Item::Raster {
        rect: Rect {
            x: 72.0,
            y: 0.0,
            width: 72.0,
            height: 72.0,
        },
        items: vec![solid(box_path(72.0, 0.0, 144.0, 72.0), UNDER)],
    });
    let pdf = render_pdf(&scene, 10).unwrap();
    // The image is placed one step of the written coordinates, 0.001 pt, inside its pixel
    // edges, which lie on 72 and 144 pt across and on 0 and 72 pt down.
    let content = page_content(&pdf);
    let place = content
        .lines()
        .take_while(|line| !line.ends_with(" Do"))
        .filter(|line| line.ends_with(" cm"))
        .last()
        .unwrap();
    let numbers: Vec<f64> = place
        .split(' ')
        .take(6)
        .map(|v| v.parse().unwrap())
        .collect();
    let [width, _, _, height, left, bottom] = numbers[..] else {
        panic!("{place}");
    };
    let (right, top) = (left + width, bottom + height);
    for (edge, at) in [(left, 72.0), (right, 144.0), (top, 0.0), (bottom, 72.0)] {
        assert!((edge - at).abs() <= 0.001 + 1e-9, "{place}");
    }
    let png = png_raster(&render_png(&scene, 720).unwrap());
    for (reader, page) in readers(&dir, "low", &pdf, 720, &png) {
        let (_, height) = page.size();
        for y in (10..height - 10).step_by(37) {
            for x in 700..740 {
                let [r, g, b] = page.at(x, y);
                assert!(
                    r >= 240 && g <= 30 && b <= 30,
                    "{reader}: ({x}, {y}) is {r} {g} {b}"
                );
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn deep_nesting_inside_an_image_of_marks_is_cut_at_the_nesting_limit() {
    // A fill inside 20,000 groups, or 20,000 images of marks, inside an image of marks: the
    // content past the nesting limit is not drawn, and nothing walks it, in every backend.
    for raster_groups in [false, true] {
        let mut item = solid(box_path(10.0, 10.0, 30.0, 30.0), UNDER);
        let whole = Rect {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 40.0,
        };
        for _ in 0..20_000 {
            item = if raster_groups {
                Item::Raster {
                    rect: whole,
                    items: vec![item],
                }
            } else {
                Item::Group {
                    clip: None,
                    items: vec![item],
                }
            };
        }
        let mut scene = Scene::new(40.0, 40.0, OVER);
        scene.push(Item::Raster {
            rect: whole,
            items: vec![item],
        });
        let empty = Scene::new(40.0, 40.0, OVER);
        assert_eq!(
            render_png(&scene, 72).unwrap(),
            render_png(&empty, 72).unwrap()
        );
        let svg = render_svg(&scene, SvgText::Outlines, 300).unwrap();
        assert_eq!(svg, render_svg(&empty, SvgText::Outlines, 300).unwrap());
        assert_eq!(pdf_images(&render_pdf(&scene, 300).unwrap()), 0);
    }
}

/// The largest mean channel difference between the PNG of a figure at 300 dpi, resampled by
/// area to 72, 100, 144 or 200 dpi, and its PNG drawn at that resolution: what a reader that
/// resampled the 300 dpi image perfectly would show. The largest measured is 1.81 (a scatter of
/// 5,000 points over a band, at 72 dpi), with 0.002% of pixels over 64 apart; reader
/// comparisons beyond this are the readers' resampling, not the image.
const MAX_CONTROL_MEAN: f64 = 2.0;
/// The largest share of pixels with a channel more than 64 apart between the two.
const MAX_CONTROL_FAR: f64 = 0.0005;

#[test]
fn an_image_resampled_by_area_is_close_to_the_png_at_the_lower_resolution() {
    let mut cases = pairs();
    cases.push((
        "scatter over a band",
        banded(MarkRender::Raster),
        banded(MarkRender::Vector),
    ));
    for (name, raster, _) in cases {
        let high = png_raster(&raster.to_png(300).unwrap());
        for dpi in [72, 100, 144, 200] {
            let png = png_raster(&raster.to_png(i64::from(dpi)).unwrap());
            let control = area_downsampled(&high, 300, dpi, png.size());
            let (mean, far) = raster_difference(&control, &png);
            eprintln!(
                "{name}: 300 dpi resampled to {dpi} dpi against its PNG: {mean:.4} / {far:.5}"
            );
            assert!(
                mean <= MAX_CONTROL_MEAN && far <= MAX_CONTROL_FAR,
                "{name} at {dpi} dpi: {mean} / {far}"
            );
        }
    }
}
