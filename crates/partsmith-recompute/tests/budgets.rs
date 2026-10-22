//! The release-only build-time budgets of the per-feature specs (features.md section 6.1), a test
//! target of their own (ADR 0035), so each median is timed beside only the other budgets, never
//! beside the document tests of `tests/integration.rs`. Debug builds are far slower than the
//! reference machine, so they run only in release builds:
//! `cargo test -p partsmith-recompute --release --test budgets`. Each module is named after the
//! file of the feature it times, so a budget's test id is the one it had in that file
//! (`feature_box::t010_default_box_builds_within_2_ms`).

mod feature_support;

mod feature_boolean {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t016_join_builds_within_15_ms() {
        let document = load_example("boolean/join.ptsm");

        assert_median_build_within(&document, "f-bool1", 21, Duration::from_millis(15));
    }
}

mod feature_box {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t010_default_box_builds_within_2_ms() {
        let document = load_example("box/default.ptsm");

        assert_median_build_within(&document, "f-box1", 21, Duration::from_millis(2));
    }
}

mod feature_chamfer {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t011_performance() {
        assert_median_build_within(
            &load_example("chamfer/top-loop.ptsm"),
            "f-cha1",
            5,
            Duration::from_millis(60),
        );
    }
}

mod feature_circular_pattern {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t010_six_around_builds_within_40_ms() {
        let document = load_example("circular_pattern/six-around.ptsm");

        assert_median_build_within(&document, "f-cp1", 21, Duration::from_millis(40));
    }
}

mod feature_cone {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t008_frustum_builds_within_2_ms() {
        let document = load_example("cone/frustum.ptsm");

        assert_median_build_within(&document, "f-cone1", 21, Duration::from_millis(2));
    }
}

mod feature_cylinder {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t007_default_cylinder_builds_within_2_ms() {
        let document = load_example("cylinder/default.ptsm");

        assert_median_build_within(&document, "f-cyl1", 21, Duration::from_millis(2));
    }
}

mod feature_datum_plane {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t015_xy_offset_builds_within_0_1_ms() {
        let document = load_example("datum_plane/xy-offset.ptsm");

        assert_median_build_within(&document, "f-dp1", 21, Duration::from_micros(100));
    }
}

mod feature_extrude {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t039_builds_within_budget() {
        assert_median_build_within(
            &load_example("extrude/distance.ptsm"),
            "f-ext1",
            21,
            Duration::from_millis(3),
        );
        assert_median_build_within(
            &load_example("extrude/through-all-cut.ptsm"),
            "f-ext1",
            21,
            Duration::from_millis(30),
        );
    }
}

mod feature_fillet {
    use std::time::Duration;

    use crate::feature_support::fillet::{fillet_record, plate_document};
    use crate::feature_support::{
        PLATE, PLATE_TOP_EDGES, assert_median_build_within, document, load_example,
        median_failed_build_time,
    };

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t015_performance() {
        assert_median_build_within(
            &load_example("fillet/every-edge.ptsm"),
            "f-fil1",
            5,
            Duration::from_millis(100),
        );
        let too_large =
            median_failed_build_time(&plate_document("3"), "f-fil1", "fillet_too_large", 3);
        assert!(
            too_large < Duration::from_millis(1200),
            "the failing fillet with its search took {too_large:?}, the budget is 1.2 s"
        );
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn a_failed_fillet_of_twelve_edges_with_its_search_and_limit_within_budget() {
        // features.md section 5: the search, `width_across` of the faces next to the edges and the attempt without the
        // narrowest face's edge, on a 2.4 mm plate whose every edge is rounded at 3 mm.
        let mut edges: Vec<&str> = PLATE_TOP_EDGES.to_vec();
        edges.extend([
            r#"{"q": "edge(f-box1/bottom|f-box1/front)", "fp": "line|n=1,0,0|c=0,-10,0|l=20"}"#,
            r#"{"q": "edge(f-box1/back|f-box1/bottom)", "fp": "line|n=1,0,0|c=0,10,0|l=20"}"#,
            r#"{"q": "edge(f-box1/bottom|f-box1/left)", "fp": "line|n=0,1,0|c=-10,0,0|l=20"}"#,
            r#"{"q": "edge(f-box1/bottom|f-box1/right)", "fp": "line|n=0,1,0|c=10,0,0|l=20"}"#,
            r#"{"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-10,-10,1.2|l=2.4"}"#,
            r#"{"q": "edge(f-box1/front|f-box1/right)", "fp": "line|n=0,0,1|c=10,-10,1.2|l=2.4"}"#,
            r#"{"q": "edge(f-box1/back|f-box1/left)", "fp": "line|n=0,0,1|c=-10,10,1.2|l=2.4"}"#,
            r#"{"q": "edge(f-box1/back|f-box1/right)", "fp": "line|n=0,0,1|c=10,10,1.2|l=2.4"}"#,
        ]);
        let document = document(&["b-1"], &[PLATE, &fillet_record(&edges, "3")]);

        let failed = median_failed_build_time(&document, "f-fil1", "fillet_too_large", 3);

        assert!(
            failed < Duration::from_millis(1300),
            "the failing fillet of 12 edges with its search took {failed:?}, the budget is 1.3 s"
        );
    }
}

mod feature_hole {
    use std::time::Duration;

    use crate::feature_support::hole::THROUGH_ALL;
    use crate::feature_support::{
        Run, assert_face_count, assert_median_build_within, document, load_example, output_body,
    };
    use partsmith_model::Document;

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t021_two_points_build_within_25_ms() {
        let document = load_example("hole/two-points.ptsm");

        assert_median_build_within(&document, "f-hole1", 21, Duration::from_millis(25));
    }

    /// Test 33: the 200 x 200 x 10 plate with 194 through holes of diameter 8 (200 faces), then ten
    /// holes of diameter 6 between them.
    fn ten_holes_in_a_200_face_body() -> Document {
        let plate = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["200", "200", "10"], "position": ["0", "0", "-10"], "op": "new"}"#;
        let sketch = |id: &str, points: &[[f64; 2]]| {
            let entities: Vec<String> = (1..)
                .zip(points)
                .map(|(number, [x, y])| {
                    format!(r#"{{"id": "e-p{number}", "type": "point", "position": [{x}, {y}]}}"#)
                })
                .collect();
            format!(
                r#"{{"id": "{id}", "type": "sketch", "plane": {{"kind": "origin", "plane": "XY"}}, "entities": [{}]}}"#,
                entities.join(", ")
            )
        };
        let hole = |id: &str, sketch: &str, count: usize, diameter: &str| {
            let points: Vec<String> = (1..=count)
                .map(|number| format!(r#""e-p{number}""#))
                .collect();
            format!(
                r#"{{"id": "{id}", "type": "hole", "body": "b-1", "sketch": "{sketch}", "points": [{}], "diameter": "{diameter}", {THROUGH_ALL}}}"#,
                points.join(", ")
            )
        };
        let existing: Vec<[f64; 2]> = (0..194_u32)
            .map(|index| {
                [
                    -90.0 + 13.5 * f64::from(index % 14),
                    -90.0 + 13.5 * f64::from(index / 14),
                ]
            })
            .collect();
        let new: Vec<[f64; 2]> = (0..10_u32)
            .map(|index| [-83.25 + 13.5 * f64::from(index), -83.25])
            .collect();
        document(
            &["b-1"],
            &[
                plate,
                &sketch("f-sk1", &existing),
                &hole("f-hole1", "f-sk1", existing.len(), "8"),
                &sketch("f-sk2", &new),
                &hole("f-hole2", "f-sk2", new.len(), "6"),
            ],
        )
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t033_ten_holes_in_a_200_face_body_within_80_ms() {
        let document = ten_holes_in_a_200_face_body();
        let run = Run::new(document.clone());
        run.assert_all_ok();
        let plate = output_body(&run, "f-hole1", "b-1");
        assert_face_count(plate.shape().expect("the plate is a solid"), 200);
        assert_face_count(run.shape("b-1"), 210);

        assert_median_build_within(&document, "f-hole2", 7, Duration::from_millis(80));
    }
}

mod feature_import {
    use std::time::{Duration, Instant};

    use crate::feature_support::import::{assets, block_bytes};
    use crate::feature_support::{load_example, median_build_time_with_assets, median_duration};
    use partsmith_occt_sys::read_step;

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t008_op_new_builds_within_10_ms_plus_reading() {
        let bytes = block_bytes();
        let document = load_example("import/new-body.ptsm");
        let runs = 21;
        let read_time = median_duration(runs, || {
            let start = Instant::now();
            read_step(&bytes).expect("the kernel reads its own STEP output");
            start.elapsed()
        });

        let duration = median_build_time_with_assets(&document, "f-imp1", runs, &assets(bytes));
        assert!(
            duration < Duration::from_millis(10) + read_time,
            "the import took {duration:?} to build (median of {runs} runs), over the 10 ms budget plus {read_time:?} of read_step",
        );
    }
}

mod feature_linear_pattern {
    use std::time::Duration;

    use crate::feature_support::linear_pattern::{BODIES, CUBE10, X, direction, pattern};
    use crate::feature_support::{assert_median_build_within, document};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t014_one_hundred_instances_build_within_500_ms() {
        let document = document(&BODIES, &[CUBE10, &pattern(&direction(X, "100", "20"), "")]);

        assert_median_build_within(&document, "f-lp1", 5, Duration::from_millis(500));
    }
}

mod feature_mirror {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t012_join_apart_builds_within_15_ms() {
        let document = load_example("mirror/join-apart.ptsm");

        assert_median_build_within(&document, "f-mir1", 21, Duration::from_millis(15));
    }
}

mod feature_move_body {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t009_rotate_then_translate_builds_within_5_ms() {
        let document = load_example("move_body/rotate-translate.ptsm");

        assert_median_build_within(&document, "f-mv1", 21, Duration::from_millis(5));
    }
}

mod feature_revolve {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t020_full_ring_builds_within_5_ms() {
        let document = load_example("revolve/full-ring.ptsm");

        assert_median_build_within(&document, "f-rev1", 21, Duration::from_millis(5));
    }
}

mod feature_shell {
    use std::time::Duration;

    use crate::feature_support::shell::{ROUNDED_TOP_FACE, rounded_cube_document};
    use crate::feature_support::{
        assert_median_build_within, load_example, median_failed_build_time,
    };

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t012_performance() {
        assert_median_build_within(
            &load_example("shell/open-top.ptsm"),
            "f-sh1",
            5,
            Duration::from_millis(50),
        );
        // Section 5: a wall within 0.01 mm of the radius of the rounds, on test 19 at 2 mm.
        assert_median_build_within(
            &rounded_cube_document("2", &[ROUNDED_TOP_FACE]),
            "f-sh1",
            5,
            Duration::from_millis(250),
        );
        // Section 5: a failing shell including the search, on test 20.
        let too_thick = median_failed_build_time(
            &rounded_cube_document("12", &[ROUNDED_TOP_FACE]),
            "f-sh1",
            "shell_too_thick",
            3,
        );
        assert!(
            too_thick < Duration::from_millis(1200),
            "the failing shell with its search took {too_thick:?}, the budget is 1.2 s"
        );
    }
}

mod feature_sketch {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t017_rectangle_builds_within_2_ms() {
        let document = load_example("sketch/rectangle-xy.ptsm");

        // The budget excludes the solve and the profile, which the report does not time separately;
        // for these four unconstrained lines they take a negligible part of the whole build.
        assert_median_build_within(&document, "f-sk1", 21, Duration::from_millis(2));
    }
}

mod feature_sphere {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t006_default_sphere_builds_within_2_ms() {
        let document = load_example("sphere/default.ptsm");

        assert_median_build_within(&document, "f-sph1", 21, Duration::from_millis(2));
    }
}

mod feature_split_body {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t010_split_into_two_bodies_builds_within_15_ms() {
        let document = load_example("split_body/into-two-bodies.ptsm");

        assert_median_build_within(&document, "f-split1", 21, Duration::from_millis(15));
    }
}

mod feature_torus {
    use std::time::Duration;

    use crate::feature_support::{assert_median_build_within, load_example};

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "performance budgets hold for release builds: run with cargo test --release"
    )]
    fn t006_default_torus_builds_within_2_ms() {
        let document = load_example("torus/default.ptsm");

        assert_median_build_within(&document, "f-tor1", 21, Duration::from_millis(2));
    }
}
