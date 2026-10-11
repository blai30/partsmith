//! Reads a `Document` from parsed JSON, reporting every problem with the path of the offending
//! key (file-format.md section 3.7, step 5).
//!
//! The record types also derive `Deserialize`, but serde's errors do not say where a problem is.
//! This reader walks the three record types by hand, in file order like serde, and deserializes
//! each field on its own so its error carries the field's path. Messages come from serde, so they
//! read the same as the derived implementations' messages.

use serde::de::{DeserializeOwned, Error as _, Unexpected};
use serde_json::{Map, Value};

use super::LoadError;
use crate::path::JsonPath;
use crate::{Body, BodyRole, Document, FeatureRecord, Id, Parameter, Units};

/// Keys of a `Document`, in declaration order.
const DOCUMENT_FIELDS: &[&str] = &[
    "format",
    "format_version",
    "units",
    "printer",
    "parameters",
    "bodies",
    "features",
    "rollback",
];
/// Keys of a `Parameter`, in declaration order.
const PARAMETER_FIELDS: &[&str] = &["id", "name", "expr", "comment"];
/// Keys of a `Body`, in declaration order.
const BODY_FIELDS: &[&str] = &["id", "name", "material", "color", "visible", "role"];

/// Reads the document at the root of `value`.
pub(super) fn document(value: Value) -> Result<Document, LoadError> {
    let root = JsonPath::ROOT;
    let members = object(value, &root, "struct Document")?;
    let mut format = None;
    let mut format_version = None;
    let mut units = None;
    let mut printer = None;
    let mut parameters = Vec::new();
    let mut bodies = Vec::new();
    let mut features = Vec::new();
    let mut rollback = None;
    for (key, value) in members {
        let path = root.key(&key);
        match key.as_str() {
            "format" => format = Some(field::<String>(value, &path)?),
            "format_version" => format_version = Some(field::<u32>(value, &path)?),
            "units" => units = Some(field::<Units>(value, &path)?),
            "printer" => printer = field::<Option<String>>(value, &path)?,
            "parameters" => parameters = list(value, &path, parameter)?,
            "bodies" => bodies = list(value, &path, body)?,
            "features" => features = list(value, &path, feature)?,
            "rollback" => rollback = field::<Option<Id>>(value, &path)?,
            _ => return Err(unknown_field(&key, &path, DOCUMENT_FIELDS)),
        }
    }
    Ok(Document {
        format: required(format, "format", &root)?,
        format_version: required(format_version, "format_version", &root)?,
        units: required(units, "units", &root)?,
        printer,
        parameters,
        bodies,
        features,
        rollback,
    })
}

/// Reads one parameter.
fn parameter(value: Value, path: &JsonPath<'_>) -> Result<Parameter, LoadError> {
    let mut id = None;
    let mut name = None;
    let mut expr = None;
    let mut comment = None;
    for (key, value) in object(value, path, "struct Parameter")? {
        let field_path = path.key(&key);
        match key.as_str() {
            "id" => id = Some(field(value, &field_path)?),
            "name" => name = Some(field(value, &field_path)?),
            "expr" => expr = Some(field(value, &field_path)?),
            "comment" => comment = field(value, &field_path)?,
            _ => return Err(unknown_field(&key, &field_path, PARAMETER_FIELDS)),
        }
    }
    Ok(Parameter {
        id: required(id, "id", path)?,
        name: required(name, "name", path)?,
        expr: required(expr, "expr", path)?,
        comment,
    })
}

/// Reads one body.
fn body(value: Value, path: &JsonPath<'_>) -> Result<Body, LoadError> {
    let mut id = None;
    let mut name = None;
    let mut material = None;
    let mut color = None;
    let mut visible = true;
    let mut role = BodyRole::default();
    for (key, value) in object(value, path, "struct Body")? {
        let field_path = path.key(&key);
        match key.as_str() {
            "id" => id = Some(field(value, &field_path)?),
            "name" => name = Some(field(value, &field_path)?),
            "material" => material = field(value, &field_path)?,
            "color" => color = field(value, &field_path)?,
            "visible" => visible = field(value, &field_path)?,
            "role" => role = field(value, &field_path)?,
            _ => return Err(unknown_field(&key, &field_path, BODY_FIELDS)),
        }
    }
    Ok(Body {
        id: required(id, "id", path)?,
        name: required(name, "name", path)?,
        material,
        color,
        visible,
        role,
    })
}

/// Reads one feature record; every key other than the four record keys is a param, in file order.
fn feature(value: Value, path: &JsonPath<'_>) -> Result<FeatureRecord, LoadError> {
    let mut id = None;
    let mut kind = None;
    let mut name = None;
    let mut suppressed = false;
    let mut params = Map::new();
    for (key, value) in object(value, path, "struct FeatureRecord")? {
        match key.as_str() {
            "id" => id = Some(field(value, &path.key("id"))?),
            "type" => kind = Some(field(value, &path.key("type"))?),
            "name" => name = field(value, &path.key("name"))?,
            "suppressed" => suppressed = field(value, &path.key("suppressed"))?,
            _ => {
                params.insert(key, value);
            }
        }
    }
    Ok(FeatureRecord {
        id: required(id, "id", path)?,
        kind: required(kind, "type", path)?,
        name,
        suppressed,
        params,
    })
}

/// The members of the object `value`, or `Invalid` at `path` saying it is not the `expected` type.
fn object(
    value: Value,
    path: &JsonPath<'_>,
    expected: &'static str,
) -> Result<Map<String, Value>, LoadError> {
    match value {
        Value::Object(members) => Ok(members),
        other => Err(type_error(&other, path, expected)),
    }
}

/// Reads the array `value` with `read`, giving each element its index in the path.
fn list<T>(
    value: Value,
    path: &JsonPath<'_>,
    read: fn(Value, &JsonPath<'_>) -> Result<T, LoadError>,
) -> Result<Vec<T>, LoadError> {
    let Value::Array(items) = value else {
        return Err(type_error(&value, path, "a sequence"));
    };
    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| read(item, &path.index(index)))
        .collect()
}

/// Deserializes one field value, reporting its error at `path`.
fn field<T: DeserializeOwned>(value: Value, path: &JsonPath<'_>) -> Result<T, LoadError> {
    serde_json::from_value(value).map_err(|error| invalid(path, error.to_string()))
}

/// The value of a required field, or `Invalid` at the object that lacks it.
fn required<T>(value: Option<T>, name: &'static str, path: &JsonPath<'_>) -> Result<T, LoadError> {
    value.ok_or_else(|| invalid(path, serde_json::Error::missing_field(name).to_string()))
}

/// `Invalid` for a key the object does not have, at the key's path.
fn unknown_field(key: &str, path: &JsonPath<'_>, expected: &'static [&'static str]) -> LoadError {
    invalid(
        path,
        serde_json::Error::unknown_field(key, expected).to_string(),
    )
}

/// `Invalid` at `path` for a value of the wrong JSON type, in serde's wording.
fn type_error(value: &Value, path: &JsonPath<'_>, expected: &'static str) -> LoadError {
    let unexpected = match value {
        Value::Null => Unexpected::Unit,
        Value::Bool(flag) => Unexpected::Bool(*flag),
        Value::Number(number) => match (number.as_u64(), number.as_i64()) {
            (Some(unsigned), _) => Unexpected::Unsigned(unsigned),
            (None, Some(signed)) => Unexpected::Signed(signed),
            (None, None) => Unexpected::Float(number.as_f64().unwrap_or_default()),
        },
        Value::String(text) => Unexpected::Str(text),
        Value::Array(_) => Unexpected::Seq,
        Value::Object(_) => Unexpected::Map,
    };
    invalid(
        path,
        serde_json::Error::invalid_type(unexpected, &expected).to_string(),
    )
}

/// `Invalid` at `path`.
fn invalid(path: &JsonPath<'_>, message: String) -> LoadError {
    LoadError::Invalid {
        path: path.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn error_at(value: Value) -> (String, String) {
        match document(value) {
            Err(LoadError::Invalid { path, message }) => (path, message),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    fn minimal() -> Value {
        json!({"format": "partsmith", "format_version": 1, "units": "mm"})
    }

    fn with(key: &str, value: Value) -> Value {
        let mut document = minimal();
        document[key] = value;
        document
    }

    #[test]
    fn a_minimal_document_reads() {
        assert_eq!(document(minimal()), Ok(Document::default()));
    }

    #[test]
    fn a_list_of_the_wrong_type_is_reported_at_the_list() {
        let (path, message) = error_at(with("bodies", json!({})));

        assert_eq!(path, "bodies");
        assert_eq!(message, "invalid type: map, expected a sequence");
    }

    #[test]
    fn a_record_of_the_wrong_type_is_reported_at_its_index() {
        let (path, message) = error_at(with("parameters", json!([1])));

        assert_eq!(path, "parameters[0]");
        assert_eq!(
            message,
            "invalid type: integer `1`, expected struct Parameter"
        );
    }

    #[test]
    fn missing_record_fields_are_reported_at_the_record() {
        let (path, message) = error_at(with("bodies", json!([{"id": "b-1"}])));

        assert_eq!(path, "bodies[0]");
        assert_eq!(message, "missing field `name`");
    }

    #[test]
    fn unknown_keys_name_the_expected_fields() {
        let (path, message) = error_at(with(
            "parameters",
            json!([{"id": "p-1", "name": "a", "expr": "1", "unit": "mm"}]),
        ));

        assert_eq!(path, "parameters[0].unit");
        assert!(
            message.starts_with("unknown field `unit`, expected one of"),
            "{message}"
        );
    }

    #[test]
    fn the_first_problem_in_file_order_wins() {
        let (path, _) = error_at(with("bodies", json!([{"name": 1, "id": "b 1"}])));

        assert_eq!(path, "bodies[0].name");
    }

    #[test]
    fn null_optional_fields_are_absent() {
        let read = document(with(
            "features",
            json!([{"id": "f-1", "type": "box", "name": null, "x": null}]),
        ))
        .expect("the document reads");

        assert_eq!(read.features[0].name, None);
        assert_eq!(read.features[0].params["x"], Value::Null);
    }

    #[test]
    fn null_flags_are_invalid() {
        let (path, _) = error_at(with(
            "features",
            json!([{"id": "f-1", "type": "box", "suppressed": null}]),
        ));

        assert_eq!(path, "features[0].suppressed");
    }

    #[test]
    fn invalid_units_carry_the_units_message() {
        let (path, message) = error_at(with("units", json!("yd")));

        assert_eq!(path, "units");
        assert_eq!(message, "'yd' is not a document unit. Use mm, cm, m or in.");
    }

    #[test]
    fn invalid_rollback_ids_carry_the_id_message() {
        let (path, message) = error_at(with("rollback", json!("")));

        assert_eq!(path, "rollback");
        assert_eq!(message, "An id cannot be empty.");
    }
}
