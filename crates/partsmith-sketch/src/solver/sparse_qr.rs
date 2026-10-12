//! A sparse Householder QR factorization that takes columns one at a time, without column pivoting.
//!
//! This is the rank-revealing factorization of spec sections 3.7.4 and 3.8. Each column is a sparse
//! vector. It is transformed by the reflectors of the columns accepted before it; what is left outside
//! their pivot rows is its component orthogonal to their span. A column is dependent when that part
//! is at most `tolerance` times its original norm, and then it gets no reflector. Otherwise a
//! Householder reflector maps that part onto one new pivot row (the lowest row index it touches,
//! which keeps fill local for document-ordered sketches). Reflectors are stored sparsely, and only
//! the reflectors whose rows overlap a column are applied to it, in ascending order.
//!
//! With the accepted columns as `A`, the factorization is `A = Q [R; 0]` with `Q` the product of the
//! reflectors and `R` upper triangular in acceptance order.

use std::collections::BinaryHeap;

/// One Householder reflector `I - beta v v^T` and the column of `R` it was made from.
#[derive(Clone, Debug)]
struct Reflector {
    /// The row the column was mapped onto.
    pivot: usize,
    /// Rows of `v`, ascending.
    rows: Vec<usize>,
    /// Entries of `v` on those rows.
    vector: Vec<f64>,
    beta: f64,
    /// Entries of the column of `R` above the diagonal: (earlier reflector, value), ascending.
    above: Vec<(usize, f64)>,
    /// Diagonal entry of `R`.
    diagonal: f64,
}

/// What happened to an added column.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Added {
    /// The column is independent of the accepted ones and now has a reflector.
    Accepted,
    /// The column lies in the span of the accepted columns; `projection` holds its coordinates
    /// `Q^T column` on the pivot rows as (reflector, value), ascending.
    Dependent { projection: Vec<(usize, f64)> },
}

/// A dense scratch vector with the list of rows it touches.
#[derive(Clone, Debug)]
struct Workspace {
    values: Vec<f64>,
    touched: Vec<bool>,
    support: Vec<usize>,
}

impl Workspace {
    fn new(dimension: usize) -> Workspace {
        Workspace {
            values: vec![0.0; dimension],
            touched: vec![false; dimension],
            support: Vec::new(),
        }
    }

    fn add(&mut self, row: usize, value: f64) {
        if !self.touched[row] {
            self.touched[row] = true;
            self.support.push(row);
        }
        self.values[row] += value;
    }

    /// Returns the touched (row, value) pairs in ascending row order and clears the workspace.
    fn take(&mut self) -> Vec<(usize, f64)> {
        self.support.sort_unstable();
        let entries = self
            .support
            .iter()
            .map(|&row| (row, self.values[row]))
            .collect();
        for &row in &self.support {
            self.values[row] = 0.0;
            self.touched[row] = false;
        }
        self.support.clear();
        entries
    }
}

/// The factorization of the columns added so far.
#[derive(Clone, Debug)]
pub(crate) struct SparseQr {
    reflectors: Vec<Reflector>,
    /// For each row, the reflectors whose rows contain it, ascending.
    touching: Vec<Vec<usize>>,
    /// For each row, the reflector it is the pivot of.
    pivot_of: Vec<Option<usize>>,
    workspace: Workspace,
    /// For each reflector, whether it waits in the queue of `apply`; all false between calls.
    queued: Vec<bool>,
}

/// The order in which `SparseQr::apply` applies reflectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Order {
    /// First to last: `Q^T x`.
    Ascending,
    /// Last to first: `Q x`.
    Descending,
}

impl SparseQr {
    /// An empty factorization of columns with `dimension` rows.
    pub(crate) fn new(dimension: usize) -> SparseQr {
        SparseQr {
            reflectors: Vec::new(),
            touching: vec![Vec::new(); dimension],
            pivot_of: vec![None; dimension],
            workspace: Workspace::new(dimension),
            queued: Vec::new(),
        }
    }

    /// Number of accepted columns.
    pub(crate) fn rank(&self) -> usize {
        self.reflectors.len()
    }

    /// Adds a column given as (row, value) pairs; repeated rows are summed.
    ///
    /// The column is dependent when its part orthogonal to the accepted columns has at most `tolerance`
    /// times its own norm. Columns are Jacobian rows, whose entries are dimensionless and of order 1,
    /// so the norm is taken as at least 1: a column shorter than `tolerance` is zero up to rounding
    /// (an exactly zero column always is dependent).
    pub(crate) fn add(&mut self, column: &[(usize, f64)], tolerance: f64) -> Added {
        for &(row, value) in column {
            self.workspace.add(row, value);
        }
        let original_norm = norm(
            self.workspace
                .support
                .iter()
                .map(|&row| self.workspace.values[row]),
        );
        self.apply(Order::Ascending);
        let mut projection = Vec::new();
        let mut residual = Vec::new();
        for (row, value) in self.workspace.take() {
            match self.pivot_of[row] {
                Some(reflector) => projection.push((reflector, value)),
                None if value != 0.0 => residual.push((row, value)),
                None => {}
            }
        }
        projection.sort_unstable_by_key(|&(reflector, _)| reflector);
        let residual_norm = norm(residual.iter().map(|&(_, value)| value));
        let independent =
            residual_norm > tolerance * original_norm.max(1.0) && residual_norm.is_finite();
        if !independent {
            return Added::Dependent { projection };
        }
        let index = self.reflectors.len();
        let (pivot, pivot_value) = residual[0];
        let diagonal = if pivot_value >= 0.0 {
            -residual_norm
        } else {
            residual_norm
        };
        let rows: Vec<usize> = residual.iter().map(|&(row, _)| row).collect();
        let mut vector: Vec<f64> = residual.iter().map(|&(_, value)| value).collect();
        vector[0] -= diagonal;
        for &row in &rows {
            self.touching[row].push(index);
        }
        self.pivot_of[pivot] = Some(index);
        self.queued.push(false);
        self.reflectors.push(Reflector {
            pivot,
            rows,
            vector,
            beta: 1.0 / (residual_norm * (residual_norm + pivot_value.abs())),
            above: projection,
            diagonal,
        });
        Added::Accepted
    }

    /// Applies to the workspace, in ascending order (`Q^T x`) or descending order (`Q x`), every reflector
    /// whose rows overlap it. A reflector that fills in new rows queues the reflectors after it in that
    /// order which touch them; each reflector is queued at most once.
    fn apply(&mut self, order: Order) {
        let last = self.reflectors.len().saturating_sub(1);
        // The heap pops its largest key first, so ascending order keys each reflector by its distance from the last one.
        let key = |reflector: usize| match order {
            Order::Ascending => last - reflector,
            Order::Descending => reflector,
        };
        let mut queue = BinaryHeap::new();
        for &row in &self.workspace.support {
            for &reflector in &self.touching[row] {
                if !self.queued[reflector] {
                    self.queued[reflector] = true;
                    queue.push(key(reflector));
                }
            }
        }
        while let Some(top) = queue.pop() {
            let reflector = key(top);
            self.queued[reflector] = false;
            let touched_before = self.workspace.support.len();
            reflect(&self.reflectors[reflector], &mut self.workspace);
            for &row in &self.workspace.support[touched_before..] {
                let list = &self.touching[row];
                let following = match order {
                    Order::Ascending => &list[list.partition_point(|&other| other <= reflector)..],
                    Order::Descending => &list[..list.partition_point(|&other| other < reflector)],
                };
                for &next in following {
                    if !self.queued[next] {
                        self.queued[next] = true;
                        queue.push(key(next));
                    }
                }
            }
        }
    }

    /// The minimum-norm `h` with `A^T h = right_side`, where `right_side` has one entry per accepted column.
    pub(crate) fn minimum_norm_solution(&self, right_side: &[f64]) -> Vec<f64> {
        let mut solution = vec![0.0; self.touching.len()];
        let mut forward = vec![0.0; self.reflectors.len()];
        for (index, reflector) in self.reflectors.iter().enumerate() {
            let known: f64 = reflector
                .above
                .iter()
                .map(|&(earlier, value)| value * forward[earlier])
                .sum();
            forward[index] = (right_side[index] - known) / reflector.diagonal;
            solution[reflector.pivot] = forward[index];
        }
        for reflector in self.reflectors.iter().rev() {
            reflect_dense(reflector, &mut solution);
        }
        solution
    }

    /// The `h` minimizing `norm(A h - right_side)`, one entry per accepted column; `right_side` is dense.
    pub(crate) fn least_squares(&self, right_side: &[f64]) -> Vec<f64> {
        let mut transformed = right_side.to_vec();
        for reflector in &self.reflectors {
            reflect_dense(reflector, &mut transformed);
        }
        let projection: Vec<f64> = self
            .reflectors
            .iter()
            .map(|reflector| transformed[reflector.pivot])
            .collect();
        self.back_substitute(projection)
    }

    /// The least-squares coefficients over the accepted columns of a dependent column, from its projection.
    pub(crate) fn coefficients(&self, projection: &[(usize, f64)]) -> Vec<f64> {
        let mut dense = vec![0.0; self.reflectors.len()];
        for &(reflector, value) in projection {
            dense[reflector] = value;
        }
        self.back_substitute(dense)
    }

    /// Solves `R x = right_side` in place.
    fn back_substitute(&self, mut values: Vec<f64>) -> Vec<f64> {
        for (index, reflector) in self.reflectors.iter().enumerate().rev() {
            values[index] /= reflector.diagonal;
            let solved = values[index];
            if solved != 0.0 {
                for &(earlier, value) in &reflector.above {
                    values[earlier] -= value * solved;
                }
            }
        }
        values
    }

    /// For each row, the distance from its unit vector to the span of the accepted columns.
    pub(crate) fn complement_distances(&mut self) -> Vec<f64> {
        let mut squared = vec![0.0; self.touching.len()];
        for row in 0..self.touching.len() {
            if self.pivot_of[row].is_some() {
                continue;
            }
            // Q e_row is a unit vector orthogonal to every accepted column; together they span the complement.
            self.workspace.add(row, 1.0);
            self.apply(Order::Descending);
            for (other, value) in self.workspace.take() {
                squared[other] += value * value;
            }
        }
        squared.into_iter().map(f64::sqrt).collect()
    }
}

/// Applies one reflector to the workspace; rows it touches for the first time are appended to its support.
fn reflect(reflector: &Reflector, workspace: &mut Workspace) {
    let product: f64 = reflector
        .rows
        .iter()
        .zip(&reflector.vector)
        .map(|(&row, &value)| value * workspace.values[row])
        .sum();
    if product == 0.0 {
        return;
    }
    let factor = reflector.beta * product;
    for (&row, &value) in reflector.rows.iter().zip(&reflector.vector) {
        workspace.add(row, -factor * value);
    }
}

/// Applies one reflector to a dense vector.
fn reflect_dense(reflector: &Reflector, values: &mut [f64]) {
    let product: f64 = reflector
        .rows
        .iter()
        .zip(&reflector.vector)
        .map(|(&row, &value)| value * values[row])
        .sum();
    if product == 0.0 {
        return;
    }
    let factor = reflector.beta * product;
    for (&row, &value) in reflector.rows.iter().zip(&reflector.vector) {
        values[row] -= factor * value;
    }
}

fn norm(values: impl Iterator<Item = f64>) -> f64 {
    values.map(|value| value * value).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dense(column: &[f64]) -> Vec<(usize, f64)> {
        column
            .iter()
            .enumerate()
            .filter(|(_, value)| **value != 0.0)
            .map(|(row, value)| (row, *value))
            .collect()
    }

    #[test]
    fn independent_columns_are_accepted() {
        let mut qr = SparseQr::new(3);

        assert_eq!(qr.add(&dense(&[1.0, 2.0, 0.0]), 1e-8), Added::Accepted);
        assert_eq!(qr.add(&dense(&[0.0, 1.0, 1.0]), 1e-8), Added::Accepted);
        assert_eq!(qr.rank(), 2);
    }

    #[test]
    fn zero_column_is_dependent() {
        let mut qr = SparseQr::new(2);

        assert_eq!(
            qr.add(&[], 1e-8),
            Added::Dependent {
                projection: Vec::new()
            }
        );
    }

    #[test]
    fn combination_of_accepted_columns_is_dependent_with_its_coefficients() {
        let mut qr = SparseQr::new(3);
        qr.add(&dense(&[1.0, 2.0, 0.0]), 1e-8);
        qr.add(&dense(&[0.0, 1.0, 1.0]), 1e-8);

        let added = qr.add(&dense(&[2.0, 1.0, -3.0]), 1e-8);

        let Added::Dependent { projection } = added else {
            panic!("the column is 2 a - 3 b, found {added:?}");
        };
        let coefficients = qr.coefficients(&projection);
        assert!((coefficients[0] - 2.0).abs() < 1e-12);
        assert!((coefficients[1] + 3.0).abs() < 1e-12);
    }

    #[test]
    fn column_that_is_zero_up_to_rounding_is_dependent() {
        let mut qr = SparseQr::new(2);

        assert!(matches!(
            qr.add(&[(1, 1e-30)], 1e-8),
            Added::Dependent { .. }
        ));
    }

    #[test]
    fn repeated_rows_are_summed() {
        let mut qr = SparseQr::new(2);
        qr.add(&[(0, 1.0)], 1e-8);

        let added = qr.add(&[(1, 1.0), (1, -1.0), (0, 3.0)], 1e-8);

        assert!(matches!(added, Added::Dependent { .. }));
    }

    #[test]
    fn minimum_norm_solution_satisfies_the_rows_without_extra_motion() {
        let rows = [[1.0, 1.0, 0.0], [0.0, 1.0, 1.0]];
        let mut qr = SparseQr::new(3);
        for row in &rows {
            qr.add(&dense(row), 1e-8);
        }

        let solution = qr.minimum_norm_solution(&[2.0, 1.0]);

        for (row, expected) in rows.iter().zip([2.0, 1.0]) {
            let value: f64 = row.iter().zip(&solution).map(|(a, b)| a * b).sum();
            assert!((value - expected).abs() < 1e-12);
        }
        // The minimum-norm solution lies in the row space, so it is orthogonal to the null vector (1, -1, 1).
        let null: f64 = solution[0] - solution[1] + solution[2];
        assert!(null.abs() < 1e-12);
    }

    #[test]
    fn least_squares_fits_an_overdetermined_system() {
        // Columns of A = [[1, 0], [1, 1], [1, 2]]; the best line through (0, 1), (1, 2), (2, 2).
        let mut qr = SparseQr::new(3);
        qr.add(&dense(&[1.0, 1.0, 1.0]), 0.0);
        qr.add(&dense(&[0.0, 1.0, 2.0]), 0.0);

        let solution = qr.least_squares(&[1.0, 2.0, 2.0]);

        assert!((solution[0] - 7.0 / 6.0).abs() < 1e-12);
        assert!((solution[1] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn complement_distances_measure_undetermined_rows() {
        let mut qr = SparseQr::new(3);
        qr.add(&dense(&[1.0, 0.0, 0.0]), 1e-8);
        qr.add(&dense(&[0.0, 1.0, 1.0]), 1e-8);

        let distances = qr.complement_distances();

        assert!(distances[0].abs() < 1e-12);
        assert!((distances[1] - 0.5_f64.sqrt()).abs() < 1e-12);
        assert!((distances[2] - 0.5_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn fill_in_is_followed_through_later_reflectors() {
        // A chain of overlapping columns where later columns only meet earlier ones through fill.
        let mut qr = SparseQr::new(4);
        qr.add(&dense(&[1.0, 1.0, 0.0, 0.0]), 1e-8);
        qr.add(&dense(&[0.0, 1.0, 1.0, 0.0]), 1e-8);
        qr.add(&dense(&[0.0, 0.0, 1.0, 1.0]), 1e-8);

        let added = qr.add(&dense(&[1.0, 0.0, 0.0, 1.0]), 1e-8);

        let Added::Dependent { projection } = added else {
            panic!("the column is a - b + c, found {added:?}");
        };
        let coefficients = qr.coefficients(&projection);
        for (actual, expected) in coefficients.iter().zip([1.0, -1.0, 1.0]) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }
}
