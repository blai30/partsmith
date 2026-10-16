//! Sketch solves, drags and their projection into protocol sketch solutions (engine.md
//! section 3.6).

use std::collections::{BTreeMap, BTreeSet};

use partsmith_features::{Registry, SketchResult};
use partsmith_model::{Dimension, Document, FeatureRecord, Id, Scope};
use partsmith_protocol::{
    ConstraintSolveState, ConstraintState, RegionInfo, SketchSolution, SolvedEntity, SolvedPoint,
};
use partsmith_sketch::{
    DimensionValues, DragSession, DragTarget, EntityState, Geometry, NativeDragSession,
    NativeSolver, Placement, PointRef, PointRole, Profile, Region, SketchEntity, SketchParams,
    SolveResult, Solver, ValueKind, build_profile,
};

use crate::error::EngineError;
use crate::status::render_feature_error;

/// The type name of sketch features.
pub(crate) const SKETCH_TYPE: &str = "sketch";

/// The type name of datum plane features.
pub(crate) const DATUM_PLANE_TYPE: &str = "datum_plane";

/// A sketch solution and the regions its display fills come from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Solved {
    /// The solution, without display buffers.
    pub(crate) solution: SketchSolution,
    /// The closed regions, in solution order.
    pub(crate) regions: Vec<Region>,
}

/// The params of a sketch record, validated and canonicalized by the registry.
pub(crate) fn sketch_params(
    registry: &Registry,
    document: &Document,
    record: &FeatureRecord,
) -> Result<SketchParams, EngineError> {
    let validated = registry
        .validate_and_canonicalize(record)
        .map_err(|error| EngineError::InvalidSketch {
            message: render_feature_error(&error, document, registry, &record.id),
        })?;
    serde_json::from_value(serde_json::Value::Object(validated.record.params))
        .map_err(EngineError::unexpected)
}

/// The record of a sketch command's feature: `unknown_feature` when missing, `not_a_sketch` when
/// it is another type.
pub(crate) fn sketch_record<'d>(
    document: &'d Document,
    feature: &Id,
) -> Result<(usize, &'d FeatureRecord), EngineError> {
    let (position, record) = document
        .features
        .iter()
        .enumerate()
        .find(|(_, record)| record.id == *feature)
        .ok_or_else(|| EngineError::UnknownFeature {
            id: feature.clone(),
        })?;
    if record.kind != SKETCH_TYPE {
        return Err(EngineError::NotASketch {
            id: feature.clone(),
            feature_type: record.kind.clone(),
        });
    }
    Ok((position, record))
}

/// The values of the driving dimensions, evaluated in `scope` (`expression_error` otherwise).
pub(crate) fn dimension_values(
    params: &SketchParams,
    scope: &Scope<'_>,
) -> Result<DimensionValues, EngineError> {
    let mut values = DimensionValues::new();
    for input in params.driving_dimensions() {
        let expected = match input.kind {
            ValueKind::Length => Dimension::LENGTH,
            ValueKind::Angle => Dimension::ANGLE,
        };
        let quantity = input
            .expr
            .evaluate(scope, Some(expected))
            .map_err(|error| EngineError::Expression {
                message: error.to_string(),
                span: None,
            })?;
        values.insert(input.constraint.clone(), quantity.value);
    }
    Ok(values)
}

/// Solves `params` with `values` (`invalid_params` with the sketch crate's message when the
/// solver rejects them) and projects the result.
pub(crate) fn solve(
    feature: &Id,
    params: &SketchParams,
    values: &DimensionValues,
    placement: Placement,
) -> Result<Solved, EngineError> {
    let result = NativeSolver
        .solve(params, values)
        .map_err(EngineError::invalid_sketch)?;
    let profile = build_profile(&result.entities);
    Ok(project(
        feature, params, &result, profile, placement, values,
    ))
}

/// The solution of a sketch that built, from its recompute output.
pub(crate) fn built(feature: &Id, params: &SketchParams, result: &SketchResult) -> Solved {
    project(
        feature,
        params,
        &result.solved,
        result.profile.clone(),
        result.placement,
        &DimensionValues::new(),
    )
}

/// Projects a solve into a `SketchSolution` (section 3.6). `values` are the driving dimension
/// values of the solve, used to find the constraints of failed components.
fn project(
    feature: &Id,
    params: &SketchParams,
    result: &SolveResult,
    profile: Profile,
    placement: Placement,
    values: &DimensionValues,
) -> Solved {
    let entities = result
        .entities
        .iter()
        .map(|entity| SolvedEntity {
            entity: entity.clone(),
            constrained: result
                .entity_states
                .get(&entity.id)
                .copied()
                .unwrap_or(EntityState::UnderConstrained),
        })
        .collect();
    let points = result
        .entities
        .iter()
        .flat_map(|entity| {
            roles(&entity.geometry).iter().filter_map(|&role| {
                Some(SolvedPoint {
                    at: entity.point(role)?,
                    point: PointRef {
                        entity: entity.id.clone(),
                        role,
                    },
                })
            })
        })
        .collect();
    let unsatisfied = if result.failed {
        failed_constraints(params, result, values)
    } else {
        BTreeSet::new()
    };
    let constraints = params
        .constraints
        .iter()
        .map(|constraint| ConstraintState {
            id: constraint.id.clone(),
            state: if result.conflicting.contains(&constraint.id) {
                ConstraintSolveState::Conflicting
            } else if result.redundant.contains(&constraint.id) {
                ConstraintSolveState::Redundant
            } else if unsatisfied.contains(&constraint.id) {
                ConstraintSolveState::Unsatisfied
            } else {
                ConstraintSolveState::Satisfied
            },
            value: constraint
                .dimension()
                .and_then(|_| result.measured.get(&constraint.id).copied()),
        })
        .collect();
    let regions = profile
        .regions
        .iter()
        .map(|region| RegionInfo {
            reference: region.reference(),
            area: region.area,
            centroid: region.centroid,
        })
        .collect();
    Solved {
        solution: SketchSolution {
            feature: feature.clone(),
            status: result.status,
            failed: result.failed,
            dof: result.dof,
            redundant: result.redundant.clone(),
            conflicting: result.conflicting.clone(),
            placement,
            entities,
            points,
            constraints,
            regions,
            profile_findings: profile.findings,
            // The sketch crate renders these from phase 3 of the reference stability plan (engine.md section 3.6).
            messages: Vec::new(),
            display: None,
        },
        regions: profile.regions,
    }
}

/// The point roles of an entity in `PointRole` declaration order.
fn roles(geometry: &Geometry) -> &'static [PointRole] {
    match geometry {
        Geometry::Point { .. } => &[PointRole::Point],
        Geometry::Line { .. } => &[PointRole::Start, PointRole::End],
        Geometry::Circle { .. } => &[PointRole::Center],
        Geometry::Arc { .. } => &[PointRole::Start, PointRole::End, PointRole::Center],
    }
}

/// The constraints of components that do not converge (sketch.md section 3.7.1): each
/// component of the result geometry is solved on its own, and the constraints of the ones that
/// fail are returned.
fn failed_constraints(
    params: &SketchParams,
    result: &SolveResult,
    values: &DimensionValues,
) -> BTreeSet<Id> {
    let positions: BTreeMap<&Id, usize> = result
        .entities
        .iter()
        .enumerate()
        .map(|(position, entity)| (&entity.id, position))
        .collect();
    let mut roots: Vec<usize> = (0..result.entities.len()).collect();
    let mut component_of = Vec::with_capacity(params.constraints.len());
    for constraint in &params.constraints {
        let members: Vec<usize> = constraint
            .referenced_entities()
            .into_iter()
            .filter_map(|entity| positions.get(entity).copied())
            .collect();
        for pair in members.windows(2) {
            let (first, second) = (root(&mut roots, pair[0]), root(&mut roots, pair[1]));
            roots[first.max(second)] = first.min(second);
        }
        component_of.push(members.first().copied());
    }
    let mut components: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (constraint, member) in component_of.iter().enumerate() {
        if let Some(member) = member {
            components
                .entry(root(&mut roots, *member))
                .or_default()
                .push(constraint);
        }
    }
    let mut failed = BTreeSet::new();
    for (component, constraints) in components {
        let entities: Vec<SketchEntity> = (0..result.entities.len())
            .filter(|&entity| root(&mut roots, entity) == component)
            .map(|entity| result.entities[entity].clone())
            .collect();
        let part = SketchParams {
            plane: params.plane.clone(),
            entities,
            constraints: constraints
                .iter()
                .map(|&constraint| params.constraints[constraint].clone())
                .collect(),
        };
        let converges = NativeSolver
            .solve(&part, values)
            .is_ok_and(|solved| !solved.failed);
        if !converges {
            failed.extend(part.constraints.into_iter().map(|constraint| constraint.id));
        }
    }
    failed
}

/// The representative of `item` in a union-find forest, halving paths on the way.
fn root(roots: &mut [usize], mut item: usize) -> usize {
    while roots[item] != item {
        roots[item] = roots[roots[item]];
        item = roots[item];
    }
    item
}

/// The drag state of one sketch: the solver's session and the last frame.
pub(crate) struct Drag {
    /// The dragged point.
    pub(crate) point: PointRef,
    /// The solver's drag session.
    session: NativeDragSession,
    /// The params the drag began with.
    pub(crate) params: SketchParams,
    /// The driving dimension values the drag began with.
    values: DimensionValues,
    /// The sketch placement.
    pub(crate) placement: Placement,
    /// The last frame's result.
    pub(crate) last: SolveResult,
}

impl std::fmt::Debug for Drag {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Drag")
            .field("point", &self.point)
            .field("placement", &self.placement)
            .finish_non_exhaustive()
    }
}

impl Drag {
    /// Begins a drag of `point` and solves its first frame at `to`.
    pub(crate) fn begin(
        params: SketchParams,
        values: DimensionValues,
        placement: Placement,
        point: PointRef,
        to: [f64; 2],
    ) -> Result<Drag, EngineError> {
        let target = DragTarget::Point {
            point: point.clone(),
        };
        let mut session = NativeSolver
            .begin_drag(&params, &values, &target)
            .map_err(EngineError::invalid_sketch)?;
        let last = frame(&mut session, to)?;
        Ok(Drag {
            point,
            session,
            params,
            values,
            placement,
            last,
        })
    }

    /// Solves the next frame at `to`, starting from the previous one.
    pub(crate) fn update(&mut self, to: [f64; 2]) -> Result<(), EngineError> {
        self.last = frame(&mut self.session, to)?;
        Ok(())
    }

    /// The solution of the last frame.
    pub(crate) fn solved(&self, feature: &Id) -> Solved {
        let profile = build_profile(&self.last.entities);
        project(
            feature,
            &self.params,
            &self.last,
            profile,
            self.placement,
            &self.values,
        )
    }
}

/// One drag frame (`invalid_params` when the solver rejects the cursor).
fn frame(session: &mut NativeDragSession, to: [f64; 2]) -> Result<SolveResult, EngineError> {
    session.update(to).map_err(EngineError::invalid_sketch)
}

/// True when `point` addresses a point of an entity of `params` (reserved references are not
/// points of the sketch).
pub(crate) fn has_point(params: &SketchParams, point: &PointRef) -> bool {
    params
        .entity(&point.entity)
        .is_some_and(|entity| entity.point(point.role).is_some())
}

#[cfg(test)]
mod tests {
    use partsmith_model::{EmptyContext, ParameterValues, Units};
    use partsmith_sketch::{
        Alignment, ConstraintKind, DimensionExpr, DistanceTarget, OriginPlane, SketchConstraint,
        SketchPlane, SketchStatus,
    };

    use super::*;
    use crate::test_support::id;

    fn line(name: &str, start: [f64; 2], end: [f64; 2]) -> SketchEntity {
        SketchEntity {
            id: id(name),
            geometry: Geometry::Line { start, end },
            construction: false,
        }
    }

    fn constraint(name: &str, kind: ConstraintKind) -> SketchConstraint {
        SketchConstraint { id: id(name), kind }
    }

    fn drag_sketch() -> SketchParams {
        SketchParams {
            plane: SketchPlane::Origin {
                plane: OriginPlane::Xy,
            },
            entities: vec![line("e-1", [0.0, 0.0], [10.0, 0.0])],
            constraints: vec![
                constraint(
                    "c-1",
                    ConstraintKind::Fix {
                        target: "e-1.start".parse().expect("a valid point"),
                    },
                ),
                constraint(
                    "c-2",
                    ConstraintKind::Horizontal(Alignment::Line { line: id("e-1") }),
                ),
            ],
        }
    }

    fn solve_drag_sketch() -> Solved {
        solve(
            &id("f-sk1"),
            &drag_sketch(),
            &DimensionValues::new(),
            OriginPlane::Xy.placement(),
        )
        .expect("the sketch solves")
    }

    #[test]
    fn points_follow_entities_then_roles() {
        let solved = solve_drag_sketch();

        let points: Vec<String> = solved
            .solution
            .points
            .iter()
            .map(|point| point.point.to_string())
            .collect();
        assert_eq!(points, ["e-1.start", "e-1.end"]);
        assert_eq!(solved.solution.dof, 1);
        assert_eq!(solved.solution.status, SketchStatus::UnderConstrained);
    }

    #[test]
    fn constraints_are_satisfied_after_a_good_solve() {
        let solved = solve_drag_sketch();

        assert!(
            solved
                .solution
                .constraints
                .iter()
                .all(|state| state.state == ConstraintSolveState::Satisfied)
        );
    }

    #[test]
    fn dimensions_report_their_measured_value() {
        let mut params = drag_sketch();
        params.constraints.push(constraint(
            "c-3",
            ConstraintKind::Distance {
                target: DistanceTarget::Line { line: id("e-1") },
                dimension: DimensionExpr {
                    expr: "10 mm".into(),
                    driving: true,
                },
            },
        ));
        let values = DimensionValues::from([(id("c-3"), 10.0)]);

        let solved = solve(&id("f-sk1"), &params, &values, OriginPlane::Xy.placement())
            .expect("the sketch solves");

        let distance = &solved.solution.constraints[2];
        assert_eq!(distance.id, id("c-3"));
        assert!((distance.value.expect("a measured value") - 10.0).abs() < 1e-9);
        assert_eq!(solved.solution.constraints[0].value, None);
    }

    #[test]
    fn conflicts_mark_their_constraints_and_the_rest_of_the_failed_component() {
        let mut params = drag_sketch();
        params.constraints.push(constraint(
            "c-3",
            ConstraintKind::Vertical(Alignment::Line { line: id("e-1") }),
        ));
        params.constraints.push(constraint(
            "c-4",
            ConstraintKind::Distance {
                target: DistanceTarget::Line { line: id("e-1") },
                dimension: DimensionExpr {
                    expr: "10 mm".into(),
                    driving: true,
                },
            },
        ));
        let values = DimensionValues::from([(id("c-4"), 10.0)]);

        let solved = solve(&id("f-sk1"), &params, &values, OriginPlane::Xy.placement())
            .expect("the sketch solves");

        assert_eq!(solved.solution.status, SketchStatus::Conflicting);
        assert!(solved.solution.failed);
        let states: Vec<ConstraintSolveState> = solved
            .solution
            .constraints
            .iter()
            .map(|state| state.state)
            .collect();
        assert!(states.contains(&ConstraintSolveState::Conflicting));
        assert!(!states.contains(&ConstraintSolveState::Satisfied));
    }

    #[test]
    fn closed_outlines_become_regions() {
        let params = SketchParams {
            plane: SketchPlane::Origin {
                plane: OriginPlane::Xy,
            },
            entities: vec![
                line("e-1", [0.0, 0.0], [80.0, 0.0]),
                line("e-2", [80.0, 0.0], [80.0, 50.0]),
                line("e-3", [80.0, 50.0], [0.0, 50.0]),
                line("e-4", [0.0, 50.0], [0.0, 0.0]),
            ],
            constraints: Vec::new(),
        };

        let solved = solve(
            &id("f-sk1"),
            &params,
            &DimensionValues::new(),
            OriginPlane::Xy.placement(),
        )
        .expect("the sketch solves");

        assert_eq!(solved.solution.regions.len(), 1);
        assert!((solved.solution.regions[0].area - 4000.0).abs() < 1e-9);
        assert_eq!(solved.regions.len(), 1);
    }

    #[test]
    fn drags_follow_the_cursor_within_the_constraints() {
        let mut drag = Drag::begin(
            drag_sketch(),
            DimensionValues::new(),
            OriginPlane::Xy.placement(),
            "e-1.end".parse().expect("a valid point"),
            [20.0, 5.0],
        )
        .expect("the drag begins");

        drag.update([30.0, -5.0]).expect("the frame solves");

        let end = drag.last.entities[0]
            .point(PointRole::End)
            .expect("a line has an end");
        assert!((end[0] - 30.0).abs() < 1e-9 && end[1].abs() < 1e-9);
    }

    #[test]
    fn reserved_and_missing_points_are_not_sketch_points() {
        let params = drag_sketch();

        assert!(has_point(
            &params,
            &"e-1.end".parse().expect("a valid point")
        ));
        assert!(!has_point(
            &params,
            &"e-1.center".parse().expect("a valid point")
        ));
        assert!(!has_point(
            &params,
            &"origin".parse().expect("a valid point")
        ));
        assert!(!has_point(
            &params,
            &"e-9.end".parse().expect("a valid point")
        ));
    }

    #[test]
    fn driving_dimensions_evaluate_in_document_units() {
        let mut params = drag_sketch();
        params.constraints.push(constraint(
            "c-3",
            ConstraintKind::Distance {
                target: DistanceTarget::Line { line: id("e-1") },
                dimension: DimensionExpr {
                    expr: "2".into(),
                    driving: true,
                },
            },
        ));
        let parameters = ParameterValues::default();
        let scope = Scope {
            units: Units::In,
            parameters: &parameters,
            context: &EmptyContext,
        };

        let values = dimension_values(&params, &scope).expect("the dimension evaluates");

        assert!((values[&id("c-3")] - 50.8).abs() < 1e-9);
    }

    #[test]
    fn broken_dimensions_are_expression_errors() {
        let mut params = drag_sketch();
        params.constraints.push(constraint(
            "c-3",
            ConstraintKind::Distance {
                target: DistanceTarget::Line { line: id("e-1") },
                dimension: DimensionExpr {
                    expr: "wdth".into(),
                    driving: true,
                },
            },
        ));
        let parameters = ParameterValues::default();
        let scope = Scope {
            units: Units::Mm,
            parameters: &parameters,
            context: &EmptyContext,
        };

        let error = dimension_values(&params, &scope).expect_err("wdth is unknown");

        assert!(matches!(error, EngineError::Expression { .. }));
        assert!(error.to_string().contains("wdth"));
    }

    #[test]
    fn union_find_joins_and_finds() {
        let mut roots = vec![0, 0, 1, 3];

        assert_eq!(root(&mut roots, 2), 0);
        assert_eq!(roots[2], 0);
        assert_eq!(root(&mut roots, 3), 3);
    }
}
