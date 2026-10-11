//! Role words used by the M1 naming rules (naming.md section 3.6).

/// Side face swept from a sketch curve (extrude, revolve).
pub const LATERAL: &str = "lateral";
/// Cap at the start of a sweep.
pub const START_CAP: &str = "start-cap";
/// Cap at the end of a sweep.
pub const END_CAP: &str = "end-cap";
/// Box face facing +z, or the cylinder and cone end at their height.
pub const TOP: &str = "top";
/// Box face facing -z, the cylinder and cone end at z = 0, or the flat end of a blind hole.
pub const BOTTOM: &str = "bottom";
/// Box face facing -y.
pub const FRONT: &str = "front";
/// Box face facing +y.
pub const BACK: &str = "back";
/// Box face facing -x.
pub const LEFT: &str = "left";
/// Box face facing +x.
pub const RIGHT: &str = "right";
/// Curved side of a cylinder or cone.
pub const SIDE: &str = "side";
/// The one face of a sphere or torus.
pub const SURFACE: &str = "surface";
/// Fillet face generated from an edge.
pub const FILLET: &str = "fillet";
/// Chamfer face generated from an edge.
pub const CHAMFER: &str = "chamfer";
/// Fillet or chamfer face generated from a vertex.
pub const CORNER: &str = "corner";
/// Inner offset face of a shell.
pub const INNER: &str = "inner";
/// Rim a shell leaves where a removed face was.
pub const RIM: &str = "rim";
/// Disc closing a hole tool at its start plane.
pub const ENTRY: &str = "entry";
/// Main wall of a hole.
pub const BORE: &str = "bore";
/// Conical drill-point end of a blind hole.
pub const TIP: &str = "tip";
/// Counterbore wall of a hole.
pub const COUNTERBORE: &str = "counterbore";
/// Annular step at the counterbore depth.
pub const COUNTERBORE_FLOOR: &str = "counterbore-floor";
/// Countersink cone of a hole.
pub const COUNTERSINK: &str = "countersink";
/// Section face on the side of a split plane its normal points into.
pub const SPLIT_POSITIVE: &str = "split-positive";
/// Section face on the side of a split plane its normal points away from.
pub const SPLIT_NEGATIVE: &str = "split-negative";
/// Fallback role for faces without a semantic generator, such as imported faces.
pub const FACE: &str = "face";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::is_word;

    #[test]
    fn every_role_is_a_valid_word() {
        let roles = [
            LATERAL,
            START_CAP,
            END_CAP,
            TOP,
            BOTTOM,
            FRONT,
            BACK,
            LEFT,
            RIGHT,
            SIDE,
            SURFACE,
            FILLET,
            CHAMFER,
            CORNER,
            INNER,
            RIM,
            ENTRY,
            BORE,
            TIP,
            COUNTERBORE,
            COUNTERBORE_FLOOR,
            COUNTERSINK,
            SPLIT_POSITIVE,
            SPLIT_NEGATIVE,
            FACE,
        ];

        for role in roles {
            assert!(is_word(role), "{role}");
        }
    }
}
