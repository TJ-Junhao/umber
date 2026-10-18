//! Properties of the contour geometry: lines lie on their level, bands tile the finite cells,
//! saddles and holes behave as the module documentation says.

use proptest::prelude::*;

use super::*;

/// The field along the grid line through `p` (one of its coordinates whole), interpolated
/// linearly between the two grid points around it; `None` off the grid lines or the grid.
fn on_grid_line(z: &[f64], columns: usize, rows: usize, p: GridPoint) -> Option<f64> {
    let value = |i: usize, j: usize| z.get(j * columns + i).copied();
    let split = |at: f64, count: usize| {
        let i = (at.floor().max(0.0) as usize).min(count - 2);
        (i, at - i as f64)
    };
    if p.x.fract() == 0.0 && (0.0..=(columns - 1) as f64).contains(&p.x) {
        let (j, t) = split(p.y, rows);
        let i = p.x as usize;
        return Some(value(i, j)? * (1.0 - t) + value(i, j + 1)? * t);
    }
    if p.y.fract() == 0.0 && (0.0..=(rows - 1) as f64).contains(&p.y) {
        let (i, t) = split(p.x, columns);
        let j = p.y as usize;
        return Some(value(i, j)? * (1.0 - t) + value(i + 1, j)? * t);
    }
    None
}

/// The signed area of the closed ring `ring` (first point not repeated), positive when it runs
/// counter-clockwise with `y` up.
fn signed_area(ring: &[GridPoint]) -> f64 {
    (0..ring.len())
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

/// The area of a band's region: the sum of the signed areas of its rings.
fn region_area(band: &BandPolygons) -> f64 {
    band.rings.iter().map(|ring| signed_area(ring)).sum()
}

/// The area of the part of a unit cell at or above `level`, computed cell by cell, apart from
/// the module's code: the high corners and the crossings walked counter-clockwise, with the
/// saddle rule of the module documentation.
fn cell_high_area(corners: [f64; 4], level: f64) -> f64 {
    let at = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    let high = corners.map(|v| v >= level);
    let crossing = |k: usize| {
        let (a, b) = (at[k], at[(k + 1) % 4]);
        let t = (level - corners[k]) / (corners[(k + 1) % 4] - corners[k]);
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    };
    let crosses = |k: usize| high[k] != high[(k + 1) % 4];
    let area = |points: &[(f64, f64)]| {
        (0..points.len())
            .map(|i| {
                let (a, b) = (points[i], points[(i + 1) % points.len()]);
                a.0 * b.1 - b.0 * a.1
            })
            .sum::<f64>()
            / 2.0
    };
    let saddle = (0..4).filter(|&k| crosses(k)).count() == 4;
    let mean = corners.iter().sum::<f64>() / 4.0;
    if saddle && mean < level {
        // Two corner triangles.
        return (0..4)
            .filter(|&k| high[k])
            .map(|k| area(&[at[k], crossing(k), crossing((k + 3) % 4)]).abs())
            .sum();
    }
    let mut polygon = Vec::new();
    for k in 0..4 {
        if high[k] {
            polygon.push(at[k]);
        }
        if crosses(k) {
            polygon.push(crossing(k));
        }
    }
    if polygon.len() < 3 {
        0.0
    } else {
        area(&polygon)
    }
}

/// Whether `p` lies inside `band` under the even-odd rule.
fn inside(band: &BandPolygons, p: GridPoint) -> bool {
    let mut odd = false;
    for ring in &band.rings {
        for i in 0..ring.len() {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                odd = !odd;
            }
        }
    }
    odd
}

/// A grid of `columns × rows` values in `-10..10`, some of them NaN.
fn grids() -> impl Strategy<Value = (usize, usize, Vec<f64>)> {
    (2usize..7, 2usize..7).prop_flat_map(|(columns, rows)| {
        let value = prop_oneof![
            8 => -10.0f64..10.0,
            1 => Just(f64::NAN),
            1 => (-3i32..3).prop_map(f64::from),
        ];
        (
            Just(columns),
            Just(rows),
            proptest::collection::vec(value, columns * rows),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn level_lines_lie_on_level((columns, rows, z) in grids(), level in -10.0f64..10.0) {
        let grid = Grid::new(&z, columns, rows).unwrap();
        for line in iso_lines(&grid, level) {
            prop_assert!(line.points.len() >= 2);
            if line.closed {
                prop_assert_eq!(line.points.first(), line.points.last());
            }
            for p in &line.points {
                let value = on_grid_line(&z, columns, rows, *p);
                prop_assert!(value.is_some(), "{p:?} is off the grid lines");
                let value = value.unwrap();
                prop_assert!((value - level).abs() <= 1e-9 * 20.0, "{p:?}: {value} vs {level}");
            }
            // Each segment crosses one finite cell.
            for pair in line.points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let i = a.x.min(b.x).floor().min((columns - 2) as f64) as usize;
                let j = a.y.min(b.y).floor().min((rows - 2) as f64) as usize;
                prop_assert!((a.x - b.x).abs() <= 1.0 && (a.y - b.y).abs() <= 1.0);
                prop_assert!(grid.is_finite_cell(i, j), "{a:?} to {b:?}");
            }
        }
    }

    #[test]
    fn bands_tile_without_gaps(
        (columns, rows, z) in grids(),
        mut levels in proptest::collection::vec(-10.0f64..10.0, 1..4),
    ) {
        levels.sort_by(f64::total_cmp);
        levels.dedup();
        let grid = Grid::new(&z, columns, rows).unwrap();
        let finite: Vec<(usize, usize)> = grid
            .cells()
            .filter(|&(i, j)| grid.is_finite_cell(i, j))
            .collect();
        let bands = bands(&grid, &levels);
        prop_assert_eq!(bands.len(), levels.len() + 1);
        let areas: Vec<f64> = bands.iter().map(region_area).collect();
        prop_assert!((areas[0] - finite.len() as f64).abs() < 1e-9);
        for (k, &level) in levels.iter().enumerate() {
            let expected: f64 = finite
                .iter()
                .map(|&(i, j)| cell_high_area(grid.corner_values(i, j), level))
                .sum();
            prop_assert!((areas[k + 1] - expected).abs() < 1e-9, "level {level}");
        }
        // Each band shows its region less the next: nested regions, whose shown parts add up
        // to the finite cells exactly.
        let shown: Vec<f64> = (0..bands.len())
            .map(|k| areas[k] - areas.get(k + 1).copied().unwrap_or(0.0))
            .collect();
        prop_assert!(shown.iter().all(|&a| a >= -1e-9), "{shown:?}");
        prop_assert!((shown.iter().sum::<f64>() - finite.len() as f64).abs() < 1e-9);
        // Sample points: inside the first band exactly in finite cells, and inside a band only
        // where inside the band before it.
        for (i, j) in grid.cells() {
            for (u, v) in [(0.31, 0.47), (0.83, 0.12), (0.5, 0.91)] {
                let p = GridPoint { x: i as f64 + u, y: j as f64 + v };
                prop_assert_eq!(inside(&bands[0], p), grid.is_finite_cell(i, j));
                for pair in bands.windows(2) {
                    prop_assert!(!inside(&pair[1], p) || inside(&pair[0], p), "{p:?}");
                }
            }
        }
    }
}

#[test]
fn saddle_is_deterministic() {
    let point = |x: f64, y: f64| GridPoint { x, y };
    // Row 0 holds (0, 0) = 1 and (1, 0) = 0; row 1 holds (0, 1) = 0 and (1, 1) = 1.
    let z = [1.0, 0.0, 0.0, 1.0];
    let grid = Grid::new(&z, 2, 2).unwrap();
    // The mean, 0.5, is at the level: the high corners join across the centre.
    let joined = iso_lines(&grid, 0.5);
    assert_eq!(
        joined,
        [
            Polyline {
                points: vec![point(0.5, 0.0), point(1.0, 0.5)],
                closed: false,
            },
            Polyline {
                points: vec![point(0.5, 1.0), point(0.0, 0.5)],
                closed: false,
            },
        ]
    );
    // Above the mean, they are cut apart.
    let separated = iso_lines(&grid, 0.6);
    assert_eq!(
        separated,
        [
            Polyline {
                points: vec![point(0.4, 0.0), point(0.0, 0.4)],
                closed: false,
            },
            Polyline {
                points: vec![point(0.6, 1.0), point(1.0, 0.6)],
                closed: false,
            },
        ]
    );
    let areas: Vec<f64> = bands(&grid, &[0.5, 0.6]).iter().map(region_area).collect();
    assert!((areas[0] - 1.0).abs() < 1e-12);
    assert!((areas[1] - 0.75).abs() < 1e-12, "{areas:?}");
    assert!((areas[2] - 0.16).abs() < 1e-12, "{areas:?}");
    // The same values give the same geometry, point for point, every time.
    for _ in 0..3 {
        assert_eq!(iso_lines(&grid, 0.5), joined);
        assert_eq!(bands(&grid, &[0.5]), bands(&grid, &[0.5]));
    }
    // The mirrored saddle joins its own high corners.
    let mirrored = [0.0, 1.0, 1.0, 0.0];
    let grid = Grid::new(&mirrored, 2, 2).unwrap();
    assert_eq!(
        iso_lines(&grid, 0.5),
        [
            Polyline {
                points: vec![point(1.0, 0.5), point(0.5, 1.0)],
                closed: false,
            },
            Polyline {
                points: vec![point(0.0, 0.5), point(0.5, 0.0)],
                closed: false,
            },
        ]
    );
}

#[test]
fn nan_cells_are_holes() {
    // A 5 × 5 grid of z = x + y with the middle value missing: the four cells around it are a
    // hole in every line and band.
    let mut z: Vec<f64> = (0..25).map(|k| f64::from(k % 5 + k / 5)).collect();
    z[12] = f64::NAN;
    let grid = Grid::new(&z, 5, 5).unwrap();
    let hole = |p: &GridPoint| p.x > 1.0 && p.x < 3.0 && p.y > 1.0 && p.y < 3.0;
    for level in [1.5, 3.5, 4.0, 6.5] {
        for line in iso_lines(&grid, level) {
            assert!(!line.points.iter().any(hole), "{level}: {line:?}");
        }
    }
    let bands = bands(&grid, &[4.0]);
    // The finite cells: the outer ring around the hole, counter-clockwise, and the hole,
    // clockwise.
    assert_eq!(bands[0].rings.len(), 2);
    let areas: Vec<f64> = bands[0]
        .rings
        .iter()
        .map(|ring| signed_area(ring))
        .collect();
    assert_eq!(areas, [16.0, -4.0]);
    assert!(!inside(&bands[0], GridPoint { x: 2.0, y: 2.0 }));
    assert!(!inside(&bands[1], GridPoint { x: 2.5, y: 2.5 }));
    assert!(inside(&bands[1], GridPoint { x: 3.5, y: 3.5 }));
    // The line at 4 runs into the hole's edge and out again: two open lines.
    let lines = iso_lines(&grid, 4.0);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines.iter().all(|line| !line.closed));
}

#[test]
fn closed_lines_close_and_open_lines_end_on_edges() {
    // A peak in the middle of a 3 × 3 grid: the line around it is closed.
    let z = [0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0];
    let grid = Grid::new(&z, 3, 3).unwrap();
    let lines = iso_lines(&grid, 2.0);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].closed);
    assert_eq!(lines[0].points.len(), 5);
    // Counter-clockwise around the peak, the high side on the left.
    let ring = &lines[0].points[..4];
    assert!(signed_area(ring) > 0.0);
    // A ramp: one open line from edge to edge.
    let ramp = [0.0, 1.0, 2.0, 0.0, 1.0, 2.0];
    let grid = Grid::new(&ramp, 3, 2).unwrap();
    let lines = iso_lines(&grid, 1.5);
    assert_eq!(
        lines,
        [Polyline {
            points: vec![GridPoint { x: 1.5, y: 1.0 }, GridPoint { x: 1.5, y: 0.0 }],
            closed: false,
        }]
    );
}

#[test]
fn grids_need_two_rows_and_columns_of_the_right_size() {
    assert!(Grid::new(&[1.0, 2.0], 2, 1).is_none());
    assert!(Grid::new(&[1.0, 2.0, 3.0], 2, 2).is_none());
    assert!(Grid::new(&[1.0; 4], 2, 2).is_some());
    // Constant and all-NaN fields have no lines and, all NaN, no band area.
    let grid = Grid::new(&[3.0; 4], 2, 2).unwrap();
    assert!(iso_lines(&grid, 3.0).is_empty());
    let nan = [f64::NAN; 4];
    let grid = Grid::new(&nan, 2, 2).unwrap();
    assert!(iso_lines(&grid, 0.0).is_empty());
    assert!(
        bands(&grid, &[0.0])
            .iter()
            .all(|band| band.rings.is_empty())
    );
}

#[test]
fn labels_sit_upright_at_the_middle_of_long_lines() {
    // A line from right to left: the label reads left to right, upright.
    let line = [(100.0, 50.0), (0.0, 50.0)];
    let (placement, pieces) = labels::place(&line, false, 20.0, 3.0).unwrap();
    assert_eq!(placement.center, (50.0, 50.0));
    assert_eq!(placement.angle, 0.0);
    assert_eq!(
        pieces,
        [
            vec![(100.0, 50.0), (63.0, 50.0)],
            vec![(37.0, 50.0), (0.0, 50.0)]
        ]
    );
    // Falling to the right on the page: a positive (clockwise) angle.
    let (placement, _) = labels::place(&[(0.0, 0.0), (60.0, 60.0)], false, 10.0, 3.0).unwrap();
    assert!((placement.angle - 45.0).abs() < 1e-9);
    // Too short for its label: none.
    assert!(labels::place(&line, false, 40.0, 3.0).is_none());
    // A closed square: one piece round from the far side of the label to its near side.
    let square = [
        (0.0, 0.0),
        (40.0, 0.0),
        (40.0, 40.0),
        (0.0, 40.0),
        (0.0, 0.0),
    ];
    let started = labels::rotated_ring(&square, 0);
    assert_eq!(started.first(), started.last());
    assert_eq!(started[0], (40.0, 0.0));
    let (placement, pieces) = labels::place(&started, true, 10.0, 3.0).unwrap();
    assert_eq!(placement.center, (0.0, 40.0));
    assert_eq!(pieces.len(), 1);
    assert_eq!(
        pieces[0],
        [
            (0.0, 32.0),
            (0.0, 0.0),
            (40.0, 0.0),
            (40.0, 40.0),
            (8.0, 40.0)
        ]
    );
}
