//! Native and WebAssembly agree (spec section 3.12, test 255).
//!
//! The literals below are the values the native build gives on x86_64 Linux, where the examples' expected results are blessed: the chain drag of section 5.2 (test 198) after frames 50 and 100, and the chain with every `radius` value set to 6 (test 199) after `solve`. Each is the pivots `e-k-al.center` for `k` from 1 to 24, then the free end `e-24-ar.center`. Every target computes them again and holds each coordinate within 1e-9 of its literal; CI runs this file natively and on `wasm32-unknown-emscripten` (delivery.md section 2.4, `kernel-wasm`). A failure prints every value it computed in the form of the literals, so an intended change to the solver updates them by pasting.

use crate::common;

use common::benchmark::{CHAIN_FREE_END, CHAIN_LINKS};
use common::{
    TOLERANCE, arc_parts, assert_point_close, chain_drag_cursor, chain_sketch,
    chain_with_slot_radius, drag_point, find, frame, point_ref,
};
use partsmith_sketch::SketchEntity;

/// The points test 255 holds, one per link: the pivots `e-k-al.center` for `k` from 1 to 24, then the free end.
type ChainPoints = [[f64; 2]; CHAIN_LINKS];

/// The chain drag after frame 50, native on x86_64 Linux.
const DRAG_FRAME_50: ChainPoints = [
    [17.32050807568877, 10.000000000000002],
    [34.64101615137754, -3.226250672177409e-16],
    [51.96152422706631, 10.000000000000007],
    [69.28203230275506, -4.219320415578685e-14],
    [86.60254037844395, 9.999999999999766],
    [103.92304845413513, 3.960049943549486e-12],
    [121.24355652984407, 9.999999999969036],
    [138.56406460564992, 1.7182876988601838e-10],
    [155.88457268179363, 9.999999999383874],
    [173.20508075778682, -8.886045120679593e-11],
    [190.52558881945112, 10.000000024202224],
    [207.84609673790044, -2.48144473751442e-7],
    [225.16660370948904, 10.000001664212878],
    [242.48710625262095, -7.918450583781416e-6],
    [259.80759855138956, 10.000019407926924],
    [277.1281517111678, 9.749626692236353e-5],
    [294.44972433582205, 9.998253416676846],
    [311.77959567513363, 0.01448862763526943],
    [329.1575901915683, 9.914251583724987],
    [346.73752244831326, 0.3776913103214265],
    [364.9009725817421, 8.749613386549962],
    [384.5361593159595, 4.947056100668063],
    [394.80732225836505, 22.10815198090121],
    [391.31237288808387, 41.80041769359083],
    [383.01402670689623, 59.99760018586607],
];

/// The chain drag after frame 100, native on x86_64 Linux.
const DRAG_FRAME_100: ChainPoints = [
    [17.320508075689276, 9.99999999999913],
    [34.64101615138391, 9.278254114193614e-12],
    [51.96152422711417, 9.999999999937431],
    [69.28203230301212, 2.997266586317483e-10],
    [86.60254037930733, 9.99999999924934],
    [103.92304845339476, -3.5242716995963234e-9],
    [121.24355648952928, 10.000000064985691],
    [138.56406421468668, -5.42152428840027e-7],
    [155.88457014222703, 10.000003178548834],
    [173.2050691389864, -1.2546601690687474e-5],
    [190.52556403169518, 10.000010286954733],
    [207.84627813216065, 0.00036714082516781],
    [225.16942523162058, 9.995794824044784],
    [242.50960172326495, 0.029939153876564626],
    [259.9365850100373, 9.843208411496972],
    [277.673671557875, 0.6022257367748436],
    [296.1095737967297, 8.355773869042602],
    [315.44751965919403, 3.252456338220297],
    [335.0104337063886, 7.410868773522669],
    [352.69455747389287, 16.752804685077063],
    [359.55218971821, 35.54037717333915],
    [357.5655828868879, 55.441467427336406],
    [351.26197227233115, 74.42210211693641],
    [342.70207661405465, 92.49772620905061],
    [333.01546340291617, 109.99542675791695],
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

/// The points test 255 holds, read from `entities`, a solved chain.
fn chain_points(entities: &[SketchEntity]) -> ChainPoints {
    let free_end = point_ref(CHAIN_FREE_END).entity;
    std::array::from_fn(|index| {
        let link = index + 1;
        let entity = if link < CHAIN_LINKS {
            format!("e-{link}-al")
        } else {
            free_end.to_string()
        };
        arc_parts(find(entities, &entity)).center
    })
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
        let result = frame(&mut session, chain_drag_cursor(start, i));
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
    // The values of test 199.
    let sketch = chain_with_slot_radius(6.0);

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
    assert_point_close(points[0], step, TOLERANCE, "first pivot");
    assert_point_close(points[1], [2.0 * step[0], 0.0], TOLERANCE, "second pivot");
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
