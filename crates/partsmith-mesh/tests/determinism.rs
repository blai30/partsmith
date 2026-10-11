//! Acceptance tests for deterministic results (spec section 6.12).

mod common;

use std::hash::{DefaultHasher, Hash, Hasher};
use std::process::Command;

use common::{BOX_FACE_IDS, faceted_box_data, mesh_with_face_ids, sphere};
use partsmith_mesh::{Point3, WELD_TOLERANCE};

/// Name of the fresh-process test, used to run only that test in a child process.
const FRESH_PROCESS_TEST: &str = "t089_fresh_process_gives_identical_results";
/// Set in the child process so the fresh-process test reports its digest instead of spawning
/// another child.
const CHILD_ENVIRONMENT_VARIABLE: &str = "PARTSMITH_MESH_DETERMINISM_CHILD";
/// Prefix of the line the child process writes its digest on.
const DIGEST_PREFIX: &str = "determinism digest: ";

/// Every result test 89 compares, rendered with `Debug`, which prints each `f64` as its shortest
/// round-trip form (and `-0.0` with its sign), so equal text means bit-identical numbers.
fn sphere_results() -> String {
    let mesh = sphere(10.0, 64, 32);
    let heights: Vec<f64> = (0..100_i32).map(|i| -9.9 + 0.2 * f64::from(i)).collect();

    let welded = mesh
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");
    let validity = mesh.validity();
    let shells = mesh.shells();
    let slices = mesh.slice_many(&heights).expect("every height is finite");
    let buffers = mesh.render_buffers();

    format!("{welded:?}\n{validity:?}\n{shells:?}\n{slices:?}\n{buffers:?}")
}

/// A 64-bit digest of the results text. `DefaultHasher::new` uses fixed keys, so the digest is
/// stable across runs of one test binary.
fn digest(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn t089_repeated_calls_give_identical_results() {
    let first = sphere_results();
    let second = sphere_results();

    assert!(first == second, "two runs in one process differ");
}

#[test]
fn t089_fresh_process_gives_identical_results() {
    let own_digest = digest(&sphere_results());
    if std::env::var_os(CHILD_ENVIRONMENT_VARIABLE).is_some() {
        eprintln!("{DIGEST_PREFIX}{own_digest}");
        return;
    }

    let executable = std::env::current_exe().expect("the test binary knows its own path");
    let output = Command::new(executable)
        .args([
            FRESH_PROCESS_TEST,
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENVIRONMENT_VARIABLE, "1")
        .output()
        .expect("the test binary can run itself");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the child process failed:\n{stderr}"
    );

    let child_digest = stderr
        .lines()
        .find_map(|line| line.strip_prefix(DIGEST_PREFIX))
        .unwrap_or_else(|| panic!("the child process reported no digest:\n{stderr}"));
    assert_eq!(child_digest, own_digest.to_string());
}

#[test]
fn t090_weld_picks_representatives_in_vertex_order() {
    let (positions, triangles) = faceted_box_data([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    let last = u32::try_from(positions.len() - 1).expect("24 vertices fit in u32");
    let reversed_positions: Vec<Point3> = positions.iter().rev().copied().collect();
    let renumbered: Vec<[u32; 3]> = triangles
        .iter()
        .map(|triangle| triangle.map(|vertex| last - vertex))
        .collect();
    let mesh = mesh_with_face_ids(reversed_positions, renumbered, BOX_FACE_IDS.to_vec());

    let welded = mesh
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");

    let expected: [Point3; 4] = [
        [10.0, 10.0, 10.0],
        [10.0, 0.0, 10.0],
        [10.0, 10.0, 0.0],
        [10.0, 0.0, 0.0],
    ];
    assert_eq!(welded.mesh.positions()[0..4], expected);
}
