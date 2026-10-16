//! The words repairs give lost elements, regions and candidates (engine.md section 3.4): regions
//! by their centroids, and candidates by the face they keep and how far they are.

use partsmith_features::{FieldUnit, format_value, joined, sketch_coordinates};
use partsmith_model::{Dimension, LINEAR_TOLERANCE, Units};
use partsmith_naming::ElementKind;
use partsmith_sketch::RegionRef;

/// The region or regions of a sketch in words, by the centroids of their fingerprints:
/// `the region of 'Sketch 1' around (40, 25)`, or `the 2 regions of 'Sketch 1' around (40, 25)
/// and (175, 10)` (section 3.4).
pub(crate) fn region_words(sketch: &str, regions: &[RegionRef], units: Units) -> String {
    let centroids: Vec<String> = regions
        .iter()
        .map(|region| sketch_coordinates(region.fp.centroid, units))
        .collect();
    match centroids.as_slice() {
        [] => format!("a region of '{sketch}'"),
        [one] => format!("the region of '{sketch}' around {one}"),
        several => format!(
            "the {} regions of '{sketch}' around {}",
            several.len(),
            joined(several)
        ),
    }
}

/// How a candidate of `kind` relates to the lost element (section 3.4): `next to {kept}` for a
/// face and `on {kept}` for an edge or vertex that keeps a face of it, then `distance`.
pub(crate) fn relation(kind: ElementKind, kept: Option<&String>, distance: String) -> String {
    match kept {
        Some(face) if kind == ElementKind::Face => format!("next to {face}, {distance}"),
        Some(face) => format!("on {face}, {distance}"),
        None => distance,
    }
}

/// The relative difference up to which a rounded distance still equals the distance: the 1e-6
/// precision of fingerprint numbers (naming.md section 3.7.3), so the float noise of centroid
/// differences (15.3 - 15 is 0.3000000000000007) is no rounding.
const ROUNDING_TOLERANCE: f64 = 1e-6;

/// How far a candidate is from where the lost element was (section 3.4): the distance in the
/// document units rounded to 2 significant digits, preceded by `about` when rounding changed it
/// by more than `ROUNDING_TOLERANCE`, then `from where it was`; `where it was` below
/// `LINEAR_TOLERANCE`.
pub(crate) fn distance_words(millimeters: f64, units: Units) -> String {
    if millimeters < LINEAR_TOLERANCE {
        return "where it was".to_string();
    }
    let factor = Dimension::LENGTH.display_factor(units);
    let shown = millimeters / factor;
    let rounded = round_to_two_digits(shown);
    let text = format_value(FieldUnit::Length, rounded * factor, units);
    if (rounded - shown).abs() <= ROUNDING_TOLERANCE * shown {
        format!("{text} from where it was")
    } else {
        format!("about {text} from where it was")
    }
}

/// `value` rounded to the nearest number with two significant digits.
fn round_to_two_digits(value: f64) -> f64 {
    format!("{value:.1e}").parse().unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(key: &str, centroid: [f64; 2]) -> RegionRef {
        serde_json::from_value(serde_json::json!({
            "key": key,
            "fp": format!("c={},{}|a=100", centroid[0], centroid[1])
        }))
        .expect("a valid region reference")
    }

    #[test]
    fn distances_are_rounded_to_two_digits_in_the_document_units() {
        assert_eq!(distance_words(5.0, Units::Mm), "5 mm from where it was");
        assert_eq!(
            distance_words(20.615528, Units::Mm),
            "about 21 mm from where it was"
        );
        assert_eq!(
            distance_words(20.615528, Units::In),
            "about 0.81 in from where it was"
        );
    }

    #[test]
    fn candidates_relate_by_the_face_they_keep_then_their_distance() {
        let top = "the top of 'Box 1'".to_string();
        let distance = || "5 mm from where it was".to_string();

        assert_eq!(
            relation(ElementKind::Face, Some(&top), distance()),
            "next to the top of 'Box 1', 5 mm from where it was"
        );
        assert_eq!(
            relation(ElementKind::Edge, Some(&top), distance()),
            "on the top of 'Box 1', 5 mm from where it was"
        );
        assert_eq!(
            relation(ElementKind::Vertex, None, distance()),
            "5 mm from where it was"
        );
    }

    #[test]
    fn float_noise_of_a_decimal_centroid_is_no_rounding() {
        let distance = 15.3 - 15.0;
        assert_ne!(distance, 0.3);

        assert_eq!(
            distance_words(distance, Units::Mm),
            "0.3 mm from where it was"
        );
        assert_eq!(
            distance_words(0.3 + 2e-6, Units::Mm),
            "about 0.3 mm from where it was"
        );
    }

    #[test]
    fn a_candidate_closer_than_the_tolerance_is_where_it_was() {
        assert_eq!(distance_words(0.0, Units::Mm), "where it was");
        assert_eq!(
            distance_words(LINEAR_TOLERANCE / 2.0, Units::Mm),
            "where it was"
        );
    }

    #[test]
    fn two_significant_digits_round_to_nearest() {
        assert_eq!(round_to_two_digits(20.615528), 21.0);
        assert_eq!(round_to_two_digits(0.8116), 0.81);
        assert_eq!(round_to_two_digits(1234.0), 1200.0);
        assert_eq!(round_to_two_digits(5.0), 5.0);
    }

    #[test]
    fn regions_are_described_by_their_centroids() {
        let first = region("e-1,e-2,e-3,e-4", [40.0, 25.0]);
        let second = region("e-5", [175.0, 10.0]);
        let third = region("e-6", [1.0, 2.0]);

        assert_eq!(
            region_words("Sketch 1", std::slice::from_ref(&first), Units::Mm),
            "the region of 'Sketch 1' around (40, 25)"
        );
        assert_eq!(
            region_words("Sketch 1", &[first.clone(), second.clone()], Units::Mm),
            "the 2 regions of 'Sketch 1' around (40, 25) and (175, 10)"
        );
        assert_eq!(
            region_words("Sketch 1", &[first, second, third], Units::Mm),
            "the 3 regions of 'Sketch 1' around (40, 25), (175, 10) and (1, 2)"
        );
    }
}
