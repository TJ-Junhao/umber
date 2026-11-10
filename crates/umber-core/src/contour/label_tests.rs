//! Level labels on lines: the middle first, the positions tried after it, the room they must
//! fit in and its tree of boundary edges.

use super::labels::{self, Bands, LabelBox, MAX_STEPS, PagePoint, Placement, Room, STEP_FACTOR};

/// The box of a label `width` pt wide and 10 pt high, centred.
fn sized(width: f64) -> LabelBox {
    LabelBox {
        left: -width / 2.0,
        top: -5.0,
        right: width / 2.0,
        bottom: 5.0,
    }
}

/// A plot area from (-1000, -1000) to (1000, 1000) filled by a field without holes.
fn open() -> Room {
    holed(&[])
}

/// The rectangle from `(left, top)` to `(right, bottom)` as a ring.
fn rectangle(left: f64, top: f64, right: f64, bottom: f64) -> Vec<PagePoint> {
    vec![(left, top), (right, top), (right, bottom), (left, bottom)]
}

/// The field inside `(-1000, -1000)` to `(1000, 1000)` with the rectangular `holes`.
fn holed(holes: &[[f64; 4]]) -> Room {
    let mut rings = vec![rectangle(-1000.0, -1000.0, 1000.0, 1000.0)];
    rings.extend(holes.iter().map(|&[l, t, r, b]| rectangle(l, t, r, b)));
    Room::new((-1000.0, -1000.0, 1000.0, 1000.0), &rings)
}

#[test]
fn labels_sit_upright_at_the_middle_of_long_lines() {
    // A line from right to left: the label reads left to right, upright.
    let line = [(100.0, 50.0), (0.0, 50.0)];
    let (placement, pieces) = labels::place(&line, false, (sized(20.0), 3.0), &open()).unwrap();
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
    let (placement, _) = labels::place(
        &[(0.0, 0.0), (60.0, 60.0)],
        false,
        (sized(10.0), 3.0),
        &open(),
    )
    .unwrap();
    assert!((placement.angle - 45.0).abs() < 1e-9);
    // Too short for its label: none.
    assert!(labels::place(&line, false, (sized(40.0), 3.0), &open()).is_none());
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
    let (placement, pieces) = labels::place(&started, true, (sized(10.0), 3.0), &open()).unwrap();
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

#[test]
fn a_label_over_a_hole_moves_along_its_line_nearest_the_middle_first() {
    // A line along y = 50 from x = 0 to 200, a hole from x = 90 to 110 ending 4 pt above it,
    // within the 8 pt the box with its gap reaches up from the line.
    let line = [(0.0, 50.0), (200.0, 50.0)];
    let room = holed(&[[90.0, 30.0, 110.0, 46.0]]);
    let (placement, pieces) = labels::place(&line, false, (sized(20.0), 3.0), &room).unwrap();
    // The box with its gap reaches 13 pt either side of its centre along the line, so the
    // first position clear of the hole is 25 pt past the middle (five steps of 5 pt), which
    // is tried before the one 25 pt before it.
    let step = STEP_FACTOR * 20.0;
    assert_eq!(step, 5.0);
    assert_eq!(placement.center, (125.0, 50.0));
    assert_eq!(placement.angle, 0.0);
    let expected = [[(0.0, 50.0), (112.0, 50.0)], [(138.0, 50.0), (200.0, 50.0)]];
    assert_eq!(pieces.len(), 2);
    for (piece, expected) in pieces.iter().zip(expected) {
        assert_eq!(piece.len(), 2);
        for (p, q) in piece.iter().zip(expected) {
            assert!((p.0 - q.0).abs() < 1e-9 && p.1 == q.1, "{pieces:?}");
        }
    }
    // With the hole a little further along, the position two steps before the middle is the
    // first clear one: two steps after it still reaches the hole.
    let room = holed(&[[106.0, 30.0, 126.0, 46.0]]);
    let (placement, _) = labels::place(&line, false, (sized(20.0), 3.0), &room).unwrap();
    assert_eq!(placement.center, (90.0, 50.0));
}

#[test]
fn a_label_with_no_position_clear_of_holes_is_dropped() {
    let line = [(0.0, 50.0), (200.0, 50.0)];
    // A strip of field too narrow for the box with its gap, 16 pt high.
    let room = holed(&[[-500.0, 0.0, 500.0, 43.0], [-500.0, 57.0, 500.0, 100.0]]);
    assert!(labels::place(&line, false, (sized(20.0), 3.0), &room).is_none());
    // With room for it, the label sits at the middle.
    let room = holed(&[[-500.0, 0.0, 500.0, 41.0], [-500.0, 59.0, 500.0, 100.0]]);
    let (placement, _) = labels::place(&line, false, (sized(20.0), 3.0), &room).unwrap();
    assert_eq!(placement.center, (100.0, 50.0));
}

#[test]
fn positions_stop_at_the_ends_of_the_line_and_after_the_last_step() {
    // A hole beside all of a line but its first 29 pt: the box with its gap reaches 13 pt
    // either side, so only centres from 13 pt (the end of the line) to 16 pt fit; nine steps
    // back from the middle reach 15 pt.
    let line = [(0.0, 50.0), (120.0, 50.0)];
    let room = holed(&[[29.0, 0.0, 500.0, 46.0]]);
    let (placement, _) = labels::place(&line, false, (sized(20.0), 3.0), &room).unwrap();
    assert_eq!(placement.center, (15.0, 50.0));
    // Past the end of the line no position is tried, even where the box would fit: with the
    // hole from 24 pt only centres below 11 pt fit, such as ten steps back at 10 pt.
    let room = holed(&[[24.0, 0.0, 500.0, 46.0]]);
    assert!(labels::place(&line, false, (sized(20.0), 3.0), &room).is_none());
    // On a long line the label goes no further than the last step from the middle.
    let reach = f64::from(MAX_STEPS) * STEP_FACTOR * 20.0;
    let long = [(0.0, 50.0), (1000.0, 50.0)];
    let near = holed(&[[500.0 - reach + 14.0, 0.0, 500.0 + reach - 14.0, 46.0]]);
    let (placement, _) = labels::place(&long, false, (sized(20.0), 3.0), &near).unwrap();
    assert_eq!(placement.center, (500.0 + reach, 50.0));
    let far = holed(&[[500.0 - reach - 14.0, 0.0, 500.0 + reach + 14.0, 46.0]]);
    assert!(labels::place(&long, false, (sized(20.0), 3.0), &far).is_none());
}

#[test]
fn a_box_that_touches_the_boundary_or_the_plot_edge_does_not_fit() {
    let at = |x: f64, y: f64| Placement {
        center: (x, y),
        angle: 0.0,
    };
    let room = holed(&[[10.0, -5.0, 20.0, 5.0]]);
    // The box from -10 to 10 touches the hole at x = 10; a little to the left it fits.
    assert!(!room.holds(at(0.0, 0.0), sized(20.0)));
    assert!(room.holds(at(-0.001, 0.0), sized(20.0)));
    // An edge through the box with every corner outside the hole still meets it.
    let thin = holed(&[[-100.0, -1.0, 100.0, 1.0]]);
    assert!(!thin.holds(at(0.0, 0.0), sized(20.0)));
    // The plot area: a box reaching its edge does not fit.
    let plot = Room::new(
        (0.0, 0.0, 100.0, 100.0),
        &[rectangle(-50.0, -50.0, 150.0, 150.0)],
    );
    assert!(!plot.holds(at(10.0, 50.0), sized(20.0)));
    assert!(plot.holds(at(10.001, 50.0), sized(20.0)));
    assert!(!plot.holds(at(50.0, 95.0), sized(20.0)));
    // A turned box: at 45 degrees its corners reach 10 × √2 pt from the centre along each
    // axis, so a hole 12 pt to the right is clear of it and one 7 pt to the right is not.
    let turned = Placement {
        center: (0.0, 0.0),
        angle: 45.0,
    };
    let square = LabelBox {
        left: -10.0,
        top: -10.0,
        right: 10.0,
        bottom: 10.0,
    };
    assert!(holed(&[[14.2, -1.0, 20.0, 1.0]]).holds(turned, square));
    assert!(!holed(&[[14.1, -1.0, 20.0, 1.0]]).holds(turned, square));
    // A boundary point that is not finite leaves no room.
    let broken = Room::new(
        (-1000.0, -1000.0, 1000.0, 1000.0),
        &[vec![(0.0, 0.0), (f64::NAN, 1.0), (1.0, 1.0)]],
    );
    assert!(!broken.holds(at(500.0, 500.0), sized(20.0)));
}

/// Twice the signed area of the triangle `a`, `b`, `c`.
fn orient(a: PagePoint, b: PagePoint, c: PagePoint) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Whether the segment from `a` to `b` meets the box `[left, right] × [top, bottom]`, for
/// points in general position: an end inside the box, or a crossing of one of its sides.
fn crosses(a: PagePoint, b: PagePoint, [left, top, right, bottom]: [f64; 4]) -> bool {
    let inside = |(x, y): PagePoint| left < x && x < right && top < y && y < bottom;
    let corners = [(left, top), (right, top), (right, bottom), (left, bottom)];
    inside(a)
        || inside(b)
        || (0..4).any(|k| {
            let (p, q) = (corners[k], corners[(k + 1) % 4]);
            orient(p, q, a) * orient(p, q, b) < 0.0 && orient(a, b, p) * orient(a, b, q) < 0.0
        })
}

#[test]
fn the_tree_of_boundary_edges_agrees_with_every_edge() {
    // Many small triangles as rings, scattered by a fixed generator: the tree's answer matches
    // an independent test of every edge, for boxes at many places.
    let mut state: u64 = 11;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1_u64 << 53) as f64
    };
    let rings: Vec<Vec<PagePoint>> = (0..400)
        .map(|_| {
            let (x, y) = (next() * 400.0, next() * 400.0);
            vec![
                (x, y),
                (x + next() * 8.0, y + next() * 2.0),
                (x + next() * 3.0, y + next() * 8.0),
            ]
        })
        .collect();
    // The triangles are holes in a field around them; where they overlap, the even-odd rule
    // fills the overlap again.
    let mut field = vec![rectangle(-20.0, -20.0, 420.0, 420.0)];
    field.extend(rings.iter().cloned());
    let room = Room::new((-10.0, -10.0, 410.0, 410.0), &field);
    let mut agreed = [0, 0];
    for _ in 0..400 {
        let center = (next() * 380.0 + 10.0, next() * 380.0 + 10.0);
        let placement = Placement { center, angle: 0.0 };
        let bounds = sized(12.0);
        let edge_box = [
            center.0 - 6.0,
            center.1 - 5.0,
            center.0 + 6.0,
            center.1 + 5.0,
        ];
        let any = rings.iter().any(|ring| {
            ring.iter()
                .zip(ring.iter().cycle().skip(1))
                .any(|(&a, &b)| crosses(a, b, edge_box))
        });
        // Inside the field when inside an even number of triangles, tested by the signs of the
        // areas the centre makes with each side.
        let covering = rings
            .iter()
            .filter(|ring| {
                let signs: Vec<bool> = (0..3)
                    .map(|k| orient(ring[k], ring[(k + 1) % 3], center) > 0.0)
                    .collect();
                signs.iter().all(|&s| s) || signs.iter().all(|&s| !s)
            })
            .count();
        let holds = room.holds(placement, bounds);
        assert_eq!(holds, !any && covering % 2 == 0, "{center:?}");
        agreed[usize::from(holds)] += 1;
    }
    assert!(agreed[0] > 20 && agreed[1] > 20, "{agreed:?}");
}

#[test]
fn a_box_outside_the_field_does_not_fit_even_clear_of_its_edges() {
    let at = |x: f64, y: f64| Placement {
        center: (x, y),
        angle: 0.0,
    };
    // A field from 0 to 100 with a hole from 20 to 80, in a larger plot area.
    let room = Room::new(
        (-1000.0, -1000.0, 1000.0, 1000.0),
        &[
            rectangle(0.0, 0.0, 100.0, 100.0),
            rectangle(20.0, 20.0, 80.0, 80.0),
        ],
    );
    // Between the field's edge and the hole, the box fits.
    assert!(room.holds(at(10.0, 50.0), sized(10.0)));
    // Wholly inside the hole, or wholly beyond the field, it touches no edge and still does
    // not fit.
    assert!(!room.holds(at(50.0, 50.0), sized(10.0)));
    assert!(!room.holds(at(500.0, 50.0), sized(10.0)));
    assert!(!room.holds(at(50.0, -300.0), sized(10.0)));
    // A ray along a horizontal edge, or through a corner, counts as the even-odd rule does.
    assert!(!room.holds(at(-50.0, 0.0), sized(10.0)));
    assert!(room.holds(at(10.0, 20.0), sized(10.0)));
    let diamond = Room::new(
        (-1000.0, -1000.0, 1000.0, 1000.0),
        &[vec![(0.0, -50.0), (50.0, 0.0), (0.0, 50.0), (-50.0, 0.0)]],
    );
    assert!(diamond.holds(at(0.0, 0.0), sized(10.0)));
    assert!(!diamond.holds(at(-100.0, 0.0), sized(10.0)));
    // Without any boundary there is no field to hold a label.
    let empty = Room::new((-1000.0, -1000.0, 1000.0, 1000.0), &[]);
    assert!(!empty.holds(at(0.0, 0.0), sized(10.0)));
}

#[test]
fn placing_a_label_gives_the_same_answer_every_time() {
    let line: Vec<PagePoint> = (0..200)
        .map(|i| {
            let x = f64::from(i) * 2.0;
            (x, 100.0 + 30.0 * (x / 40.0).sin())
        })
        .collect();
    // Holes beside the line, the first just above its middle.
    let room = holed(&[[185.0, 50.0, 215.0, 66.0], [230.0, 60.0, 260.0, 75.0]]);
    let first = labels::place(&line, false, (sized(24.0), 3.0), &room);
    let middle = labels::place(&line, false, (sized(24.0), 3.0), &open());
    assert!(first.is_some() && first != middle, "{first:?}");
    for _ in 0..5 {
        assert_eq!(
            labels::place(&line, false, (sized(24.0), 3.0), &room),
            first
        );
    }
}

#[test]
fn the_boundary_test_stops_at_the_first_edge_that_touches_the_box() {
    // 2,500 holes of a hundredth of a point inside a 10-pt box: 10,004 edges in all, of which
    // the test looks at a handful before it finds one that touches the box.
    let mut rings = vec![rectangle(-1000.0, -1000.0, 1000.0, 1000.0)];
    for i in 0..50 {
        for j in 0..50 {
            let (x, y) = (-4.9 + f64::from(i) * 0.2, -4.9 + f64::from(j) * 0.2);
            rings.push(rectangle(x, y, x + 0.01, y + 0.01));
        }
    }
    let room = Room::new((-1000.0, -1000.0, 1000.0, 1000.0), &rings);
    let placement = Placement {
        center: (0.0, 0.0),
        angle: 0.0,
    };
    let bounds = sized(10.0);
    assert!(!room.holds(placement, bounds));
    let looked = room.edges_looked_at(placement, bounds);
    assert!((1..=64).contains(&looked), "{looked} edges");
    // Clear of every hole, the test looks only at the few edges in leaves near the box.
    let clear = Placement {
        center: (500.0, 500.0),
        angle: 0.0,
    };
    assert!(room.holds(clear, bounds));
    assert!(room.edges_looked_at(clear, bounds) <= 64);
}

#[test]
fn the_bands_under_a_label_are_those_painted_under_its_text() {
    let at = |x: f64, y: f64| Placement {
        center: (x, y),
        angle: 0.0,
    };
    let text = sized(20.0);
    // The whole field, and over its right half the band above it.
    let bands = Bands::new(&[
        vec![rectangle(0.0, 0.0, 100.0, 100.0)],
        vec![rectangle(50.0, 0.0, 100.0, 100.0)],
    ]);
    assert_eq!(bands.under(at(50.0, 50.0), text), [0, 1]);
    assert_eq!(bands.under(at(20.0, 50.0), text), [0]);
    // Where the later band covers the first, only the later one shows.
    assert_eq!(bands.under(at(80.0, 50.0), text), [1]);
    // Outside every band, none.
    assert!(bands.under(at(500.0, 50.0), text).is_empty());
    // A band whose rings cancel out under the even-odd rule paints nothing there.
    let cancelled = Bands::new(&[
        vec![rectangle(0.0, 0.0, 100.0, 100.0)],
        vec![
            rectangle(50.0, 0.0, 100.0, 100.0),
            rectangle(50.0, 0.0, 100.0, 100.0),
        ],
    ]);
    assert_eq!(cancelled.under(at(80.0, 50.0), text), [0]);
    // A bow tie, whose two lobes run opposite ways round so that their signed areas cancel,
    // still paints both by the even-odd rule.
    let bow = Bands::new(&[
        vec![rectangle(0.0, 0.0, 100.0, 100.0)],
        vec![vec![
            (50.0, 0.0),
            (100.0, 100.0),
            (100.0, 0.0),
            (50.0, 100.0),
        ]],
    ]);
    assert_eq!(bow.under(at(90.0, 50.0), text), [1]);
    // A turned label samples along its own frame.
    let turned = Placement {
        center: (50.0, 20.0),
        angle: 90.0,
    };
    assert_eq!(bands.under(turned, text), [0, 1]);
}
