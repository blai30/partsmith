//! Temporary profile of descriptor costs (removed before finishing).
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use partsmith_features::{CancelToken, Registry};
use partsmith_model::load;
use partsmith_occt_sys::SubShapeKind;
use partsmith_print::{PrinterValues, generic_fdm};
use partsmith_recompute::{Input, Recompute};
use serde_json::json;

fn main() {
    let direction = |axis: &str| json!({"axis": {"kind": "origin", "axis": axis}, "count": "13", "spacing": "20 mm"});
    let grid = json!({
        "format": "partsmith", "format_version": 1, "units": "mm", "parameters": [],
        "bodies": [{"id": "b-1", "name": "Grid"}],
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
            {"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": direction("X"), "second_direction": direction("Y")}
        ]
    }).to_string();
    let document = load(&grid).unwrap().document;
    let mut recompute = Recompute::new(Arc::new(Registry::with_builtins()));
    let profile = generic_fdm();
    let context = PrinterValues::new(&profile);
    let assets = BTreeMap::new();
    let start = Instant::now();
    let report = recompute.run(&Input { document: &document, context: &context, stop_at: None, assets: &assets }, &CancelToken::new());
    println!("recompute {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    let state = &report.bodies[0].state;
    let shape = state.shape().unwrap();
    let start = Instant::now();
    let topology = shape.topology().unwrap();
    println!("Shape::topology {:.1} ms ({} faces, {} edges, {} vertices)", start.elapsed().as_secs_f64() * 1000.0, topology.faces.len(), topology.edges.len(), topology.vertices.len());
    for kind in [SubShapeKind::Face, SubShapeKind::Edge, SubShapeKind::Vertex] {
        let start = Instant::now();
        let geometry = shape.elements(kind).unwrap();
        println!("Shape::elements({kind:?}) {:.1} ms ({} items)", start.elapsed().as_secs_f64() * 1000.0, geometry.len());
    }
    let start = Instant::now();
    let info = partsmith_features::topology_info(shape).unwrap();
    println!("topology_info {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    let start = Instant::now();
    let infos = state.naming().element_infos(&info).unwrap();
    println!("NamingMap::element_infos {:.1} ms ({} infos)", start.elapsed().as_secs_f64() * 1000.0, infos.len());
    let start = Instant::now();
    let infos = state.elements().unwrap();
    println!("BodyState::elements (first) {:.1} ms, {} infos", start.elapsed().as_secs_f64() * 1000.0, infos.len());
    let start = Instant::now();
    let _ = shape.element(SubShapeKind::Face, 500).unwrap();
    println!("Shape::element(Face, 500) {:.3} ms", start.elapsed().as_secs_f64() * 1000.0);
    let start = Instant::now();
    let _ = shape.tessellate(&profile.tessellation_tolerance()).unwrap();
    println!("tessellate {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
}
