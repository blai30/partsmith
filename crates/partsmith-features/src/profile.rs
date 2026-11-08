//! The union of a profile's resolved regions as kernel faces (features.md section 3.7).
//!
//! The sketch crate builds the union of the regions in the sketch plane (`union_regions`, sketch.md section 3.19).
//! Its parts become kernel faces here, with each curve tagged by the sketch entity it came from, so a union sweeps the
//! faces of the single region covering the same area.

use partsmith_model::Id;
use partsmith_occt_sys::{
    PlanarCurve, PlanarLoop, PlanarProfile, PlanarRegion, Point2, TaggedCurve, make_profile,
};
use partsmith_sketch::{ProfileCurve, ProfileLoop, Region, union_regions};

use crate::context::ResolvedProfile;
use crate::error::FeatureError;
use crate::params::PlacementExt;

/// The union of a profile's resolved regions as kernel faces on the sketch plane (section 3.7).
#[derive(Clone, Debug)]
pub struct ProfileFaces {
    /// One planar face per part of the union. The tag of each curve indexes `entities`.
    pub profile: PlanarProfile,
    /// The sketch entity each curve tag came from: tag `i` names `entities[i]`. Without duplicates,
    /// in order of first use.
    pub entities: Vec<Id>,
}

impl ResolvedProfile<'_> {
    /// Builds the union of the resolved regions (sketch.md section 3.19) as kernel faces with kernel
    /// `make_profile`, each curve tagged by the sketch entity it came from. Boundary pieces two
    /// resolved regions share are dropped, so no lateral face is ever swept from them, and the
    /// pieces of one entity that then follow each other along a line or circle become one curve.
    pub fn faces(&self) -> Result<ProfileFaces, FeatureError> {
        let regions = self
            .regions
            .iter()
            .map(|&index| {
                self.sketch.profile.regions.get(index).ok_or_else(|| {
                    FeatureError::internal(format!("the profile has no region {index}"))
                })
            })
            .collect::<Result<Vec<&Region>, FeatureError>>()?;
        let mut entities = Vec::new();
        let union = union_regions(&regions)
            .map_err(|error| FeatureError::internal(error.to_string()))?
            .into_iter()
            .map(|part| PlanarRegion {
                outer: planar_loop(&part.outer, &mut entities),
                holes: part
                    .holes
                    .iter()
                    .map(|hole| planar_loop(hole, &mut entities))
                    .collect(),
            })
            .collect::<Vec<_>>();
        let profile = make_profile(&self.sketch.placement.to_kernel(), &union)?;
        Ok(ProfileFaces { profile, entities })
    }
}

/// The kernel loop of a profile loop; each curve is tagged with its entity's position in `entities`.
fn planar_loop(profile_loop: &ProfileLoop, entities: &mut Vec<Id>) -> PlanarLoop {
    PlanarLoop {
        curves: profile_loop
            .edges
            .iter()
            .map(|edge| TaggedCurve {
                tag: tag(&edge.entity, entities),
                curve: planar_curve(&edge.curve),
            })
            .collect(),
    }
}

/// The tag of an entity: its position in `entities`, added when new.
fn tag(entity: &Id, entities: &mut Vec<Id>) -> u32 {
    let position = entities
        .iter()
        .position(|known| known == entity)
        .unwrap_or_else(|| {
            entities.push(entity.clone());
            entities.len() - 1
        });
    u32::try_from(position).expect("a sketch has far fewer than 2^32 entities")
}

/// The kernel curve of a profile curve (sketch.md section 3.15): kernel arcs always run
/// counterclockwise, so a clockwise arc is passed with its ends swapped.
fn planar_curve(curve: &ProfileCurve) -> PlanarCurve {
    let point = |[x, y]: [f64; 2]| Point2 { x, y };
    match *curve {
        ProfileCurve::Line { start, end } => PlanarCurve::Line {
            start: point(start),
            end: point(end),
        },
        ProfileCurve::Arc {
            center,
            radius,
            start,
            end,
            ccw,
        } => {
            let (start, end) = if ccw { (start, end) } else { (end, start) };
            PlanarCurve::Arc {
                center: point(center),
                radius,
                start: point(start),
                end: point(end),
            }
        }
        ProfileCurve::Circle { center, radius, .. } => PlanarCurve::Circle {
            center: point(center),
            radius,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clockwise_arcs_swap_their_ends_for_the_kernel() {
        let curve = ProfileCurve::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start: [1.0, 0.0],
            end: [0.0, 1.0],
            ccw: false,
        };

        assert_eq!(
            planar_curve(&curve),
            PlanarCurve::Arc {
                center: Point2 { x: 0.0, y: 0.0 },
                radius: 1.0,
                start: Point2 { x: 0.0, y: 1.0 },
                end: Point2 { x: 1.0, y: 0.0 },
            }
        );
    }

    #[test]
    fn tags_number_entities_in_order_of_first_use() {
        let mut entities = Vec::new();
        let first = Id::new("e-7").expect("valid id");
        let second = Id::new("e-2").expect("valid id");

        assert_eq!(tag(&first, &mut entities), 0);
        assert_eq!(tag(&second, &mut entities), 1);
        assert_eq!(tag(&first, &mut entities), 0);
        assert_eq!(entities, [first, second]);
    }
}
