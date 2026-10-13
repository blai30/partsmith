//! A small JSON Schema (draft 2020-12) validator for the keywords `schemars` emits, so test 41 can check the spec examples without a validator dependency.
//!
//! Supported: boolean schemas, local `$ref`, `type`, `const`, `enum`, `properties`, `patternProperties` (patterns of the form `^[...]+$` only, which `schemars` emits for maps keyed by element names), `required`, `additionalProperties`, `unevaluatedProperties` (with annotation tracking through `$ref`, `allOf`, `anyOf`, `oneOf` and `if`), `propertyNames`, `minProperties`, `maxProperties`, `dependentRequired`, `items`, `prefixItems`, `minItems`, `maxItems`, `uniqueItems`, `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `multipleOf`, `minLength`, `maxLength`, `allOf`, `anyOf`, `oneOf`, `not`, `if`, `then` and `else`.
//! `pattern` and `format` are treated as annotations (not checked). Assertion keywords outside this list, and `patternProperties` patterns of another form, make validation panic, so a schema the validator cannot judge never passes silently.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use serde_json::{Map, Number, Value};

/// Keywords this validator does not implement.
const UNSUPPORTED_KEYWORDS: [&str; 7] = [
    "unevaluatedItems",
    "dependentSchemas",
    "contains",
    "minContains",
    "maxContains",
    "$dynamicRef",
    "$recursiveRef",
];

/// Validates `instance` against the subschema of `root` at the JSON Pointer `pointer` (empty for the root itself).
pub fn validate(root: &Value, pointer: &str, instance: &Value) -> Result<(), String> {
    let schema = root
        .pointer(pointer)
        .ok_or_else(|| format!("the schema has nothing at {pointer:?}"))?;
    Validator { root }.check(schema, instance, "").map(|_| ())
}

/// Validation state: the root schema that `$ref` values resolve against.
struct Validator<'a> {
    root: &'a Value,
}

impl Validator<'_> {
    /// Checks `instance` against `schema`; on success returns the names of the instance properties the schema evaluated (for `unevaluatedProperties`).
    fn check(
        &self,
        schema: &Value,
        instance: &Value,
        location: &str,
    ) -> Result<BTreeSet<String>, String> {
        let keywords = match schema {
            Value::Bool(true) => return Ok(BTreeSet::new()),
            Value::Bool(false) => return Err(format!("{location}: no value is allowed here")),
            Value::Object(keywords) => keywords,
            other => panic!("{other} is not a JSON Schema"),
        };
        for keyword in keywords.keys() {
            assert!(
                !UNSUPPORTED_KEYWORDS.contains(&keyword.as_str()),
                "the test validator does not support the keyword {keyword}"
            );
        }

        let mut evaluated = BTreeSet::new();
        if let Some(reference) = keywords.get("$ref") {
            let target = self.resolve(reference)?;
            evaluated.extend(self.check(target, instance, location)?);
        }
        if let Some(expected) = keywords.get("type") {
            check_type(expected, instance, location)?;
        }
        if let Some(constant) = keywords.get("const")
            && !json_equal(constant, instance)
        {
            return Err(format!("{location}: expected {constant}, found {instance}"));
        }
        if let Some(allowed) = keywords.get("enum").and_then(Value::as_array)
            && !allowed.iter().any(|value| json_equal(value, instance))
        {
            return Err(format!("{location}: {instance} is not one of {allowed:?}"));
        }
        match instance {
            Value::Number(number) => check_number(keywords, number, location)?,
            Value::String(text) => check_string(keywords, text, location)?,
            Value::Array(items) => self.check_array(keywords, items, location)?,
            Value::Object(members) => {
                evaluated.extend(self.check_object(keywords, members, location)?);
            }
            Value::Null | Value::Bool(_) => {}
        }
        evaluated.extend(self.check_combinations(keywords, instance, location)?);

        if let (Some(unevaluated), Value::Object(members)) =
            (keywords.get("unevaluatedProperties"), instance)
        {
            for (name, value) in members {
                if !evaluated.contains(name) {
                    self.check(unevaluated, value, &format!("{location}/{name}"))?;
                    evaluated.insert(name.clone());
                }
            }
        }
        Ok(evaluated)
    }

    /// Resolves a local `$ref` such as `#/$defs/Command`.
    fn resolve(&self, reference: &Value) -> Result<&Value, String> {
        let text = reference
            .as_str()
            .ok_or_else(|| format!("$ref {reference} is not a string"))?;
        text.strip_prefix('#')
            .and_then(|pointer| self.root.pointer(pointer))
            .ok_or_else(|| format!("$ref {text} does not resolve inside the schema"))
    }

    /// Applies `allOf`, `anyOf`, `oneOf`, `not` and `if`/`then`/`else`; returns the properties evaluated by the subschemas that passed.
    fn check_combinations(
        &self,
        keywords: &Map<String, Value>,
        instance: &Value,
        location: &str,
    ) -> Result<BTreeSet<String>, String> {
        let mut evaluated = BTreeSet::new();
        if let Some(members) = keywords.get("allOf").and_then(Value::as_array) {
            for member in members {
                evaluated.extend(self.check(member, instance, location)?);
            }
        }
        if let Some(options) = keywords.get("anyOf").and_then(Value::as_array) {
            let mut passed = false;
            let mut reasons = Vec::new();
            for option in options {
                match self.check(option, instance, location) {
                    Ok(names) => {
                        passed = true;
                        evaluated.extend(names);
                    }
                    Err(reason) => reasons.push(reason),
                }
            }
            if !passed {
                return Err(format!(
                    "{location}: matches no anyOf option ({})",
                    reasons.join("; ")
                ));
            }
        }
        if let Some(options) = keywords.get("oneOf").and_then(Value::as_array) {
            let mut matches = Vec::new();
            let mut reasons = Vec::new();
            for option in options {
                match self.check(option, instance, location) {
                    Ok(names) => matches.push(names),
                    Err(reason) => reasons.push(reason),
                }
            }
            match matches.len() {
                1 => evaluated.extend(matches.remove(0)),
                0 => {
                    return Err(format!(
                        "{location}: matches no oneOf option ({})",
                        reasons.join("; ")
                    ));
                }
                count => {
                    return Err(format!(
                        "{location}: matches {count} oneOf options, not one"
                    ));
                }
            }
        }
        if let Some(negated) = keywords.get("not")
            && self.check(negated, instance, location).is_ok()
        {
            return Err(format!("{location}: matches a schema under not"));
        }
        if let Some(condition) = keywords.get("if") {
            match self.check(condition, instance, location) {
                Ok(names) => {
                    evaluated.extend(names);
                    if let Some(then) = keywords.get("then") {
                        evaluated.extend(self.check(then, instance, location)?);
                    }
                }
                Err(_) => {
                    if let Some(otherwise) = keywords.get("else") {
                        evaluated.extend(self.check(otherwise, instance, location)?);
                    }
                }
            }
        }
        Ok(evaluated)
    }

    /// Applies the array keywords.
    fn check_array(
        &self,
        keywords: &Map<String, Value>,
        items: &[Value],
        location: &str,
    ) -> Result<(), String> {
        let prefix = keywords
            .get("prefixItems")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        for (index, (item, item_schema)) in items.iter().zip(prefix).enumerate() {
            self.check(item_schema, item, &format!("{location}/{index}"))?;
        }
        if let Some(item_schema) = keywords.get("items") {
            for (index, item) in items.iter().enumerate().skip(prefix.len()) {
                self.check(item_schema, item, &format!("{location}/{index}"))?;
            }
        }
        if let Some(minimum) = keywords.get("minItems").and_then(Value::as_u64)
            && (items.len() as u64) < minimum
        {
            return Err(format!(
                "{location}: {} items, fewer than {minimum}",
                items.len()
            ));
        }
        if let Some(maximum) = keywords.get("maxItems").and_then(Value::as_u64)
            && (items.len() as u64) > maximum
        {
            return Err(format!(
                "{location}: {} items, more than {maximum}",
                items.len()
            ));
        }
        if keywords.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
            for (index, item) in items.iter().enumerate() {
                if items[index + 1..]
                    .iter()
                    .any(|other| json_equal(item, other))
                {
                    return Err(format!("{location}: item {index} is repeated"));
                }
            }
        }
        Ok(())
    }

    /// Applies the object keywords; returns the properties evaluated by `properties`, `patternProperties` and `additionalProperties`.
    fn check_object(
        &self,
        keywords: &Map<String, Value>,
        members: &Map<String, Value>,
        location: &str,
    ) -> Result<BTreeSet<String>, String> {
        let mut evaluated = BTreeSet::new();
        let declared = keywords.get("properties").and_then(Value::as_object);
        if let Some(declared) = declared {
            for (name, property_schema) in declared {
                if let Some(value) = members.get(name) {
                    self.check(property_schema, value, &format!("{location}/{name}"))?;
                    evaluated.insert(name.clone());
                }
            }
        }
        let patterned = keywords.get("patternProperties").and_then(Value::as_object);
        if let Some(patterned) = patterned {
            for (pattern, property_schema) in patterned {
                for (name, value) in members {
                    if pattern_matches(pattern, name) {
                        self.check(property_schema, value, &format!("{location}/{name}"))?;
                        evaluated.insert(name.clone());
                    }
                }
            }
        }
        if let Some(additional) = keywords.get("additionalProperties") {
            for (name, value) in members {
                let is_declared = declared.is_some_and(|declared| declared.contains_key(name));
                let is_patterned = patterned.is_some_and(|patterned| {
                    patterned
                        .keys()
                        .any(|pattern| pattern_matches(pattern, name))
                });
                if !is_declared && !is_patterned {
                    self.check(additional, value, &format!("{location}/{name}"))?;
                    evaluated.insert(name.clone());
                }
            }
        }
        if let Some(required) = keywords.get("required").and_then(Value::as_array) {
            for name in required.iter().filter_map(Value::as_str) {
                if !members.contains_key(name) {
                    return Err(format!(
                        "{location}: the required property {name} is missing"
                    ));
                }
            }
        }
        if let Some(names_schema) = keywords.get("propertyNames") {
            for name in members.keys() {
                self.check(names_schema, &Value::String(name.clone()), location)?;
            }
        }
        if let Some(minimum) = keywords.get("minProperties").and_then(Value::as_u64)
            && (members.len() as u64) < minimum
        {
            return Err(format!("{location}: fewer than {minimum} properties"));
        }
        if let Some(maximum) = keywords.get("maxProperties").and_then(Value::as_u64)
            && (members.len() as u64) > maximum
        {
            return Err(format!("{location}: more than {maximum} properties"));
        }
        if let Some(dependencies) = keywords.get("dependentRequired").and_then(Value::as_object) {
            for (name, needed) in dependencies {
                if !members.contains_key(name) {
                    continue;
                }
                for other in needed
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    if !members.contains_key(other) {
                        return Err(format!("{location}: {name} requires {other}"));
                    }
                }
            }
        }
        Ok(evaluated)
    }
}

/// Whether `text` matches `pattern`, which must have the form `^[...]+$`: one repeated character class of literal
/// characters and ranges such as `A-Z`. Any other pattern panics, so a schema the validator cannot judge never passes.
fn pattern_matches(pattern: &str, text: &str) -> bool {
    let class: Vec<char> = pattern
        .strip_prefix("^[")
        .and_then(|rest| rest.strip_suffix("]+$"))
        .filter(|class| !class.is_empty() && !class.starts_with('^'))
        .filter(|class| !class.contains(['[', ']', '\\']))
        .unwrap_or_else(|| {
            panic!("the test validator supports only patterns of the form ^[...]+$, not {pattern}")
        })
        .chars()
        .collect();
    let allowed = |character: char| {
        let mut index = 0;
        while index < class.len() {
            if index + 2 < class.len() && class[index + 1] == '-' {
                if (class[index]..=class[index + 2]).contains(&character) {
                    return true;
                }
                index += 3;
            } else {
                if class[index] == character {
                    return true;
                }
                index += 1;
            }
        }
        false
    };
    !text.is_empty() && text.chars().all(allowed)
}

/// Applies `type`, a type name or a list of type names.
fn check_type(expected: &Value, instance: &Value, location: &str) -> Result<(), String> {
    let names: Vec<&str> = match expected {
        Value::String(name) => vec![name.as_str()],
        Value::Array(names) => names.iter().filter_map(Value::as_str).collect(),
        other => panic!("type {other} is neither a string nor a list"),
    };
    if names.iter().any(|name| has_type(instance, name)) {
        Ok(())
    } else {
        Err(format!("{location}: {instance} is not of type {expected}"))
    }
}

/// Whether `instance` has the JSON Schema type `name`.
fn has_type(instance: &Value, name: &str) -> bool {
    match name {
        "null" => instance.is_null(),
        "boolean" => instance.is_boolean(),
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "number" => instance.is_number(),
        "integer" => match instance {
            Value::Number(number) => {
                number.is_i64()
                    || number.is_u64()
                    || number.as_f64().is_some_and(|value| value.fract() == 0.0)
            }
            _ => false,
        },
        other => panic!("{other} is not a JSON Schema type"),
    }
}

/// Applies the numeric keywords.
fn check_number(
    keywords: &Map<String, Value>,
    number: &Number,
    location: &str,
) -> Result<(), String> {
    let bound = |keyword: &str| keywords.get(keyword).and_then(Value::as_number);
    if let Some(minimum) = bound("minimum")
        && compare_numbers(number, minimum) == Ordering::Less
    {
        return Err(format!(
            "{location}: {number} is below the minimum {minimum}"
        ));
    }
    if let Some(maximum) = bound("maximum")
        && compare_numbers(number, maximum) == Ordering::Greater
    {
        return Err(format!(
            "{location}: {number} is above the maximum {maximum}"
        ));
    }
    if let Some(minimum) = bound("exclusiveMinimum")
        && compare_numbers(number, minimum) != Ordering::Greater
    {
        return Err(format!("{location}: {number} is not above {minimum}"));
    }
    if let Some(maximum) = bound("exclusiveMaximum")
        && compare_numbers(number, maximum) != Ordering::Less
    {
        return Err(format!("{location}: {number} is not below {maximum}"));
    }
    if let Some(divisor) = bound("multipleOf").and_then(Number::as_f64) {
        let value = number.as_f64().expect("a JSON number converts to f64");
        let quotient = value / divisor;
        if (quotient - quotient.round()).abs() > 1e-9 {
            return Err(format!(
                "{location}: {number} is not a multiple of {divisor}"
            ));
        }
    }
    Ok(())
}

/// Applies the string length keywords; `pattern` and `format` are not checked.
fn check_string(keywords: &Map<String, Value>, text: &str, location: &str) -> Result<(), String> {
    let characters = text.chars().count() as u64;
    if let Some(minimum) = keywords.get("minLength").and_then(Value::as_u64)
        && characters < minimum
    {
        return Err(format!("{location}: {text:?} is shorter than {minimum}"));
    }
    if let Some(maximum) = keywords.get("maxLength").and_then(Value::as_u64)
        && characters > maximum
    {
        return Err(format!("{location}: {text:?} is longer than {maximum}"));
    }
    Ok(())
}

/// Compares two JSON numbers exactly when both are integers, otherwise as `f64`.
fn compare_numbers(left: &Number, right: &Number) -> Ordering {
    if let (Some(left), Some(right)) = (left.as_u64(), right.as_u64()) {
        return left.cmp(&right);
    }
    if let (Some(left), Some(right)) = (left.as_i64(), right.as_i64()) {
        return left.cmp(&right);
    }
    let left = left.as_f64().expect("a JSON number converts to f64");
    let right = right.as_f64().expect("a JSON number converts to f64");
    left.partial_cmp(&right).expect("JSON numbers are finite")
}

/// JSON Schema equality: numbers compare by value, so `1` equals `1.0`.
fn json_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            compare_numbers(left, right) == Ordering::Equal
        }
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| json_equal(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(name, value)| {
                    right
                        .get(name)
                        .is_some_and(|other| json_equal(value, other))
                })
        }
        _ => left == right,
    }
}
