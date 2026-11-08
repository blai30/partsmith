//! Acceptance tests for curve chains (spec section 3.18 and the chain rules of sections 3.17, 4.4 and 4.5; section
//! 6.23, tests 263 to 288, 337 and 338, and the chain scenario of section 6.24, test 302).

use crate::common;

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use common::benchmark::{
    LONG_OUTLINE_LINES, LONG_OUTLINE_SEED, long_outline_benchmark, long_outline_chain_key,
};
use common::matching::key_edge;
use common::messages::{assert_plain_message, context};
use common::{
    Sketch, TOLERANCE, arc, as_construction, assert_bits, assert_bitwise, assert_close,
    assert_point_bitwise, assert_point_close, circle, id, ids, line, point, point_at_angle,
    point_ref, rect, rect_entities,
};
use partsmith_model::Units;
use partsmith_sketch::{
    CONNECT_DISTANCE, Chain, ChainFingerprint, ChainKey, ChainLoss, ChainMatch, ChainRef,
    EntityKind, MessageContext, ProfileCurve, ProfileEdge, SketchEntity, SketchError, SketchFix,
    Succession, build_chain, chain_through, match_chain,
};

/// Turns hold within this tolerance unless a test says bitwise (spec section 6.23).
const TURN_TOLERANCE: f64 = 1e-12;

/// `P-KEY`: the key of the whole of `PATH`.
const P_KEY: &str = "e-1,e-2,e-3";

/// The fingerprint text of `PATH`'s chain (test 267).
const P_FINGERPRINT: &str = "s=0,0|e=50,40|l=85.707963";

// ---------------------------------------------------------------------------
// Sketches of section 6.23
// ---------------------------------------------------------------------------

/// `PATH`: a line, a quarter arc tangent to it, and a line tangent to the arc.
fn path() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        arc("e-2", [40.0, 10.0], 10.0, -FRAC_PI_2, 0.0),
        line("e-3", [50.0, 10.0], [50.0, 40.0]),
    ]
}

/// `ELBOW`: a corner, with `e-2` drawn toward it.
fn elbow() -> Vec<SketchEntity> {
    elbow_ending_at([40.0, 0.0])
}

/// `ELBOW` with `e-2` ending at `end` in place of (40, 0).
fn elbow_ending_at(end: [f64; 2]) -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        line("e-2", [40.0, 30.0], end),
    ]
}

/// `HOOK`: an arc that leaves the line's end going back the way the line came.
fn hook() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        arc("e-2", [40.0, -10.0], 10.0, FRAC_PI_2, PI),
    ]
}

/// `ZIGZAG`: `e-3` crosses `e-1` at (30, 0).
fn zigzag() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        line("e-2", [40.0, 0.0], [40.0, 20.0]),
        line("e-3", [40.0, 20.0], [20.0, -20.0]),
    ]
}

/// `circle e-5 c(40,25) r5`.
fn small_circle() -> SketchEntity {
    circle("e-5", [40.0, 25.0], 5.0)
}

/// `line e-7 (10,10)->(10,10.0000004)`: a line without length.
fn short_line() -> SketchEntity {
    line("e-7", [10.0, 10.0], [10.0, 10.000_000_4])
}

/// `entities` with `extra` drawn after them.
fn with(mut entities: Vec<SketchEntity>, extra: SketchEntity) -> Vec<SketchEntity> {
    entities.push(extra);
    entities
}

/// `entities` with the entity of the same id as `entity` replaced by it, in its place.
fn replaced(mut entities: Vec<SketchEntity>, entity: SketchEntity) -> Vec<SketchEntity> {
    let slot = entities
        .iter_mut()
        .find(|existing| existing.id == entity.id)
        .expect("the replaced entity exists");
    *slot = entity;
    entities
}

/// `entities` with the entity `entity` marked as construction geometry.
fn with_construction(entities: Vec<SketchEntity>, entity: &str) -> Vec<SketchEntity> {
    entities
        .into_iter()
        .map(|candidate| {
            if candidate.id.as_str() == entity {
                as_construction(candidate)
            } else {
                candidate
            }
        })
        .collect()
}

/// `RECT` with `e-4` from (0, 50) to (0, 0.3): its last end no longer meets its first start.
fn opened_rect() -> Vec<SketchEntity> {
    replaced(rect_entities(), line("e-4", [0.0, 50.0], [0.0, 0.3]))
}

// ---------------------------------------------------------------------------
// Keys, references and results
// ---------------------------------------------------------------------------

/// A chain key, panicking on a fixture mistake.
fn chain_key(text: &str) -> ChainKey {
    text.parse()
        .unwrap_or_else(|error| panic!("{text} is a chain key: {error}"))
}

/// A chain fingerprint, panicking on a fixture mistake.
fn fingerprint(text: &str) -> ChainFingerprint {
    text.parse()
        .unwrap_or_else(|error| panic!("{text} is a chain fingerprint: {error}"))
}

/// `{key, fp, closed}`.
fn chain_ref(key: &str, fp: &str, closed: bool) -> ChainRef {
    ChainRef {
        key: chain_key(key),
        fp: fingerprint(fp),
        closed,
    }
}

/// The reference of test 266: `PATH`'s chain as test 267 builds it.
fn path_reference() -> ChainRef {
    chain_ref(P_KEY, P_FINGERPRINT, false)
}

/// The chain `key` names on `entities`, panicking when it does not build.
fn built(entities: &[SketchEntity], key: &str) -> Chain {
    build_chain(entities, &chain_key(key))
        .unwrap_or_else(|loss| panic!("{key} builds, but it is lost: {loss}"))
}

/// Why `key` does not build on `entities`, panicking when it builds.
fn lost(entities: &[SketchEntity], key: &str) -> ChainLoss {
    match build_chain(entities, &chain_key(key)) {
        Err(loss) => loss,
        Ok(chain) => panic!("{key} is lost, but it builds: {chain:?}"),
    }
}

/// The chain and successions of a resolved match, panicking on any other result.
fn resolved(found: ChainMatch) -> (Chain, Vec<Succession>) {
    match found {
        ChainMatch::Resolved { chain, successions } => (chain, successions),
        other => panic!("expected a resolved chain, got {other:?}"),
    }
}

/// The loss of a missing match, panicking on any other result.
fn missing(found: ChainMatch) -> ChainLoss {
    match found {
        ChainMatch::Missing { loss } => loss,
        other => panic!("expected a missing chain, got {other:?}"),
    }
}

/// `chain_through(entities, seed)` as key text.
fn picked(entities: &[SketchEntity], seed: &str) -> Option<String> {
    chain_through(entities, &id(seed)).map(|key| key.to_string())
}

/// `Deleted { entities, all }`.
fn deleted(entities: &[&str], all: bool) -> ChainLoss {
    ChainLoss::Deleted {
        entities: ids(entities),
        all,
    }
}

/// The loss rendered in the context of section 6.10 on `entities`, in millimeters.
fn reason(loss: &ChainLoss, entities: &[SketchEntity]) -> String {
    loss.render(&context(&Sketch::new(entities.to_vec())))
}

// ---------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------

/// The start and end of a line or an arc edge.
#[track_caller]
fn ends(edge: &ProfileEdge) -> ([f64; 2], [f64; 2]) {
    match edge.curve {
        ProfileCurve::Line { start, end } | ProfileCurve::Arc { start, end, .. } => (start, end),
        ProfileCurve::Circle { .. } => panic!("{} is a circle edge", edge.entity),
    }
}

/// Asserts that an edge has this entity, flag and curve, coordinates within the tolerance of section 6.0.
#[track_caller]
fn assert_edge(actual: &ProfileEdge, entity: &str, reversed: bool, curve: &ProfileCurve) {
    assert_eq!(actual.entity.as_str(), entity, "entity of {actual:?}");
    assert_eq!(actual.reversed, reversed, "reversed of {actual:?}");
    let what = format!("edge of {entity}");
    match (&actual.curve, curve) {
        (
            ProfileCurve::Line { start, end },
            ProfileCurve::Line {
                start: expected_start,
                end: expected_end,
            },
        ) => {
            assert_point_close(*start, *expected_start, TOLERANCE, &what);
            assert_point_close(*end, *expected_end, TOLERANCE, &what);
        }
        (
            ProfileCurve::Arc {
                center,
                radius,
                start,
                end,
                ccw,
            },
            ProfileCurve::Arc {
                center: expected_center,
                radius: expected_radius,
                start: expected_start,
                end: expected_end,
                ccw: expected_ccw,
            },
        ) => {
            assert_point_close(*center, *expected_center, TOLERANCE, &what);
            assert_close(*radius, *expected_radius, TOLERANCE, &what);
            assert_point_close(*start, *expected_start, TOLERANCE, &what);
            assert_point_close(*end, *expected_end, TOLERANCE, &what);
            assert_eq!(ccw, expected_ccw, "{what}: ccw");
        }
        (
            ProfileCurve::Circle {
                center,
                radius,
                ccw,
            },
            ProfileCurve::Circle {
                center: expected_center,
                radius: expected_radius,
                ccw: expected_ccw,
            },
        ) => {
            assert_point_close(*center, *expected_center, TOLERANCE, &what);
            assert_close(*radius, *expected_radius, TOLERANCE, &what);
            assert_eq!(ccw, expected_ccw, "{what}: ccw");
        }
        (actual_curve, _) => panic!("{what}: expected {curve:?}, got {actual_curve:?}"),
    }
}

/// Asserts the points and turns of a chain's junctions, in order.
#[track_caller]
fn assert_junctions(chain: &Chain, expected: &[([f64; 2], f64)]) {
    assert_eq!(
        chain.junctions.len(),
        expected.len(),
        "junctions: {:?}",
        chain.junctions
    );
    for (i, (junction, (point, turn))) in chain.junctions.iter().zip(expected).enumerate() {
        assert_point_close(junction.point, *point, TOLERANCE, &format!("junction {i}"));
        assert_close(
            junction.turn,
            *turn,
            TURN_TOLERANCE,
            &format!("turn of junction {i}"),
        );
    }
}

/// Asserts that `loss` is `Disconnected` between these point references with a gap within 1e-12.
#[track_caller]
fn assert_disconnected(loss: &ChainLoss, end: &str, start: &str, gap: f64) {
    let ChainLoss::Disconnected {
        end: actual_end,
        start: actual_start,
        gap: actual_gap,
    } = loss
    else {
        panic!("expected disconnected ends, got {loss:?}");
    };
    assert_eq!(*actual_end, point_ref(end));
    assert_eq!(*actual_start, point_ref(start));
    assert_close(*actual_gap, gap, 1e-12, "gap");
}

// ---------------------------------------------------------------------------
// Keys, fingerprints and references (3.18.1, 3.18.5)
// ---------------------------------------------------------------------------

#[test]
fn t263_chain_keys_keep_their_order_and_directions() {
    let key = chain_key("e-1,~e-2,e-3");

    assert_eq!(
        key.entries(),
        vec![
            key_edge("e-1", false),
            key_edge("e-2", true),
            key_edge("e-3", false)
        ]
    );
    assert_eq!(key.to_string(), "e-1,~e-2,e-3");
    let reversed = key.reversed();
    assert_eq!(reversed.to_string(), "~e-3,e-2,~e-1");
    assert_eq!(reversed.reversed(), key);
}

#[test]
fn t264_invalid_chain_keys_fail_with_their_text() {
    for text in [
        "",
        "e-1,,e-2",
        "e-1,~~e-2",
        "e-1;e-2",
        "e-1, e-2",
        "e-1,e-1",
        "e-1,~e-1",
    ] {
        assert_eq!(
            text.parse::<ChainKey>().err(),
            Some(SketchError::InvalidChainKey {
                text: text.to_owned()
            }),
            "{text:?}"
        );
    }
    assert_eq!(
        ChainKey::new(Vec::new()).err(),
        Some(SketchError::InvalidChainKey {
            text: String::new()
        })
    );
    assert_eq!(
        ChainKey::new(vec![key_edge("e-1", false), key_edge("e-1", true)]).err(),
        Some(SketchError::InvalidChainKey {
            text: "e-1,~e-1".to_owned()
        })
    );
}

#[test]
fn chain_keys_from_entries_are_not_sorted() {
    let key = ChainKey::new(vec![key_edge("e-3", false), key_edge("e-1", true)])
        .expect("two entities named once make a key");

    assert_eq!(key.to_string(), "e-3,~e-1");
    assert_eq!(key, chain_key("e-3,~e-1"));
}

#[test]
fn a_chain_key_of_one_reversed_entry_round_trips() {
    let key = chain_key("~e_circle-5");

    assert_eq!(key.entries(), vec![key_edge("e_circle-5", true)]);
    assert_eq!(key.to_string(), "~e_circle-5");
    assert_eq!(key.reversed().to_string(), "e_circle-5");
}

#[test]
fn t265_fingerprint_text() {
    let path = ChainFingerprint {
        start: [0.0, 0.0],
        end: [50.0, 40.0],
        length: 70.0 + 5.0 * PI,
    };
    assert_eq!(path.to_string(), "s=0,0|e=50,40|l=85.707963");
    let near_zero = ChainFingerprint {
        start: [-0.0, 1e-7],
        end: [50.0, 40.0],
        length: 70.0 + 5.0 * PI,
    };
    let text = near_zero.to_string();
    assert!(text.starts_with("s=0,0|"), "{text}");

    for text in [
        "s=0,0|e=1,1",
        "s=0,0|e=1,1|l=-1",
        "e=1,1|s=0,0|l=1",
        "s=0,0|e=1,1|l=x",
        "s=0,0|e=1,1|l=1e3",
        "s=+1,0|e=1,1|l=1",
        "s=0,0|e=1,1|l=.5",
    ] {
        assert_eq!(
            text.parse::<ChainFingerprint>().err(),
            Some(SketchError::InvalidChainFingerprint {
                text: text.to_owned()
            }),
            "{text:?}"
        );
    }
    let negative_zero = fingerprint("s=-0,0|e=1,1|l=1");
    assert_bits(negative_zero.start[0], 0.0, "start u of s=-0,0");
}

#[test]
fn a_fingerprint_text_reads_back_to_the_same_text() {
    for text in [P_FINGERPRINT, "s=-12.5,0.000001|e=3,-4|l=0.5"] {
        assert_eq!(fingerprint(text).to_string(), text);
    }
}

#[test]
fn t266_reference_json() {
    let open: ChainRef =
        serde_json::from_str(r#"{"key": "e-1,e-2,e-3", "fp": "s=0,0|e=50,40|l=85.707963"}"#)
            .expect("an open chain reference deserializes");
    assert!(!open.closed);
    assert_eq!(open.key, chain_key(P_KEY));
    assert_eq!(
        serde_json::to_string(&open).expect("the reference serializes"),
        r#"{"key":"e-1,e-2,e-3","fp":"s=0,0|e=50,40|l=85.707963"}"#
    );

    let closed: ChainRef = serde_json::from_str(
        r#"{"key": "e-1,e-2,e-3", "fp": "s=0,0|e=50,40|l=85.707963", "closed": true}"#,
    )
    .expect("a closed chain reference deserializes");
    assert!(closed.closed);
    assert_eq!(
        serde_json::to_string(&closed).expect("the reference serializes"),
        r#"{"key":"e-1,e-2,e-3","fp":"s=0,0|e=50,40|l=85.707963","closed":true}"#
    );

    let unknown = serde_json::from_str::<ChainRef>(
        r#"{"key": "e-1,e-2,e-3", "fp": "s=0,0|e=50,40|l=85.707963", "outline": "e-1"}"#,
    );
    assert!(unknown.is_err(), "an unknown member is refused");
}

/// Every `pattern` string anywhere in a JSON Schema.
fn patterns(schema: &serde_json::Value) -> Vec<String> {
    match schema {
        serde_json::Value::Object(members) => members
            .iter()
            .flat_map(|(name, value)| match (name.as_str(), value) {
                ("pattern", serde_json::Value::String(pattern)) => vec![pattern.clone()],
                _ => patterns(value),
            })
            .collect(),
        serde_json::Value::Array(items) => items.iter().flat_map(patterns).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn t266_reference_schema_holds_the_key_and_fingerprint_patterns() {
    let schema =
        serde_json::to_value(schemars::schema_for!(ChainRef)).expect("the schema serializes");

    let found = patterns(&schema);
    assert!(
        found.contains(&r"^~?[A-Za-z0-9_-]{1,64}(,~?[A-Za-z0-9_-]{1,64})*$".to_owned()),
        "the key pattern is missing: {found:?}"
    );
    assert!(
        found.contains(
            &r"^s=-?[0-9]+(\.[0-9]+)?,-?[0-9]+(\.[0-9]+)?\|e=-?[0-9]+(\.[0-9]+)?,-?[0-9]+(\.[0-9]+)?\|l=[0-9]+(\.[0-9]+)?$"
                .to_owned()
        ),
        "the fingerprint pattern is missing: {found:?}"
    );
    let required: Vec<&str> = schema["required"]
        .as_array()
        .expect("a chain reference requires fields")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert_eq!(required, ["key", "fp"]);
}

// ---------------------------------------------------------------------------
// Building a chain (3.18.2)
// ---------------------------------------------------------------------------

#[test]
fn t267_a_smooth_open_chain() {
    let chain = built(&path(), P_KEY);

    assert!(!chain.closed);
    assert_eq!(chain.key, chain_key(P_KEY));
    assert_eq!(chain.edges.len(), 3);
    assert_edge(
        &chain.edges[0],
        "e-1",
        false,
        &ProfileCurve::Line {
            start: [0.0, 0.0],
            end: [40.0, 0.0],
        },
    );
    assert_edge(
        &chain.edges[1],
        "e-2",
        false,
        &ProfileCurve::Arc {
            center: [40.0, 10.0],
            radius: 10.0,
            start: [40.0, 0.0],
            end: [50.0, 10.0],
            ccw: true,
        },
    );
    assert_edge(
        &chain.edges[2],
        "e-3",
        false,
        &ProfileCurve::Line {
            start: [50.0, 10.0],
            end: [50.0, 40.0],
        },
    );
    assert_junctions(&chain, &[([40.0, 0.0], 0.0), ([50.0, 10.0], 0.0)]);
    assert!(chain.junctions.iter().all(|junction| junction.is_tangent()));
    assert!(chain.is_smooth());
    assert_close(chain.length, 70.0 + 5.0 * PI, 1e-12, "length");
    assert_point_close(chain.start(), [0.0, 0.0], TOLERANCE, "start");
    assert_point_close(chain.end(), [50.0, 40.0], TOLERANCE, "end");
    assert_point_close(chain.start_tangent(), [1.0, 0.0], TOLERANCE, "start tangent");
    assert_point_close(chain.end_tangent(), [0.0, 1.0], TOLERANCE, "end tangent");
    assert_eq!(
        serde_json::to_string(&chain.reference()).expect("the reference serializes"),
        r#"{"key":"e-1,e-2,e-3","fp":"s=0,0|e=50,40|l=85.707963"}"#
    );
}

#[test]
fn t268_junctions_share_their_points_bitwise() {
    // The arc starts 0.0000005 mm above the end of e-1, within PROFILE_TOLERANCE.
    let center = [40.0, 10.000_000_5];
    let entities = replaced(path(), arc("e-2", center, 10.0, -FRAC_PI_2, 0.0));
    let arc_start = point_at_angle(center, 10.0, -FRAC_PI_2);

    let chain = built(&entities, P_KEY);

    let (first_start, first_end) = ends(&chain.edges[0]);
    let (arc_edge_start, arc_edge_end) = ends(&chain.edges[1]);
    let (last_start, _) = ends(&chain.edges[2]);
    assert_point_bitwise(first_end, arc_start, "end of edge 0");
    assert_point_bitwise(arc_edge_start, arc_start, "start of edge 1");
    assert_point_bitwise(arc_edge_end, [50.0, 10.0], "end of edge 1");
    assert_point_bitwise(last_start, [50.0, 10.0], "start of edge 2");
    assert_point_bitwise(first_start, [0.0, 0.0], "start of edge 0");
}

#[test]
fn t269_a_reversed_entry_and_a_corner() {
    let chain = built(&elbow(), "e-1,~e-2");

    assert_edge(
        &chain.edges[1],
        "e-2",
        true,
        &ProfileCurve::Line {
            start: [40.0, 0.0],
            end: [40.0, 30.0],
        },
    );
    let [junction] = chain.junctions.as_slice() else {
        panic!("expected one junction, got {:?}", chain.junctions);
    };
    assert_point_close(junction.point, [40.0, 0.0], TOLERANCE, "junction");
    assert_bits(junction.turn, FRAC_PI_2, "turn");
    assert!(!junction.is_tangent());
    assert!(!chain.is_smooth());
    assert_close(chain.length, 70.0, TOLERANCE, "length");
    assert_eq!(chain.fingerprint().to_string(), "s=0,0|e=40,30|l=70");
}

#[test]
fn t270_closed_chains() {
    let chain = built(&rect_entities(), "e-1,e-2,e-3,e-4");

    assert!(chain.closed);
    assert_junctions(
        &chain,
        &[
            ([80.0, 0.0], FRAC_PI_2),
            ([80.0, 50.0], FRAC_PI_2),
            ([0.0, 50.0], FRAC_PI_2),
            ([0.0, 0.0], FRAC_PI_2),
        ],
    );
    assert_close(chain.length, 260.0, TOLERANCE, "length");
    assert_eq!(chain.fingerprint().to_string(), "s=0,0|e=0,0|l=260");
    assert!(chain.reference().closed);
    let (_, last_end) = ends(&chain.edges[3]);
    assert_point_bitwise(last_end, [0.0, 0.0], "end of edge 3");

    let rotated = built(&rect_entities(), "e-3,e-4,e-1,e-2");

    assert!(rotated.closed);
    assert_point_close(rotated.start(), [80.0, 50.0], TOLERANCE, "start");
}

#[test]
fn t271_a_circle() {
    let circle_only = vec![small_circle()];

    let chain = built(&circle_only, "e-5");

    assert!(chain.closed);
    assert_eq!(chain.edges.len(), 1);
    assert_edge(
        &chain.edges[0],
        "e-5",
        false,
        &ProfileCurve::Circle {
            center: [40.0, 25.0],
            radius: 5.0,
            ccw: true,
        },
    );
    assert!(chain.junctions.is_empty());
    assert_point_close(chain.start(), [45.0, 25.0], TOLERANCE, "start");
    assert_point_close(chain.end(), [45.0, 25.0], TOLERANCE, "end");
    assert_point_close(chain.start_tangent(), [0.0, 1.0], TOLERANCE, "start tangent");
    assert_point_close(chain.end_tangent(), [0.0, 1.0], TOLERANCE, "end tangent");
    assert_close(chain.length, 10.0 * PI, TOLERANCE, "length");
    assert!(chain.is_smooth());

    let clockwise = built(&circle_only, "~e-5");

    assert_edge(
        &clockwise.edges[0],
        "e-5",
        true,
        &ProfileCurve::Circle {
            center: [40.0, 25.0],
            radius: 5.0,
            ccw: false,
        },
    );
    assert_point_close(
        clockwise.start_tangent(),
        [0.0, -1.0],
        TOLERANCE,
        "start tangent",
    );
    assert_point_close(
        clockwise.end_tangent(),
        [0.0, -1.0],
        TOLERANCE,
        "end tangent",
    );

    // Its ends lie 1e-9 mm apart.
    let closed_arc = vec![arc("e-6", [0.0, 0.0], 10.0, 0.0, TAU - 1e-10)];

    let arc_chain = built(&closed_arc, "e-6");

    assert!(arc_chain.closed);
    assert_eq!(arc_chain.edges.len(), 1);
    assert_edge(
        &arc_chain.edges[0],
        "e-6",
        false,
        &ProfileCurve::Circle {
            center: [0.0, 0.0],
            radius: 10.0,
            ccw: true,
        },
    );
    assert_point_bitwise(arc_chain.start(), [10.0, 0.0], "start");
    assert_point_bitwise(arc_chain.end(), [10.0, 0.0], "end");
    let [junction] = arc_chain.junctions.as_slice() else {
        panic!("expected one junction, got {:?}", arc_chain.junctions);
    };
    assert_point_close(junction.point, [10.0, 0.0], TOLERANCE, "junction");
    assert!(junction.is_tangent(), "turn {}", junction.turn);

    let clockwise_arc = built(&closed_arc, "~e-6");

    assert_edge(
        &clockwise_arc.edges[0],
        "e-6",
        true,
        &ProfileCurve::Circle {
            center: [0.0, 0.0],
            radius: 10.0,
            ccw: false,
        },
    );

    assert_eq!(
        lost(&with(rect_entities(), small_circle()), "e-1,e-5"),
        ChainLoss::NotCurve {
            entity: id("e-5"),
            kind: EntityKind::Circle,
        }
    );
}

#[test]
fn a_closed_chain_of_two_arcs_shares_both_junctions() {
    // The two halves of one circle meet at both their ends; neither meeting point is a crossing.
    let halves = vec![
        arc("e-1", [0.0, 0.0], 10.0, 0.0, PI),
        arc("e-2", [0.0, 0.0], 10.0, PI, TAU),
    ];

    let chain = built(&halves, "e-1,e-2");

    assert!(chain.closed);
    assert_junctions(&chain, &[([-10.0, 0.0], 0.0), ([10.0, 0.0], 0.0)]);
    assert!(chain.is_smooth());
    assert_close(chain.length, 20.0 * PI, TOLERANCE, "length");
}

#[test]
fn t272_deleted_entities() {
    let one_missing = lost(&path(), "e-1,e-9,e-3");
    let all_missing = lost(&path(), "e-9,e-8");

    assert_eq!(one_missing, deleted(&["e-9"], false));
    assert_eq!(all_missing, deleted(&["e-8", "e-9"], true));
    assert_eq!(
        missing(match_chain(
            &path(),
            &chain_ref("e-1,e-9,e-3", P_FINGERPRINT, false)
        )),
        one_missing
    );
    assert_eq!(
        missing(match_chain(
            &path(),
            &chain_ref("e-9,e-8", P_FINGERPRINT, false)
        )),
        all_missing
    );
}

#[test]
fn t273_kinds_construction_and_size() {
    let path_with_point = with(path(), point("e-4", 60.0, 40.0));
    assert_eq!(
        lost(&path_with_point, "e-3,e-4"),
        ChainLoss::NotCurve {
            entity: id("e-4"),
            kind: EntityKind::Point,
        }
    );

    let construction_arc = with_construction(path(), "e-2");
    assert_eq!(
        lost(&construction_arc, P_KEY),
        ChainLoss::Construction { entity: id("e-2") }
    );
    assert_eq!(
        lost(&construction_arc, "e-1,e-2,e-9"),
        deleted(&["e-9"], false)
    );

    // All three checks run on one entry before the next entry.
    let construction_line = with_construction(path_with_point, "e-1");
    assert_eq!(
        lost(&construction_line, "e-1,e-4"),
        ChainLoss::Construction { entity: id("e-1") }
    );

    assert_eq!(
        lost(&[short_line()], "e-7"),
        ChainLoss::ZeroLength { entity: id("e-7") }
    );
}

#[test]
fn t274_ends_that_do_not_meet() {
    assert_disconnected(
        &lost(&elbow_ending_at([40.0, 0.3]), "e-1,~e-2"),
        "e-1.end",
        "e-2.end",
        0.3,
    );
    assert_disconnected(
        &lost(&elbow_ending_at([40.0, 3.0]), "e-1,~e-2"),
        "e-1.end",
        "e-2.end",
        3.0,
    );

    let chain = built(&elbow_ending_at([40.0, 0.000_000_9]), "e-1,~e-2");

    let (_, first_end) = ends(&chain.edges[0]);
    assert_point_bitwise(first_end, [40.0, 0.000_000_9], "end of edge 0");
}

#[test]
fn t275_turning_back() {
    let loss = lost(&hook(), "e-1,e-2");

    let ChainLoss::TurnsBack { entities, point } = &loss else {
        panic!("expected a turn back, got {loss:?}");
    };
    assert_eq!(entities, &[id("e-1"), id("e-2")]);
    assert_point_close(*point, [40.0, 0.0], TOLERANCE, "point");
}

#[test]
fn t276_crossing() {
    let loss = lost(&zigzag(), "e-1,e-2,e-3");

    let ChainLoss::Crosses { entities, point } = &loss else {
        panic!("expected a crossing, got {loss:?}");
    };
    assert_eq!(entities, &[id("e-1"), id("e-3")]);
    assert_point_close(*point, [30.0, 0.0], TOLERANCE, "point");

    // The arc touches both lines, but consecutive entries that meet only at their junction do not cross.
    assert!(build_chain(&path(), &chain_key(P_KEY)).is_ok());
}

#[test]
fn t277_the_order_of_the_entities_does_not_matter() {
    let mut reversed_path = path();
    reversed_path.reverse();

    assert_bitwise(
        &built(&reversed_path, P_KEY),
        &built(&path(), P_KEY),
        "chain",
    );
}

#[test]
fn t337_pieces_of_one_line_or_one_circle() {
    let line_pieces = vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        line("e-2", [40.0, 0.0], [80.0, 0.0]),
    ];

    let straight = built(&line_pieces, "e-1,e-2");

    assert!(!straight.closed);
    assert_junctions(&straight, &[([40.0, 0.0], 0.0)]);
    assert!(straight.junctions[0].is_tangent());
    assert!(straight.is_smooth());

    let arc_pieces = vec![
        arc("e-1", [0.0, 0.0], 10.0, 0.0, FRAC_PI_2),
        arc("e-2", [0.0, 0.0], 10.0, FRAC_PI_2, PI),
    ];

    let round = built(&arc_pieces, "e-1,e-2");

    assert!(!round.closed);
    assert_junctions(&round, &[([0.0, 10.0], 0.0)]);
    assert!(round.junctions[0].is_tangent());
    assert!(round.is_smooth());
}

#[test]
fn t338_a_shared_stretch() {
    // e-5 runs back over e-1 from (40, 0) to (20, 0).
    let entities = vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        line("e-2", [40.0, 0.0], [40.0, 10.0]),
        line("e-3", [40.0, 10.0], [60.0, 10.0]),
        line("e-4", [60.0, 10.0], [60.0, 0.0]),
        line("e-5", [60.0, 0.0], [20.0, 0.0]),
    ];

    let loss = lost(&entities, "e-1,e-2,e-3,e-4,e-5");

    let ChainLoss::Crosses { entities, point } = &loss else {
        panic!("expected a crossing, got {loss:?}");
    };
    assert_eq!(entities, &[id("e-1"), id("e-5")]);
    assert_point_close(*point, [20.0, 0.0], TOLERANCE, "point");
}

// ---------------------------------------------------------------------------
// Matching (3.18.3), picks (3.18.4) and mirrored drawings (3.17)
// ---------------------------------------------------------------------------

#[test]
fn t278_matching_follows_the_key() {
    // The fingerprint decides nothing.
    let (chain, successions) = resolved(match_chain(
        &path(),
        &chain_ref(P_KEY, "s=1,1|e=2,2|l=1", false),
    ));
    assert_bitwise(&chain, &built(&path(), P_KEY), "chain");
    assert!(successions.is_empty());

    let redrawn_arc = vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        arc("e-6", [40.0, 10.0], 10.0, -FRAC_PI_2, 0.0),
        line("e-3", [50.0, 10.0], [50.0, 40.0]),
    ];
    assert_eq!(
        missing(match_chain(&redrawn_arc, &path_reference())),
        deleted(&["e-2"], false)
    );

    let closed_rect = chain_ref("e-1,e-2,e-3,e-4", "s=0,0|e=0,0|l=260", true);
    assert_disconnected(
        &missing(match_chain(&opened_rect(), &closed_rect)),
        "e-4.end",
        "e-1.start",
        0.3,
    );

    let open_rect = ChainRef {
        closed: false,
        ..closed_rect
    };
    let (opened, _) = resolved(match_chain(&opened_rect(), &open_rect));
    assert!(!opened.closed);
    assert_eq!(opened.junctions.len(), 3);

    let (closed, _) = resolved(match_chain(&rect_entities(), &open_rect));
    assert!(closed.closed);
}

#[test]
fn t279_the_chain_through_a_pick() {
    for seed in ["e-1", "e-2", "e-3"] {
        assert_eq!(
            picked(&path(), seed).as_deref(),
            Some("e-1,e-2,e-3"),
            "{seed}"
        );
    }
    assert_eq!(picked(&elbow(), "e-1").as_deref(), Some("e-1,~e-2"));
    assert_eq!(picked(&elbow(), "e-2").as_deref(), Some("e-2,~e-1"));
    assert_eq!(
        picked(&rect_entities(), "e-3").as_deref(),
        Some("e-3,e-4,e-1,e-2")
    );

    // Three ends meet at (40, 0).
    let fork = with(path(), line("e-4", [40.0, 0.0], [40.0, -20.0]));
    assert_eq!(picked(&fork, "e-3").as_deref(), Some("e-2,e-3"));
    assert_eq!(picked(&fork, "e-1").as_deref(), Some("e-1"));
    assert_eq!(picked(&fork, "e-4").as_deref(), Some("e-4"));
}

#[test]
fn t280_what_a_pick_skips() {
    let construction_end = with_construction(path(), "e-3");
    assert_eq!(picked(&construction_end, "e-1").as_deref(), Some("e-1,e-2"));
    assert_eq!(picked(&construction_end, "e-3"), None);

    let path_with_point = with(path(), point("e-4", 60.0, 40.0));
    assert_eq!(picked(&path_with_point, "e-4"), None);
    assert_eq!(picked(&path_with_point, "e-9"), None);

    assert_eq!(picked(&[short_line()], "e-7"), None);

    assert_eq!(
        picked(&with(rect_entities(), small_circle()), "e-5").as_deref(),
        Some("e-5")
    );

    // e-8 crosses the interior of e-1, which is never an end.
    let crossed = with(path(), line("e-8", [20.0, -10.0], [20.0, 10.0]));
    assert_eq!(picked(&crossed, "e-1").as_deref(), Some("e-1,e-2,e-3"));
}

#[test]
fn a_picked_chain_keeps_the_direction_of_its_seed() {
    // ELBOW picked at e-2 runs from e-2's start, so e-1 runs against its own direction.
    let key = chain_through(&elbow(), &id("e-2")).expect("e-2 is a curve");

    let chain = build_chain(&elbow(), &key).expect("the picked chain builds");

    assert_point_close(chain.start(), [40.0, 30.0], TOLERANCE, "start");
    assert_point_close(chain.end(), [0.0, 0.0], TOLERANCE, "end");
}

#[test]
fn t281_mirrored_chains() {
    let path_sketch = Sketch::new(path());

    let mirrored = path_sketch.params.mirrored_chain(&path_reference());

    assert_eq!(mirrored.key.to_string(), "e-1,~e-2,e-3");
    assert_eq!(mirrored.fp.to_string(), "s=0,0|e=50,-40|l=85.707963");
    assert!(!mirrored.closed);
    let chain = built(&path_sketch.params.mirrored().entities, "e-1,~e-2,e-3");
    assert_edge(
        &chain.edges[1],
        "e-2",
        true,
        &ProfileCurve::Arc {
            center: [40.0, -10.0],
            radius: 10.0,
            start: [40.0, 0.0],
            end: [50.0, -10.0],
            ccw: false,
        },
    );

    let rect_sketch = rect();
    let closed = built(&rect_sketch.params.entities, "e-1,e-2,e-3,e-4").reference();

    let mirrored_rect = rect_sketch.params.mirrored_chain(&closed);

    assert_eq!(mirrored_rect.key.to_string(), "e-1,e-2,e-3,e-4");
    assert_eq!(mirrored_rect.fp.to_string(), closed.fp.to_string());
    assert!(mirrored_rect.closed);
    let mirrored_chain = built(
        &rect_sketch.params.mirrored().entities,
        &mirrored_rect.key.to_string(),
    );
    assert_junctions(
        &mirrored_chain,
        &[
            ([80.0, 0.0], -FRAC_PI_2),
            ([80.0, -50.0], -FRAC_PI_2),
            ([0.0, -50.0], -FRAC_PI_2),
            ([0.0, 0.0], -FRAC_PI_2),
        ],
    );

    let rect_with_circle = Sketch::new(with(rect_entities(), small_circle()));
    let circle_reference = built(&rect_with_circle.params.entities, "e-5").reference();
    assert_eq!(
        rect_with_circle
            .params
            .mirrored_chain(&circle_reference)
            .key
            .to_string(),
        "~e-5"
    );

    let unknown = chain_ref("e-1,~e-9", "s=0,0|e=40,0|l=40", false);
    assert_eq!(
        path_sketch.params.mirrored_chain(&unknown).key.to_string(),
        "e-1,~e-9"
    );
}

// ---------------------------------------------------------------------------
// Reasons, fixes and details (4.1, 4.4, 4.5)
// ---------------------------------------------------------------------------

#[test]
fn t282_reasons_for_deleted_entities() {
    assert_eq!(
        reason(&deleted(&["e-9"], false), &path()),
        "one of them is no longer in the sketch"
    );
    assert_eq!(
        reason(&deleted(&["e-8", "e-9"], false), &path()),
        "2 of them are no longer in the sketch"
    );
    assert_eq!(
        reason(&deleted(&["e-8", "e-9"], true), &path()),
        "none of them is in the sketch anymore"
    );
}

#[test]
fn t283_reasons_that_name_an_entity() {
    let path_with_point = with(path(), point("e-4", 60.0, 40.0));
    assert_eq!(
        reason(&lost(&path_with_point, "e-3,e-4"), &path_with_point),
        "the point at (60, 40) is not a line or an arc"
    );

    let construction_arc = with_construction(path(), "e-2");
    assert_eq!(
        reason(&lost(&construction_arc, P_KEY), &construction_arc),
        "the arc from (40, 0) to (50, 10) around (40, 10) is construction geometry"
    );

    let short = vec![short_line()];
    assert_eq!(
        reason(&lost(&short, "e-7"), &short),
        "the line from (10, 10) to (10, 10) has no length"
    );

    let rect_with_circle = with(rect_entities(), small_circle());
    assert_eq!(
        reason(&lost(&rect_with_circle, "e-1,e-5"), &rect_with_circle),
        "the circle at (40, 25) with radius 5 mm has no ends to join to the other curves"
    );
}

#[test]
fn t284_reasons_at_junctions() {
    let gap = elbow_ending_at([40.0, 0.3]);
    let disconnected = lost(&gap, "e-1,~e-2");
    assert_eq!(
        reason(&disconnected, &gap),
        "the end of the line from (0, 0) to (40, 0) and the end of the line from (40, 30) to (40, 0.3) are 0.3 mm apart"
    );
    let sketch = Sketch::new(gap);
    let inches = MessageContext {
        units: Units::In,
        ..context(&sketch)
    };
    assert_eq!(
        disconnected.render(&inches),
        "the end of the line from (0, 0) to (1.574803, 0) and the end of the line from (1.574803, 1.181102) to (1.574803, 0.011811) are 0.011811 in apart"
    );

    assert_eq!(
        reason(&lost(&hook(), "e-1,e-2"), &hook()),
        "the arc from (40, 0) to (30, -10) around (40, -10) turns back along the line from (0, 0) to (40, 0) at (40, 0)"
    );
    assert_eq!(
        reason(&lost(&zigzag(), "e-1,e-2,e-3"), &zigzag()),
        "the line from (0, 0) to (40, 0) and the line from (40, 20) to (20, -20) cross at (30, 0)"
    );
}

/// Every loss of tests 272 to 276, with the entities it was found on.
fn losses_of_tests_272_to_276() -> Vec<(ChainLoss, Vec<SketchEntity>)> {
    let path_with_point = with(path(), point("e-4", 60.0, 40.0));
    let construction_arc = with_construction(path(), "e-2");
    let construction_line = with_construction(path_with_point.clone(), "e-1");
    let short = vec![short_line()];
    let gap = elbow_ending_at([40.0, 0.3]);
    let wide_gap = elbow_ending_at([40.0, 3.0]);
    vec![
        (lost(&path(), "e-1,e-9,e-3"), path()),
        (lost(&path(), "e-9,e-8"), path()),
        (lost(&path_with_point, "e-3,e-4"), path_with_point),
        (lost(&construction_arc, P_KEY), construction_arc.clone()),
        (lost(&construction_arc, "e-1,e-2,e-9"), construction_arc),
        (lost(&construction_line, "e-1,e-4"), construction_line),
        (lost(&short, "e-7"), short),
        (lost(&gap, "e-1,~e-2"), gap),
        (lost(&wide_gap, "e-1,~e-2"), wide_gap),
        (lost(&hook(), "e-1,e-2"), hook()),
        (lost(&zigzag(), "e-1,e-2,e-3"), zigzag()),
    ]
}

#[test]
fn t285_fixes_and_shown_entities() {
    let connect = SketchFix::ConnectEnds {
        first: point_ref("e-1.end"),
        second: point_ref("e-2.end"),
    };
    let near = lost(&elbow_ending_at([40.0, 0.3]), "e-1,~e-2");
    assert_eq!(near.fixes(), [connect.clone()]);
    assert_eq!(near.entity(), Some(&id("e-2")));

    let far = lost(&elbow_ending_at([40.0, 3.0]), "e-1,~e-2");
    assert!(far.fixes().is_empty(), "{:?}", far.fixes());

    // A gap of exactly CONNECT_DISTANCE is offered.
    let at_limit = lost(&elbow_ending_at([40.0, CONNECT_DISTANCE]), "e-1,~e-2");
    assert_disconnected(&at_limit, "e-1.end", "e-2.end", CONNECT_DISTANCE);
    assert_eq!(at_limit.fixes(), [connect]);

    for (loss, _) in losses_of_tests_272_to_276() {
        if loss != near {
            assert!(loss.fixes().is_empty(), "{loss:?}: {:?}", loss.fixes());
        }
    }

    // Both ends belong to e-1, so they are not offered to connect.
    let open_arc = vec![arc("e-1", [0.0, 0.0], 10.0, 0.0, 6.2)];
    let closed_reference = chain_ref("e-1", "s=10,0|e=10,0|l=62.831853", true);
    let own_ends = missing(match_chain(&open_arc, &closed_reference));
    let ChainLoss::Disconnected { end, start, gap } = &own_ends else {
        panic!("expected disconnected ends, got {own_ends:?}");
    };
    assert_eq!(*end, point_ref("e-1.end"));
    assert_eq!(*start, point_ref("e-1.start"));
    assert!(*gap < CONNECT_DISTANCE, "gap {gap}");
    assert!(own_ends.fixes().is_empty(), "{:?}", own_ends.fixes());

    assert_eq!(deleted(&["e-9"], false).entity(), None);
    assert_eq!(
        lost(&with(path(), point("e-4", 60.0, 40.0)), "e-3,e-4").entity(),
        Some(&id("e-4"))
    );
    assert_eq!(
        lost(&with_construction(path(), "e-2"), P_KEY).entity(),
        Some(&id("e-2"))
    );
    assert_eq!(lost(&[short_line()], "e-7").entity(), Some(&id("e-7")));
    assert_eq!(
        lost(&with(rect_entities(), small_circle()), "e-1,e-5").entity(),
        Some(&id("e-5"))
    );
    assert_eq!(lost(&hook(), "e-1,e-2").entity(), Some(&id("e-2")));
    assert_eq!(lost(&zigzag(), "e-1,e-2,e-3").entity(), Some(&id("e-3")));
}

#[test]
fn t286_details() {
    let path_with_point = with(path(), point("e-4", 60.0, 40.0));
    let details = [
        (deleted(&["e-9"], false), "deleted entities=e-9 all=false"),
        (deleted(&["e-8", "e-9"], true), "deleted entities=e-8,e-9 all=true"),
        (
            lost(&path_with_point, "e-3,e-4"),
            "not_curve entity=e-4 kind=point",
        ),
        (
            lost(&with_construction(path(), "e-2"), P_KEY),
            "construction entity=e-2",
        ),
        (lost(&[short_line()], "e-7"), "zero_length entity=e-7"),
        (
            lost(&elbow_ending_at([40.0, 0.3]), "e-1,~e-2"),
            "disconnected end=e-1.end start=e-2.end gap=0.3",
        ),
        (
            lost(&hook(), "e-1,e-2"),
            "turns_back entities=e-1,e-2 point=40,0",
        ),
        (
            lost(&zigzag(), "e-1,e-2,e-3"),
            "crosses entities=e-1,e-3 point=30,0",
        ),
    ];

    for (loss, detail) in details {
        assert_eq!(loss.to_string(), detail);
    }
}

#[test]
fn t287_unreadable_chains() {
    let sketch = Sketch::new(path());
    let errors = [
        (
            SketchError::InvalidChainKey {
                text: "e-1,,e-2".to_owned(),
            },
            r#"invalid_chain_key text="e-1,,e-2""#,
        ),
        (
            SketchError::InvalidChainFingerprint {
                text: "s=0,0".to_owned(),
            },
            r#"invalid_chain_fingerprint text="s=0,0""#,
        ),
    ];

    for (error, detail) in errors {
        assert_eq!(
            error.render(&context(&sketch)),
            "A chain of curves stored in this document cannot be read. Pick the curves again."
        );
        assert!(error.fixes().is_empty(), "{:?}", error.fixes());
        assert_eq!(error.to_string(), detail);
    }
}

#[test]
fn t287_every_reason_passes_the_lint_and_names_no_ids() {
    let mut rendered = Vec::new();
    for (loss, entities) in losses_of_tests_272_to_276() {
        rendered.push((reason(&loss, &entities), entities));
    }
    let rect_with_circle = with(rect_entities(), small_circle());
    rendered.push((
        reason(&lost(&rect_with_circle, "e-1,e-5"), &rect_with_circle),
        rect_with_circle,
    ));
    for loss in [deleted(&["e-8", "e-9"], false), deleted(&["e-8", "e-9"], true)] {
        rendered.push((reason(&loss, &path()), path()));
    }
    let gap = elbow_ending_at([40.0, 0.3]);
    let gap_sketch = Sketch::new(gap.clone());
    let inches = MessageContext {
        units: Units::In,
        ..context(&gap_sketch)
    };
    rendered.push((lost(&gap, "e-1,~e-2").render(&inches), gap));
    let path_sketch = Sketch::new(path());
    for error in [
        SketchError::InvalidChainKey {
            text: "e-1,,e-2".to_owned(),
        },
        SketchError::InvalidChainFingerprint {
            text: "s=0,0".to_owned(),
        },
    ] {
        rendered.push((error.render(&context(&path_sketch)), path()));
    }

    for (message, entities) in &rendered {
        assert!(!message.is_empty());
        assert_plain_message(message, &Sketch::new(entities.clone()));
    }
}

// ---------------------------------------------------------------------------
// JSON of results (2.10)
// ---------------------------------------------------------------------------

#[test]
fn t288_match_results_as_json() {
    let lost_match = match_chain(&path(), &chain_ref("e-1,e-9,e-3", P_FINGERPRINT, false));
    assert_eq!(
        serde_json::to_string(&lost_match).expect("the match serializes"),
        r#"{"kind":"missing","loss":{"kind":"deleted","entities":["e-9"],"all":false}}"#
    );

    let found = match_chain(&path(), &chain_ref(P_KEY, "s=1,1|e=2,2|l=1", false));
    let json = serde_json::to_value(&found).expect("the match serializes");
    assert_eq!(json["kind"], "resolved");
    assert_eq!(json["successions"], serde_json::json!([]));
}

#[test]
fn junctions_and_losses_serialize_with_every_field() {
    let chain = built(&elbow(), "e-1,~e-2");

    assert_eq!(
        serde_json::to_value(&chain.junctions[0]).expect("the junction serializes"),
        serde_json::json!({"point": [40.0, 0.0], "turn": FRAC_PI_2})
    );
    assert_eq!(
        serde_json::to_value(missing(match_chain(
            &hook(),
            &chain_ref("e-1,e-2", "s=0,0|e=30,-10|l=55.707963", false)
        )))
        .expect("the loss serializes"),
        serde_json::json!({"kind": "turns_back", "entities": ["e-1", "e-2"], "point": [40.0, 0.0]})
    );
    let json = serde_json::to_value(&chain).expect("the chain serializes");
    for member in ["key", "edges", "closed", "junctions", "length"] {
        assert!(json.get(member).is_some(), "{member} is missing: {json}");
    }
    assert_eq!(json["key"], "e-1,~e-2");
}

// ---------------------------------------------------------------------------
// The chain scenarios of the long outline (5.1, 5.3)
// ---------------------------------------------------------------------------

#[test]
fn t302_the_curve_chain_scenarios_of_the_long_outline() {
    let outline = long_outline_benchmark();
    let key = long_outline_chain_key();

    let chain = build_chain(&outline, &key).expect("the long outline is one chain");

    assert!(chain.closed);
    assert_eq!(chain.edges.len(), LONG_OUTLINE_LINES);
    assert_eq!(chain.junctions.len(), LONG_OUTLINE_LINES);
    assert!(
        chain.junctions.iter().all(|junction| !junction.is_tangent()),
        "no junction of the polygon is tangent"
    );
    assert_eq!(chain_through(&outline, &id(LONG_OUTLINE_SEED)), Some(key));
}
