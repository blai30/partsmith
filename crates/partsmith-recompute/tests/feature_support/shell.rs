//! The shell records that `feature_shell.rs` shares with the shell budget of `budgets.rs`.

use partsmith_model::Document;

use super::{CUBE20, document};

/// `F-ROUND`: `CUBE20`'s four edges parallel to Z rounded at radius 2 by `f-fil1`, which gives the
/// body of ui.md's `cube-features.ptsm` (volume 7680 + 80 pi, 10 faces).
const VERTICAL_ROUNDS: &str = r#"{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-10,-10,10|l=20"}, {"q": "edge(f-box1/front|f-box1/right)", "fp": "line|n=0,0,1|c=10,-10,10|l=20"}, {"q": "edge(f-box1/back|f-box1/left)", "fp": "line|n=0,0,1|c=-10,10,10|l=20"}, {"q": "edge(f-box1/back|f-box1/right)", "fp": "line|n=0,0,1|c=10,10,10|l=20"}], "radius": "2"}"#;

/// The top face of `CUBE20` after `VERTICAL_ROUNDS`: a 20 mm square with corners of radius 2.
pub const ROUNDED_TOP_FACE: &str =
    r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=396.566371"}"#;

/// `SHELL(thickness, faces)`: the shell `f-sh1` of `b-1`, without `remove_faces` when `faces` is
/// empty.
pub fn shell_record(thickness: &str, faces: &[&str]) -> String {
    if faces.is_empty() {
        format!(r#"{{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "{thickness}"}}"#)
    } else {
        format!(
            r#"{{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "{thickness}", "remove_faces": [{}]}}"#,
            faces.join(", "),
        )
    }
}

/// Tests 12, 19, 20 and 22: `CUBE20` with `VERTICAL_ROUNDS`, shelled with a wall of `thickness`
/// and `faces` removed (a closed hollow when empty).
pub fn rounded_cube_document(thickness: &str, faces: &[&str]) -> Document {
    document(
        &["b-1"],
        &[CUBE20, VERTICAL_ROUNDS, &shell_record(thickness, faces)],
    )
}
