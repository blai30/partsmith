//! The framework rows of the performance budgets in `docs/spec/features.md` section 5, timed with
//! `std::time::Instant` (decision D7). Budgets hold for release builds on the reference machine, so
//! these tests are ignored in debug builds: run them with `cargo test --release`.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

use partsmith_features::{
    BodyState, BuildContext, BuildInputs, CancelToken, PythonNames, Registry, Values, name_shape,
};
use partsmith_model::{EmptyContext, FeatureRecord, Id, ParameterValues, Scope, Units};
use partsmith_naming::{ElementName, FaceSource};
use partsmith_occt_sys::{Plane, Point, Shape, SubShapeKind, cut, make_box, make_cylinder};
use partsmith_sketch::RegionRef;
use serde_json::json;

/// Runs of each measurement; the median is compared with the budget.
const RUNS: usize = 31;

/// The median time of `RUNS` calls of `operation`.
fn median(mut operation: impl FnMut()) -> Duration {
    let mut durations: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            operation();
            started.elapsed()
        })
        .collect();
    durations.sort();
    durations[RUNS / 2]
}

/// Asserts that `measured` is within `budget`.
fn assert_within(what: &str, measured: Duration, budget: Duration) {
    assert!(
        measured <= budget,
        "{what} took {measured:?} (median of {RUNS} runs), over the {budget:?} budget"
    );
}

/// A record from its JSON form.
fn record(value: serde_json::Value) -> FeatureRecord {
    serde_json::from_value(value).expect("records written in tests are well-formed")
}

/// The extrude record of features/extrude.md test 1.
fn extrude_record() -> FeatureRecord {
    record(json!({
        "id": "f-ext1", "type": "extrude", "body": "b-1",
        "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
        "extent": {"kind": "distance", "expr": "30 mm"}, "op": "new"
    }))
}

/// Script names where `b-1` is `block` and every op defaults to new.
struct Names;

impl PythonNames for Names {
    fn units(&self) -> Units {
        Units::Mm
    }

    fn body(&self, body: &Id) -> Option<String> {
        (body.as_str() == "b-1").then(|| "block".to_string())
    }

    fn feature(&self, _feature: &Id) -> Option<String> {
        None
    }

    fn entity(&self, _sketch: &Id, _entity: &Id) -> Option<String> {
        None
    }

    fn parameter(&self, _name: &str) -> Option<String> {
        None
    }

    fn regions(&self, _sketch: &Id) -> Option<Vec<RegionRef>> {
        None
    }

    fn default_op(&self, _body: &Id) -> partsmith_features::BodyOp {
        partsmith_features::BodyOp::New
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn validate_and_canonicalize_takes_under_50_microseconds() {
    let registry = Registry::with_builtins();
    let extrude = extrude_record();

    let measured = median(|| {
        black_box(registry.validate_and_canonicalize(black_box(&extrude))).expect("valid params");
    });

    assert_within(
        "validate_and_canonicalize",
        measured,
        Duration::from_micros(50),
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn dependencies_take_under_20_microseconds() {
    let registry = Registry::with_builtins();
    let extrude = extrude_record();

    let measured = median(|| {
        black_box(registry.dependencies(black_box(&extrude))).expect("valid params");
    });

    assert_within("dependencies", measured, Duration::from_micros(20));
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn python_takes_under_50_microseconds() {
    let registry = Registry::with_builtins();
    let block = record(json!({
        "id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "40"], "op": "new"
    }));

    let measured = median(|| {
        black_box(registry.python(black_box(&block), &Names));
    });

    assert_within("Registry::python", measured, Duration::from_micros(50));
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn params_schemas_of_every_builtin_take_under_20_milliseconds() {
    let measured = median(|| {
        let registry = Registry::with_builtins();
        black_box(registry.params_schemas());
    });

    assert_within(
        "params_schemas with registration",
        measured,
        Duration::from_millis(20),
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn context_overhead_of_a_build_is_under_50_microseconds() {
    // A datum plane on an origin plane does no kernel work and reads no element info, so its
    // whole build is context overhead: one evaluated value and one plane.
    let registry = Registry::with_builtins();
    let datum = record(json!({
        "id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "10"
    }));
    let dependencies = registry.dependencies(&datum).expect("valid params");
    let manifest = registry
        .get("datum_plane")
        .expect("datum_plane is registered")
        .manifest()
        .clone();
    let parameters = ParameterValues::default();
    let scope = Scope {
        units: Units::Mm,
        parameters: &parameters,
        context: &EmptyContext,
    };
    let values = Values::evaluate(&dependencies.expressions, &scope, &manifest);
    let bodies = BTreeMap::new();
    let upstream = BTreeMap::new();
    let assets = BTreeMap::new();
    let cancel = CancelToken::new();

    let measured = median(|| {
        let context = BuildContext::new(BuildInputs {
            feature: &datum.id,
            manifest: &manifest,
            units: Units::Mm,
            values: &values,
            bodies: &bodies,
            upstream: &upstream,
            assets: &assets,
            cancel: &cancel,
        });
        black_box(registry.build(black_box(&datum), &context)).expect("the datum builds");
    });

    assert_within("context overhead", measured, Duration::from_micros(50));
}

/// A plate with 194 round through holes: 200 faces.
fn plate_with_holes() -> Shape {
    let plate = make_box(&Plane::XY, 300.0, 300.0, 5.0)
        .expect("the plate builds")
        .shape;
    let holes: Vec<Shape> = (0..194)
        .map(|index| {
            let placement = Plane {
                origin: Point::new(
                    10.0 + f64::from(index % 14) * 20.0,
                    10.0 + f64::from(index / 14) * 20.0,
                    -1.0,
                ),
                ..Plane::XY
            };
            make_cylinder(&placement, 3.0, 7.0)
                .expect("the hole tool builds")
                .shape
        })
        .collect();
    cut(&plate, &holes).expect("the holes cut").shape
}

#[test]
#[ignore = "breakdown for the kernel gap; run with cargo test --release -- --ignored --nocapture"]
fn kernel_queries_of_a_200_face_body() {
    let shape = plate_with_holes();
    let kernel = median(|| {
        black_box(shape.topology()).expect("topology");
        for kind in [SubShapeKind::Face, SubShapeKind::Edge, SubShapeKind::Vertex] {
            black_box(shape.elements(kind)).expect("element geometry");
        }
    });
    let faces = median(|| {
        black_box(shape.elements(SubShapeKind::Face)).expect("face geometry");
    });
    println!("kernel queries {kernel:?}, of which elements(Face) {faces:?}");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn elements_of_a_200_face_body_take_under_10_milliseconds() {
    let shape = plate_with_holes();
    assert_eq!(shape.count(SubShapeKind::Face), 200);
    let feature = Id::new("f-plate1").expect("valid id");
    let name = ElementName::generated(&feature, "face", None).expect("valid name");
    let faces = vec![FaceSource::New(name); 200];
    let naming = Arc::new(name_shape(&shape, faces, &[]).expect("the plate is named"));

    let measured = median(|| {
        // A new state computes its element infos on first use.
        let state = BodyState::new(shape.clone(), Arc::clone(&naming));
        black_box(state.elements()).expect("element infos");
    });

    assert_within(
        "BodyState::elements of 200 faces",
        measured,
        Duration::from_millis(10),
    );
}
