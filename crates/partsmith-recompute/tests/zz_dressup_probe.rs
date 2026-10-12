//! Temporary probe of review findings (removed after use).

mod feature_support;

use feature_support::{Run, face_names};

const CYL: &str = r#"{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "10", "height": "20", "op": "new"}"#;
const RIM: &str = r#"{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-cyl1/side|f-cyl1/top)", "fp": "circle|n=0,0,1|c=0,0,20|l=62.831853|r=10"}], "radius": "2"}"#;

#[test]
fn probe_torus() {
    for edge in [
        r#"{"q": "edge(f-cyl1/side|f-fil1/fillet/edge(f-cyl1/side|f-cyl1/top))", "fp": "circle|n=0,0,1|c=0,0,18|l=62.831853|r=10"}"#,
        r#"{"q": "edge(f-cyl1/top|f-fil1/fillet/edge(f-cyl1/side|f-cyl1/top))", "fp": "circle|n=0,0,1|c=0,0,20|l=50.265482|r=8"}"#,
    ] {
        let second = format!(r#"{{"id": "f-fil2", "type": "fillet", "body": "b-1", "edges": [{edge}], "radius": "1"}}"#);
        let run = Run::records(&["b-1"], &[CYL, RIM, &second]);
        println!("{}", run.summary());
        println!("{:?}", face_names(run.body("b-1")));
        let chamfer = format!(r#"{{"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [{edge}], "distance": {{"kind": "equal", "expr": "1"}}}}"#);
        let run = Run::records(&["b-1"], &[CYL, RIM, &chamfer]);
        println!("{}", run.summary());
    }
    let shape = Run::records(&["b-1"], &[CYL, RIM]);
    let body = shape.body("b-1");
    let s = body.shape().unwrap();
    for (i, f) in s.elements(partsmith_occt_sys::SubShapeKind::Face).unwrap().iter().enumerate() {
        println!("face {} {:?}", i + 1, f);
    }
}

const BOX: &str = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["30", "20", "10"], "op": "new"}"#;
const VERTICAL: &str = r#"{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-15,-10,5|l=10"}, {"q": "edge(f-box1/front|f-box1/right)", "fp": "line|n=0,0,1|c=15,-10,5|l=10"}, {"q": "edge(f-box1/back|f-box1/left)", "fp": "line|n=0,0,1|c=-15,10,5|l=10"}, {"q": "edge(f-box1/back|f-box1/right)", "fp": "line|n=0,0,1|c=15,10,5|l=10"}], "radius": "3"}"#;
const E_FT: &str = r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,10|l=24"}"#;
const TL: &str = r#"{"q": "edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "fp": "circle|n=0,0,1|c=-12,-7,10|r=3"}"#;

#[test]
fn probe_chain() {
    for edges in [vec![E_FT], vec![TL], vec![E_FT, TL], vec![TL, E_FT]] {
        let chamfer = format!(r#"{{"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [{}], "distance": {{"kind": "two_distances", "expr": "1", "second_expr": "2"}}}}"#, edges.join(", "));
        let run = Run::records(&["b-1"], &[BOX, VERTICAL, &chamfer]);
        println!("{} -> {:.6}", run.summary(), run.shape("b-1").volume().unwrap());
    }
    let smooth = r#"{"q": "edge(f-box1/front|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "fp": "line|n=0,0,1|c=-12,-10,5|l=10"}"#;
    let chamfer = format!(r#"{{"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [{smooth}], "distance": {{"kind": "equal", "expr": "1"}}}}"#);
    let run = Run::records(&["b-1"], &[BOX, VERTICAL, &chamfer]);
    println!("{}", run.summary());
}

#[test]
fn probe_two_distance_too_large() {
    let plate = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"}"#;
    let edges = [
        r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,2.4|l=20"}"#,
        r#"{"q": "edge(f-box1/back|f-box1/top)", "fp": "line|n=1,0,0|c=0,10,2.4|l=20"}"#,
        r#"{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-10,0,2.4|l=20"}"#,
        r#"{"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0|c=10,0,2.4|l=20"}"#,
    ];
    for (a, b) in [("3", "4"), ("4", "3")] {
        let chamfer = format!(r#"{{"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [{}], "distance": {{"kind": "two_distances", "expr": "{a}", "second_expr": "{b}"}}}}"#, edges.join(", "));
        let run = Run::records(&["b-1"], &[plate, &chamfer]);
        println!("{}", run.summary());
        println!("{:?}", run.feature("f-cha1").error);
    }
}

#[test]
fn probe_shell_step() {
    let box1 = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "10"], "op": "new"}"#;
    let box2 = r#"{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "20", "10"], "position": ["10", "0", "10"], "op": "join"}"#;
    for (face, t) in [("f-box1/top", "2"), ("f-box1/top", "0.5"), ("f-box1/bottom", "2"), ("f-box2/top", "2")] {
        let shell = format!(r#"{{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "{t}", "remove_faces": [{{"q": "{face}", "fp": "plane|n=0,0,1"}}]}}"#);
        let run = Run::records(&["b-1"], &[box1, box2, &shell]);
        println!("{face} {t}: {}", run.summary());
        if run.feature("f-sh1").error.is_none() {
            println!("{:?}", face_names(run.body("b-1")));
        }
    }
}
