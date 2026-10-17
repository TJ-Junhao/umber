//! Coverage of the final drawn geometry, used only for legends inside a plot.
//!
//! Points and segments are counted once. Filled paths are measured in square points by
//! horizontal scanlines at most one point apart, with the path's own winding rule. This also
//! handles overlapping contours and areas that cross themselves, rather than cancelling their
//! signed areas. Curves in rounded bar corners are flattened before scoring. Images count the
//! exact area of their pixels that are not transparent.

use crate::layout::{Area, Frame};
use crate::{FillRule, Item, Paint, Path, PathVerb, Point};

/// A point in the final scene, widened to f64 for intersections.
type XY = [f64; 2];

/// One filled path, whose contours share a winding rule.
#[derive(Debug, Clone, PartialEq)]
struct Filled {
    contours: Vec<Vec<XY>>,
    rule: FillRule,
}

/// The pixels of one image that are not transparent, each a rectangle of its share of the
/// image's rectangle.
#[derive(Debug, Clone, PartialEq)]
struct Pixels {
    /// The left, top, right and bottom edges of the image.
    bounds: [f64; 4],
    /// The number of pixel columns.
    width: usize,
    /// Whether each pixel, row by row from the top, is drawn (not transparent).
    drawn: Vec<bool>,
}

impl Pixels {
    /// The area of the drawn pixels inside `box_`, in square points.
    fn overlap(&self, box_: &Frame) -> f64 {
        let [left, top, right, bottom] = self.bounds;
        let height = self.drawn.len() / self.width.max(1);
        // The overlap of each column with the box along x, and of each row along y.
        let lengths = |start: f64, end: f64, count: usize, lo: f64, hi: f64| -> Vec<f64> {
            let size = (end - start) / count as f64;
            (0..count)
                .map(|i| {
                    let (a, b) = (start + size * i as f64, start + size * (i + 1) as f64);
                    (b.min(hi) - a.max(lo)).max(0.0)
                })
                .collect()
        };
        let columns = lengths(left, right, self.width, box_.x, box_.right());
        let rows = lengths(top, bottom, height, box_.y, box_.bottom());
        let mut area = 0.0;
        for (row, &along_y) in rows.iter().enumerate() {
            if along_y == 0.0 {
                continue;
            }
            let drawn = &self.drawn[row * self.width..(row + 1) * self.width];
            let along_x: f64 = columns
                .iter()
                .zip(drawn)
                .filter(|&(_, &d)| d)
                .map(|(&x, _)| x)
                .sum();
            area += along_x * along_y;
        }
        area
    }
}

/// Geometry after axis mapping, clipping and downsampling, in drawing order within each kind.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Geometry {
    points: Vec<XY>,
    segments: Vec<[XY; 2]>,
    fills: Vec<Filled>,
    images: Vec<Pixels>,
}

fn xy(p: Point) -> XY {
    [f64::from(p.x), f64::from(p.y)]
}

fn visible(paint: &Paint) -> bool {
    match paint {
        Paint::Solid(color) => color.a > 0,
        Paint::Linear(gradient) => gradient.stops.iter().any(|s| s.color.a > 0),
        Paint::Sampled { .. } => true,
    }
}

impl Geometry {
    /// Records the same scene primitives used for drawing a resolved mark.
    pub fn of(item: &Item) -> Self {
        let mut geometry = Self::default();
        geometry.append(item);
        geometry
    }

    fn append(&mut self, item: &Item) {
        match item {
            Item::Group { items, .. } => {
                for item in items {
                    self.append(item);
                }
            }
            Item::Markers {
                positions,
                fill,
                stroke,
                ..
            } => {
                if fill.is_some_and(|c| c.a > 0)
                    || stroke
                        .as_ref()
                        .is_some_and(|s| s.color.a > 0 && s.width > 0.0)
                {
                    self.points.extend(positions.iter().copied().map(xy));
                }
            }
            Item::Stroke { path, style } => {
                if style.color.a > 0 && style.width > 0.0 {
                    for contour in contours(path) {
                        self.segments
                            .extend(contour.windows(2).map(|p| [p[0], p[1]]));
                    }
                }
            }
            Item::Fill { path, paint, rule } => {
                if visible(paint) {
                    self.fills.push(Filled {
                        contours: contours(path),
                        rule: *rule,
                    });
                }
            }
            Item::Image {
                rect,
                width,
                height,
                pixels,
            } => {
                if *width > 0 && pixels.len() == (*width as usize) * (*height as usize) {
                    let (x, y) = (f64::from(rect.x), f64::from(rect.y));
                    self.images.push(Pixels {
                        bounds: [x, y, x + f64::from(rect.width), y + f64::from(rect.height)],
                        width: *width as usize,
                        drawn: pixels.iter().map(|c| c.a > 0).collect(),
                    });
                }
            }
            Item::Text(_) => {}
        }
    }

    /// Counts only geometry inside both the plot and the candidate box.
    pub fn coverage(&self, area: &Area, box_: &Frame) -> f64 {
        let left = area.x.max(box_.x);
        let top = area.y.max(box_.y);
        let right = area.right().min(box_.right());
        let bottom = area.bottom().min(box_.bottom());
        if right <= left || bottom <= top {
            return 0.0;
        }
        let bounds = Frame {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        };
        let points = self.points.iter().filter(|p| inside(**p, &bounds)).count() as f64;
        let segments = self
            .segments
            .iter()
            .filter(|s| crosses(**s, &bounds))
            .count() as f64;
        let fills = self.fills.iter().fold(points + segments, |score, fill| {
            score + fill.overlap(&bounds)
        });
        self.images
            .iter()
            .fold(fills, |score, image| score + image.overlap(&bounds))
    }
}

fn inside(p: XY, b: &Frame) -> bool {
    p[0] >= b.x && p[0] <= b.right() && p[1] >= b.y && p[1] <= b.bottom()
}

/// Slab intersection includes segments whose two endpoints lie outside the candidate.
fn crosses([a, b]: [XY; 2], box_: &Frame) -> bool {
    let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
    for (axis, lo, hi) in [(0, box_.x, box_.right()), (1, box_.y, box_.bottom())] {
        let delta = b[axis] - a[axis];
        if delta == 0.0 {
            if a[axis] < lo || a[axis] > hi {
                return false;
            }
        } else {
            let t0 = (lo - a[axis]) / delta;
            let t1 = (hi - a[axis]) / delta;
            enter = enter.max(t0.min(t1));
            leave = leave.min(t0.max(t1));
            if enter > leave {
                return false;
            }
        }
    }
    true
}

/// Flatten only the curves used in filled bar corners; straight data segments stay unchanged.
fn contours(path: &Path) -> Vec<Vec<XY>> {
    let mut all = Vec::new();
    let mut run = Vec::new();
    for verb in path.verbs() {
        match *verb {
            PathVerb::MoveTo(p) => {
                if !run.is_empty() {
                    all.push(std::mem::take(&mut run));
                }
                run.push(xy(p));
            }
            PathVerb::LineTo(p) => run.push(xy(p)),
            PathVerb::QuadTo(c, p) => {
                if let Some(&start) = run.last() {
                    let (c, p) = (xy(c), xy(p));
                    for i in 1..=16 {
                        let t = f64::from(i) / 16.0;
                        let u = 1.0 - t;
                        run.push(std::array::from_fn(|j| {
                            u * u * start[j] + 2.0 * u * t * c[j] + t * t * p[j]
                        }));
                    }
                }
            }
            PathVerb::CubicTo(c1, c2, p) => {
                if let Some(&start) = run.last() {
                    let (c1, c2, p) = (xy(c1), xy(c2), xy(p));
                    for i in 1..=16 {
                        let t = f64::from(i) / 16.0;
                        let u = 1.0 - t;
                        run.push(std::array::from_fn(|j| {
                            u * u * u * start[j]
                                + 3.0 * u * u * t * c1[j]
                                + 3.0 * u * t * t * c2[j]
                                + t * t * t * p[j]
                        }));
                    }
                }
            }
            PathVerb::Close => {
                if let Some(&first) = run.first() {
                    run.push(first);
                    all.push(std::mem::take(&mut run));
                }
            }
        }
    }
    if !run.is_empty() {
        all.push(run);
    }
    all
}

impl Filled {
    /// Filled horizontal intervals are integrated at no more than 1 pt spacing. Include every
    /// vertex and every crossing of a vertical box edge as a row boundary: for non-crossing
    /// polygon edges the midpoint integration is exact, even on sub-point slivers.
    fn overlap(&self, box_: &Frame) -> f64 {
        let mut edges = Vec::new();
        let mut left_events = Vec::new();
        let mut rows = vec![box_.y, box_.bottom()];
        for contour in &self.contours {
            for (&a, &b) in contour
                .iter()
                .zip(contour.iter().cycle().skip(1))
                .take(contour.len())
            {
                if a[1] == b[1] || a[1].max(b[1]) <= box_.y || a[1].min(b[1]) >= box_.bottom() {
                    continue;
                }
                // Keep the old quadrature boundaries, even for vertices outside the box.
                // Dropping their y values changes the samples on self-intersecting paths.
                // The vertices and edges themselves are culled from the scan below.
                for y in [a[1], b[1]] {
                    if y > box_.y && y < box_.bottom() {
                        rows.push(y);
                    }
                }
                let direction = if b[1] > a[1] { 1_i64 } else { -1_i64 };
                if a[0].max(b[0]) < box_.x {
                    // Entirely left edges affect winding, but never an intersection's x.
                    left_events.push((a[1].min(b[1]), direction));
                    left_events.push((a[1].max(b[1]), -direction));
                    continue;
                }
                if a[0].min(b[0]) > box_.right() {
                    continue;
                }
                edges.push([a, b]);
                if a[0] != b[0] {
                    for x in [box_.x, box_.right()] {
                        let t = (x - a[0]) / (b[0] - a[0]);
                        let y = a[1] + t * (b[1] - a[1]);
                        if t > 0.0 && t < 1.0 && y > box_.y && y < box_.bottom() {
                            rows.push(y);
                        }
                    }
                }
            }
        }
        rows.sort_by(f64::total_cmp);
        rows.dedup();
        edges.sort_by(|[a, b], [c, d]| a[1].min(b[1]).total_cmp(&c[1].min(d[1])));
        left_events.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (mut next_edge, mut next_left, mut left_winding) = (0, 0, 0_i64);
        let mut active: Vec<[XY; 2]> = Vec::new();
        let mut area = 0.0;
        let mut hits = Vec::new();
        for pair in rows.windows(2) {
            let height = pair[1] - pair[0];
            let count = height.ceil().max(1.0) as usize;
            let dy = height / count as f64;
            for i in 0..count {
                let y = pair[0] + (i as f64 + 0.5) * dy;
                active.retain(|[a, b]| y < a[1].max(b[1]));
                while let Some(&[a, b]) = edges.get(next_edge) {
                    if a[1].min(b[1]) > y {
                        break;
                    }
                    if y < a[1].max(b[1]) {
                        active.push([a, b]);
                    }
                    next_edge += 1;
                }
                while let Some(&(at, direction)) = left_events.get(next_left) {
                    if at > y {
                        break;
                    }
                    left_winding += direction;
                    next_left += 1;
                }
                hits.clear();
                for &[a, b] in &active {
                    let x = a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                    hits.push((x, if b[1] > a[1] { 1_i64 } else { -1_i64 }));
                }
                hits.sort_by(|a, b| a.0.total_cmp(&b.0));
                let (mut winding, mut previous, mut width) = (left_winding, box_.x, 0.0);
                for &(x, direction) in &hits {
                    let filled = match self.rule {
                        FillRule::NonZero => winding != 0,
                        FillRule::EvenOdd => winding % 2 != 0,
                    };
                    if filled {
                        width += (x.min(box_.right()) - previous.max(box_.x)).max(0.0);
                    }
                    winding += direction;
                    previous = x;
                }
                let filled = match self.rule {
                    FillRule::NonZero => winding != 0,
                    FillRule::EvenOdd => winding % 2 != 0,
                };
                if filled {
                    width += (box_.right() - previous.max(box_.x)).max(0.0);
                }
                area += width * dy;
            }
        }
        area
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original full scan, to pin the sampling as well as the winding semantics.
    fn unculled(fill: &Filled, box_: &Frame) -> f64 {
        let mut edges = Vec::new();
        let mut rows = vec![box_.y, box_.bottom()];
        for contour in &fill.contours {
            for (&a, &b) in contour
                .iter()
                .zip(contour.iter().cycle().skip(1))
                .take(contour.len())
            {
                if a[1] == b[1] || a[1].max(b[1]) <= box_.y || a[1].min(b[1]) >= box_.bottom() {
                    continue;
                }
                edges.push([a, b]);
                for y in [a[1], b[1]] {
                    if y > box_.y && y < box_.bottom() {
                        rows.push(y);
                    }
                }
                if a[0] != b[0] {
                    for x in [box_.x, box_.right()] {
                        let t = (x - a[0]) / (b[0] - a[0]);
                        let y = a[1] + t * (b[1] - a[1]);
                        if t > 0.0 && t < 1.0 && y > box_.y && y < box_.bottom() {
                            rows.push(y);
                        }
                    }
                }
            }
        }
        rows.sort_by(f64::total_cmp);
        rows.dedup();
        let mut area = 0.0;
        let mut hits = Vec::new();
        for pair in rows.windows(2) {
            let height = pair[1] - pair[0];
            let count = height.ceil().max(1.0) as usize;
            let dy = height / count as f64;
            for i in 0..count {
                let y = pair[0] + (i as f64 + 0.5) * dy;
                hits.clear();
                for &[a, b] in &edges {
                    if y >= a[1].min(b[1]) && y < a[1].max(b[1]) {
                        let x = a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                        hits.push((x, if b[1] > a[1] { 1_i64 } else { -1_i64 }));
                    }
                }
                hits.sort_by(|a, b| a.0.total_cmp(&b.0));
                let (mut winding, mut previous, mut width) = (0_i64, box_.x, 0.0);
                for &(x, direction) in &hits {
                    let filled = match fill.rule {
                        FillRule::NonZero => winding != 0,
                        FillRule::EvenOdd => winding % 2 != 0,
                    };
                    if filled {
                        width += (x.min(box_.right()) - previous.max(box_.x)).max(0.0);
                    }
                    winding += direction;
                    previous = x;
                }
                area += width * dy;
            }
        }
        area
    }

    #[test]
    #[ignore = "large-data coverage comparison; run explicitly in release"]
    fn large_marks_preserve_coverage_bits() {
        use crate::{
            AreaOptions, AxisData, BandOptions, Figure, FontLibrary, LegendPosition, LegendStyle,
            LineOptions, ScaleKind, Series, Theme,
        };
        let library = FontLibrary::global();
        for (kind, n) in [
            ("line", 1_000_000),
            ("area", 1_000_000),
            ("band", 1_000_000),
            ("stacked_area", 100_000),
        ] {
            let x: Vec<_> = (0..n).map(|i| i as f64 / n as f64 * 100.0).collect();
            let y: Vec<_> = (0..n)
                .map(|i| {
                    let mut h = (i as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
                    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                    h ^= h >> 31;
                    let unit = (h >> 11) as f64 / (1u64 << 53) as f64;
                    (i as f64 * 0.001).sin() * 10.0 + unit * 2.0
                })
                .collect();
            let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let mut axes = figure
                .add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap();
            axes.legend_at(LegendStyle::Header, LegendPosition::Best)
                .unwrap();
            match kind {
                "line" => {
                    axes.line(
                        AxisData::Numbers(x),
                        &y,
                        LineOptions {
                            label: "Signal".into(),
                            ..LineOptions::default()
                        },
                    )
                    .unwrap();
                }
                "area" => {
                    axes.area(
                        AxisData::Numbers(x),
                        &y,
                        AreaOptions {
                            label: "Signal".into(),
                            ..AreaOptions::default()
                        },
                    )
                    .unwrap();
                }
                "band" => {
                    axes.band(
                        &x,
                        &y,
                        &y.iter().map(|v| v + 2.0).collect::<Vec<_>>(),
                        BandOptions {
                            label: "Signal".into(),
                            ..BandOptions::default()
                        },
                    )
                    .unwrap();
                }
                "stacked_area" => {
                    axes.stacked_area(
                        &x,
                        (0..3)
                            .map(|k| Series {
                                label: format!("Layer {k}"),
                                values: y.iter().map(|v| v + 12.0 + k as f64).collect(),
                                ..Series::default()
                            })
                            .collect(),
                    )
                    .unwrap();
                }
                _ => unreachable!(),
            }
            let (resolved, layout) = crate::pipeline::plan_figure(&figure, (0, 0), library);
            let area = layout.plot;
            let background = layout.legend.background.unwrap();
            let left = area.x + 6.0;
            let right = area.right() - 6.0 - background.width;
            let middle_x = area.x + (area.width - background.width) / 2.0;
            let top = area.y + 6.0;
            let bottom = area.bottom() - 6.0 - background.height;
            let middle_y = area.y + (area.height - background.height) / 2.0;
            let mut checked = 0;
            for mark in crate::resolve::resolve_marks(figure.single_axes().marks(), figure.theme())
            {
                let item = crate::draw::marks(
                    figure.theme(),
                    &resolved,
                    &layout,
                    std::slice::from_ref(&mark),
                )
                .unwrap();
                let geometry = Geometry::of(&item);
                for (x, y) in [
                    (right, top),
                    (left, top),
                    (right, bottom),
                    (left, bottom),
                    (middle_x, top),
                    (right, middle_y),
                    (middle_x, bottom),
                    (left, middle_y),
                ] {
                    let box_ = Frame {
                        x,
                        y,
                        width: background.width,
                        height: background.height,
                    };
                    // Match Geometry::coverage's intersection arithmetic before comparing bits.
                    let bounds = Frame {
                        x,
                        y,
                        width: area.right().min(box_.right()) - x,
                        height: area.bottom().min(box_.bottom()) - y,
                    };
                    let count = geometry
                        .points
                        .iter()
                        .filter(|p| inside(**p, &bounds))
                        .count() as f64
                        + geometry
                            .segments
                            .iter()
                            .filter(|s| crosses(**s, &bounds))
                            .count() as f64;
                    let expected = geometry
                        .fills
                        .iter()
                        .fold(count, |score, fill| score + unculled(fill, &bounds));
                    let actual = geometry.coverage(&area, &box_);
                    assert_eq!(actual.to_bits(), expected.to_bits(), "{kind} {box_:?}");
                    checked += 1;
                }
            }
            println!("{kind}: n={n}, {checked} candidate coverage values bit-identical");
        }
    }

    #[test]
    fn culled_scan_preserves_coverage_bits() {
        let mut state = 1_u64;
        let mut coordinate = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((state >> 32) % 2000) as f64 / 100.0 - 10.0
        };
        for _ in 0..128 {
            let contours = (0..3)
                .map(|_| (0..12).map(|_| [coordinate(), coordinate()]).collect())
                .collect::<Vec<Vec<XY>>>();
            for rule in [FillRule::NonZero, FillRule::EvenOdd] {
                let fill = Filled {
                    contours: contours.clone(),
                    rule,
                };
                for x in [-8.0, -2.0, 4.0] {
                    let box_ = Frame {
                        x,
                        y: -3.0,
                        width: 4.0,
                        height: 7.0,
                    };
                    assert_eq!(
                        fill.overlap(&box_).to_bits(),
                        unculled(&fill, &box_).to_bits(),
                        "{rule:?} {box_:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn culled_edges_keep_enclosing_fills_holes_and_slivers() {
        let outer = vec![
            [-100.0, -100.0],
            [100.0, -100.0],
            [100.0, 100.0],
            [-100.0, 100.0],
        ];
        let hole = vec![[-50.0, -50.0], [-50.0, 50.0], [50.0, 50.0], [50.0, -50.0]];
        for contours in [
            vec![outer.clone()],
            vec![outer.clone(), hole],
            vec![outer.clone(), outer],
            vec![vec![[-100.0, 0.0], [100.0, 0.0001], [-100.0, 0.0002]]],
            vec![vec![
                [-100.0, -2.0],
                [100.0, 2.0],
                [-100.0, 2.0],
                [100.0, -2.0],
            ]],
        ] {
            for rule in [FillRule::NonZero, FillRule::EvenOdd] {
                let fill = Filled {
                    contours: contours.clone(),
                    rule,
                };
                let box_ = Frame {
                    x: -2.0,
                    y: -3.0,
                    width: 4.0,
                    height: 7.0,
                };
                assert_eq!(
                    fill.overlap(&box_).to_bits(),
                    unculled(&fill, &box_).to_bits()
                );
            }
        }
    }

    #[test]
    fn crossing_segments_and_points_count_once() {
        let area = Area {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let box_ = Frame {
            x: 40.0,
            y: 40.0,
            width: 20.0,
            height: 20.0,
        };
        let geometry = Geometry {
            points: vec![[50.0, 50.0], [10.0, 10.0]],
            segments: vec![[[0.0, 50.0], [100.0, 50.0]], [[0.0, 0.0], [10.0, 10.0]]],
            fills: Vec::new(),
            images: Vec::new(),
        };
        assert_eq!(geometry.coverage(&area, &box_), 2.0);
    }

    #[test]
    fn filled_overlap_uses_square_points_and_winding() {
        let fill = Filled {
            contours: vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]],
            rule: FillRule::NonZero,
        };
        let box_ = Frame {
            x: 5.0,
            y: 5.0,
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(fill.overlap(&box_), 25.0);
        let twice = Filled {
            contours: [fill.contours.clone(), fill.contours].concat(),
            rule: FillRule::EvenOdd,
        };
        assert_eq!(twice.overlap(&box_), 0.0);
    }
}
