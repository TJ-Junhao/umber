//! A small linear program: the least total of nonnegative variables under linear lower bounds.
//!
//! The program is: minimize `x₀ + x₁ + … + xₙ₋₁` subject to `aₖ · x ≥ bₖ` for every row `k`
//! and `x ≥ 0`. It is solved through its dual, maximize `b · y` subject to `Aᵀ y ≤ 1` and
//! `y ≥ 0`, whose origin is a feasible basis, by the simplex method: Bland's rule chooses the
//! entering column, and the leaving row is the one with the largest pivot among those whose ratio
//! is the least to within a tolerance, so that every basis stays well conditioned. The dual is unbounded exactly when the program has no solution. At the dual's
//! optimum the prices of its rows, `c_B B⁻¹`, are an optimal `x`.
//!
//! The dual has one row per variable, so its tableau stays small however many rows the program
//! has: `n × (rows + n)` entries. Pivoting accumulates rounding in the tableau, so the tableau
//! is rebuilt from the program and the current basis ([`Simplex::refactor`]) every
//! [`REFACTOR_EVERY`] pivots, and always before the solve concludes: an optimum or an
//! unbounded column found on an accumulated tableau is confirmed on a rebuilt one, and the
//! solution is read from the rebuilt one.

/// The smallest reduced cost, relative to the largest bound, that counts as improving, and the
/// smallest pivot of a basis matrix that counts as nonsingular.
const PIVOT_TOLERANCE: f64 = 1e-12;

/// The smallest tableau entry a pivot may have: smaller ones would make the next basis close
/// to singular.
const SMALLEST_PIVOT: f64 = 1e-9;

/// How far, relative to the right-hand side, a ratio may exceed the least one and still
/// compete for the leaving row on the size of its pivot.
const RATIO_TOLERANCE: f64 = 1e-9;

/// The most simplex steps before giving up: the leaving rule trades the guarantee of Bland's
/// rule against cycling for well-conditioned bases, and this bounds the work if it cycles.
const MAX_STEPS: usize = 100_000;

/// The pivots between two rebuilds of the tableau.
const REFACTOR_EVERY: usize = 16;

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
///
/// The result is exact up to the rounding of one solve of the optimal basis; callers that need
/// every row to hold check it against their own model.
pub(super) fn least_total(vars: usize, rows: &[Row]) -> Option<Vec<f64>> {
    Simplex::new(vars, rows).solve()
}

/// The dual's simplex tableau and basis.
struct Simplex<'a> {
    /// The program.
    rows: &'a [Row],
    /// The number of the program's variables: the dual's rows.
    vars: usize,
    /// One row per dual row: the dual columns (one per program row, then one slack per
    /// variable) and the right-hand side.
    table: Vec<Vec<f64>>,
    /// The reduced costs of the dual columns and the objective's value.
    objective: Vec<f64>,
    /// The basic dual column of each tableau row.
    basis: Vec<usize>,
}

impl<'a> Simplex<'a> {
    /// The tableau of the dual's origin: the slacks basic.
    fn new(vars: usize, rows: &'a [Row]) -> Simplex<'a> {
        let duals = rows.len();
        let mut simplex = Simplex {
            rows,
            vars,
            table: vec![vec![0.0; duals + vars + 1]; vars],
            objective: vec![0.0; duals + vars + 1],
            basis: (duals..duals + vars).collect(),
        };
        simplex.refactor();
        simplex
    }

    /// The number of dual columns.
    fn columns(&self) -> usize {
        self.rows.len() + self.vars
    }

    /// Entry `i` of dual column `j`, as the program gives it.
    fn entry(&self, i: usize, j: usize) -> f64 {
        match self.rows.get(j) {
            Some(row) => row.coefficients[i],
            None => f64::from(u8::from(j - self.rows.len() == i)),
        }
    }

    /// The dual objective's coefficient of column `j`.
    fn cost(&self, j: usize) -> f64 {
        self.rows.get(j).map_or(0.0, |row| row.bound)
    }

    /// Rebuilds the tableau and the reduced costs from the program for the current basis:
    /// `B⁻¹ [Aᵀ | I | 1]` and `c_B B⁻¹ [Aᵀ | I | 1] - [b | 0 | 0]`. Returns false, leaving the
    /// tableau as it was, when the basis matrix is singular to working precision.
    fn refactor(&mut self) -> bool {
        let m = self.vars;
        let columns = self.columns();
        let basis_matrix: Vec<Vec<f64>> = (0..m)
            .map(|i| self.basis.iter().map(|&j| self.entry(i, j)).collect())
            .collect();
        let Some(inverse) = invert(basis_matrix) else {
            return false;
        };
        // Row r of B⁻¹ [Aᵀ | I | 1].
        for (r, line) in self.table.iter_mut().enumerate() {
            let weights = &inverse[r];
            for (j, row) in self.rows.iter().enumerate() {
                line[j] = weights
                    .iter()
                    .zip(&row.coefficients)
                    .map(|(w, a)| w * a)
                    .sum();
            }
            line[self.rows.len()..columns].copy_from_slice(weights);
            line[columns] = weights.iter().sum::<f64>().max(0.0);
        }
        // The prices c_B B⁻¹, then the reduced costs.
        let prices: Vec<f64> = (0..m)
            .map(|i| {
                self.basis
                    .iter()
                    .enumerate()
                    .map(|(r, &j)| self.cost(j) * inverse[r][i])
                    .sum()
            })
            .collect();
        for (j, row) in self.rows.iter().enumerate() {
            let price: f64 = prices
                .iter()
                .zip(&row.coefficients)
                .map(|(p, a)| p * a)
                .sum();
            self.objective[j] = price - row.bound;
        }
        self.objective[self.rows.len()..columns].copy_from_slice(&prices);
        self.objective[columns] = prices.iter().sum();
        // Basic columns are exactly unit columns with no reduced cost.
        for (r, &j) in self.basis.iter().enumerate() {
            for (i, line) in self.table.iter_mut().enumerate() {
                line[j] = f64::from(u8::from(i == r));
            }
            self.objective[j] = 0.0;
        }
        true
    }

    /// The least-total `x`, or `None` when the dual is unbounded.
    fn solve(mut self) -> Option<Vec<f64>> {
        let columns = self.columns();
        let duals = self.rows.len();
        let scale = self
            .rows
            .iter()
            .map(|row| row.bound.abs())
            .fold(1.0, f64::max);
        let mut since_rebuild = 0;
        for _ in 0..MAX_STEPS {
            if since_rebuild == REFACTOR_EVERY {
                self.refactor();
                since_rebuild = 0;
            }
            // Bland's rule: the first column that improves the objective enters...
            let entering = (0..columns).find(|&j| self.objective[j] < -PIVOT_TOLERANCE * scale);
            let Some(entering) = entering else {
                if since_rebuild > 0 && self.refactor() {
                    since_rebuild = 0;
                    continue;
                }
                // ...and when none does, the prices of the rebuilt tableau are the program's x.
                return Some(
                    (0..self.vars)
                        .map(|i| self.objective[duals + i].max(0.0))
                        .collect(),
                );
            };
            // ...and of the rows that bound it, the one with the largest pivot among those whose
            // ratio is the least to within a tolerance, ties broken by the smallest basic
            // column, leaves; pivots too small to keep the basis well conditioned never do.
            let candidates: Vec<(usize, f64, f64)> = self
                .table
                .iter()
                .enumerate()
                .filter(|(_, line)| line[entering] > SMALLEST_PIVOT)
                .map(|(i, line)| (i, line[entering], line[columns]))
                .collect();
            let least = candidates
                .iter()
                .map(|&(_, a, rhs)| (rhs + RATIO_TOLERANCE * (1.0 + rhs.abs())) / a)
                .fold(f64::INFINITY, f64::min);
            let leaving = candidates
                .iter()
                .filter(|&&(_, a, rhs)| rhs / a <= least)
                .fold(None, |best: Option<(usize, f64)>, &(i, a, _)| match best {
                    Some((kept, largest))
                        if largest > a || (largest == a && self.basis[kept] < self.basis[i]) =>
                    {
                        best
                    }
                    _ => Some((i, a)),
                });
            let Some((row, _)) = leaving else {
                if since_rebuild > 0 && self.refactor() {
                    since_rebuild = 0;
                    continue;
                }
                // An unbounded dual, on a rebuilt tableau: no x meets every row.
                return None;
            };
            pivot(&mut self.table, &mut self.objective, row, entering);
            self.basis[row] = entering;
            since_rebuild += 1;
        }
        None
    }
}

/// The inverse of the square matrix `matrix`, by Gauss–Jordan elimination with partial
/// pivoting; `None` when it is singular to working precision.
fn invert(mut matrix: Vec<Vec<f64>>) -> Option<Vec<Vec<f64>>> {
    let n = matrix.len();
    let mut inverse: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    for c in 0..n {
        let (p, largest) = (c..n)
            .map(|r| (r, matrix[r][c].abs()))
            .fold(
                (c, -1.0),
                |best, next| if next.1 > best.1 { next } else { best },
            );
        if largest <= PIVOT_TOLERANCE {
            return None;
        }
        matrix.swap(c, p);
        inverse.swap(c, p);
        let d = matrix[c][c];
        for v in &mut matrix[c] {
            *v /= d;
        }
        for v in &mut inverse[c] {
            *v /= d;
        }
        let (pivot_line, pivot_inverse) = (matrix[c].clone(), inverse[c].clone());
        for r in (0..n).filter(|&r| r != c) {
            let factor = matrix[r][c];
            if factor != 0.0 {
                for (v, p) in matrix[r].iter_mut().zip(&pivot_line) {
                    *v -= factor * p;
                }
                for (v, p) in inverse[r].iter_mut().zip(&pivot_inverse) {
                    *v -= factor * p;
                }
            }
        }
    }
    Some(inverse)
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
