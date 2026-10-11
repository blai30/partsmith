//! Fingerprint similarity (naming.md section 3.9.1).

use crate::fingerprint::Fingerprint;
use crate::kind::{ElementKind, GeometryKind};

/// Weight of the direction similarity.
const DIRECTION_WEIGHT: f64 = 0.25;
/// Weight of the centroid similarity.
const CENTROID_WEIGHT: f64 = 0.35;
/// Weight of the size similarity.
const SIZE_WEIGHT: f64 = 0.2;
/// Weight of the radius similarity.
const RADIUS_WEIGHT: f64 = 0.1;
/// Weight of the adjacency similarity.
const ADJACENCY_WEIGHT: f64 = 0.1;
/// Smallest length scale for centroid distances, in millimeters.
const MIN_LENGTH_SCALE: f64 = 1.0;

/// Similarity of a candidate fingerprint to a query fingerprint, in [0, 1] (section 3.9.1).
pub fn score(query: &Fingerprint, candidate: &Fingerprint, kind: ElementKind) -> f64 {
    if query.kind != candidate.kind {
        return 0.0;
    }
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;
    let mut add = |weight: f64, similarity: f64| {
        weighted_sum += weight * similarity.clamp(0.0, 1.0);
        weight_sum += weight;
    };
    if let (Some(first), Some(second)) = (query.direction, candidate.direction) {
        add(
            DIRECTION_WEIGHT,
            direction_similarity(first, second, query.kind),
        );
    }
    if let (Some(first), Some(second)) = (query.centroid, candidate.centroid) {
        let scale = length_scale(query, candidate, kind);
        add(CENTROID_WEIGHT, (-distance(first, second) / scale).exp());
    }
    if let (Some(first), Some(second)) = (query.size, candidate.size) {
        add(SIZE_WEIGHT, ratio(first, second));
    }
    if let (Some(first), Some(second)) = (query.radius, candidate.radius) {
        add(RADIUS_WEIGHT, ratio(first, second));
    }
    if let (Some(first), Some(second)) = (query.adjacency, candidate.adjacency) {
        let faces = ratio(f64::from(first[0]), f64::from(second[0]));
        let edges = ratio(f64::from(first[1]), f64::from(second[1]));
        add(ADJACENCY_WEIGHT, (faces + edges) / 2.0);
    }
    if weight_sum == 0.0 {
        1.0
    } else {
        weighted_sum / weight_sum
    }
}

/// Direction similarity: the dot product for signed plane normals (opposite normals score 0), its
/// absolute value for axes.
fn direction_similarity(first: [f64; 3], second: [f64; 3], kind: GeometryKind) -> f64 {
    let dot: f64 = first
        .iter()
        .zip(second)
        .map(|(first, second)| first * second)
        .sum();
    if kind.keeps_direction_sign() {
        dot.max(0.0)
    } else {
        dot.abs()
    }
}

/// The length scale `L` that centroid distances are measured in.
fn length_scale(query: &Fingerprint, candidate: &Fingerprint, kind: ElementKind) -> f64 {
    let size = query.size.or(candidate.size).unwrap_or(0.0);
    let size_scale = match kind {
        ElementKind::Face => size.sqrt(),
        ElementKind::Edge => size,
        ElementKind::Vertex => 0.0,
    };
    size_scale.max(MIN_LENGTH_SCALE)
}

/// Euclidean distance between two points.
fn distance(first: [f64; 3], second: [f64; 3]) -> f64 {
    first
        .iter()
        .zip(second)
        .map(|(first, second)| (first - second).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// `min / max` of two non-negative values, 1 when both are 0.
fn ratio(first: f64, second: f64) -> f64 {
    let larger = first.max(second);
    if larger == 0.0 {
        1.0
    } else {
        first.min(second) / larger
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(text: &str) -> Fingerprint {
        Fingerprint::parse(text).expect("the test fingerprint is valid")
    }

    #[test]
    fn different_kinds_score_zero() {
        let plane = fingerprint("plane|c=0,0,0");
        let cylinder = fingerprint("cylinder|c=0,0,0");

        assert_eq!(score(&plane, &cylinder, ElementKind::Face), 0.0);
    }

    #[test]
    fn nothing_in_common_scores_one() {
        let query = fingerprint("plane|n=0,0,1");
        let candidate = fingerprint("plane|c=0,0,0");

        assert_eq!(score(&query, &candidate, ElementKind::Face), 1.0);
    }

    #[test]
    fn opposite_plane_normals_score_zero_direction() {
        assert_eq!(
            direction_similarity([0.0, 0.0, 1.0], [0.0, 0.0, -1.0], GeometryKind::Plane),
            0.0
        );
    }

    #[test]
    fn opposite_axes_score_full_direction() {
        assert_eq!(
            direction_similarity([0.0, 0.0, 1.0], [0.0, 0.0, -1.0], GeometryKind::Line),
            1.0
        );
    }

    #[test]
    fn only_direction_present_scores_the_direction() {
        let query = fingerprint("line|n=1,0,0");
        let candidate = fingerprint("line|n=0,1,0");

        assert_eq!(score(&query, &candidate, ElementKind::Edge), 0.0);
    }

    #[test]
    fn length_scale_of_face_is_root_of_query_area() {
        let query = fingerprint("plane|a=400");
        let candidate = fingerprint("plane|a=9");

        assert_eq!(length_scale(&query, &candidate, ElementKind::Face), 20.0);
    }

    #[test]
    fn length_scale_falls_back_to_candidate_size() {
        let query = fingerprint("line|c=0,0,0");
        let candidate = fingerprint("line|l=7");

        assert_eq!(length_scale(&query, &candidate, ElementKind::Edge), 7.0);
    }

    #[test]
    fn length_scale_is_at_least_one_millimeter() {
        let query = fingerprint("line|l=0.5");

        assert_eq!(length_scale(&query, &query, ElementKind::Edge), 1.0);
    }

    #[test]
    fn length_scale_of_vertex_is_one_millimeter() {
        let query = fingerprint("point|c=0,0,0");

        assert_eq!(length_scale(&query, &query, ElementKind::Vertex), 1.0);
    }

    #[test]
    fn centroid_similarity_decays_with_distance() {
        let query = fingerprint("point|c=0,0,0");
        let candidate = fingerprint("point|c=0,0,1");

        let vertex_score = score(&query, &candidate, ElementKind::Vertex);

        assert!((vertex_score - (-1.0_f64).exp()).abs() < 1e-12);
    }

    #[test]
    fn ratio_of_zeros_is_one() {
        assert_eq!(ratio(0.0, 0.0), 1.0);
        assert_eq!(ratio(2.0, 4.0), 0.5);
        assert_eq!(ratio(4.0, 2.0), 0.5);
    }

    #[test]
    fn adjacency_similarity_is_mean_of_ratios() {
        let query = fingerprint("point|adj=2,4");
        let candidate = fingerprint("point|adj=4,4");

        let vertex_score = score(&query, &candidate, ElementKind::Vertex);

        assert!((vertex_score - 0.75).abs() < 1e-12, "{vertex_score}");
    }

    #[test]
    fn radius_similarity_is_ratio() {
        let query = fingerprint("sphere|r=1");
        let candidate = fingerprint("sphere|r=4");

        let face_score = score(&query, &candidate, ElementKind::Face);

        assert!((face_score - 0.25).abs() < 1e-12, "{face_score}");
    }

    #[test]
    fn similarity_is_clamped_to_one() {
        let query = Fingerprint {
            kind: GeometryKind::Plane,
            direction: Some([0.0, 0.0, 1.00005]),
            centroid: None,
            size: None,
            radius: None,
            adjacency: None,
        };

        assert_eq!(score(&query, &query, ElementKind::Face), 1.0);
    }
}
