//! Native and WebAssembly agree (spec section 3.12, test 255).
//!
//! The literals below are the values the native build gives on x86_64 Linux, where the examples' expected results are blessed: the chain drag of section 5.2 (test 198) after frames 50 and 100, and the chain with every `radius` value set to 6 (test 199) after `solve`. Each is the pivots `e-k-al.center` for `k` from 1 to 24, then the free end `e-24-ar.center`. Every target computes them again and holds each coordinate within 1e-9 of its literal; CI runs this file natively and on `wasm32-unknown-emscripten` (delivery.md section 2.4, `kernel-wasm`). A failure prints every value it computed in the form of the literals, so an intended change to the solver updates them by pasting.

use crate::common;

use common::benchmark::{CHAIN_FREE_END, CHAIN_LINKS, chain_benchmark};
use common::{Sketch, TOLERANCE, arc_parts, drag_point, find, frame, id};
use partsmith_sketch::SketchEntity;

/// The points test 255 holds, one per link: the pivots `e-k-al.center` for `k` from 1 to 24, then the free end.
type ChainPoints = [[f64; 2]; CHAIN_LINKS];

/// The chain drag after frame 50, native on x86_64 Linux.
const DRAG_FRAME_50: ChainPoints = [
    [17.32050807568877, 10.000000000000005],
    [34.64101615137754, 5.944162839875529e-15],
    [51.96152422706631, 10.000000000000009],
    [69.28203230275506, -4.750075852576199e-14],
    [86.60254037844395, 9.999999999999739],
    [103.9230484541352, 4.038732166378875e-12],
    [121.24355652984451, 9.999999999968466],
    [138.56406460565265, 1.7521648436525414e-10],
    [155.88457268180562, 9.999999999371184],
    [173.20508075780788, -8.587513463610374e-11],
    [190.52558881921559, 10.00000002464966],
    [207.84609673465775, -2.52905603514291e-7],
    [225.16660368469186, 10.000001696785183],
    [242.48710611745346, -8.077045035699924e-6],
    [259.8075980836701, 10.000019825327247],
    [277.12815195773726, 9.915086497212951e-5],
    [294.4497445653102, 9.998220451253063],
    [311.77979636947504, 0.014768920596788417],
    [329.1589089026137, 9.912569058026497],
    [346.7437986897021, 0.385153292145717],
    [364.9219685052866, 8.725065890954657],
    [384.5588420742612, 4.9312292529181505],
    [394.8320647652684, 22.0910921760587],
    [391.2336046463368, 41.76470609858088],
    [383.01402506111015, 59.997602507054374],
];

/// The chain drag after frame 100, native on x86_64 Linux.
const DRAG_FRAME_100: ChainPoints = [
    [17.320508075689297, 9.99999999999909],
    [34.64101615138417, 9.654896743072283e-12],
    [51.96152422711592, 9.999999999935211],
    [69.28203230302027, 3.0860225131580586e-10],
    [86.60254037932583, 9.999999999240313],
    [103.9230484532793, -3.765289435767211e-9],
    [121.24355648763198, 10.000000067830937],
    [138.56406419933344, -5.626135538721225e-7],
    [155.88457005436592, 10.00000328367499],
    [173.20506880604498, -1.2865966174578676e-5],
    [190.52556389148623, 10.000009633768798],
    [207.8462892540406, 0.00038599510291928635],
    [225.1695461014942, 9.99562347087868],
    [242.51043918287968, 0.031014740980795843],
    [259.94082104838185, 9.838246155746805],
    [277.68886286838494, 0.6183214513134034],
    [296.1459274544009, 8.321357662291138],
    [315.5035536436724, 3.293206081796423],
    [335.0882422091216, 7.347829836766083],
    [352.7524486932907, 16.727371898329444],
    [359.57539711399187, 35.52756803689278],
    [357.55207327180796, 55.42495883643357],
    [351.218666226114, 74.39567192776343],
    [342.62479531155617, 92.45516756590119],
    [333.01546051751797, 109.99542653822216],
];

/// The chain with every `radius` value set to 6, after `solve`, native on x86_64 Linux.
const RADIUS_6: ChainPoints = [
    [17.32050807568877, 10.0],
    [34.64101615137754, 5.551115123125783e-17],
    [51.96152422706631, 10.000000000000002],
    [69.28203230275508, -3.1086244689504383e-15],
    [86.60254037844385, 10.000000000000005],
    [103.92304845413261, -5.551115123125783e-15],
    [121.24355652982138, 10.000000000000005],
    [138.56406460551014, -6.050715484207103e-15],
    [155.8845726811989, 10.000000000000007],
    [173.20508075688767, -7.216449660063518e-15],
    [190.52558883257643, 10.000000000000007],
    [207.8460969082652, -7.16093850883226e-15],
    [225.16660498395396, 10.000000000000007],
    [242.48711305964272, -8.493206138382448e-15],
    [259.8076211353315, 9.999999999999998],
    [277.1281292110203, 1.149080830487037e-14],
    [294.4486372867091, 9.999999999999984],
    [311.76914536239786, 1.765254609153999e-14],
    [329.08965343808666, 9.999999999999982],
    [346.41016151377545, 1.759703494030873e-14],
    [363.73066958946424, 9.999999999999982],
    [381.05117766515303, 1.73749903353837e-14],
    [398.3716857408418, 9.999999999999984],
    [415.6921938165306, 1.2712053631958042e-14],
    [433.0127018922194, 9.999999999999991],
];

/// The chain benchmark of section 5.2.
fn chain_sketch() -> Sketch {
    let (params, values) = chain_benchmark();
    Sketch { params, values }
}

/// The points test 255 holds, read from `entities`, a solved chain.
fn chain_points(entities: &[SketchEntity]) -> ChainPoints {
    let mut points = [[0.0; 2]; CHAIN_LINKS];
    for (k, point) in points.iter_mut().enumerate().take(CHAIN_LINKS - 1) {
        *point = arc_parts(find(entities, &format!("e-{}-al", k + 1))).center;
    }
    points[CHAIN_LINKS - 1] = arc_parts(find(entities, "e-24-ar")).center;
    points
}

/// `points` as a Rust constant named `name`, each number in the shortest form that reads back to the same `f64`, in the form of the literals of this file.
fn as_literal(name: &str, points: &ChainPoints) -> String {
    let rows: String = points
        .iter()
        .map(|[x, y]| format!("    [{x:?}, {y:?}],\n"))
        .collect();
    format!("const {name}: ChainPoints = [\n{rows}];\n")
}

/// The coordinates of `computed` that are more than [`TOLERANCE`] from those of `literal`, as `{name}[{point}][{axis}]`.
fn points_apart(name: &str, computed: &ChainPoints, literal: &ChainPoints) -> Vec<String> {
    let mut apart = Vec::new();
    for (point, (computed_point, literal_point)) in computed.iter().zip(literal).enumerate() {
        for axis in 0..2 {
            let difference = (computed_point[axis] - literal_point[axis]).abs();
            // A NaN is never within the tolerance.
            if difference.is_nan() || difference > TOLERANCE {
                apart.push(format!(
                    "{name}[{point}][{axis}]: {} computed, {} native",
                    computed_point[axis], literal_point[axis]
                ));
            }
        }
    }
    apart
}

/// Asserts that every set of computed points holds within [`TOLERANCE`] of its literal; otherwise fails with the coordinates that do not, then every computed set in the form of the literals.
#[track_caller]
fn assert_native_values(sets: &[(&str, ChainPoints, ChainPoints)]) {
    let apart: Vec<String> = sets
        .iter()
        .flat_map(|(name, computed, literal)| points_apart(name, computed, literal))
        .collect();
    let literals: String = sets
        .iter()
        .map(|(name, computed, _)| as_literal(name, computed))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        apart.is_empty(),
        "{} coordinates differ from the native values by more than {TOLERANCE}:\n{}\nThe values computed here:\n\n{literals}",
        apart.len(),
        apart.join("\n")
    );
}

#[test]
fn t255_chain_drag_frames_50_and_100_hold_the_native_values() {
    let sketch = chain_sketch();
    let start = arc_parts(find(&sketch.params.entities, "e-24-ar")).center;
    let mut session = sketch.begin_drag(&drag_point(CHAIN_FREE_END));
    let mut after_50 = None;
    let mut after_100 = None;

    // The frames of test 198.
    for i in 1..=100 {
        let step = f64::from(i);
        let result = frame(&mut session, [start[0] - step, start[1] + step]);
        match i {
            50 => after_50 = Some(chain_points(&result.entities)),
            100 => after_100 = Some(chain_points(&result.entities)),
            _ => {}
        }
    }

    assert_native_values(&[
        (
            "DRAG_FRAME_50",
            after_50.expect("frame 50 ran"),
            DRAG_FRAME_50,
        ),
        (
            "DRAG_FRAME_100",
            after_100.expect("frame 100 ran"),
            DRAG_FRAME_100,
        ),
    ]);
}

#[test]
fn t255_chain_with_radius_6_holds_the_native_values() {
    let mut sketch = chain_sketch();
    // The values of test 199.
    for k in 0..CHAIN_LINKS {
        sketch.values.insert(id(&format!("c-{k}-10")), 6.0);
    }

    let result = sketch.solve();

    assert!(!result.failed);
    assert_native_values(&[("RADIUS_6", chain_points(&result.entities), RADIUS_6)]);
}

// The helpers of test 255.

#[test]
fn chain_points_are_the_pivots_then_the_free_end() {
    let sketch = chain_sketch();

    let points = chain_points(&sketch.params.entities);

    // The unsolved chain of section 5.2: p_(k+1) = p_k + 20 u_k, with theta_k = +30 deg for even k and -30 deg for odd k.
    let step = [
        20.0 * 30_f64.to_radians().cos(),
        20.0 * 30_f64.to_radians().sin(),
    ];
    assert!(
        (points[0][0] - step[0]).abs() <= TOLERANCE && (points[0][1] - step[1]).abs() <= TOLERANCE,
        "{:?}",
        points[0]
    );
    assert!(
        (points[1][0] - 2.0 * step[0]).abs() <= TOLERANCE && points[1][1].abs() <= TOLERANCE,
        "{:?}",
        points[1]
    );
    assert_eq!(
        points[CHAIN_LINKS - 1],
        arc_parts(find(&sketch.params.entities, "e-24-ar")).center
    );
}

#[test]
fn literals_read_back_to_the_values_they_print() {
    let mut points = [[0.0; 2]; CHAIN_LINKS];
    points[0] = [1.0 / 3.0, -5.944162839875529e-15];

    let text = as_literal("SAMPLE", &points);

    assert!(
        text.starts_with("const SAMPLE: ChainPoints = [\n    [0.3333333333333333, -5.944162839875529e-15],\n    [0.0, 0.0],\n"),
        "{text}"
    );
    assert_eq!("0.3333333333333333".parse::<f64>(), Ok(1.0 / 3.0));
    assert!(text.ends_with("];\n"), "{text}");
}

#[test]
fn coordinates_within_the_tolerance_are_not_apart() {
    let mut computed = [[1.0, 2.0]; CHAIN_LINKS];
    computed[3][1] += 0.9e-9;

    assert!(points_apart("SET", &computed, &[[1.0, 2.0]; CHAIN_LINKS]).is_empty());
}

#[test]
fn coordinates_past_the_tolerance_are_named() {
    let mut computed = [[1.0, 2.0]; CHAIN_LINKS];
    computed[3][1] += 2e-9;

    let apart = points_apart("SET", &computed, &[[1.0, 2.0]; CHAIN_LINKS]);

    assert_eq!(apart.len(), 1, "{apart:?}");
    assert!(apart[0].starts_with("SET[3][1]: "), "{apart:?}");
}

#[test]
fn a_coordinate_that_is_not_a_number_is_apart() {
    let mut computed = [[1.0, 2.0]; CHAIN_LINKS];
    computed[0][0] = f64::NAN;

    assert_eq!(
        points_apart("SET", &computed, &[[1.0, 2.0]; CHAIN_LINKS]).len(),
        1
    );
}
