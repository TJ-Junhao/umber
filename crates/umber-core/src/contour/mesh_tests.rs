//! Properties of the contour geometry on a triangle mesh: one triangle against every kind of
//! level, crossings shared by neighbours, lines that join, close and end on the boundary,
//! holes, bands that nest and cover the mesh once, levels equal to values, and independence of
//! the order and winding of the triangles.

use std::collections::HashSet;

use proptest::prelude::*;

use super::*;
use crate::tri::{TriMesh, Triangles};
use crate::{ContourStyle, Transform, delaunay};

/// The mesh of the points `(x[i], y[i])` joined by `rows`.
fn mesh(x: &[f64], y: &[f64], rows: &[[i64; 3]]) -> TriMesh {
    let given = Triangles::Given(rows);
    match TriMesh::new("test", [Transform::Linear; 2], (x, y), given) {
        Ok((mesh, _)) => mesh,
        Err(error) => panic!("{error}"),
    }
}

/// The data point of `p` on the mesh of the points `(x[i], y[i])`.
fn at(x: &[f64], y: &[f64], p: MeshPoint) -> (f64, f64) {
    match p {
        MeshPoint::Corner(i) => (x[i], y[i]),
        MeshPoint::Edge { from, to, t } => (
            x[from] + (x[to] - x[from]) * t,
            y[from] + (y[to] - y[from]) * t,
        ),
    }
}

/// The value of the field `z` at `p`, interpolated along its edge.
fn value(z: &[f64], p: MeshPoint) -> f64 {
    match p {
        MeshPoint::Corner(i) => z[i],
        MeshPoint::Edge { from, to, t } => z[from] + (z[to] - z[from]) * t,
    }
}

/// The signed area of the closed polygon `points`, positive counter-clockwise with y up.
fn signed_area(points: &[(f64, f64)]) -> f64 {
    (0..points.len())
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0
}

/// The area of a band: the sum of the signed areas of its rings.
fn band_area(x: &[f64], y: &[f64], band: &BandPolygons<MeshPoint>) -> f64 {
    band.rings
        .iter()
        .map(|ring| {
            let points: Vec<(f64, f64)> = ring.iter().map(|&p| at(x, y, p)).collect();
            signed_area(&points)
        })
        .sum()
}

/// The area of the part of the triangles of `mesh` whose values `z` are all finite where the
/// field is at or above `level` (every such triangle for `None`), computed triangle by triangle
/// apart from the module's code: each triangle clipped to the half-space of its linear field.
fn high_area(mesh: &TriMesh, z: &[f64], level: Option<f64>) -> f64 {
    let (x, y) = (mesh.x(), mesh.y());
    mesh.triangles()
        .iter()
        .filter(|corners| corners.iter().all(|&i| z[i].is_finite()))
        .map(|&corners| {
            let points = corners.map(|i| (x[i], y[i], z[i]));
            let Some(level) = level else {
                return signed_area(&points.map(|(x, y, _)| (x, y))).abs();
            };
            let mut clipped = Vec::new();
            for k in 0..3 {
                let (a, b) = (points[k], points[(k + 1) % 3]);
                if a.2 >= level {
                    clipped.push((a.0, a.1));
                }
                if (a.2 >= level) != (b.2 >= level) {
                    let t = (level - a.2) / (b.2 - a.2);
                    clipped.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
                }
            }
            if clipped.len() < 3 {
                0.0
            } else {
                signed_area(&clipped).abs()
            }
        })
        .sum()
}

/// The edges of `mesh` that border one triangle whose values `z` are all finite, each as its
/// two point indices in increasing order.
fn boundary_edges(mesh: &TriMesh, z: &[f64]) -> HashSet<[usize; 2]> {
    let mut seen: HashSet<[usize; 2]> = HashSet::new();
    let mut twice: HashSet<[usize; 2]> = HashSet::new();
    for &[a, b, c] in mesh.triangles() {
        if [a, b, c].iter().any(|&i| !z[i].is_finite()) {
            continue;
        }
        for edge in [[a, b], [a, c], [b, c]] {
            if !seen.insert(edge) {
                twice.insert(edge);
            }
        }
    }
    seen.difference(&twice).copied().collect()
}

/// Whether `p` lies on one of the edges `edges`.
fn on_edges(edges: &HashSet<[usize; 2]>, p: MeshPoint) -> bool {
    match p {
        MeshPoint::Edge { from, to, .. } => edges.contains(&[from, to]),
        MeshPoint::Corner(i) => edges.iter().any(|edge| edge.contains(&i)),
    }
}

/// The shapes of `style` for the values `z` on `mesh` at `levels`.
fn shapes(
    mesh: &TriMesh,
    z: &[f64],
    levels: &[f64],
    style: ContourStyle,
) -> ContourShapes<MeshPoint> {
    ContourShapes::of(&TriField::new(mesh, z), levels, style)
}

/// One triangle, counter-clockwise, at (0, 0), (1, 0) and (0, 1).
const X1: [f64; 3] = [0.0, 1.0, 0.0];
const Y1: [f64; 3] = [0.0, 0.0, 1.0];

/// The lines and the region of the one triangle with the values `z` at `level`.
fn one_triangle(z: [f64; 3], level: f64) -> Traced<MeshPoint> {
    let mesh = mesh(&X1, &Y1, &[[0, 1, 2]]);
    TriField::new(&mesh, &z).trace(level, ContourStyle::Both)
}

#[test]
fn one_triangle_against_every_kind_of_level() {
    let z = [0.0, 1.0, 2.0];
    // Below every value: no line, and the region is the whole triangle.
    let below = one_triangle(z, -1.0);
    assert!(below.lines.is_empty());
    assert_eq!(
        below.rings,
        [vec![
            MeshPoint::Corner(0),
            MeshPoint::Corner(1),
            MeshPoint::Corner(2)
        ]]
    );
    // Above every value: nothing.
    let above = one_triangle(z, 3.0);
    assert!(above.lines.is_empty() && above.rings.is_empty());
    // Through it: one straight piece, high side on its left, from edge to edge.
    let through = one_triangle(z, 0.5);
    let half = MeshPoint::Edge {
        from: 0,
        to: 1,
        t: 0.5,
    };
    let quarter = MeshPoint::Edge {
        from: 0,
        to: 2,
        t: 0.25,
    };
    assert_eq!(
        through.lines,
        [Polyline {
            points: vec![quarter, half],
            closed: false
        }]
    );
    assert_eq!(
        through.rings,
        [vec![
            quarter,
            half,
            MeshPoint::Corner(1),
            MeshPoint::Corner(2)
        ]]
    );
    // At a corner: the line starts at the corner itself.
    let corner = one_triangle(z, 1.0);
    let middle = MeshPoint::Edge {
        from: 0,
        to: 2,
        t: 0.5,
    };
    assert_eq!(
        corner.lines,
        [Polyline {
            points: vec![middle, MeshPoint::Corner(1)],
            closed: false
        }]
    );
    assert_eq!(
        corner.rings,
        [vec![middle, MeshPoint::Corner(1), MeshPoint::Corner(2)]]
    );
    // The only high corner on the level, or a whole edge on it: no area, so nothing.
    for z in [[0.0, 0.0, 1.0], [1.0, 1.0, 0.0], [1.0, 0.0, 1.0]] {
        let traced = one_triangle(z, 1.0);
        assert!(traced.lines.is_empty() && traced.rings.is_empty(), "{z:?}");
    }
    // A flat triangle on the level is wholly high: the whole region, and no line.
    let flat = one_triangle([1.0; 3], 1.0);
    assert!(flat.lines.is_empty());
    assert_eq!(flat.rings.len(), 1);
    assert_eq!(flat.rings[0].len(), 3);
}

#[test]
fn an_edge_on_the_level_carries_a_line_only_between_two_bands() {
    // A unit square split along its diagonal from point 0 to point 2.
    let (x, y) = ([0.0, 1.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
    let square = mesh(&x, &y, &[[0, 1, 2], [0, 2, 3]]);
    // A flat triangle beside a lower one: one line along the diagonal, once, with the flat
    // triangle on its left.
    let plateau = TriField::new(&square, &[1.0, 1.0, 1.0, 0.0]).trace(1.0, ContourStyle::Both);
    assert_eq!(
        plateau.lines,
        [Polyline {
            points: vec![MeshPoint::Corner(2), MeshPoint::Corner(0)],
            closed: false
        }]
    );
    assert_eq!(plateau.rings.len(), 1);
    assert!(
        (band_area(
            &x,
            &y,
            &BandPolygons {
                rings: plateau.rings
            }
        ) - 0.5)
            .abs()
            < 1e-12
    );
    // A ridge on the level with lower values on both sides: no area, so no line and no region.
    let ridge = TriField::new(&square, &[1.0, 0.0, 1.0, 0.0]).trace(1.0, ContourStyle::Both);
    assert!(ridge.lines.is_empty() && ridge.rings.is_empty());
    // Higher values on both sides: the diagonal is inside the region.
    let valley = TriField::new(&square, &[1.0, 2.0, 1.0, 2.0]).trace(1.0, ContourStyle::Both);
    assert!(valley.lines.is_empty());
    assert_eq!(valley.rings.len(), 1);
}

/// `count` deterministic values from 0 to 1.
fn uniform(count: usize, seed: u64) -> Vec<f64> {
    let mut state = seed;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1_u64 << 53) as f64
        })
        .collect()
}

/// Scattered points in the square from -1 to 1, with its four corners, and their Delaunay
/// triangles whose centres lie outside the circle of radius `hole` around the origin.
fn scattered(count: usize, seed: u64, hole: f64) -> (Vec<f64>, Vec<f64>, Vec<[i64; 3]>) {
    let (u, v) = (uniform(count, seed), uniform(count, seed + 1));
    let mut x: Vec<f64> = vec![-1.0, 1.0, 1.0, -1.0];
    let mut y: Vec<f64> = vec![-1.0, -1.0, 1.0, 1.0];
    x.extend(u.iter().map(|u| 2.0 * u - 1.0));
    y.extend(v.iter().map(|v| 2.0 * v - 1.0));
    let rows = match delaunay(&x, &y) {
        Ok(rows) => rows,
        Err(error) => panic!("{error}"),
    };
    let rows = rows
        .into_iter()
        .filter(|row| {
            let [a, b, c] = row.map(|i| i as usize);
            let (cx, cy) = ((x[a] + x[b] + x[c]) / 3.0, (y[a] + y[b] + y[c]) / 3.0);
            cx.hypot(cy) >= hole
        })
        .collect();
    (x, y, rows)
}

#[test]
fn lines_join_across_the_mesh_close_into_loops_and_end_on_its_edges() {
    let (x, y, rows) = scattered(300, 7, 0.0);
    let square = mesh(&x, &y, &rows);
    // Circles around the origin: closed loops, each repeating its first point.
    let bowl: Vec<f64> = x.iter().zip(&y).map(|(x, y)| x * x + y * y).collect();
    let rings = shapes(&square, &bowl, &[0.25, 0.5], ContourStyle::Lines);
    for lines in &rings.lines {
        assert_eq!(lines.len(), 1, "{lines:?}");
        let line = &lines[0];
        assert!(line.closed);
        assert_eq!(line.points.first(), line.points.last());
    }
    // A slope cut by a hole: open lines, each from edge to edge of the mesh or the hole.
    let (x, y, rows) = scattered(300, 7, 0.4);
    let holed = mesh(&x, &y, &rows);
    let slope: Vec<f64> = x.clone();
    let edges = boundary_edges(&holed, &slope);
    let cut = shapes(&holed, &slope, &[-0.7, 0.0, 0.7], ContourStyle::Lines);
    // Away from the hole one line crosses the mesh; through it, two.
    let counts: Vec<usize> = cut.lines.iter().map(Vec::len).collect();
    assert_eq!(counts, [1, 2, 1]);
    for line in cut.lines.iter().flatten() {
        assert!(!line.closed);
        let (first, last) = (line.points[0], line.points[line.points.len() - 1]);
        assert!(
            on_edges(&edges, first) && on_edges(&edges, last),
            "{line:?}"
        );
        // Every point between lies on an edge inside the mesh.
        for &p in &line.points[1..line.points.len() - 1] {
            assert!(!on_edges(&edges, p), "{p:?}");
        }
    }
}

#[test]
fn neighbouring_triangles_share_their_crossings_bit_for_bit() {
    let (x, y, rows) = scattered(200, 11, 0.0);
    let square = mesh(&x, &y, &rows);
    let z: Vec<f64> = x
        .iter()
        .zip(&y)
        .map(|(x, y)| (3.0 * x).sin() * (2.0 * y).cos())
        .collect();
    let edges = boundary_edges(&square, &z);
    let levels = [-0.5, -0.1, 0.0, 0.3, 0.7];
    let drawn = shapes(&square, &z, &levels, ContourStyle::Both);
    for (lines, &level) in drawn.lines.iter().zip(&levels) {
        let mut crossings: HashSet<(usize, usize)> = HashSet::new();
        for line in lines {
            for &p in &line.points {
                if let MeshPoint::Edge { from, to, t } = p {
                    // Computed from the edge alone, from its end with the lower index.
                    assert!(from < to);
                    let expected = ((level - z[from]) / (z[to] - z[from])).clamp(0.0, 1.0);
                    assert_eq!(t.to_bits(), expected.to_bits());
                    crossings.insert((from, to));
                }
            }
            // A line only ends on the edge of the mesh: inside, the triangles on both sides of
            // a crossing agree on it, so the line runs on.
            if !line.closed {
                assert!(on_edges(&edges, line.points[0]));
                assert!(on_edges(&edges, line.points[line.points.len() - 1]));
            }
        }
        // Each crossing is passed once, by one line: no piece is drawn twice.
        let passes: usize = lines
            .iter()
            .map(|line| {
                line.points
                    .iter()
                    .filter(|p| matches!(p, MeshPoint::Edge { .. }))
                    .count()
                    - usize::from(line.closed)
            })
            .sum();
        assert_eq!(passes, crossings.len());
    }
}

#[test]
fn nan_points_make_holes() {
    let (x, y, rows) = scattered(200, 13, 0.0);
    let square = mesh(&x, &y, &rows);
    let mut z: Vec<f64> = x.iter().zip(&y).map(|(x, y)| x + 0.5 * y).collect();
    // The interior point nearest the origin, and the triangles around it.
    let centre = (4..x.len())
        .min_by(|&a, &b| x[a].hypot(y[a]).total_cmp(&x[b].hypot(y[b])))
        .unwrap_or(4);
    z[centre] = f64::NAN;
    let drawn = shapes(&square, &z, &[-0.5, 0.0, 0.5], ContourStyle::Both);
    let touches = |p: MeshPoint| match p {
        MeshPoint::Corner(i) => i == centre,
        MeshPoint::Edge { from, to, .. } => from == centre || to == centre,
    };
    for line in drawn.lines.iter().flatten() {
        assert!(!line.points.iter().any(|&p| touches(p)), "{line:?}");
    }
    // The first band leaves out the triangles around the point: a hole, wound clockwise.
    let whole = high_area(&square, &z, None);
    assert!((band_area(&x, &y, &drawn.bands[0]) - whole).abs() < 1e-9);
    assert!(whole < 4.0 - 1e-6);
    assert_eq!(drawn.bands[0].rings.len(), 2);
    // A NaN everywhere draws nothing.
    let none = shapes(
        &square,
        &vec![f64::NAN; x.len()],
        &[0.0],
        ContourStyle::Both,
    );
    assert!(none.lines.iter().all(Vec::is_empty));
    assert!(none.bands.iter().all(|band| band.rings.is_empty()));
}

/// Checks the shapes of the values `z` on `mesh` at `levels`: each band covers exactly the
/// part of the mesh at or above its level, so the bands nest and, painted in order, cover the
/// mesh once; lines lie on their level, with no piece of zero length and none drawn twice.
fn check_shapes(mesh: &TriMesh, z: &[f64], levels: &[f64]) -> Result<(), String> {
    let (x, y) = (mesh.x(), mesh.y());
    let drawn = shapes(mesh, z, levels, ContourStyle::Both);
    let mut previous = f64::INFINITY;
    for (index, band) in drawn.bands.iter().enumerate() {
        let level = index.checked_sub(1).map(|i| levels[i]);
        let area = band_area(x, y, band);
        let expected = high_area(mesh, z, level);
        if (area - expected).abs() > 1e-9 {
            return Err(format!("band {index}: area {area}, expected {expected}"));
        }
        if area > previous + 1e-12 {
            return Err(format!("band {index} is larger than the band before it"));
        }
        previous = area;
    }
    for (lines, &level) in drawn.lines.iter().zip(levels) {
        let mut pieces: HashSet<[(u64, u64); 2]> = HashSet::new();
        for line in lines {
            for pair in line.points.windows(2) {
                let [a, b] = [pair[0], pair[1]].map(|p| at(x, y, p));
                if a == b {
                    return Err(format!("a piece of zero length at level {level}: {line:?}"));
                }
                let mut key = [a, b].map(|(x, y)| (x.to_bits(), y.to_bits()));
                key.sort_unstable();
                if !pieces.insert(key) {
                    return Err(format!("a piece drawn twice at level {level}"));
                }
            }
            for &p in &line.points {
                if (value(z, p) - level).abs() > 1e-9 {
                    return Err(format!("{p:?} is off the level {level}"));
                }
            }
        }
    }
    Ok(())
}

/// A structured mesh of `n` by `n` unit squares, each split along the diagonal that alternates
/// from square to square.
fn structured(n: usize) -> TriMesh {
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for j in 0..=n {
        for i in 0..=n {
            x.push(i as f64);
            y.push(j as f64);
        }
    }
    let at = |i: usize, j: usize| (j * (n + 1) + i) as i64;
    let mut rows = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let [a, b, c, d] = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            if (i + j) % 2 == 0 {
                rows.extend([[a, b, c], [a, c, d]]);
            } else {
                rows.extend([[a, b, d], [b, c, d]]);
            }
        }
    }
    mesh(&x, &y, &rows)
}

#[test]
fn bands_cover_the_mesh_once_on_smooth_and_holed_fields() {
    let (x, y, rows) = scattered(250, 17, 0.35);
    let holed = mesh(&x, &y, &rows);
    let mut z: Vec<f64> = x
        .iter()
        .zip(&y)
        .map(|(x, y)| (2.0 * x).cos() + y * y - 0.3 * x * y)
        .collect();
    z[10] = f64::NAN;
    assert_eq!(
        check_shapes(&holed, &z, &[-0.5, 0.0, 0.4, 0.9, 1.5]),
        Ok(())
    );
}

proptest! {
    #[test]
    fn integer_values_on_the_levels_still_tile(
        values in prop::collection::vec(0_i32..4, 36),
        flat in prop::collection::vec(any::<bool>(), 36),
    ) {
        // Integer data on a 5 by 5 mesh, with some points pulled onto the level 2: corners,
        // edges and whole triangles on the levels.
        let grid = structured(5);
        let z: Vec<f64> = values
            .iter()
            .zip(&flat)
            .map(|(&v, &on)| if on { 2.0 } else { f64::from(v) })
            .collect();
        prop_assert_eq!(check_shapes(&grid, &z, &[1.0, 2.0, 3.0]), Ok(()));
    }
}

#[test]
fn every_3_by_3_mesh_of_0_1_2_tiles_at_level_1() {
    let grid = structured(2);
    for code in 0..3_usize.pow(9) {
        let z: Vec<f64> = (0..9)
            .map(|k| ((code / 3_usize.pow(k)) % 3) as f64)
            .collect();
        assert_eq!(check_shapes(&grid, &z, &[1.0]), Ok(()), "{z:?}");
    }
}

#[test]
fn order_and_winding_of_the_triangles_do_not_matter() {
    let (x, y, rows) = scattered(120, 19, 0.3);
    let z: Vec<f64> = x.iter().zip(&y).map(|(x, y)| x * y + 0.2 * x).collect();
    let levels = [-0.3, 0.0, 0.2];
    let forward = shapes(&mesh(&x, &y, &rows), &z, &levels, ContourStyle::Both);
    // Rows reversed, each turned the other way and started at another corner.
    let shuffled: Vec<[i64; 3]> = rows.iter().rev().map(|&[a, b, c]| [c, b, a]).collect();
    let backward = shapes(&mesh(&x, &y, &shuffled), &z, &levels, ContourStyle::Both);
    assert_eq!(forward, backward);
    assert!(forward.lines.iter().any(|lines| !lines.is_empty()));
}

/// `count` points on the unit circle with values alternating 0 and 2, and a centre point of
/// value 1, joined into the fan of triangles around the centre.
fn fan(count: usize) -> (TriMesh, Vec<f64>) {
    let (mut x, mut y, mut z) = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..count {
        let angle = std::f64::consts::TAU * k as f64 / count as f64;
        x.push(angle.cos());
        y.push(angle.sin());
        z.push(if k % 2 == 0 { 0.0 } else { 2.0 });
    }
    let centre = count as i64;
    x.push(0.0);
    y.push(0.0);
    z.push(1.0);
    let rows: Vec<[i64; 3]> = (0..count)
        .map(|k| [k as i64, ((k + 1) % count) as i64, centre])
        .collect();
    (mesh(&x, &y, &rows), z)
}

#[test]
fn a_point_on_the_level_shared_by_many_triangles_joins_every_line_through_it() {
    // The centre lies on the level: every rim point above it sends a line in and out of it.
    let (wheel, z) = fan(64);
    assert_eq!(check_shapes(&wheel, &z, &[1.0]), Ok(()));
    let drawn = shapes(&wheel, &z, &[1.0], ContourStyle::Both);
    let centre = MeshPoint::Corner(64);
    let through: usize = drawn.lines[0]
        .iter()
        .map(|line| line.points.iter().filter(|&&p| p == centre).count())
        .sum();
    // One line in and out per high rim point, and a closed line repeats its start.
    assert!(through >= 32, "{through}");
    // Joining takes time in proportion to the pieces, however many meet at the centre; a scan
    // from the first piece leaving the centre at every step would take time in proportion to
    // their square.
    let (wheel, z) = fan(60_000);
    let start = std::time::Instant::now();
    let drawn = shapes(&wheel, &z, &[1.0], ContourStyle::Both);
    let elapsed = start.elapsed();
    assert!(!drawn.lines[0].is_empty());
    assert!(elapsed.as_secs_f64() < 3.0, "{elapsed:?}");
}
