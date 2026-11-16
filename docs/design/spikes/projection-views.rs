//! Scratch check for the projection spike's round 3 (not committed): test 10's cylinder seen
//! across its axis from directions around Z, and obliquely, by each silhouette construction.
//! To run it at spike/kernel-projection 1fe1c9a: copy it to
//! crates/partsmith-occt-sys/tests/projection_views.rs, add `mod projection_views;` to
//! tests/integration.rs, and run
//! `cargo test -p partsmith-occt-sys --test integration projection_views -- --nocapture`.

use partsmith_occt_sys::{Interrupt, Plane, SubShapeKind, Vector};

use super::common;
use super::projection_inputs::{Construction, Input, SILHOUETTES};

#[test]
fn cylinder_seen_across_its_axis_from_each_direction() {
    let views: Vec<(String, Vector, Vector)> = [
        0.0, 10.0, 30.0, 45.0, 60.0, 80.0, 85.0, 87.0, 88.0, 89.0, 89.5, 89.9, 90.0, 90.1, 91.0, 92.0,
        95.0, 100.0, 135.0, 180.0, 225.0, 269.0, 270.0, 271.0, 315.0,
    ]
    .iter()
    .map(|degrees: &f64| {
        let angle = degrees.to_radians();
        (
            format!("{degrees} degrees, x axis +Z"),
            Vector::new(angle.cos(), angle.sin(), 0.0),
            Vector::new(0.0, 0.0, 1.0),
        )
    })
    .chain([(
        "the spike's view: normal +Y, x axis +X".to_owned(),
        Vector::new(0.0, 1.0, 0.0),
        Vector::new(1.0, 0.0, 0.0),
    )])
    .chain([0.0, 45.0, 90.0].iter().map(|degrees: &f64| {
        let angle = degrees.to_radians();
        (
            format!("oblique, normal at {degrees} degrees around Z and 45 degrees to the axis"),
            Vector::new(angle.cos(), angle.sin(), 1.0),
            Vector::new(angle.sin(), -angle.cos(), 0.0),
        )
    }))
    .chain([(
        "the spike's oblique view: normal (0, 1, 1), x axis +X".to_owned(),
        Vector::new(0.0, 1.0, 1.0),
        Vector::new(1.0, 0.0, 0.0),
    )])
    .collect();
    for (label, normal, x_axis) in views {
        let input = Input {
            name: "cylinder-perpendicular",
            shape: common::cylinder_primitive().shape,
            edges: Vec::new(),
            plane: Plane {
                origin: common::ORIGIN,
                normal,
                x_axis,
            },
            constructions: SILHOUETTES,
            outline: None,
            budget: None,
        };
        for construction in [
            Construction::SilhouetteExact,
            Construction::SilhouetteMesh,
            Construction::SilhouetteExactWithEdges,
            Construction::SilhouetteMeshWithEdges,
        ] {
            let curves = construction
                .run(&input, &Interrupt::new())
                .expect("the silhouette runs");
            let elements = curves
                .shape
                .elements(SubShapeKind::Edge)
                .expect("edges can be read");
            let kinds: Vec<_> = elements.iter().map(|edge| edge.kind).collect();
            let lines = kinds
                .iter()
                .filter(|kind| **kind == partsmith_occt_sys::GeometryKind::Line)
                .count();
            let summary = if elements.len() <= 4 {
                format!("{kinds:?}")
            } else {
                format!("{lines} lines")
            };
            let ends: Vec<_> = elements
                .iter()
                .take(4)
                .map(|edge| (edge.centroid, edge.measure))
                .collect();
            println!(
                "VIEW {label} {}: {} edges, {summary}, two lines {}, valid {:?}, first edges (centroid, length) {ends:?}",
                construction.name(),
                elements.len(),
                kinds == [partsmith_occt_sys::GeometryKind::Line; 2],
                curves.shape.is_valid().map_err(|error| error.detail())
            );
        }
    }
}
