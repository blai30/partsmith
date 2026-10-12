//! Residual rows with analytic gradients (spec sections 3.6.2 and 3.6.5).
//!
//! Every row is a weighted sum of a few geometric terms plus a constant, and every residual is a length in mm.

use crate::vector::{MIN_LENGTH, add, cross, dot, norm, scale, subtract, unit};

/// A gradient being accumulated: (parameter index, partial derivative) pairs, indices possibly repeated.
pub(crate) type Gradient = Vec<(usize, f64)>;

/// A point of the parameter vector, or a constant point such as the origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PointTerm {
    /// The point whose `x` is the parameter at this index and whose `y` follows it.
    Parameters(usize),
    /// A constant point.
    Constant([f64; 2]),
}

impl PointTerm {
    pub(crate) fn value(self, x: &[f64]) -> [f64; 2] {
        match self {
            PointTerm::Parameters(index) => [x[index], x[index + 1]],
            PointTerm::Constant(point) => point,
        }
    }

    fn add_gradient(self, partial: [f64; 2], gradient: &mut Gradient) {
        if let PointTerm::Parameters(index) = self {
            gradient.push((index, partial[0]));
            gradient.push((index + 1, partial[1]));
        }
    }

    fn parameters(self, out: &mut Vec<usize>) {
        if let PointTerm::Parameters(index) = self {
            out.extend([index, index + 1]);
        }
    }
}

/// A straight line through two points, directed from `start` to `end`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LineTerm {
    pub(crate) start: PointTerm,
    pub(crate) end: PointTerm,
}

/// The point a signed distance is measured from: a point, or the midpoint of two points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Anchor {
    Point(PointTerm),
    Midpoint(PointTerm, PointTerm),
}

impl Anchor {
    fn value(self, x: &[f64]) -> [f64; 2] {
        match self {
            Anchor::Point(point) => point.value(x),
            Anchor::Midpoint(first, second) => {
                let first = first.value(x);
                let second = second.value(x);
                [(first[0] + second[0]) / 2.0, (first[1] + second[1]) / 2.0]
            }
        }
    }

    fn add_gradient(self, partial: [f64; 2], gradient: &mut Gradient) {
        match self {
            Anchor::Point(point) => point.add_gradient(partial, gradient),
            Anchor::Midpoint(first, second) => {
                let half = scale(partial, 0.5);
                first.add_gradient(half, gradient);
                second.add_gradient(half, gradient);
            }
        }
    }

    fn parameters(self, out: &mut Vec<usize>) {
        match self {
            Anchor::Point(point) => point.parameters(out),
            Anchor::Midpoint(first, second) => {
                first.parameters(out);
                second.parameters(out);
            }
        }
    }
}

/// One geometric quantity of a row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Term {
    /// A single parameter.
    Coordinate(usize),
    /// `norm(to - from)`.
    Length { from: PointTerm, to: PointTerm },
    /// `sd(point, line)`: signed distance to the infinite line, positive on its left.
    SignedDistance { point: Anchor, line: LineTerm },
    /// `dot(d, to - from) / safe(d)` with `d` the line's direction.
    Projection {
        from: PointTerm,
        to: PointTerm,
        line: LineTerm,
    },
    /// `cross(u1, u2) * cos - dot(u1, u2) * sin` with `u1`, `u2` the unit directions of the two lines.
    Angle {
        first: LineTerm,
        second: LineTerm,
        cos: f64,
        sin: f64,
    },
}

impl Term {
    /// The value of the term; when `gradient` is given, its partial derivatives times `weight` are added to it.
    fn evaluate(&self, x: &[f64], weight: f64, gradient: Option<&mut Gradient>) -> f64 {
        match *self {
            Term::Coordinate(index) => {
                if let Some(gradient) = gradient {
                    gradient.push((index, weight));
                }
                x[index]
            }
            Term::Length { from, to } => {
                let offset = subtract(to.value(x), from.value(x));
                if let Some(gradient) = gradient {
                    let direction = scale(unit(offset), weight);
                    to.add_gradient(direction, gradient);
                    from.add_gradient(scale(direction, -1.0), gradient);
                }
                norm(offset)
            }
            Term::SignedDistance { point, line } => {
                let start = line.start.value(x);
                let direction = subtract(line.end.value(x), start);
                let relative = subtract(point.value(x), start);
                let length = norm(direction);
                let safe_length = length.max(MIN_LENGTH);
                let value = cross(direction, relative) / safe_length;
                if let Some(gradient) = gradient {
                    let by_point = scale([-direction[1], direction[0]], 1.0 / safe_length);
                    let mut by_direction = scale([relative[1], -relative[0]], 1.0 / safe_length);
                    if length >= MIN_LENGTH {
                        by_direction = subtract(by_direction, scale(direction, value / (length * length)));
                    }
                    point.add_gradient(scale(by_point, weight), gradient);
                    line.end.add_gradient(scale(by_direction, weight), gradient);
                    line.start.add_gradient(
                        scale(add(by_direction, by_point), -weight),
                        gradient,
                    );
                }
                value
            }
            Term::Projection { from, to, line } => {
                let start = line.start.value(x);
                let direction = subtract(line.end.value(x), start);
                let span = subtract(to.value(x), from.value(x));
                let length = norm(direction);
                let safe_length = length.max(MIN_LENGTH);
                let value = dot(direction, span) / safe_length;
                if let Some(gradient) = gradient {
                    let by_span = scale(direction, 1.0 / safe_length);
                    let mut by_direction = scale(span, 1.0 / safe_length);
                    if length >= MIN_LENGTH {
                        by_direction = subtract(by_direction, scale(direction, value / (length * length)));
                    }
                    to.add_gradient(scale(by_span, weight), gradient);
                    from.add_gradient(scale(by_span, -weight), gradient);
                    line.end.add_gradient(scale(by_direction, weight), gradient);
                    line.start.add_gradient(scale(by_direction, -weight), gradient);
                }
                value
            }
            Term::Angle {
                first,
                second,
                cos,
                sin,
            } => {
                let first_direction = subtract(first.end.value(x), first.start.value(x));
                let second_direction = subtract(second.end.value(x), second.start.value(x));
                let first_unit = unit(first_direction);
                let second_unit = unit(second_direction);
                let value = cross(first_unit, second_unit) * cos - dot(first_unit, second_unit) * sin;
                if let Some(gradient) = gradient {
                    let by_first_unit = subtract(
                        scale([second_unit[1], -second_unit[0]], cos),
                        scale(second_unit, sin),
                    );
                    let by_second_unit = subtract(
                        scale([-first_unit[1], first_unit[0]], cos),
                        scale(first_unit, sin),
                    );
                    let by_first = unit_gradient(first_direction, first_unit, by_first_unit);
                    let by_second = unit_gradient(second_direction, second_unit, by_second_unit);
                    first.end.add_gradient(scale(by_first, weight), gradient);
                    first.start.add_gradient(scale(by_first, -weight), gradient);
                    second.end.add_gradient(scale(by_second, weight), gradient);
                    second.start.add_gradient(scale(by_second, -weight), gradient);
                }
                value
            }
        }
    }

    fn parameters(&self, out: &mut Vec<usize>) {
        match *self {
            Term::Coordinate(index) => out.push(index),
            Term::Length { from, to } => {
                from.parameters(out);
                to.parameters(out);
            }
            Term::SignedDistance { point, line } => {
                point.parameters(out);
                line.start.parameters(out);
                line.end.parameters(out);
            }
            Term::Projection { from, to, line } => {
                from.parameters(out);
                to.parameters(out);
                line.start.parameters(out);
                line.end.parameters(out);
            }
            Term::Angle { first, second, .. } => {
                for point in [first.start, first.end, second.start, second.end] {
                    point.parameters(out);
                }
            }
        }
    }
}

/// What a row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowOwner {
    /// The internal arc rule of the entity at this index.
    ArcRule(usize),
    /// The constraint at this index.
    Constraint(usize),
}

/// The trivial equality a row of `coincident`, `concentric`, `horizontal` or `vertical` states (section 3.7.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Equality {
    /// `x_i = x_j`.
    Pair(usize, usize),
    /// `x_i = 0`, against a coordinate of `origin`.
    Zero(usize),
}

/// One residual: `sum(weight * term) + constant`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) owner: RowOwner,
    pub(crate) terms: Vec<(f64, Term)>,
    pub(crate) constant: f64,
    pub(crate) equality: Option<Equality>,
}

impl Row {
    /// A row with these terms and constant and no equality.
    pub(crate) fn new(owner: RowOwner, terms: Vec<(f64, Term)>, constant: f64) -> Row {
        Row {
            owner,
            terms,
            constant,
            equality: None,
        }
    }

    /// The residual at `x`.
    pub(crate) fn value(&self, x: &[f64]) -> f64 {
        self.sum(x, None)
    }

    /// The residual at `x`; its gradient is appended to `gradient`.
    pub(crate) fn value_and_gradient(&self, x: &[f64], gradient: &mut Gradient) -> f64 {
        self.sum(x, Some(gradient))
    }

    fn sum(&self, x: &[f64], mut gradient: Option<&mut Gradient>) -> f64 {
        let mut total = 0.0;
        for (weight, term) in &self.terms {
            total += weight * term.evaluate(x, *weight, gradient.as_deref_mut());
        }
        total + self.constant
    }

    /// The parameters the row reads, possibly repeated.
    pub(crate) fn parameters(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for (_, term) in &self.terms {
            term.parameters(&mut out);
        }
        out
    }
}

/// The derivative with respect to `direction` of a function of `unit(direction)` whose derivative with respect to the unit vector is `by_unit`; zero below the minimum length.
fn unit_gradient(direction: [f64; 2], unit_direction: [f64; 2], by_unit: [f64; 2]) -> [f64; 2] {
    let length = norm(direction);
    if length < MIN_LENGTH {
        return [0.0, 0.0];
    }
    let along = dot(unit_direction, by_unit);
    scale(subtract(by_unit, scale(unit_direction, along)), 1.0 / length)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks the analytic gradient of `row` at `x` against central differences.
    fn assert_gradient_matches(row: &Row, x: &[f64]) {
        let mut gradient = Gradient::new();
        row.value_and_gradient(x, &mut gradient);
        let mut analytic = vec![0.0; x.len()];
        for (index, value) in gradient {
            analytic[index] += value;
        }
        for i in 0..x.len() {
            let step = 1e-6;
            let mut plus = x.to_vec();
            let mut minus = x.to_vec();
            plus[i] += step;
            minus[i] -= step;
            let numeric = (row.value(&plus) - row.value(&minus)) / (2.0 * step);
            assert!(
                (numeric - analytic[i]).abs() < 1e-6,
                "partial {i}: analytic {} numeric {numeric}",
                analytic[i]
            );
        }
    }

    fn line(start: usize, end: usize) -> LineTerm {
        LineTerm {
            start: PointTerm::Parameters(start),
            end: PointTerm::Parameters(end),
        }
    }

    fn row(terms: Vec<(f64, Term)>) -> Row {
        Row::new(RowOwner::Constraint(0), terms, 0.5)
    }

    const X: [f64; 8] = [0.3, -1.2, 4.0, 2.5, -2.0, 3.0, 1.5, -0.7];

    #[test]
    fn length_gradient() {
        let term = Term::Length {
            from: PointTerm::Parameters(0),
            to: PointTerm::Parameters(2),
        };

        assert_gradient_matches(&row(vec![(2.0, term)]), &X);
    }

    #[test]
    fn signed_distance_gradient() {
        let term = Term::SignedDistance {
            point: Anchor::Point(PointTerm::Parameters(0)),
            line: line(2, 4),
        };

        assert_gradient_matches(&row(vec![(-1.0, term)]), &X);
    }

    #[test]
    fn signed_distance_of_a_midpoint_gradient() {
        let term = Term::SignedDistance {
            point: Anchor::Midpoint(PointTerm::Parameters(0), PointTerm::Parameters(6)),
            line: line(2, 4),
        };

        assert_gradient_matches(&row(vec![(1.0, term)]), &X);
    }

    #[test]
    fn projection_gradient() {
        let term = Term::Projection {
            from: PointTerm::Parameters(0),
            to: PointTerm::Parameters(6),
            line: line(2, 4),
        };

        assert_gradient_matches(&row(vec![(1.0, term)]), &X);
    }

    #[test]
    fn angle_gradient() {
        let term = Term::Angle {
            first: line(0, 2),
            second: line(4, 6),
            cos: 0.6,
            sin: 0.8,
        };

        assert_gradient_matches(&row(vec![(3.0, term)]), &X);
    }

    #[test]
    fn constants_have_no_gradient() {
        let term = Term::Length {
            from: PointTerm::Constant([0.0, 0.0]),
            to: PointTerm::Constant([3.0, 4.0]),
        };
        let mut gradient = Gradient::new();

        let value = row(vec![(1.0, term)]).value_and_gradient(&X, &mut gradient);

        assert_eq!(value, 5.5);
        assert!(gradient.is_empty());
    }

}
