//! Geometric fingerprints and their canonical text form (naming.md sections 2.3 and 3.7).

mod number;
mod parse;

use std::borrow::Cow;
use std::fmt::{self, Write as _};
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize, Serializer};

use crate::error::NamingError;
use crate::kind::{ElementKind, GeometryKind};
use number::{round_to_text, write_integer, write_number};

/// Shortest direction `Fingerprint::new` normalizes.
const MIN_DIRECTION_LENGTH: f64 = 1e-12;

/// Components at most this large never decide the sign of a canonical axis.
const AXIS_SIGN_TOLERANCE: f64 = 1e-9;

/// Geometric fingerprint of an element. Values are stored already rounded to the text precision
/// (section 3.7), so `parse(display(fp)) == fp`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "String")]
pub struct Fingerprint {
    /// Surface, curve or point kind.
    pub kind: GeometryKind,
    /// Unit outward normal (planes) or canonical unit axis or direction (other kinds), if the kind has one.
    pub direction: Option<[f64; 3]>,
    /// Area centroid (faces), length centroid (edges) or position (vertices), in millimeters.
    pub centroid: Option<[f64; 3]>,
    /// Area in square millimeters (faces) or length in millimeters (edges); `None` for vertices.
    pub size: Option<f64>,
    /// Radius in millimeters for cylinders, spheres, tori (major radius) and circles.
    pub radius: Option<f64>,
    /// Adjacency counts as defined in section 3.7.2.
    pub adjacency: Option<[u32; 2]>,
}

/// A field of the text form, in canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    /// `n`: normal, axis or direction.
    Direction,
    /// `c`: centroid or position.
    Centroid,
    /// `a`: area of a face.
    Area,
    /// `l`: length of an edge.
    Length,
    /// `r`: radius.
    Radius,
    /// `adj`: adjacency counts.
    Adjacency,
}

impl Field {
    /// Every field.
    const ALL: [Field; 6] = [
        Field::Direction,
        Field::Centroid,
        Field::Area,
        Field::Length,
        Field::Radius,
        Field::Adjacency,
    ];

    /// The field name in the text form.
    fn token(self) -> &'static str {
        match self {
            Field::Direction => "n",
            Field::Centroid => "c",
            Field::Area => "a",
            Field::Length => "l",
            Field::Radius => "r",
            Field::Adjacency => "adj",
        }
    }

    /// The field named `token`, if any.
    fn from_token(token: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|field| field.token() == token)
    }

    /// Position in the canonical order; area and length share one position.
    fn rank(self) -> u8 {
        match self {
            Field::Direction => 0,
            Field::Centroid => 1,
            Field::Area | Field::Length => 2,
            Field::Radius => 3,
            Field::Adjacency => 4,
        }
    }

    /// True when fingerprints of `kind` may carry this field.
    fn applies_to(self, kind: GeometryKind) -> bool {
        match self {
            Field::Direction => kind.has_direction(),
            Field::Centroid | Field::Adjacency => true,
            Field::Area => kind.element_kind() == ElementKind::Face,
            Field::Length => kind.element_kind() == ElementKind::Edge,
            Field::Radius => kind.has_radius(),
        }
    }

    /// The size field of `kind`: area for faces, length otherwise.
    fn size_of(kind: GeometryKind) -> Field {
        if kind.element_kind() == ElementKind::Face {
            Field::Area
        } else {
            Field::Length
        }
    }
}

impl Fingerprint {
    /// Builds a fingerprint from computed geometry: drops fields the kind does not allow, normalizes and
    /// canonicalizes the direction, rounds every value, and leaves `adjacency` as `None`.
    pub fn new(
        kind: GeometryKind,
        direction: Option<[f64; 3]>,
        centroid: [f64; 3],
        size: Option<f64>,
        radius: Option<f64>,
    ) -> Result<Fingerprint, NamingError> {
        let given = Fingerprint {
            kind,
            direction: direction.filter(|_| kind.has_direction()),
            centroid: Some(centroid),
            size: size.filter(|_| kind.has_size()),
            radius: radius.filter(|_| kind.has_radius()),
            adjacency: None,
        };
        let invalid = |defect: FingerprintDefect| NamingError::InvalidFingerprint {
            text: given.to_string(),
            reason: defect.to_string(),
        };
        if kind.requires_direction() && given.direction.is_none() {
            return Err(invalid(FingerprintDefect::Required(Field::Direction, kind)));
        }
        if kind.has_size() && given.size.is_none() {
            return Err(invalid(FingerprintDefect::Required(
                Field::size_of(kind),
                kind,
            )));
        }
        let values = given
            .direction
            .into_iter()
            .flatten()
            .chain(centroid)
            .chain(given.size)
            .chain(given.radius);
        for value in values {
            if !value.is_finite() {
                return Err(invalid(FingerprintDefect::NotANumber(value.to_string())));
            }
        }
        if given.size.is_some_and(is_negative) || given.radius.is_some_and(is_negative) {
            return Err(invalid(FingerprintDefect::Negative));
        }
        let direction = match given.direction {
            Some(direction) => {
                let unit = unit_direction(direction, kind);
                Some(unit.ok_or_else(|| invalid(FingerprintDefect::NotUnit))?)
            }
            None => None,
        };
        Ok(Fingerprint {
            kind,
            direction,
            centroid: Some(centroid.map(round_to_text)),
            size: given.size.map(round_to_text),
            radius: given.radius.map(round_to_text),
            adjacency: None,
        })
    }

    /// The same fingerprint with adjacency counts set.
    pub fn with_adjacency(self, adjacency: [u32; 2]) -> Fingerprint {
        Fingerprint {
            adjacency: Some(adjacency),
            ..self
        }
    }
}

/// Why values or a text do not make a valid fingerprint. Displays as the `reason` of
/// `NamingError::InvalidFingerprint`.
#[derive(Debug, Clone, PartialEq)]
enum FingerprintDefect {
    /// The text does not start with a geometry kind.
    UnknownKind(String),
    /// A field name of the text is not one of the text form's.
    UnknownField(String),
    /// A field of the text repeats or comes after a field it should precede.
    OutOfOrder(Field),
    /// A field of the text that the geometry kind does not have.
    DoesNotApply(Field, GeometryKind),
    /// A field `Fingerprint::new` needs for the geometry kind but was not given.
    Required(Field, GeometryKind),
    /// A field with the wrong number of values; it needs `count`.
    ValueCount(Field, usize),
    /// A value that is not a finite number in the text's number format.
    NotANumber(String),
    /// A size or radius below zero.
    Negative,
    /// A direction that cannot be or is not a unit vector.
    NotUnit,
}

impl fmt::Display for FingerprintDefect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FingerprintDefect::UnknownKind(token) => write!(formatter, "unknown kind \"{token}\""),
            FingerprintDefect::UnknownField(name) => write!(formatter, "unknown field \"{name}\""),
            FingerprintDefect::OutOfOrder(field) => write!(
                formatter,
                "field \"{}\" is repeated or out of order",
                field.token()
            ),
            FingerprintDefect::DoesNotApply(field, kind) => write!(
                formatter,
                "field \"{}\" does not apply to {kind}",
                field.token()
            ),
            FingerprintDefect::Required(field, kind) => write!(
                formatter,
                "field \"{}\" is required for {kind}",
                field.token()
            ),
            FingerprintDefect::ValueCount(field, count) => write!(
                formatter,
                "field \"{}\" needs {count} values",
                field.token()
            ),
            FingerprintDefect::NotANumber(value) => {
                write!(formatter, "\"{value}\" is not a number")
            }
            FingerprintDefect::Negative => {
                formatter.write_str("a size or radius cannot be negative")
            }
            FingerprintDefect::NotUnit => formatter.write_str("the direction is not a unit vector"),
        }
    }
}

/// True for values below zero (negative zero is not negative).
fn is_negative(value: f64) -> bool {
    value < 0.0
}

/// Euclidean length of a vector.
fn length(vector: [f64; 3]) -> f64 {
    vector
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt()
}

/// The unit direction of `direction`, rounded to text precision and, for every kind but a plane,
/// canonicalized; `None` when `direction` is too short to normalize.
///
/// Rounding comes before canonicalization so the sign is decided by the components the text keeps,
/// which keeps `parse(display(fp)) == fp` even when a component smaller than the text precision
/// would otherwise decide the sign.
fn unit_direction(direction: [f64; 3], kind: GeometryKind) -> Option<[f64; 3]> {
    let largest = direction
        .iter()
        .fold(0.0_f64, |largest, component| largest.max(component.abs()));
    if largest == 0.0 {
        return None;
    }
    // Dividing by the largest component first keeps the squares of the length in range, so a
    // direction with huge components does not overflow to an infinite length.
    let scaled = direction.map(|component| component / largest);
    let scaled_length = length(scaled);
    if largest * scaled_length < MIN_DIRECTION_LENGTH {
        return None;
    }
    let unit = scaled.map(|component| round_to_text(component / scaled_length));
    Some(if kind.keeps_direction_sign() {
        unit
    } else {
        canonical_axis(unit)
    })
}

/// `axis`, negated if needed so its first component larger than 1e-9 in magnitude is positive.
fn canonical_axis(axis: [f64; 3]) -> [f64; 3] {
    let deciding = axis
        .into_iter()
        .find(|component| component.abs() > AXIS_SIGN_TOLERANCE);
    match deciding {
        // Adding zero turns the negative zeros the negation makes into positive zeros.
        Some(component) if component < 0.0 => axis.map(|component| -component + 0.0),
        _ => axis,
    }
}

/// Writes `|<field>=<x>,<y>,...` with each value in the text number format.
fn write_reals(formatter: &mut fmt::Formatter<'_>, field: Field, values: &[f64]) -> fmt::Result {
    formatter.write_char('|')?;
    formatter.write_str(field.token())?;
    formatter.write_char('=')?;
    for (position, value) in values.iter().enumerate() {
        if position > 0 {
            formatter.write_str(",")?;
        }
        write_number(formatter, *value)?;
    }
    Ok(())
}

impl fmt::Display for Fingerprint {
    /// Writes the canonical text form (section 3.7.3).
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.kind.token())?;
        if let Some(direction) = self.direction {
            write_reals(formatter, Field::Direction, &direction)?;
        }
        if let Some(centroid) = self.centroid {
            write_reals(formatter, Field::Centroid, &centroid)?;
        }
        if let Some(size) = self.size {
            write_reals(formatter, Field::size_of(self.kind), &[size])?;
        }
        if let Some(radius) = self.radius {
            write_reals(formatter, Field::Radius, &[radius])?;
        }
        if let Some([faces, edges]) = self.adjacency {
            formatter.write_str("|adj=")?;
            write_integer(formatter, u64::from(faces))?;
            formatter.write_char(',')?;
            write_integer(formatter, u64::from(edges))?;
        }
        Ok(())
    }
}

impl FromStr for Fingerprint {
    type Err = NamingError;

    fn from_str(text: &str) -> Result<Fingerprint, NamingError> {
        Fingerprint::parse(text)
    }
}

impl TryFrom<String> for Fingerprint {
    type Error = NamingError;

    fn try_from(text: String) -> Result<Fingerprint, NamingError> {
        Fingerprint::parse(&text)
    }
}

impl From<Fingerprint> for String {
    fn from(fingerprint: Fingerprint) -> String {
        fingerprint.to_string()
    }
}

impl Serialize for Fingerprint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl JsonSchema for Fingerprint {
    fn schema_name() -> Cow<'static, str> {
        "Fingerprint".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::Fingerprint").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        let kinds: Vec<&str> = GeometryKind::ALL.iter().map(|kind| kind.token()).collect();
        let pattern = format!("^({})(\\|[a-z]+=[-0-9.,]+)*$", kinds.join("|"));
        json_schema!({"type": "string", "pattern": pattern})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn parsed(text: &str) -> Fingerprint {
        Fingerprint::parse(text).expect("the test fingerprint is valid")
    }

    pub(super) fn reason(result: Result<Fingerprint, NamingError>) -> String {
        match result {
            Err(NamingError::InvalidFingerprint { reason, .. }) => reason,
            other => panic!("expected InvalidFingerprint, got {other:?}"),
        }
    }

    #[test]
    fn canonical_axis_flips_first_significant_negative_component() {
        assert_eq!(canonical_axis([0.0, -1.0, 0.5]), [0.0, 1.0, -0.5]);
        assert_eq!(canonical_axis([1e-10, -1.0, 0.0]), [-1e-10, 1.0, 0.0]);
    }

    #[test]
    fn canonical_axis_makes_positive_zeros() {
        let axis = canonical_axis([0.0, 0.0, -1.0]);

        assert!(axis[0].is_sign_positive() && axis[1].is_sign_positive());
    }

    #[test]
    fn unit_direction_decides_sign_on_rounded_components() {
        let direction = unit_direction([-1e-7, 1.0, 0.0], GeometryKind::Line);

        assert_eq!(direction, Some([0.0, 1.0, 0.0]));
    }

    #[test]
    fn unit_direction_rejects_tiny_vectors() {
        assert_eq!(unit_direction([1e-13, 0.0, 0.0], GeometryKind::Line), None);
        assert_eq!(unit_direction([5e-324, 0.0, 0.0], GeometryKind::Line), None);
    }

    /// A component of a diagonal unit vector at text precision (0.707107).
    fn diagonal() -> f64 {
        round_to_text(std::f64::consts::FRAC_1_SQRT_2)
    }

    #[test]
    fn unit_direction_keeps_vectors_just_long_enough() {
        let direction = unit_direction([9e-13, 9e-13, 0.0], GeometryKind::Line);

        assert_eq!(direction, Some([diagonal(), diagonal(), 0.0]));
    }

    #[test]
    fn unit_direction_of_huge_vectors_is_unit() {
        assert_eq!(
            unit_direction([1e200, 0.0, 0.0], GeometryKind::Line),
            Some([1.0, 0.0, 0.0])
        );
        assert_eq!(
            unit_direction([f64::MAX, -f64::MAX, 0.0], GeometryKind::Plane),
            Some([diagonal(), -diagonal(), 0.0])
        );
    }

    #[test]
    fn new_with_huge_direction_round_trips() {
        let built = Fingerprint::new(
            GeometryKind::Line,
            Some([1e200, 0.0, 0.0]),
            [0.0; 3],
            Some(1.0),
            None,
        )
        .expect("a huge direction is still a direction");
        let text = built.to_string();

        assert_eq!(text, "line|n=1,0,0|c=0,0,0|l=1");
        assert_eq!(Fingerprint::parse(&text), Ok(built));
    }

    #[test]
    fn new_drops_radius_of_plane() {
        let built = Fingerprint::new(
            GeometryKind::Plane,
            Some([0.0, 0.0, 1.0]),
            [0.0, 0.0, 0.0],
            Some(1.0),
            Some(3.0),
        )
        .expect("valid values");

        assert_eq!(built.radius, None);
    }

    #[test]
    fn new_requires_size_for_curves() {
        let result = Fingerprint::new(
            GeometryKind::Circle,
            Some([0.0, 0.0, 1.0]),
            [0.0, 0.0, 0.0],
            None,
            Some(1.0),
        );

        assert_eq!(reason(result), "field \"l\" is required for circle");
    }

    #[test]
    fn new_requires_direction_for_lines() {
        let result = Fingerprint::new(GeometryKind::Line, None, [0.0; 3], Some(1.0), None);

        assert_eq!(reason(result), "field \"n\" is required for line");
    }

    #[test]
    fn new_rejects_infinite_radius() {
        let result = Fingerprint::new(
            GeometryKind::Sphere,
            None,
            [0.0; 3],
            Some(1.0),
            Some(f64::INFINITY),
        );

        assert_eq!(reason(result), "\"inf\" is not a number");
    }

    #[test]
    fn new_rejects_negative_radius() {
        let result = Fingerprint::new(GeometryKind::Sphere, None, [0.0; 3], Some(1.0), Some(-1.0));

        assert_eq!(reason(result), "a size or radius cannot be negative");
    }

    #[test]
    fn new_error_text_shows_given_values() {
        let result = Fingerprint::new(
            GeometryKind::Plane,
            Some([0.0, 0.0, 0.0]),
            [1.0, 2.0, 3.0],
            Some(4.0),
            None,
        );

        assert_eq!(
            result,
            Err(NamingError::InvalidFingerprint {
                text: "plane|n=0,0,0|c=1,2,3|a=4".to_owned(),
                reason: "the direction is not a unit vector".to_owned()
            })
        );
    }

    #[test]
    fn new_keeps_vertex_without_size() {
        let built = Fingerprint::new(GeometryKind::Point, None, [1.0, 2.0, 3.0], None, None)
            .expect("a point needs no size");

        assert_eq!(built.to_string(), "point|c=1,2,3");
    }

    #[test]
    fn with_adjacency_sets_counts_only() {
        let built = parsed("point|c=1,2,3").with_adjacency([3, 3]);

        assert_eq!(built.to_string(), "point|c=1,2,3|adj=3,3");
    }

    #[test]
    fn display_uses_length_for_curves_and_area_for_surfaces() {
        assert_eq!(parsed("line|l=2").to_string(), "line|l=2");
        assert_eq!(parsed("cone|a=2").to_string(), "cone|a=2");
    }

    #[test]
    fn serializes_as_text_form() {
        let json = serde_json::to_string(&parsed("point|c=1,2,3|adj=3,3"))
            .expect("a fingerprint serializes");

        assert_eq!(json, "\"point|c=1,2,3|adj=3,3\"");
    }

    #[test]
    fn deserializing_invalid_text_fails() {
        assert!(serde_json::from_str::<Fingerprint>("\"plane|n=0,0,2\"").is_err());
    }

    #[test]
    fn json_schema_pattern_lists_every_kind() {
        let schema = schemars::schema_for!(Fingerprint);

        assert_eq!(
            schema.as_value()["pattern"],
            "^(plane|cylinder|cone|sphere|torus|revolution|extrusion|bspline_surface|bezier_surface|offset_surface|other_surface|line|circle|ellipse|hyperbola|parabola|bspline_curve|bezier_curve|offset_curve|other_curve|point)(\\|[a-z]+=[-0-9.,]+)*$"
        );
    }
}
