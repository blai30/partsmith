//! Ghosts (engine.md section 3.23): where a lost element or region was in its last build, and
//! where a sketch entity was before the edit a cause names, as display geometry whose positions
//! travel as a buffer of the reply that carries them. Only display sessions capture and send
//! them, and a ghost never changes a status, message or action.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;
use std::sync::Arc;

use partsmith_features::{BodyState, Repair as FeatureRepair};
use partsmith_mesh::TessellationTolerance;
use partsmith_model::Id;
use partsmith_naming::{Reference, Resolution, resolve};
use partsmith_occt_sys::ElementDisplay;
use partsmith_protocol::{Buffer, FeatureStatus, Ghost, GhostFrame, GhostKind, Repair};
use partsmith_recompute::{FeatureReport, Report};
use partsmith_sketch::{Geometry, RegionRef};

use crate::causes::{Baseline, Causes, MessageKey};
use crate::elements::sub_shape_kind;
use crate::events::Output;
use crate::state::ran;
use crate::triangulate::arc_points;

/// The codes of the messages whose element repair is a lost reference (section 3.23): guessed and
/// widened references, whose elements still exist, are not among them.
const LOST_REFERENCE_CODES: [&str; 5] = [
    "reference_missing",
    "reference_ambiguous",
    "reference_not_found",
    "references_not_found",
    "plane_fallback",
];

/// The display geometry of a ghost: how its points are joined, their frame, and their
/// coordinates, 3 per point in frame `world` and 2 in frame `sketch`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GhostShape {
    /// How the points are joined.
    kind: GhostKind,
    /// The frame of the coordinates.
    frame: GhostFrame,
    /// The coordinates, in mm.
    positions: Vec<f32>,
}

impl GhostShape {
    /// The ghost as it travels: its positions added to `output` as an `f32` buffer.
    pub(crate) fn send(&self, output: &mut Output) -> Ghost {
        Ghost {
            positions: output.buffer(Buffer::F32(self.positions.clone())),
            kind: self.kind,
            frame: self.frame,
        }
    }

    /// The ghost of a sketch entity where `geometry` puts it, in the sketch's coordinates: the
    /// points of a point entity, else its curve as lines.
    pub(crate) fn of_entity(geometry: &Geometry) -> GhostShape {
        match geometry {
            Geometry::Point { position } => GhostShape {
                kind: GhostKind::Points,
                frame: GhostFrame::Sketch,
                positions: position.map(|value| value as f32).to_vec(),
            },
            curve => GhostShape {
                kind: GhostKind::Lines,
                frame: GhostFrame::Sketch,
                positions: segments(curve),
            },
        }
    }
}

/// The ghosts of a document's lost references and regions, by feature and pointer, kept while
/// the loss lasts, and what each feature's last run leaves to capture them from.
#[derive(Debug, Default)]
pub(crate) struct Ghosts {
    /// Each ghost by the feature and the pointer of its repair.
    kept: BTreeMap<(Id, String), GhostShape>,
    /// For each feature, the last recompute in which it ran.
    last: BTreeMap<Id, LastRun>,
}

/// What a feature's last run leaves for the ghost of a reference or region it loses later.
#[derive(Debug)]
struct LastRun {
    /// The baseline of that recompute, with the solutions of its sketches.
    baseline: Arc<Baseline>,
    /// The states of the bodies the feature declared, just before it.
    bodies_before: Vec<(Id, BodyState)>,
    /// The pointers of the feature's repairs then.
    repaired: BTreeSet<String>,
}

impl LastRun {
    /// The last run of the feature `report` reports, in the recompute `baseline` describes.
    fn of(report: &FeatureReport, baseline: &Arc<Baseline>) -> LastRun {
        LastRun {
            baseline: Arc::clone(baseline),
            bodies_before: report.bodies_before.clone(),
            repaired: report
                .messages
                .iter()
                .flat_map(|message| &message.repairs)
                .map(|repair| pointer(repair).to_string())
                .collect(),
        }
    }

    /// The ghost of a reference or region lost since, captured from this run when the feature
    /// had no repair at its pointer then.
    fn capture(
        &self,
        repair: &FeatureRepair,
        tolerance: &TessellationTolerance,
    ) -> Option<GhostShape> {
        if self.repaired.contains(pointer(repair)) {
            return None;
        }
        match repair {
            FeatureRepair::Element { body, loss, .. } => {
                element_ghost(&self.bodies_before, body, &loss.reference, tolerance)
            }
            FeatureRepair::Region { sketch, region, .. } => {
                region_ghost(&self.baseline, sketch, region)
            }
        }
    }
}

impl Ghosts {
    /// The ghost of the repair of `feature` at `pointer`.
    fn get(&self, feature: &Id, pointer: &str) -> Option<&GhostShape> {
        self.kept.get(&(feature.clone(), pointer.to_string()))
    }

    /// Follows one recompute, `current`, whose baseline is `now`: captures the ghost of each
    /// reference or region of a feature that ran in `current` and is lost there, when that
    /// feature had no repair at its pointer the last time it ran, from what that run built, with
    /// the display geometry at `tolerance`; drops the ghosts of repairs a feature that ran no
    /// longer has and of features that are gone, and keeps this recompute as the last run of
    /// each feature that ran. A feature that did not run keeps its ghosts and its last run.
    pub(crate) fn observe(
        &mut self,
        now: &Arc<Baseline>,
        current: &Report,
        tolerance: &TessellationTolerance,
    ) {
        let mut kept = BTreeMap::new();
        let mut last = BTreeMap::new();
        for feature in &current.features {
            let id = &feature.feature;
            if !ran(feature.status) {
                let own = self
                    .kept
                    .range((id.clone(), String::new())..)
                    .take_while(|((owner, _), _)| owner == id);
                kept.extend(own.map(|(key, ghost)| (key.clone(), ghost.clone())));
                if let Some(run) = self.last.remove(id) {
                    last.insert(id.clone(), run);
                }
                continue;
            }
            for repair in lost_repairs(feature) {
                let key = (id.clone(), pointer(repair).to_string());
                let ghost = self.kept.remove(&key).or_else(|| {
                    self.last
                        .get(id)
                        .and_then(|run| run.capture(repair, tolerance))
                });
                if let Some(ghost) = ghost {
                    kept.insert(key, ghost);
                }
            }
            last.insert(id.clone(), LastRun::of(feature, now));
        }
        self.kept = kept;
        self.last = last;
    }

    /// Gives each repair of `statuses` its kept ghost and each cause the ghost `causes` made for
    /// it, sending their buffers to `output`, repairs before causes in each status (display
    /// sessions).
    pub(crate) fn add_to(
        &self,
        statuses: &mut [FeatureStatus],
        causes: &Causes,
        output: &mut Output,
    ) {
        for status in statuses {
            for repair in &mut status.repairs {
                let (Repair::Element { path, ghost, .. } | Repair::Region { path, ghost, .. }) =
                    repair;
                *ghost = self.get(&status.id, path).map(|shape| shape.send(output));
            }
            for diagnostic in &mut status.messages {
                let Some(cause) = &mut diagnostic.cause else {
                    continue;
                };
                let key = MessageKey::of(&status.id, &diagnostic.code, diagnostic.field.as_deref());
                cause.ghost = causes.ghost(&key).map(|shape| shape.send(output));
            }
        }
    }
}

/// The repairs of a feature's messages that are losses: an element repair of a message about a
/// lost reference, or a region repair of a region that matched nothing.
fn lost_repairs(report: &FeatureReport) -> impl Iterator<Item = &FeatureRepair> {
    report.messages.iter().flat_map(|message| {
        message.repairs.iter().filter(|repair| match repair {
            FeatureRepair::Element { .. } => LOST_REFERENCE_CODES.contains(&message.code),
            FeatureRepair::Region { loss, .. } => loss.is_some(),
        })
    })
}

/// The pointer of a repair.
fn pointer(repair: &FeatureRepair) -> &str {
    match repair {
        FeatureRepair::Element { pointer, .. } | FeatureRepair::Region { pointer, .. } => pointer,
    }
}

/// The ghost of `reference`, stored by a feature whose states of the bodies it declared were
/// `bodies_before` the last time it ran: the display geometry of each element it resolved to in
/// the state of `body`, in the resolver's order, in world coordinates.
fn element_ghost(
    bodies_before: &[(Id, BodyState)],
    body: &Id,
    reference: &Reference,
    tolerance: &TessellationTolerance,
) -> Option<GhostShape> {
    let (_, state) = bodies_before.iter().find(|(id, _)| id == body)?;
    let shape = state.shape()?;
    let set = state.element_set().ok()?;
    let Resolution::Resolved { elements, .. } = resolve(reference, &set) else {
        return None;
    };
    let mut kind = None;
    let mut positions = Vec::new();
    for name in &elements {
        let index = state.naming().index(name)?;
        let display = shape
            .element_display(sub_shape_kind(name.kind()), index, tolerance)
            .ok()?;
        let (shown, points) = match display {
            ElementDisplay::Polyline(points) => (GhostKind::Lines, segment_ends(&points)),
            ElementDisplay::Triangles(points) => (GhostKind::Triangles, points),
            ElementDisplay::Position(point) => (GhostKind::Points, vec![point]),
        };
        if *kind.get_or_insert(shown) != shown {
            return None;
        }
        positions.extend(
            points
                .iter()
                .flat_map(|point| [point.x, point.y, point.z].map(|value| value as f32)),
        );
    }
    Some(GhostShape {
        kind: kind?,
        frame: GhostFrame::World,
        positions,
    })
}

/// Each pair of consecutive points of a polyline, as the two ends of one segment.
fn segment_ends<T: Copy>(points: &[T]) -> Vec<T> {
    points
        .windows(2)
        .flat_map(|pair| [pair[0], pair[1]])
        .collect()
}

/// The ghost of a lost region: the entities of its stored outline (its `outline`, else the outer
/// loop of its key), each once, as the sketch's solution in the recompute `before` describes had
/// them, as lines in the sketch's coordinates.
fn region_ghost(before: &Baseline, sketch: &Id, region: &RegionRef) -> Option<GhostShape> {
    let result = before.sketch(sketch)?;
    let entries = region
        .outline
        .as_ref()
        .map_or_else(|| region.key.outer(), |outline| outline.entries());
    let mut drawn: Vec<&Id> = Vec::new();
    let mut positions = Vec::new();
    for entry in entries {
        if drawn.contains(&&entry.entity) {
            continue;
        }
        drawn.push(&entry.entity);
        let entity = result
            .solved
            .entities
            .iter()
            .find(|entity| entity.id == entry.entity)?;
        positions.extend(segments(&entity.geometry));
    }
    Some(GhostShape {
        kind: GhostKind::Lines,
        frame: GhostFrame::Sketch,
        positions,
    })
}

/// A sketch curve as line segments in sketch coordinates, two points each: a line as one
/// segment, a circle and an arc split as region fills split them (section 3.6); nothing for a
/// point.
fn segments(geometry: &Geometry) -> Vec<f32> {
    let points: Vec<[f64; 2]> = match *geometry {
        Geometry::Point { .. } => Vec::new(),
        Geometry::Line { start, end } => vec![start, end],
        Geometry::Circle { center, radius } => arc_points(center, radius, 0.0, TAU),
        Geometry::Arc {
            center,
            radius,
            start_angle,
            ..
        } => {
            let sweep = geometry.arc_sweep().unwrap_or(TAU);
            arc_points(center, radius, start_angle, sweep)
        }
    };
    segment_ends(&points)
        .into_iter()
        .flat_map(|point| point.map(|value| value as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use partsmith_occt_sys::Point;

    use super::*;

    /// The points of a sketch ghost, two coordinates each.
    fn points(ghost: &GhostShape) -> Vec<[f32; 2]> {
        ghost
            .positions
            .chunks(2)
            .map(|point| [point[0], point[1]])
            .collect()
    }

    #[test]
    fn a_line_is_one_segment_from_its_start_to_its_end() {
        let line = Geometry::Line {
            start: [80.0, 40.0],
            end: [70.0, 50.0],
        };

        let ghost = GhostShape::of_entity(&line);

        assert_eq!(
            (ghost.kind, ghost.frame),
            (GhostKind::Lines, GhostFrame::Sketch)
        );
        assert_eq!(points(&ghost), [[80.0, 40.0], [70.0, 50.0]]);
    }

    #[test]
    fn a_circle_is_split_into_the_segments_of_a_region_fill() {
        let circle = Geometry::Circle {
            center: [40.0, 25.0],
            radius: 10.0,
        };

        let ghost = GhostShape::of_entity(&circle);

        // 72 segments of two points each, closing where they started.
        let found = points(&ghost);
        assert_eq!(found.len(), 144);
        assert_eq!(found[0], [50.0, 25.0]);
        assert!((found[143][0] - 50.0).abs() < 1e-4 && (found[143][1] - 25.0).abs() < 1e-4);
    }

    #[test]
    fn a_quarter_arc_is_eighteen_segments_from_its_start() {
        let arc = Geometry::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        };

        let found = points(&GhostShape::of_entity(&arc));

        assert_eq!(found.len(), 36);
        assert_eq!(found[0], [1.0, 0.0]);
        assert!(found[35][0].abs() < 1e-6 && (found[35][1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_point_entity_is_one_point() {
        let ghost = GhostShape::of_entity(&Geometry::Point {
            position: [3.0, 4.0],
        });

        assert_eq!(
            (ghost.kind, ghost.positions),
            (GhostKind::Points, vec![3.0, 4.0])
        );
    }

    #[test]
    fn a_polyline_gives_one_segment_per_pair_of_consecutive_points() {
        let at = |x: f64| Point { x, y: 0.0, z: 0.0 };

        let found = segment_ends(&[at(0.0), at(1.0), at(2.0)]);

        assert_eq!(found, [at(0.0), at(1.0), at(1.0), at(2.0)]);
    }
}
