//! Content-hash cache keys (recompute.md section 3.4).

use std::fmt::{self, Write};

use partsmith_features::{OCCT_VERSION, VERSION, Values};
use partsmith_model::canonical::{format_number, format_string};
use partsmith_model::{FieldUnit, Id};
use serde_json::{Map, Value};

/// First item of every key: the key scheme and its version.
const SCHEME: &str = "partsmith-recompute 1";

/// A 32-byte BLAKE3 hash. `Display` writes 64 lowercase hex characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheKey(pub [u8; 32]);

impl fmt::Display for CacheKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Everything a feature's key is made of, in the order the key hashes it.
pub(crate) struct KeyInput<'k> {
    /// The feature type name.
    pub(crate) type_name: &'k str,
    /// The manifest version of the type.
    pub(crate) version: u32,
    /// The feature id.
    pub(crate) feature: &'k Id,
    /// The known canonical params, written compactly (see [`params_text`]).
    pub(crate) params: &'k str,
    /// The evaluated values of the declared expressions.
    pub(crate) values: &'k Values,
    /// Each feature link in order, with the key of its record when that has one.
    pub(crate) features: Vec<(&'k Id, Option<CacheKey>)>,
    /// Each body link (read, then written) in order, with the key of its provider when it has one.
    pub(crate) bodies: Vec<(&'k Id, Option<CacheKey>)>,
}

impl KeyInput<'_> {
    /// The key: the BLAKE3 hash of [`KeyInput::preimage`].
    pub(crate) fn key(&self) -> CacheKey {
        CacheKey(*blake3::hash(self.preimage().as_bytes()).as_bytes())
    }

    /// The hashed byte sequence: one item per line, each followed by a line feed.
    fn preimage(&self) -> String {
        let mut text = String::new();
        let mut item = |arguments: fmt::Arguments<'_>| {
            text.write_fmt(arguments)
                .expect("writing to a String never fails");
            text.push('\n');
        };
        item(format_args!("{SCHEME}"));
        item(format_args!(
            "partsmith-features {VERSION} occt {OCCT_VERSION}"
        ));
        item(format_args!("type {}", self.type_name));
        item(format_args!("version {}", self.version));
        item(format_args!("id {}", self.feature));
        item(format_args!("params {}", self.params));
        for (declared, value) in self.values.iter() {
            item(format_args!(
                "expr {} {}",
                declared.field,
                unit_name(declared.kind)
            ));
            match value {
                Ok(value) => item(format_args!("value {:016x}", value.to_bits())),
                Err(error) => item(format_args!("error {} {error}", error.code())),
            }
        }
        for (feature, key) in &self.features {
            match key {
                Some(key) => item(format_args!("feature {feature} {key}")),
                None => item(format_args!("feature {feature} none")),
            }
        }
        for (body, key) in &self.bodies {
            match key {
                Some(key) => item(format_args!("body {body} {key}")),
                None => item(format_args!("body {body} empty")),
            }
        }
        text
    }
}

/// The snake_case name of a value kind, as the manifest serializes it.
fn unit_name(kind: FieldUnit) -> &'static str {
    match kind {
        FieldUnit::Length => "length",
        FieldUnit::Angle => "angle",
        FieldUnit::Scalar => "scalar",
        FieldUnit::Count => "count",
    }
}

/// The known keys of canonical params written compactly: no spaces, keys in their canonical
/// order, numbers in canonical form (key item 5). Unknown keys are left out.
pub(crate) fn params_text(params: &Map<String, Value>, unknown_keys: &[String]) -> String {
    let mut text = String::new();
    text.push('{');
    let known = params.iter().filter(|(key, _)| !unknown_keys.contains(key));
    for (position, (key, value)) in known.enumerate() {
        if position > 0 {
            text.push(',');
        }
        text.push_str(&format_string(key));
        text.push(':');
        write_compact(&mut text, value);
    }
    text.push('}');
    text
}

/// Appends `value` as compact canonical JSON.
fn write_compact(text: &mut String, value: &Value) {
    match value {
        Value::Null => text.push_str("null"),
        Value::Bool(flag) => text.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => text.push_str(&format_number(number)),
        Value::String(string) => text.push_str(&format_string(string)),
        Value::Array(items) => {
            text.push('[');
            for (position, item) in items.iter().enumerate() {
                if position > 0 {
                    text.push(',');
                }
                write_compact(text, item);
            }
            text.push(']');
        }
        Value::Object(members) => {
            text.push('{');
            for (position, (key, member)) in members.iter().enumerate() {
                if position > 0 {
                    text.push(',');
                }
                text.push_str(&format_string(key));
                text.push(':');
                write_compact(text, member);
            }
            text.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use partsmith_features::{ExprUse, FeatureManifest};
    use partsmith_model::{Category, EmptyContext, Expr, ParameterValues, Scope, Units};
    use serde_json::json;

    use super::*;
    use crate::test_support::id;

    fn object(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(members) => members,
            other => panic!("not an object: {other}"),
        }
    }

    fn manifest() -> FeatureManifest {
        FeatureManifest {
            label: "Test".to_string(),
            category: Category::Create,
            icon: "test".to_string(),
            version: 1,
            fields: Vec::new(),
        }
    }

    /// Values of length uses `(field, expr)` evaluated in an mm document without parameters.
    fn values(uses: &[(&str, &str)]) -> Values {
        let uses: Vec<ExprUse> = uses
            .iter()
            .map(|(field, expr)| ExprUse {
                field: field.to_string(),
                expr: Expr::new(*expr),
                kind: FieldUnit::Length,
            })
            .collect();
        let parameters = ParameterValues::default();
        let scope = Scope {
            units: Units::Mm,
            parameters: &parameters,
            context: &EmptyContext,
        };
        Values::evaluate(&uses, &scope, &manifest())
    }

    fn input<'k>(feature: &'k Id, params: &'k str, values: &'k Values) -> KeyInput<'k> {
        KeyInput {
            type_name: "test_datum",
            version: 1,
            feature,
            params,
            values,
            features: Vec::new(),
            bodies: Vec::new(),
        }
    }

    #[test]
    fn cache_keys_display_as_lowercase_hex() {
        let mut bytes = [0_u8; 32];
        bytes[0] = 0x0a;
        bytes[31] = 0xff;

        let text = CacheKey(bytes).to_string();

        assert_eq!(text.len(), 64);
        assert!(text.starts_with("0a00"));
        assert!(text.ends_with("00ff"));
    }

    #[test]
    fn params_text_is_compact_with_canonical_numbers() {
        let params = object(
            json!({"size": [80.0, 2.5, -0.0], "name": "a \"b\"", "flag": true, "none": null}),
        );

        assert_eq!(
            params_text(&params, &[]),
            r#"{"size":[80,2.5,0],"name":"a \"b\"","flag":true,"none":null}"#
        );
    }

    #[test]
    fn params_text_keeps_nested_key_order_and_leaves_out_unknown_keys() {
        let params = object(json!({"extent": {"kind": "distance", "expr": "h"}, "taper": "2 deg"}));

        assert_eq!(
            params_text(&params, &["taper".to_string()]),
            r#"{"extent":{"kind":"distance","expr":"h"}}"#
        );
    }

    #[test]
    fn params_text_of_no_params_is_an_empty_object() {
        assert_eq!(params_text(&Map::new(), &[]), "{}");
    }

    #[test]
    fn preimage_lists_every_item_on_its_own_line() {
        let feature = id("f-n");
        let values = values(&[("values[0]", "3")]);
        let upstream = CacheKey([0x11; 32]);
        let provider = CacheKey([0x22; 32]);
        let datum = id("f-d1");
        let missing = id("f-s");
        let read = id("b-2");
        let written = id("b-1");
        let key = KeyInput {
            features: vec![(&datum, Some(upstream)), (&missing, None)],
            bodies: vec![(&read, None), (&written, Some(provider))],
            ..input(&feature, r#"{"values":["3"]}"#, &values)
        };

        let expected = format!(
            "partsmith-recompute 1\npartsmith-features {VERSION} occt {OCCT_VERSION}\ntype test_datum\nversion 1\nid f-n\nparams {{\"values\":[\"3\"]}}\nexpr values[0] length\nvalue {:016x}\nfeature f-d1 {}\nfeature f-s none\nbody b-2 empty\nbody b-1 {}\n",
            3.0_f64.to_bits(),
            "11".repeat(32),
            "22".repeat(32)
        );
        assert_eq!(key.preimage(), expected);
    }

    #[test]
    fn preimage_records_evaluation_errors_with_code_and_display_text() {
        let feature = id("f-e");
        let values = values(&[("offset", "nope")]);

        let preimage = input(&feature, "{}", &values).preimage();

        assert!(preimage.ends_with(
            "expr offset length\nerror expression offset: Unknown name 'nope'. Check the spelling, or add a parameter named nope.\n"
        ));
    }

    #[test]
    fn values_write_their_bits_as_sixteen_hex_digits() {
        let feature = id("f-z");
        let values = values(&[("offset", "0")]);

        let preimage = input(&feature, "{}", &values).preimage();

        assert!(preimage.ends_with("value 0000000000000000\n"));
    }

    #[test]
    fn the_key_is_the_blake3_hash_of_the_preimage() {
        let feature = id("f-d1");
        let values = values(&[("offset", "5")]);
        let key = input(&feature, r#"{"offset":"5"}"#, &values);

        assert_eq!(
            key.key().0,
            *blake3::hash(key.preimage().as_bytes()).as_bytes()
        );
    }

    #[test]
    fn keys_change_with_every_item() {
        let feature = id("f-d1");
        let other_feature = id("f-d2");
        let five = values(&[("offset", "5")]);
        let six = values(&[("offset", "6")]);
        let base = input(&feature, r#"{"offset":"5"}"#, &five).key();

        let changed = [
            input(&other_feature, r#"{"offset":"5"}"#, &five).key(),
            input(&feature, r#"{"offset":"6"}"#, &five).key(),
            input(&feature, r#"{"offset":"5"}"#, &six).key(),
            KeyInput {
                version: 2,
                ..input(&feature, r#"{"offset":"5"}"#, &five)
            }
            .key(),
            KeyInput {
                type_name: "test_block",
                ..input(&feature, r#"{"offset":"5"}"#, &five)
            }
            .key(),
        ];

        for key in changed {
            assert_ne!(key, base);
        }
    }

    #[test]
    fn unit_names_are_snake_case() {
        assert_eq!(unit_name(FieldUnit::Length), "length");
        assert_eq!(unit_name(FieldUnit::Angle), "angle");
        assert_eq!(unit_name(FieldUnit::Scalar), "scalar");
        assert_eq!(unit_name(FieldUnit::Count), "count");
    }
}
