//! The sparse Cholesky factorization `J J^T + mu I = C C^T` of the damped step (spec section 3.7.4,
//! ADR 0058).
//!
//! `J` is given by its sparse rows. The factorization eliminates them in their order. The entry
//! `(i, j)` of `J J^T` is the dot product of rows `i` and `j`, summed over their shared variables in
//! increasing variable order. `C` is lower triangular with a positive diagonal and holds only the
//! entries of the pattern that the elimination tree of `J J^T` gives.
//!
//! The pattern and the tree depend only on the sparsity of `J`. They are worked out once and kept
//! while the rows keep their sparsity. The factorizations of a solve and of the frames of a drag
//! then only do the arithmetic. The products of the rows depend only on `J`. They are computed once
//! per point, and each damping `mu` then costs one numeric factorization.

use std::cmp::Ordering;

/// Marks a missing node of the elimination tree.
const NONE: usize = usize::MAX;

/// The factorization of `J J^T + mu I` for the rows given to [`Cholesky::prepare`].
#[derive(Debug, Default)]
pub(super) struct Cholesky {
    /// Where each row of `J` starts in `row_variables`, plus the end of the last: the sparsity the
    /// pattern below was worked out for.
    row_starts: Vec<usize>,
    /// The variables of each row of `J`, ascending.
    row_variables: Vec<usize>,
    /// Where each row's earlier neighbors start in `neighbors`, plus the end of the last.
    neighbor_starts: Vec<usize>,
    /// For each row `k`, the rows `i < k` that share a variable with it, ascending: the entries left
    /// of the diagonal of `J J^T`.
    neighbors: Vec<usize>,
    /// The products of each row with its earlier neighbors, aligned with `neighbors`.
    products: Vec<f64>,
    /// The squared norm of each row.
    squared_norms: Vec<f64>,
    /// Where each row of `C` starts in `factor_columns` and `factor_positions`, plus the end of the
    /// last.
    factor_starts: Vec<usize>,
    /// The columns of each row's entries left of the diagonal of `C`, ascending.
    factor_columns: Vec<usize>,
    /// Where each of those entries is stored in `below_rows` and `below_values`.
    factor_positions: Vec<usize>,
    /// Where each column's entries below the diagonal of `C` start in `below_rows` and
    /// `below_values`, plus the end of the last.
    column_starts: Vec<usize>,
    /// The rows of the entries below the diagonal of `C`, ascending within each column.
    below_rows: Vec<usize>,
    /// The values of those entries.
    below_values: Vec<f64>,
    /// The diagonal of `C`.
    diagonal: Vec<f64>,
    /// Dense scratch vector with one entry per row; all zero between calls.
    scratch: Vec<f64>,
}

impl Cholesky {
    /// Takes the rows of `J` at a new point, given back to back as (variable, partial derivative)
    /// pairs, ascending within a row, with `starts` holding where each row starts plus the end of the
    /// last. The pattern of `C` is worked out again only when the sparsity of the rows changed.
    pub(super) fn prepare(&mut self, starts: &[usize], entries: &[(usize, f64)]) {
        let same_sparsity = self.row_starts == starts
            && self.row_variables.len() == entries.len()
            && self
                .row_variables
                .iter()
                .zip(entries)
                .all(|(&variable, &(entry, _))| variable == entry);
        if !same_sparsity {
            self.row_starts = starts.to_vec();
            self.row_variables = entries.iter().map(|&(variable, _)| variable).collect();
            self.analyze();
        }
        let row = |index: usize| &entries[starts[index]..starts[index + 1]];
        for (index, squared_norm) in self.squared_norms.iter_mut().enumerate() {
            *squared_norm = row(index)
                .iter()
                .map(|&(_, partial)| partial * partial)
                .sum();
        }
        for index in 0..self.squared_norms.len() {
            for slot in self.neighbor_starts[index]..self.neighbor_starts[index + 1] {
                self.products[slot] = sparse_dot(row(self.neighbors[slot]), row(index));
            }
        }
    }

    /// Works out the pattern of `C` from the sparsity of the rows: the earlier neighbors of each row,
    /// the elimination tree of `J J^T`, the entries of each row of `C` and where each is stored.
    fn analyze(&mut self) {
        let row_count = self.row_starts.len() - 1;
        self.find_neighbors(row_count);
        let parent = self.elimination_tree(row_count);
        self.find_factor_rows(&parent);
        self.place_factor_entries(row_count);
        self.products = vec![0.0; self.neighbors.len()];
        self.squared_norms = vec![0.0; row_count];
        self.diagonal = vec![0.0; row_count];
        self.scratch = vec![0.0; row_count];
    }

    /// Fills `neighbors` and `neighbor_starts`: for each row, the earlier rows that share a variable
    /// with it.
    fn find_neighbors(&mut self, row_count: usize) {
        let variable_count = self
            .row_variables
            .iter()
            .max()
            .map_or(0, |&largest| largest + 1);
        let mut rows_of_variable = vec![Vec::new(); variable_count];
        let mut mark = vec![NONE; row_count];
        self.neighbor_starts.clear();
        self.neighbor_starts.push(0);
        self.neighbors.clear();
        for index in 0..row_count {
            let start = self.neighbors.len();
            let variables = &self.row_variables[self.row_starts[index]..self.row_starts[index + 1]];
            for &variable in variables {
                for &neighbor in &rows_of_variable[variable] {
                    if mark[neighbor] != index {
                        mark[neighbor] = index;
                        self.neighbors.push(neighbor);
                    }
                }
            }
            self.neighbors[start..].sort_unstable();
            self.neighbor_starts.push(self.neighbors.len());
            for &variable in variables {
                rows_of_variable[variable].push(index);
            }
        }
    }

    /// The parent of each row in the elimination tree of `J J^T`, `NONE` for a root, by Liu's
    /// algorithm with path compression: the parent of a row is the first later row whose
    /// elimination reaches it.
    fn elimination_tree(&self, row_count: usize) -> Vec<usize> {
        let mut parent = vec![NONE; row_count];
        let mut ancestor = vec![NONE; row_count];
        for index in 0..row_count {
            for &neighbor in self.neighbors_of(index) {
                let mut node = neighbor;
                while node < index {
                    let next = ancestor[node];
                    ancestor[node] = index;
                    if next == NONE {
                        parent[node] = index;
                    }
                    node = next;
                }
            }
        }
        parent
    }

    /// Fills `factor_columns` and `factor_starts`: row `index` of `C` holds every node on the tree
    /// paths from its neighbors up to itself.
    fn find_factor_rows(&mut self, parent: &[usize]) {
        let mut mark = vec![NONE; parent.len()];
        self.factor_starts.clear();
        self.factor_starts.push(0);
        self.factor_columns.clear();
        for index in 0..parent.len() {
            let start = self.factor_columns.len();
            mark[index] = index;
            for &neighbor in
                &self.neighbors[self.neighbor_starts[index]..self.neighbor_starts[index + 1]]
            {
                let mut node = neighbor;
                while mark[node] != index {
                    mark[node] = index;
                    self.factor_columns.push(node);
                    node = parent[node];
                }
            }
            self.factor_columns[start..].sort_unstable();
            self.factor_starts.push(self.factor_columns.len());
        }
    }

    /// Fills `column_starts`, `below_rows` and `factor_positions`, with the entries of each column of
    /// `C` stored in row order, and sizes `below_values`.
    fn place_factor_entries(&mut self, row_count: usize) {
        let mut column_counts = vec![0; row_count];
        for &column in &self.factor_columns {
            column_counts[column] += 1;
        }
        self.column_starts.clear();
        self.column_starts.push(0);
        let mut total = 0;
        for count in column_counts {
            total += count;
            self.column_starts.push(total);
        }
        let mut next = self.column_starts[..row_count].to_vec();
        self.below_rows = vec![0; total];
        self.factor_positions.clear();
        for index in 0..row_count {
            for &column in
                &self.factor_columns[self.factor_starts[index]..self.factor_starts[index + 1]]
            {
                self.below_rows[next[column]] = index;
                self.factor_positions.push(next[column]);
                next[column] += 1;
            }
        }
        self.below_values = vec![0.0; total];
    }

    /// The earlier neighbors of a row.
    fn neighbors_of(&self, index: usize) -> &[usize] {
        &self.neighbors[self.neighbor_starts[index]..self.neighbor_starts[index + 1]]
    }

    /// Factors `J J^T + damping I` for the rows of the last [`Cholesky::prepare`], one row of `C` at
    /// a time. A pivot that rounding leaves at or below zero gives a diagonal entry that is zero or
    /// not a number, and the solutions that follow are not finite.
    pub(super) fn factor(&mut self, damping: f64) {
        for index in 0..self.diagonal.len() {
            for slot in self.neighbor_starts[index]..self.neighbor_starts[index + 1] {
                self.scratch[self.neighbors[slot]] = self.products[slot];
            }
            let mut pivot = self.squared_norms[index] + damping;
            for entry in self.factor_starts[index]..self.factor_starts[index + 1] {
                let column = self.factor_columns[entry];
                let position = self.factor_positions[entry];
                let value = std::mem::take(&mut self.scratch[column]) / self.diagonal[column];
                // The column's earlier entries are rows between the column and this one.
                for earlier in self.column_starts[column]..position {
                    self.scratch[self.below_rows[earlier]] -= self.below_values[earlier] * value;
                }
                self.below_values[position] = value;
                pivot -= value * value;
            }
            self.diagonal[index] = pivot.sqrt();
        }
    }

    /// The solution `y` of `(J J^T + mu I) y = right_side` with the damping of the last
    /// factorization, from `C w = right_side` and `C^T y = w`.
    pub(super) fn solve(&self, right_side: &[f64]) -> Vec<f64> {
        let mut values = right_side.to_vec();
        for column in 0..values.len() {
            values[column] /= self.diagonal[column];
            let known = values[column];
            for entry in self.column_starts[column]..self.column_starts[column + 1] {
                values[self.below_rows[entry]] -= self.below_values[entry] * known;
            }
        }
        for column in (0..values.len()).rev() {
            let mut value = values[column];
            for entry in self.column_starts[column]..self.column_starts[column + 1] {
                value -= self.below_values[entry] * values[self.below_rows[entry]];
            }
            values[column] = value / self.diagonal[column];
        }
        values
    }
}

/// The dot product of two sparse rows with ascending variables, summed in increasing variable order.
fn sparse_dot(first: &[(usize, f64)], second: &[(usize, f64)]) -> f64 {
    let mut total = 0.0;
    let (mut first_index, mut second_index) = (0, 0);
    while let (Some(&(first_variable, first_partial)), Some(&(second_variable, second_partial))) =
        (first.get(first_index), second.get(second_index))
    {
        match first_variable.cmp(&second_variable) {
            Ordering::Less => first_index += 1,
            Ordering::Greater => second_index += 1,
            Ordering::Equal => {
                total += first_partial * second_partial;
                first_index += 1;
                second_index += 1;
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sparse rows given densely, back to back, with their starts.
    fn sparse_rows(rows: &[&[f64]]) -> (Vec<usize>, Vec<(usize, f64)>) {
        let mut starts = vec![0];
        let mut entries = Vec::new();
        for row in rows {
            entries.extend(
                row.iter()
                    .enumerate()
                    .filter(|(_, value)| **value != 0.0)
                    .map(|(variable, value)| (variable, *value)),
            );
            starts.push(entries.len());
        }
        (starts, entries)
    }

    /// `(J J^T + damping I) solution` for dense rows.
    fn damped_product(rows: &[&[f64]], damping: f64, solution: &[f64]) -> Vec<f64> {
        rows.iter()
            .enumerate()
            .map(|(index, row)| {
                let image: f64 = rows
                    .iter()
                    .zip(solution)
                    .map(|(other, value)| {
                        row.iter()
                            .zip(*other)
                            .map(|(first, second)| first * second)
                            .sum::<f64>()
                            * value
                    })
                    .sum();
                image + damping * solution[index]
            })
            .collect()
    }

    fn factored(rows: &[&[f64]], damping: f64) -> Cholesky {
        let (starts, entries) = sparse_rows(rows);
        let mut cholesky = Cholesky::default();
        cholesky.prepare(&starts, &entries);
        cholesky.factor(damping);
        cholesky
    }

    fn assert_solves(rows: &[&[f64]], damping: f64, right_side: &[f64]) {
        let solution = factored(rows, damping).solve(right_side);

        let image = damped_product(rows, damping, &solution);
        for (actual, expected) in image.iter().zip(right_side) {
            assert!(
                (actual - expected).abs() <= 1e-12,
                "{image:?} for {right_side:?}"
            );
        }
    }

    #[test]
    fn solves_the_damped_system_of_overlapping_rows() {
        let rows: [&[f64]; 3] = [&[1.0, 1.0, 0.0], &[0.0, 1.0, 1.0], &[2.0, 0.0, -1.0]];

        assert_solves(&rows, 0.5, &[1.0, -2.0, 3.0]);
    }

    #[test]
    fn solves_dependent_rows_with_damping() {
        // The second row repeats the first; J J^T alone is singular.
        let rows: [&[f64]; 2] = [&[1.0, 2.0], &[1.0, 2.0]];

        assert_solves(&rows, 1e-3, &[1.0, 1.0]);
    }

    #[test]
    fn a_zero_row_takes_its_value_over_the_damping() {
        // A constant row, whose parameters are all frozen, has no entries.
        let rows: [&[f64]; 2] = [&[1.0, 0.0], &[0.0, 0.0]];

        let solution = factored(&rows, 0.25).solve(&[1.25, 2.0]);

        assert_eq!(solution, [1.0, 8.0]);
    }

    #[test]
    fn rows_that_share_no_variable_are_factored_apart() {
        let rows: [&[f64]; 2] = [&[3.0, 0.0], &[0.0, 4.0]];

        let cholesky = factored(&rows, 0.0);

        assert!(cholesky.below_values.is_empty());
        assert_eq!(cholesky.diagonal, [3.0, 4.0]);
    }

    #[test]
    fn fill_follows_the_elimination_tree() {
        // Rows 0 and 1 meet row 2 only through variables 0 and 1; row 3 meets row 0 alone. The
        // tree is 0 -> 2 -> 3 and 1 -> 2, so row 3 of C fills in at column 2.
        let rows: [&[f64]; 4] = [
            &[1.0, 0.0, 0.0, 1.0],
            &[0.0, 1.0, 0.0, 0.0],
            &[1.0, 1.0, 1.0, 0.0],
            &[0.0, 0.0, 0.0, 1.0],
        ];

        let cholesky = factored(&rows, 1.0);

        assert_eq!(cholesky.factor_starts, [0, 0, 0, 2, 4]);
        assert_eq!(cholesky.factor_columns, [0, 1, 0, 2]);
        assert_solves(&rows, 1.0, &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn the_pattern_is_kept_while_the_rows_keep_their_sparsity() {
        let (starts, entries) = sparse_rows(&[&[1.0, 1.0], &[0.0, 1.0]]);
        let mut cholesky = Cholesky::default();
        cholesky.prepare(&starts, &entries);
        let pattern = cholesky.factor_columns.clone();
        cholesky.below_rows.clear();

        let (starts, entries) = sparse_rows(&[&[2.0, 3.0], &[0.0, 5.0]]);
        cholesky.prepare(&starts, &entries);

        assert!(
            cholesky.below_rows.is_empty(),
            "the pattern was worked out again"
        );
        assert_eq!(cholesky.factor_columns, pattern);
        assert_eq!(cholesky.products, [15.0]);
        assert_eq!(cholesky.squared_norms, [13.0, 25.0]);
    }

    #[test]
    fn a_changed_sparsity_works_out_the_pattern_again() {
        let rows: [&[f64]; 2] = [&[1.0, 0.0], &[0.0, 1.0]];
        let (starts, entries) = sparse_rows(&rows);
        let mut cholesky = Cholesky::default();
        cholesky.prepare(&starts, &entries);
        assert!(cholesky.factor_columns.is_empty());

        let rows: [&[f64]; 2] = [&[1.0, 1.0], &[0.0, 1.0]];
        let (starts, entries) = sparse_rows(&rows);
        cholesky.prepare(&starts, &entries);
        cholesky.factor(0.5);

        assert_eq!(cholesky.factor_columns, [0]);
        assert_solves(&rows, 0.5, &[1.0, 1.0]);
    }

    #[test]
    fn the_scratch_vector_is_zero_after_a_factorization() {
        let rows: [&[f64]; 3] = [&[1.0, 1.0, 0.0], &[0.0, 1.0, 1.0], &[1.0, 0.0, 1.0]];

        let cholesky = factored(&rows, 1e-6);

        assert!(cholesky.scratch.iter().all(|&value| value == 0.0));
    }

    #[test]
    fn sparse_dot_sums_the_shared_variables() {
        assert_eq!(
            sparse_dot(
                &[(0, 2.0), (2, 3.0), (5, 1.0)],
                &[(1, 7.0), (2, 4.0), (5, -1.0)]
            ),
            11.0
        );
    }
}
