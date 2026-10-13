//! A small linear program: the least total of nonnegative variables under linear lower bounds.
//!
//! The program is: minimize `x₀ + x₁ + … + xₙ₋₁` subject to `aₖ · x ≥ bₖ` for every row `k`
//! and `x ≥ 0`. It is solved through its dual, maximize `b · y` subject to `Aᵀ y ≤ 1` and
//! `y ≥ 0`, whose origin is a feasible basis, by the simplex method with Bland's rule, which
//! never cycles. The dual is unbounded exactly when the program has no solution. At the dual's
//! optimum the reduced costs of its slack columns are an optimal `x`.
//!
//! The dual has one row per variable, so its tableau stays small however many rows the program
//! has: `n × (rows + n)` entries.

/// The smallest coefficient, in absolute value, that counts as nonzero when choosing a pivot.
const PIVOT_TOLERANCE: f64 = 1e-12;

/// The most simplex steps before giving up. Bland's rule always terminates; this only bounds
/// the work on inputs whose rounding keeps it from settling.
const MAX_STEPS: usize = 100_000;

/// One lower bound of a program: `coefficients · x ≥ bound`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Row {
    /// One coefficient per variable.
    pub coefficients: Vec<f64>,
    /// The bound.
    pub bound: f64,
}

/// The least-total `x ≥ 0` with `row.coefficients · x ≥ row.bound` for every row of `rows`, each
/// row holding `vars` coefficients; `None` when no `x` meets every row (or the solve does not
/// settle within [`MAX_STEPS`] steps).
pub(super) fn least_total(vars: usize, rows: &[Row]) -> Option<Vec<f64>> {
    let duals = rows.len();
    let columns = duals + vars;
    // The dual's tableau: one row per variable, `Aᵀ` then the slacks then the right-hand side.
    let mut table: Vec<Vec<f64>> = (0..vars)
        .map(|i| {
            let mut line = vec![0.0; columns + 1];
            for (k, row) in rows.iter().enumerate() {
                line[k] = row.coefficients[i];
            }
            line[duals + i] = 1.0;
            line[columns] = 1.0;
            line
        })
        .collect();
    // The objective row of `z - b · y = 0`.
    let mut objective = vec![0.0; columns + 1];
    for (k, row) in rows.iter().enumerate() {
        objective[k] = -row.bound;
    }
    let mut basis: Vec<usize> = (duals..columns).collect();
    let scale = rows.iter().map(|row| row.bound.abs()).fold(1.0, f64::max);
    for _ in 0..MAX_STEPS {
        // Bland's rule: the first column that improves the objective enters...
        let Some(entering) = (0..columns).find(|&j| objective[j] < -PIVOT_TOLERANCE * scale) else {
            // ...and when none does, the reduced costs of the slacks are the program's x.
            return Some((0..vars).map(|i| objective[duals + i].max(0.0)).collect());
        };
        // ...and of the rows that bound it, the one with the least ratio, ties broken by the
        // smallest basic column, leaves.
        let mut leaving: Option<(usize, f64)> = None;
        for (i, line) in table.iter().enumerate() {
            let a = line[entering];
            if a <= PIVOT_TOLERANCE {
                continue;
            }
            let ratio = line[columns] / a;
            let better = match leaving {
                None => true,
                Some((kept, best)) => ratio < best || (ratio == best && basis[i] < basis[kept]),
            };
            if better {
                leaving = Some((i, ratio));
            }
        }
        // An unbounded dual: no x meets every row.
        let (pivot_row, _) = leaving?;
        pivot(&mut table, &mut objective, pivot_row, entering);
        basis[pivot_row] = entering;
    }
    None
}

/// Pivots the tableau `table` with objective row `objective` on row `row`, column `column`.
fn pivot(table: &mut [Vec<f64>], objective: &mut [f64], row: usize, column: usize) {
    let divisor = table[row][column];
    for v in &mut table[row] {
        *v /= divisor;
    }
    let pivot_line = table[row].clone();
    let eliminate = |line: &mut [f64]| {
        let factor = line[column];
        if factor != 0.0 {
            for (v, p) in line.iter_mut().zip(&pivot_line) {
                *v -= factor * p;
            }
            line[column] = 0.0;
        }
    };
    for (i, line) in table.iter_mut().enumerate() {
        if i != row {
            eliminate(line);
        }
    }
    eliminate(objective);
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn row(coefficients: &[f64], bound: f64) -> Row {
        Row {
            coefficients: coefficients.to_vec(),
            bound,
        }
    }

    #[test]
    fn least_totals_of_small_programs() {
        // x ≥ 3, y ≥ 2: 5.
        let x = least_total(2, &[row(&[1.0, 0.0], 3.0), row(&[0.0, 1.0], 2.0)]).unwrap();
        assert!((x[0] - 3.0).abs() < 1e-12 && (x[1] - 2.0).abs() < 1e-12);
        // x + y ≥ 4 and x ≥ 1: total 4.
        let x = least_total(2, &[row(&[1.0, 1.0], 4.0), row(&[1.0, 0.0], 1.0)]).unwrap();
        assert!((x[0] + x[1] - 4.0).abs() < 1e-12 && x[0] >= 1.0 - 1e-12);
        // No bounds above 0: nothing.
        assert_eq!(
            least_total(3, &[row(&[1.0, 0.0, 0.0], -1.0)]).unwrap(),
            [0.0; 3]
        );
        // x ≥ 2 and -x ≥ -1: none.
        assert!(least_total(1, &[row(&[1.0], 2.0), row(&[-1.0], -1.0)]).is_none());
        // x - y ≥ 1 and y - x ≥ 1: none.
        assert!(least_total(2, &[row(&[1.0, -1.0], 1.0), row(&[-1.0, 1.0], 1.0)]).is_none());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(2_000))]

        /// Programs built around a known solution are solved, never refused, and the result
        /// meets every row with a total no larger than the known one.
        #[test]
        fn programs_with_a_solution_are_solved(
            known in prop::collection::vec(0.0..300.0_f64, 1..6),
            rows in prop::collection::vec(
                (prop::collection::vec(-1.0..2.0_f64, 6), 0.0..50.0_f64),
                0..12,
            ),
        ) {
            let vars = known.len();
            let rows: Vec<Row> = rows
                .into_iter()
                .map(|(coefficients, slack)| {
                    let coefficients = coefficients[..vars].to_vec();
                    let at: f64 = coefficients.iter().zip(&known).map(|(a, x)| a * x).sum();
                    Row { coefficients, bound: at - slack }
                })
                .collect();
            let x = least_total(vars, &rows);
            prop_assert!(x.is_some());
            let x = x.unwrap_or_default();
            for r in &rows {
                let at: f64 = r.coefficients.iter().zip(&x).map(|(a, x)| a * x).sum();
                prop_assert!(at >= r.bound - 1e-6 * (1.0 + r.bound.abs()), "{r:?} {x:?}");
            }
            prop_assert!(x.iter().all(|v| *v >= 0.0));
            let total: f64 = x.iter().sum();
            prop_assert!(total <= known.iter().sum::<f64>() + 1e-6);
        }
    }
}
