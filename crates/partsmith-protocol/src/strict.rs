//! Strict deserialization of request values (protocol.md section 2.3).
//!
//! Requests reject unknown fields. Most request types say so with `deny_unknown_fields`; two
//! cases need more:
//!
//! - Serde ignores unknown fields of unit variants in internally tagged enums, even with
//!   `deny_unknown_fields`. Request enums mark their unit variants with
//!   `#[serde(deserialize_with = "no_fields")]`.
//! - `Orientation` and `AssetData` also travel in results and events, where unknown fields are
//!   ignored (section 3.6), and partsmith-mesh's `TessellationTolerance` accepts unknown fields
//!   too. These types stay lenient. The request fields that hold them deserialize through the
//!   strict private mirrors below (`deserialize_with`) and declare a closed schema with
//!   `closed`, `closed_or_null` or `closed_list` (`schema_with`).

use std::fmt;

use partsmith_mesh::TessellationTolerance;
use partsmith_model::Id;
use partsmith_naming::Reference;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{Error, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::{AssetData, BufferIndex, Orientation};

/// Reads the fields of a variant that has none besides its tag, and rejects any field.
pub(crate) fn no_fields<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    deserializer.deserialize_map(NoFields)
}

/// Accepts only a map without entries.
struct NoFields;

impl<'de> Visitor<'de> for NoFields {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("no fields besides the tag")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        match map.next_key::<String>()? {
            Some(name) => Err(A::Error::unknown_field(&name, &[])),
            None => Ok(()),
        }
    }
}

/// `Orientation` in a request, where unknown fields are rejected.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RequestOrientation {
    /// No rotation.
    #[serde(deserialize_with = "no_fields")]
    AsModeled,
    /// Rotation by a proper orthonormal 3 × 3 matrix, row-major.
    Matrix {
        /// The matrix rows.
        rows: [[f64; 3]; 3],
    },
    /// The rotation that turns this planar face of `body` (outward normal) to -Z.
    FaceDown {
        /// Body that owns the face.
        body: Id,
        /// The face, resolved in `body`.
        face: Reference,
    },
}

impl From<RequestOrientation> for Orientation {
    fn from(orientation: RequestOrientation) -> Orientation {
        match orientation {
            RequestOrientation::AsModeled => Orientation::AsModeled,
            RequestOrientation::Matrix { rows } => Orientation::Matrix { rows },
            RequestOrientation::FaceDown { body, face } => Orientation::FaceDown { body, face },
        }
    }
}

/// `AssetData` in a request, where unknown fields are rejected.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestAsset {
    /// File name inside the document's `assets/` folder.
    name: String,
    /// The file bytes (a `u8` buffer).
    data: BufferIndex,
}

impl From<RequestAsset> for AssetData {
    fn from(asset: RequestAsset) -> AssetData {
        AssetData {
            name: asset.name,
            data: asset.data,
        }
    }
}

/// `TessellationTolerance` in a request, where unknown fields are rejected.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestTolerance {
    /// Chord deflection in mm.
    chord_deviation: f64,
    /// Angular deflection in radians.
    angular_deviation: f64,
}

impl From<RequestTolerance> for TessellationTolerance {
    fn from(tolerance: RequestTolerance) -> TessellationTolerance {
        TessellationTolerance {
            chord_deviation: tolerance.chord_deviation,
            angular_deviation: tolerance.angular_deviation,
        }
    }
}

/// Reads a request's orientation, rejecting unknown fields.
pub(crate) fn orientation<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Orientation, D::Error> {
    RequestOrientation::deserialize(deserializer).map(Orientation::from)
}

/// Reads a request's optional orientation, rejecting unknown fields; `null` is absent.
pub(crate) fn optional_orientation<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Orientation>, D::Error> {
    Ok(Option::<RequestOrientation>::deserialize(deserializer)?.map(Orientation::from))
}

/// Reads a request's assets, rejecting unknown fields.
pub(crate) fn assets<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<AssetData>, D::Error> {
    let assets = Vec::<RequestAsset>::deserialize(deserializer)?;
    Ok(assets.into_iter().map(AssetData::from).collect())
}

/// Reads a request's optional tessellation tolerance, rejecting unknown fields; `null` is absent.
pub(crate) fn optional_tolerance<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<TessellationTolerance>, D::Error> {
    Ok(Option::<RequestTolerance>::deserialize(deserializer)?.map(TessellationTolerance::from))
}

/// The schema of `T` with unknown properties rejected, for a request field of a type that
/// accepts them elsewhere. Wrapping the reference in `allOf` keeps it a reference for the
/// TypeScript generator, while `unevaluatedProperties` still sees the properties it defines.
pub(crate) fn closed<T: JsonSchema>(generator: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "allOf": [generator.subschema_for::<T>()],
        "unevaluatedProperties": false
    })
}

/// `closed::<T>`, or `null`.
pub(crate) fn closed_or_null<T: JsonSchema>(generator: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "anyOf": [closed::<T>(generator), { "type": "null" }]
    })
}

/// A list of `closed::<T>` items.
pub(crate) fn closed_list<T: JsonSchema>(generator: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "array",
        "items": closed::<T>(generator)
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    /// The reference of the top face of the box fixture (protocol.md section 2.4).
    fn top_face() -> Value {
        json!({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"})
    }

    /// Every orientation variant; the match makes a new variant fail to compile here until
    /// `RequestOrientation` has it too.
    fn every_orientation() -> Vec<Orientation> {
        let samples = vec![
            Orientation::AsModeled,
            Orientation::Matrix {
                rows: [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
            },
            Orientation::FaceDown {
                body: Id::new("b-1").expect("a valid id"),
                face: serde_json::from_value(top_face()).expect("a valid reference"),
            },
        ];
        for sample in &samples {
            match sample {
                Orientation::AsModeled | Orientation::Matrix { .. } | Orientation::FaceDown { .. } => {}
            }
        }
        samples
    }

    /// A request-style enum with one field-less variant.
    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
    enum Shape {
        #[serde(deserialize_with = "no_fields")]
        Empty,
        Square {
            side: f64,
        },
    }

    #[test]
    fn field_less_variant_without_fields_deserializes() {
        let shape: Shape = serde_json::from_str(r#"{"kind":"empty"}"#).expect("deserializes");

        assert_eq!(shape, Shape::Empty);
    }

    #[test]
    fn field_less_variant_with_a_field_is_rejected() {
        let error = serde_json::from_str::<Shape>(r#"{"kind":"empty","side":1}"#)
            .expect_err("an unknown field is rejected");

        assert!(
            error.to_string().contains("unknown field `side`"),
            "{error}"
        );
    }

    #[test]
    fn other_variants_are_unaffected() {
        let shape: Shape =
            serde_json::from_str(r#"{"kind":"square","side":2.0}"#).expect("deserializes");

        assert_eq!(shape, Shape::Square { side: 2.0 });
    }

    #[test]
    fn field_less_variant_accepts_an_empty_map() {
        let value = serde_json::json!({});

        assert!(no_fields(&value).is_ok());
    }

    #[test]
    fn request_orientation_reads_every_variant() {
        for sample in every_orientation() {
            let text = serde_json::to_value(&sample).expect("serializes");

            let read = orientation(&text).expect("a request orientation deserializes");

            assert_eq!(read, sample);
        }
    }

    #[test]
    fn request_orientation_rejects_unknown_fields_in_every_variant() {
        for sample in every_orientation() {
            let mut text = serde_json::to_value(&sample).expect("serializes");
            text["note"] = json!("x");

            let error = orientation(&text).expect_err("an unknown field is rejected");

            assert!(error.to_string().contains("unknown field `note`"), "{error}");
        }
    }

    #[test]
    fn optional_request_orientation_reads_null_as_absent() {
        assert_eq!(optional_orientation(&Value::Null).expect("null deserializes"), None);
    }

    #[test]
    fn optional_request_orientation_rejects_unknown_fields() {
        let text = json!({"kind": "as_modeled", "note": "x"});

        assert!(optional_orientation(&text).is_err());
    }

    #[test]
    fn request_assets_keep_their_order_and_contents() {
        let text = json!([{"name": "b.step", "data": 1}, {"name": "a.step", "data": 0}]);

        let read = assets(&text).expect("request assets deserialize");

        assert_eq!(
            read,
            [
                AssetData {
                    name: "b.step".to_string(),
                    data: BufferIndex(1)
                },
                AssetData {
                    name: "a.step".to_string(),
                    data: BufferIndex(0)
                }
            ]
        );
    }

    #[test]
    fn request_assets_reject_unknown_fields() {
        let text = json!([{"name": "a.step", "data": 0, "size": 14}]);

        let error = assets(&text).expect_err("an unknown field is rejected");

        assert!(error.to_string().contains("unknown field `size`"), "{error}");
    }

    #[test]
    fn optional_request_tolerance_reads_both_deviations() {
        let text = json!({"chord_deviation": 0.05, "angular_deviation": 0.1});

        let read = optional_tolerance(&text).expect("a tolerance deserializes");

        assert_eq!(
            read,
            Some(TessellationTolerance {
                chord_deviation: 0.05,
                angular_deviation: 0.1
            })
        );
    }

    #[test]
    fn optional_request_tolerance_rejects_unknown_fields() {
        let text = json!({"chord_deviation": 0.05, "angular_deviation": 0.1, "extra": 1});

        assert!(optional_tolerance(&text).is_err());
    }

    #[test]
    fn closed_schema_wraps_the_reference() {
        let mut generator = SchemaGenerator::default();

        let schema = closed::<AssetData>(&mut generator);

        assert_eq!(
            schema.as_value(),
            &json!({
                "allOf": [{"$ref": "#/$defs/AssetData"}],
                "unevaluatedProperties": false
            })
        );
    }

    #[test]
    fn closed_or_null_schema_allows_null() {
        let mut generator = SchemaGenerator::default();

        let schema = closed_or_null::<AssetData>(&mut generator);

        assert_eq!(schema.as_value()["anyOf"][1], json!({"type": "null"}));
        assert_eq!(schema.as_value()["anyOf"][0]["unevaluatedProperties"], false);
    }

    #[test]
    fn closed_list_schema_closes_each_item() {
        let mut generator = SchemaGenerator::default();

        let schema = closed_list::<AssetData>(&mut generator);

        assert_eq!(schema.as_value()["type"], "array");
        assert_eq!(schema.as_value()["items"]["unevaluatedProperties"], false);
    }
}
