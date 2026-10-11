//! Fixtures, builders and assertion helpers shared by the acceptance tests (spec section 6.0).

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

pub mod benchmark;
pub mod generator;
pub mod random;
pub mod residuals;

use std::f64::consts::FRAC_PI_2;
use std::fmt::Debug;

use partsmith_sketch::{
    Alignment, ConstraintKind, DimensionExpr, DimensionValues, DistanceTarget, DragSession,
    DragTarget, Expr, Geometry, Id, NativeDragSession, NativeSolver, OriginPlane, PointRef,
    Profile, SketchConstraint, SketchEntity, SketchError, SketchParams, SketchPlane, SolveResult,
    Solver, build_profile,
};

/// Default absolute tolerance for coordinates in mm (spec section 6.0).
pub const TOLERANCE: f64 = 1e-9;

/// Expression text of every reference dimension in the shorthand of section 6.0.
pub const REFERENCE_EXPR: &str = "7 mm";

/// The `PLATE` sketch of spec section 2.9 as `SketchParams` JSON (the feature record without `id`, `type` and `name`).
pub const PLATE_PARAMS_JSON: &str = r#"{
  "plane": {"kind": "origin", "plane": "XY"},
  "entities": [
    {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
    {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
    {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
    {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]},
    {"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5},
    {"id": "e-6", "type": "line", "start": [40, 0], "end": [40, 50], "construction": true}
  ],
  "constraints": [
    {"id": "c-1", "type": "coincident", "points": ["e-1.end", "e-2.start"]},
    {"id": "c-2", "type": "coincident", "points": ["e-2.end", "e-3.start"]},
    {"id": "c-3", "type": "coincident", "points": ["e-3.end", "e-4.start"]},
    {"id": "c-4", "type": "coincident", "points": ["e-4.end", "e-1.start"]},
    {"id": "c-5", "type": "horizontal", "line": "e-1"},
    {"id": "c-6", "type": "horizontal", "line": "e-3"},
    {"id": "c-7", "type": "vertical", "line": "e-2"},
    {"id": "c-8", "type": "vertical", "line": "e-4"},
    {"id": "c-9", "type": "fix", "target": "e-1.start"},
    {"id": "c-10", "type": "distance", "line": "e-1", "expr": "width"},
    {"id": "c-11", "type": "distance", "line": "e-2", "expr": "50 mm"},
    {"id": "c-12", "type": "midpoint", "point": "e-6.start", "line": "e-1"},
    {"id": "c-13", "type": "midpoint", "point": "e-6.end", "line": "e-3"},
    {"id": "c-14", "type": "point_on_object", "point": "e-5.center", "curve": "e-6"},
    {"id": "c-15", "type": "vertical_distance", "points": ["e-1.start", "e-5.center"], "expr": "25 mm"},
    {"id": "c-16", "type": "diameter", "curve": "e-5", "expr": "hole_d"}
  ]
}"#;

/// The face plane example of spec section 2.2.
pub const FACE_PLANE_JSON: &str = r#"{"kind": "face", "body": "b-1", "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}, "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}"#;

// ---------------------------------------------------------------------------
// Ids and references
// ---------------------------------------------------------------------------

/// A valid id, panicking on a fixture mistake.
pub fn id(text: &str) -> Id {
    Id::new(text).expect("test ids are valid")
}

/// Several ids in order.
pub fn ids(texts: &[&str]) -> Vec<Id> {
    texts.iter().map(|text| id(text)).collect()
}

/// A point reference parsed from its text form, panicking on a fixture mistake.
pub fn point_ref(text: &str) -> PointRef {
    text.parse()
        .unwrap_or_else(|error| panic!("test point reference {text} is valid: {error}"))
}

/// The text of each id, for readable comparisons.
pub fn texts(ids: &[Id]) -> Vec<String> {
    ids.iter().map(ToString::to_string).collect()
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

/// `point e (x, y)`.
pub fn point(entity: &str, x: f64, y: f64) -> SketchEntity {
    SketchEntity {
        id: id(entity),
        geometry: Geometry::Point { position: [x, y] },
        construction: false,
    }
}

/// `line e (x1, y1)->(x2, y2)`.
pub fn line(entity: &str, start: [f64; 2], end: [f64; 2]) -> SketchEntity {
    SketchEntity {
        id: id(entity),
        geometry: Geometry::Line { start, end },
        construction: false,
    }
}

/// `cline e (x1, y1)->(x2, y2)`: a construction line.
pub fn construction_line(entity: &str, start: [f64; 2], end: [f64; 2]) -> SketchEntity {
    as_construction(line(entity, start, end))
}

/// `circle e c(x, y) r R`.
pub fn circle(entity: &str, center: [f64; 2], radius: f64) -> SketchEntity {
    SketchEntity {
        id: id(entity),
        geometry: Geometry::Circle { center, radius },
        construction: false,
    }
}

/// `arc e c(x, y) r R from A0 to A1`.
pub fn arc(
    entity: &str,
    center: [f64; 2],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
) -> SketchEntity {
    SketchEntity {
        id: id(entity),
        geometry: Geometry::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        },
        construction: false,
    }
}

/// The same entity marked as construction geometry.
pub fn as_construction(mut entity: SketchEntity) -> SketchEntity {
    entity.construction = true;
    entity
}

// ---------------------------------------------------------------------------
// Constraints
// ---------------------------------------------------------------------------

/// A constraint record.
pub fn constraint(constraint_id: &str, kind: ConstraintKind) -> SketchConstraint {
    SketchConstraint {
        id: id(constraint_id),
        kind,
    }
}

/// A driving dimension with the given expression text.
pub fn dimension(expr: &str) -> DimensionExpr {
    DimensionExpr {
        expr: Expr::new(expr),
        driving: true,
    }
}

/// `coincident [P, Q]`.
pub fn coincident(first: &str, second: &str) -> ConstraintKind {
    ConstraintKind::Coincident {
        points: [point_ref(first), point_ref(second)],
    }
}

/// `point_on_object P on K`.
pub fn point_on_object(point: &str, curve: &str) -> ConstraintKind {
    ConstraintKind::PointOnObject {
        point: point_ref(point),
        curve: id(curve),
    }
}

/// `horizontal L` (line form).
pub fn horizontal(line: &str) -> ConstraintKind {
    ConstraintKind::Horizontal(Alignment::Line { line: id(line) })
}

/// `horizontal [P, Q]` (points form).
pub fn horizontal_points(first: &str, second: &str) -> ConstraintKind {
    ConstraintKind::Horizontal(Alignment::Points {
        points: [point_ref(first), point_ref(second)],
    })
}

/// `vertical L` (line form).
pub fn vertical(line: &str) -> ConstraintKind {
    ConstraintKind::Vertical(Alignment::Line { line: id(line) })
}

/// `vertical [P, Q]` (points form).
pub fn vertical_points(first: &str, second: &str) -> ConstraintKind {
    ConstraintKind::Vertical(Alignment::Points {
        points: [point_ref(first), point_ref(second)],
    })
}

/// `parallel [L1, L2]`.
pub fn parallel(first: &str, second: &str) -> ConstraintKind {
    ConstraintKind::Parallel {
        lines: [id(first), id(second)],
    }
}

/// `perpendicular [L1, L2]`.
pub fn perpendicular(first: &str, second: &str) -> ConstraintKind {
    ConstraintKind::Perpendicular {
        lines: [id(first), id(second)],
    }
}

/// `tangent [K1, K2]`.
pub fn tangent(first: &str, second: &str) -> ConstraintKind {
    ConstraintKind::Tangent {
        curves: [id(first), id(second)],
    }
}

/// `collinear [L1, L2]`.
pub fn collinear(first: &str, second: &str) -> ConstraintKind {
    ConstraintKind::Collinear {
        lines: [id(first), id(second)],
    }
}

/// `concentric [K1, K2]`.
pub fn concentric(first: &str, second: &str) -> ConstraintKind {
    ConstraintKind::Concentric {
        curves: [id(first), id(second)],
    }
}

/// `equal [K1, K2]`.
pub fn equal(first: &str, second: &str) -> ConstraintKind {
    ConstraintKind::Equal {
        curves: [id(first), id(second)],
    }
}

/// `symmetric [P, Q] about L`.
pub fn symmetric(first: &str, second: &str, line: &str) -> ConstraintKind {
    ConstraintKind::Symmetric {
        points: [point_ref(first), point_ref(second)],
        line: id(line),
    }
}

/// `midpoint P of L`.
pub fn midpoint(point: &str, line: &str) -> ConstraintKind {
    ConstraintKind::Midpoint {
        point: point_ref(point),
        line: id(line),
    }
}

/// `fix X`.
pub fn fix(target: &str) -> ConstraintKind {
    ConstraintKind::Fix {
        target: point_ref(target),
    }
}

/// `distance [P, Q] = expr`.
pub fn distance_points(first: &str, second: &str, expr: &str) -> ConstraintKind {
    ConstraintKind::Distance {
        target: DistanceTarget::Points {
            points: [point_ref(first), point_ref(second)],
        },
        dimension: dimension(expr),
    }
}

/// `distance L = expr` (line length).
pub fn distance_line(line: &str, expr: &str) -> ConstraintKind {
    ConstraintKind::Distance {
        target: DistanceTarget::Line { line: id(line) },
        dimension: dimension(expr),
    }
}

/// `distance P to L = expr`.
pub fn distance_point_line(point: &str, line: &str, expr: &str) -> ConstraintKind {
    ConstraintKind::Distance {
        target: DistanceTarget::PointLine {
            point: point_ref(point),
            line: id(line),
        },
        dimension: dimension(expr),
    }
}

/// `horizontal_distance [P, Q] = expr`.
pub fn horizontal_distance_points(first: &str, second: &str, expr: &str) -> ConstraintKind {
    ConstraintKind::HorizontalDistance {
        target: Alignment::Points {
            points: [point_ref(first), point_ref(second)],
        },
        dimension: dimension(expr),
    }
}

/// `horizontal_distance L = expr`.
pub fn horizontal_distance_line(line: &str, expr: &str) -> ConstraintKind {
    ConstraintKind::HorizontalDistance {
        target: Alignment::Line { line: id(line) },
        dimension: dimension(expr),
    }
}

/// `vertical_distance [P, Q] = expr`.
pub fn vertical_distance_points(first: &str, second: &str, expr: &str) -> ConstraintKind {
    ConstraintKind::VerticalDistance {
        target: Alignment::Points {
            points: [point_ref(first), point_ref(second)],
        },
        dimension: dimension(expr),
    }
}

/// `vertical_distance L = expr`.
pub fn vertical_distance_line(line: &str, expr: &str) -> ConstraintKind {
    ConstraintKind::VerticalDistance {
        target: Alignment::Line { line: id(line) },
        dimension: dimension(expr),
    }
}

/// `radius K = expr`.
pub fn radius(curve: &str, expr: &str) -> ConstraintKind {
    ConstraintKind::Radius {
        curve: id(curve),
        dimension: dimension(expr),
    }
}

/// `diameter K = expr`.
pub fn diameter(curve: &str, expr: &str) -> ConstraintKind {
    ConstraintKind::Diameter {
        curve: id(curve),
        dimension: dimension(expr),
    }
}

/// `angle [L1, L2] = expr`.
pub fn angle(first: &str, second: &str, expr: &str) -> ConstraintKind {
    ConstraintKind::Angle {
        lines: [id(first), id(second)],
        dimension: dimension(expr),
    }
}

/// The dimension part of a dimensional constraint kind, for changing it in place.
pub fn dimension_mut(kind: &mut ConstraintKind) -> Option<&mut DimensionExpr> {
    match kind {
        ConstraintKind::Distance { dimension, .. }
        | ConstraintKind::HorizontalDistance { dimension, .. }
        | ConstraintKind::VerticalDistance { dimension, .. }
        | ConstraintKind::Radius { dimension, .. }
        | ConstraintKind::Diameter { dimension, .. }
        | ConstraintKind::Angle { dimension, .. } => Some(dimension),
        _ => None,
    }
}

/// The trailing `ref` of the shorthand: a reference dimension (`driving: false`) with expression "7 mm".
pub fn reference(mut kind: ConstraintKind) -> ConstraintKind {
    let dimension = dimension_mut(&mut kind).expect("only dimensions can be reference dimensions");
    dimension.expr = Expr::new(REFERENCE_EXPR);
    dimension.driving = false;
    kind
}

// ---------------------------------------------------------------------------
// Sketch builder
// ---------------------------------------------------------------------------

/// A sketch plus the dimension values the shorthand implies.
#[derive(Clone, Debug)]
pub struct Sketch {
    /// The sketch params.
    pub params: SketchParams,
    /// Values passed for driving dimensions.
    pub values: DimensionValues,
}

impl Sketch {
    /// A sketch on the XY origin plane with these entities and no constraints.
    pub fn new(entities: Vec<SketchEntity>) -> Sketch {
        Sketch {
            params: SketchParams {
                plane: SketchPlane::Origin {
                    plane: OriginPlane::Xy,
                },
                entities,
                constraints: Vec::new(),
            },
            values: DimensionValues::new(),
        }
    }

    /// Appends an entity.
    pub fn entity(mut self, entity: SketchEntity) -> Sketch {
        self.params.entities.push(entity);
        self
    }

    /// Appends a constraint without a value.
    pub fn with(mut self, constraint_id: &str, kind: ConstraintKind) -> Sketch {
        self.params
            .constraints
            .push(constraint(constraint_id, kind));
        self
    }

    /// Appends a driving dimension together with its value.
    pub fn with_value(self, constraint_id: &str, kind: ConstraintKind, value: f64) -> Sketch {
        self.with(constraint_id, kind).value(constraint_id, value)
    }

    /// Sets (or replaces) the value passed for a constraint id.
    pub fn value(mut self, constraint_id: &str, value: f64) -> Sketch {
        self.values.insert(id(constraint_id), value);
        self
    }

    /// Replaces the entity with the same id.
    pub fn replace_entity(mut self, entity: SketchEntity) -> Sketch {
        let slot = self
            .params
            .entities
            .iter_mut()
            .find(|existing| existing.id == entity.id)
            .expect("the replaced entity exists");
        *slot = entity;
        self
    }

    /// `SketchParams::validate`.
    pub fn validate(&self) -> Result<(), SketchError> {
        self.params.validate()
    }

    /// `NativeSolver.solve`, returning the error.
    pub fn try_solve(&self) -> Result<SolveResult, SketchError> {
        NativeSolver.solve(&self.params, &self.values)
    }

    /// `NativeSolver.solve`, panicking on an error.
    pub fn solve(&self) -> SolveResult {
        self.try_solve()
            .unwrap_or_else(|error| panic!("the sketch solves without an error: {error}"))
    }

    /// `NativeSolver.begin_drag`, returning the error.
    pub fn try_begin_drag(&self, target: &DragTarget) -> Result<NativeDragSession, SketchError> {
        NativeSolver.begin_drag(&self.params, &self.values, target)
    }

    /// `NativeSolver.begin_drag`, panicking on an error.
    pub fn begin_drag(&self, target: &DragTarget) -> NativeDragSession {
        self.try_begin_drag(target)
            .unwrap_or_else(|error| panic!("the drag begins without an error: {error}"))
    }

    /// `build_profile` of the stored entities.
    pub fn profile(&self) -> Profile {
        build_profile(&self.params.entities)
    }
}

/// A drag target on a point reference.
pub fn drag_point(point: &str) -> DragTarget {
    DragTarget::Point {
        point: point_ref(point),
    }
}

/// A drag target on a whole entity grabbed at `grab`.
pub fn drag_entity(entity: &str, grab: [f64; 2]) -> DragTarget {
    DragTarget::Entity {
        entity: id(entity),
        grab,
    }
}

/// One drag frame, panicking on an error.
pub fn frame(session: &mut NativeDragSession, cursor: [f64; 2]) -> SolveResult {
    session
        .update(cursor)
        .unwrap_or_else(|error| panic!("the drag frame at {cursor:?} succeeds: {error}"))
}

/// The result of the full solve performed when a drag began.
pub fn initial(session: &NativeDragSession) -> &SolveResult {
    session.initial()
}

// ---------------------------------------------------------------------------
// Named fixtures
// ---------------------------------------------------------------------------

/// The four lines of `RECT`.
pub fn rect_entities() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ]
}

/// `RECT` of spec section 6.0: four lines, four coincidences, two horizontals, two verticals.
pub fn rect() -> Sketch {
    Sketch::new(rect_entities())
        .with("c-1", coincident("e-1.end", "e-2.start"))
        .with("c-2", coincident("e-2.end", "e-3.start"))
        .with("c-3", coincident("e-3.end", "e-4.start"))
        .with("c-4", coincident("e-4.end", "e-1.start"))
        .with("c-5", horizontal("e-1"))
        .with("c-6", horizontal("e-3"))
        .with("c-7", vertical("e-2"))
        .with("c-8", vertical("e-4"))
}

/// The entities of `PLATE`.
pub fn plate_entities() -> Vec<SketchEntity> {
    let mut entities = rect_entities();
    entities.push(circle("e-5", [40.0, 25.0], 5.0));
    entities.push(construction_line("e-6", [40.0, 0.0], [40.0, 50.0]));
    entities
}

/// `PLATE` of spec section 2.9 with values `{c-10: 80, c-11: 50, c-15: 25, c-16: 10}`.
pub fn plate() -> Sketch {
    let mut sketch = rect();
    sketch.params.entities = plate_entities();
    sketch
        .with("c-9", fix("e-1.start"))
        .with_value("c-10", distance_line("e-1", "width"), 80.0)
        .with_value("c-11", distance_line("e-2", "50 mm"), 50.0)
        .with("c-12", midpoint("e-6.start", "e-1"))
        .with("c-13", midpoint("e-6.end", "e-3"))
        .with("c-14", point_on_object("e-5.center", "e-6"))
        .with_value(
            "c-15",
            vertical_distance_points("e-1.start", "e-5.center", "25 mm"),
            25.0,
        )
        .with_value("c-16", diameter("e-5", "hole_d"), 10.0)
}

/// Test 157: two overlapping circles.
pub fn overlapping_circles() -> Vec<SketchEntity> {
    vec![
        circle("e-1", [0.0, 0.0], 10.0),
        circle("e-2", [10.0, 0.0], 10.0),
    ]
}

/// Test 161: two rectangles that cross each other.
pub fn crossing_rectangles() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [20.0, 0.0]),
        line("e-2", [20.0, 0.0], [20.0, 10.0]),
        line("e-3", [20.0, 10.0], [0.0, 10.0]),
        line("e-4", [0.0, 10.0], [0.0, 0.0]),
        line("e-5", [10.0, 5.0], [30.0, 5.0]),
        line("e-6", [30.0, 5.0], [30.0, 15.0]),
        line("e-7", [30.0, 15.0], [10.0, 15.0]),
        line("e-8", [10.0, 15.0], [10.0, 5.0]),
    ]
}

/// Test 163: `RECT` plus two concentric circles inside it.
pub fn nested_circles() -> Vec<SketchEntity> {
    let mut entities = rect_entities();
    entities.push(circle("e-5", [40.0, 25.0], 20.0));
    entities.push(circle("e-6", [40.0, 25.0], 10.0));
    entities
}

/// Test 166: a slot of two lines and two arcs.
pub fn slot() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, -5.0], [20.0, -5.0]),
        arc("e-2", [20.0, 0.0], 5.0, -FRAC_PI_2, FRAC_PI_2),
        line("e-3", [20.0, 5.0], [0.0, 5.0]),
        arc("e-4", [0.0, 0.0], 5.0, FRAC_PI_2, 3.0 * FRAC_PI_2),
    ]
}

// ---------------------------------------------------------------------------
// Reading results
// ---------------------------------------------------------------------------

/// The entity with this id, panicking when it is missing.
pub fn find<'a>(entities: &'a [SketchEntity], entity: &str) -> &'a SketchEntity {
    entities
        .iter()
        .find(|candidate| candidate.id.as_str() == entity)
        .unwrap_or_else(|| panic!("entity {entity} is in the result"))
}

/// Position of a point entity.
pub fn position(entity: &SketchEntity) -> [f64; 2] {
    match &entity.geometry {
        Geometry::Point { position } => *position,
        other => panic!("{} is a point, found {other:?}", entity.id),
    }
}

/// Start and end of a line entity.
pub fn line_ends(entity: &SketchEntity) -> ([f64; 2], [f64; 2]) {
    match &entity.geometry {
        Geometry::Line { start, end } => (*start, *end),
        other => panic!("{} is a line, found {other:?}", entity.id),
    }
}

/// Center and radius of a circle entity.
pub fn circle_parts(entity: &SketchEntity) -> ([f64; 2], f64) {
    match &entity.geometry {
        Geometry::Circle { center, radius } => (*center, *radius),
        other => panic!("{} is a circle, found {other:?}", entity.id),
    }
}

/// Fields of an arc entity.
#[derive(Clone, Copy, Debug)]
pub struct ArcParts {
    /// Center.
    pub center: [f64; 2],
    /// Radius.
    pub radius: f64,
    /// Start angle in radians.
    pub start_angle: f64,
    /// End angle in radians.
    pub end_angle: f64,
}

/// Center, radius and angles of an arc entity.
pub fn arc_parts(entity: &SketchEntity) -> ArcParts {
    match &entity.geometry {
        Geometry::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => ArcParts {
            center: *center,
            radius: *radius,
            start_angle: *start_angle,
            end_angle: *end_angle,
        },
        other => panic!("{} is an arc, found {other:?}", entity.id),
    }
}

/// The point at `angle` on the circle (center, radius).
pub fn point_at_angle(center: [f64; 2], radius: f64, angle: f64) -> [f64; 2] {
    [
        center[0] + radius * angle.cos(),
        center[1] + radius * angle.sin(),
    ]
}

/// The measured value of a dimension, panicking when it is missing.
pub fn measured(result: &SolveResult, constraint_id: &str) -> f64 {
    *result
        .measured
        .get(&id(constraint_id))
        .unwrap_or_else(|| panic!("{constraint_id} has a measured value"))
}

// ---------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------

/// Asserts `abs(actual - expected) <= tolerance`.
#[track_caller]
pub fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: expected {expected} within {tolerance}, got {actual} (off by {})",
        (actual - expected).abs()
    );
}

/// Asserts closeness within `relative * max(1, abs(expected))`.
#[track_caller]
pub fn assert_relative(actual: f64, expected: f64, relative: f64, what: &str) {
    let tolerance = relative * expected.abs().max(1.0);
    assert_close(actual, expected, tolerance, what);
}

/// Asserts both coordinates are within `tolerance`.
#[track_caller]
pub fn assert_point_close(actual: [f64; 2], expected: [f64; 2], tolerance: f64, what: &str) {
    assert!(
        (actual[0] - expected[0]).abs() <= tolerance
            && (actual[1] - expected[1]).abs() <= tolerance,
        "{what}: expected {expected:?} within {tolerance}, got {actual:?}"
    );
}

/// Asserts both coordinates are bitwise equal.
#[track_caller]
pub fn assert_point_bitwise(actual: [f64; 2], expected: [f64; 2], what: &str) {
    assert!(
        actual[0].to_bits() == expected[0].to_bits()
            && actual[1].to_bits() == expected[1].to_bits(),
        "{what}: expected bitwise {expected:?}, got {actual:?}"
    );
}

/// Asserts bitwise equality of two `f64`.
#[track_caller]
pub fn assert_bits(actual: f64, expected: f64, what: &str) {
    assert!(
        actual.to_bits() == expected.to_bits(),
        "{what}: expected bitwise {expected:?}, got {actual:?}"
    );
}

/// Asserts bitwise equality of two values through their `Debug` text, which writes every `f64` in its
/// shortest round-trip form and so tells apart any two different numbers (including 0.0 and -0.0).
#[track_caller]
pub fn assert_bitwise<T: Debug + ?Sized>(actual: &T, expected: &T, what: &str) {
    let actual_text = format!("{actual:?}");
    let expected_text = format!("{expected:?}");
    assert!(
        actual_text == expected_text,
        "{what}: not bitwise equal\n  actual: {actual_text}\nexpected: {expected_text}"
    );
}

/// Asserts that a list of ids is exactly the expected texts.
#[track_caller]
pub fn assert_ids(actual: &[Id], expected: &[&str], what: &str) {
    assert_eq!(texts(actual), expected, "{what}");
}
