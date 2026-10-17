//! Feature params addressed by JSON Pointer (RFC 6901), as messages and fixes name their fields:
//! the field path of a pointer, the references params store, and the changes fixes make at a
//! pointer.

use std::cmp::Ordering;

use partsmith_model::Id;
use partsmith_naming::Reference;
use serde_json::{Map, Value};

/// The field path of a JSON Pointer into a feature record (features.md section 3.2): `/edges/1`
/// is `edges[1]` and `/extent/expr` is `extent.expr`.
pub(crate) fn field_path(pointer: &str) -> String {
    let mut path = String::with_capacity(pointer.len());
    for token in tokens(pointer) {
        match token {
            Token::Index(index) => path.push_str(&format!("[{index}]")),
            Token::Key(key) => {
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(&key);
            }
        }
    }
    path
}

/// The value at `pointer` of a record's params.
pub(crate) fn value_at<'p>(params: &'p Map<String, Value>, pointer: &str) -> Option<&'p Value> {
    let (first, rest) = match pointer.strip_prefix('/')?.split_once('/') {
        Some((first, rest)) => (first, Some(rest)),
        None => (pointer.strip_prefix('/')?, None),
    };
    let value = params.get(&first.replace("~1", "/").replace("~0", "~"))?;
    match rest {
        Some(rest) => value.pointer(&format!("/{rest}")),
        None => Some(value),
    }
}

/// The pointer of the value that holds the value at `pointer`: `/profile/regions/0` gives
/// `/profile/regions`. The root holds itself.
pub(crate) fn parent(pointer: &str) -> &str {
    pointer.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// True when `pointer` names an entry of a list of `params`, false for a field that holds one
/// value.
pub(crate) fn is_list_entry(params: &Map<String, Value>, pointer: &str) -> bool {
    value_at(params, parent(pointer)).is_some_and(Value::is_array)
}

/// The reference stored at `pointer` of a record's params, under section 3.24's rule.
pub(crate) fn reference_at(params: &Map<String, Value>, pointer: &str) -> Option<Reference> {
    value_at(params, pointer).and_then(reference_in)
}

/// The reference `value` stores (engine.md section 3.24): an object with a `q` key that reads as
/// a naming `Reference`.
fn reference_in(value: &Value) -> Option<Reference> {
    value.get("q")?;
    serde_json::from_value(value.clone()).ok()
}

/// A reference a record's params store.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StoredReference {
    /// Its JSON Pointer in the params.
    pub(crate) pointer: String,
    /// The body it is resolved in: the `body` of the nearest object around it that names one.
    pub(crate) body: Option<Id>,
    /// The reference.
    pub(crate) reference: Reference,
}

/// Every reference `params` store, in params order: each object, at any depth, that has a `q`
/// key and reads as a naming `Reference` (engine.md section 3.24).
pub(crate) fn stored_references(params: &Map<String, Value>) -> Vec<StoredReference> {
    let mut found = Vec::new();
    collect_references(params, "", None, &mut found);
    found
}

/// Adds the references of the object `members` at `pointer`, whose nearest object that names a
/// body names `body`.
fn collect_references(
    members: &Map<String, Value>,
    pointer: &str,
    body: Option<&Id>,
    found: &mut Vec<StoredReference>,
) {
    let named = members
        .get("body")
        .and_then(Value::as_str)
        .and_then(|text| Id::new(text).ok());
    let body = named.as_ref().or(body);
    for (key, value) in members {
        collect_value(value, &format!("{pointer}/{}", escaped(key)), body, found);
    }
}

/// Adds the references of `value` at `pointer` (section 3.24's rule).
fn collect_value(
    value: &Value,
    pointer: &str,
    body: Option<&Id>,
    found: &mut Vec<StoredReference>,
) {
    match value {
        // An object with a `q` key holds a reference even when it does not read as one, so its
        // members are not searched.
        Value::Object(members) if members.contains_key("q") => {
            found.extend(reference_in(value).map(|reference| StoredReference {
                pointer: pointer.to_string(),
                body: body.cloned(),
                reference,
            }));
        }
        Value::Object(members) => collect_references(members, pointer, body, found),
        Value::Array(items) => {
            for (position, item) in items.iter().enumerate() {
                collect_value(item, &format!("{pointer}/{position}"), body, found);
            }
        }
        _ => {}
    }
}

/// Calls `visit` on every object, at any depth, of `value` whose `sketch` key names `sketch`:
/// the objects through which a feature uses the regions (their `regions`) and points (their
/// `points`) of that sketch, such as the profile of an extrude or the params of a hole (engine.md
/// section 3.22). Stops at the first error `visit` returns.
pub(crate) fn visit_sketch_uses<E>(
    value: &mut Value,
    sketch: &Id,
    visit: &mut impl FnMut(&mut Map<String, Value>) -> Result<(), E>,
) -> Result<(), E> {
    match value {
        Value::Object(members) => {
            if members.get("sketch").and_then(Value::as_str) == Some(sketch.as_str()) {
                visit(members)?;
            }
            members
                .values_mut()
                .try_for_each(|member| visit_sketch_uses(member, sketch, visit))
        }
        Value::Array(items) => items
            .iter_mut()
            .try_for_each(|item| visit_sketch_uses(item, sketch, visit)),
        _ => Ok(()),
    }
}

/// A key as a pointer's reference token: `~` written `~0` and `/` written `~1`.
fn escaped(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Replaces the value at `pointer` of `params` with `values`: spliced in, one entry each, when it
/// is a list entry, else set to the only value. False when `pointer` names no value, or names
/// one that is not a list entry while `values` is not one value.
pub(crate) fn replace(params: &mut Value, pointer: &str, values: Vec<Value>) -> bool {
    let Some((holder, last)) = holder_mut(params, pointer) else {
        return false;
    };
    match (holder, last) {
        (Value::Array(items), Token::Index(index)) if index < items.len() => {
            items.splice(index..=index, values);
            true
        }
        (Value::Object(members), Token::Key(key)) if members.contains_key(&key) => {
            let Ok([value]) = <[Value; 1]>::try_from(values) else {
                return false;
            };
            members.insert(key, value);
            true
        }
        _ => false,
    }
}

/// Replaces each pointer's value as `replace` does, in an order that keeps the other pointers
/// valid: later list entries first. False when one of them fails, and then `params` may be
/// partly changed.
pub(crate) fn replace_each(
    params: &mut Value,
    mut replacements: Vec<(String, Vec<Value>)>,
) -> bool {
    replacements.sort_by(|(first, _), (second, _)| later_first(first, second));
    replacements
        .into_iter()
        .all(|(pointer, values)| replace(params, &pointer, values))
}

/// Sets the value at `pointer`, adding its key to the object that holds it. False when no object
/// holds it.
pub(crate) fn set(params: &mut Value, pointer: &str, value: Value) -> bool {
    match holder_mut(params, pointer) {
        Some((Value::Object(members), Token::Key(key))) => {
            members.insert(key, value);
            true
        }
        Some((Value::Array(items), Token::Index(index))) if index < items.len() => {
            items[index] = value;
            true
        }
        _ => false,
    }
}

/// Removes the value at `pointer`: a list entry, or an object's key. False when it names no
/// value.
pub(crate) fn remove(params: &mut Value, pointer: &str) -> bool {
    match holder_mut(params, pointer) {
        Some((Value::Object(members), Token::Key(key))) => members.remove(&key).is_some(),
        Some((Value::Array(items), Token::Index(index))) if index < items.len() => {
            items.remove(index);
            true
        }
        _ => false,
    }
}

/// One reference token of a pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// A position in a list.
    Index(usize),
    /// A key of an object.
    Key(String),
}

/// The unescaped reference tokens of a pointer; a token of digits only is an index.
fn tokens(pointer: &str) -> Vec<Token> {
    pointer
        .split('/')
        .skip(1)
        .map(|token| {
            let key = token.replace("~1", "/").replace("~0", "~");
            match key.parse() {
                Ok(index) if key.bytes().all(|byte| byte.is_ascii_digit()) => Token::Index(index),
                _ => Token::Key(key),
            }
        })
        .collect()
}

/// The value that holds the value at `pointer`, and the token that names it there.
fn holder_mut<'v>(params: &'v mut Value, pointer: &str) -> Option<(&'v mut Value, Token)> {
    let mut tokens = tokens(pointer);
    let last = tokens.pop()?;
    let mut holder = params;
    for token in tokens {
        holder = match (holder, token) {
            (Value::Array(items), Token::Index(index)) => items.get_mut(index)?,
            (Value::Object(members), Token::Key(key)) => members.get_mut(&key)?,
            (Value::Object(members), Token::Index(index)) => members.get_mut(&index.to_string())?,
            _ => return None,
        };
    }
    let last = match (&holder, last) {
        (Value::Object(_), Token::Index(index)) => Token::Key(index.to_string()),
        (_, last) => last,
    };
    Some((holder, last))
}

/// Orders pointers so that, of two entries of one list, the later one comes first.
fn later_first(first: &str, second: &str) -> Ordering {
    let (first, second) = (tokens(first), tokens(second));
    for (left, right) in first.iter().zip(&second) {
        let order = match (left, right) {
            (Token::Index(left), Token::Index(right)) => right.cmp(left),
            (Token::Key(left), Token::Key(right)) => left.cmp(right),
            (Token::Index(_), Token::Key(_)) => Ordering::Less,
            (Token::Key(_), Token::Index(_)) => Ordering::Greater,
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    second.len().cmp(&first.len())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn pointers_become_field_paths() {
        assert_eq!(field_path("/edges/1"), "edges[1]");
        assert_eq!(field_path("/extent/expr"), "extent.expr");
        assert_eq!(field_path("/profile/regions/0"), "profile.regions[0]");
        assert_eq!(field_path("/a~1b/c~0d"), "a/b.c~d");
    }

    #[test]
    fn values_are_read_at_a_pointer() {
        let Value::Object(params) = json!({"profile": {"regions": [{"key": "e-1"}]}, "a/b": 1})
        else {
            panic!("params are an object");
        };

        assert_eq!(
            value_at(&params, "/profile/regions/0/key"),
            Some(&json!("e-1"))
        );
        assert_eq!(value_at(&params, "/a~1b"), Some(&json!(1)));
        assert_eq!(value_at(&params, "/profile/sketch"), None);
        assert_eq!(value_at(&params, ""), None);
    }

    #[test]
    fn the_parent_of_a_pointer_holds_its_value() {
        assert_eq!(parent("/profile/regions/0"), "/profile/regions");
        assert_eq!(parent("/edges"), "");
    }

    #[test]
    fn list_entries_are_told_from_fields_that_hold_one_value() {
        let Value::Object(params) = json!({"edges": ["a"], "base": {"face": "b"}}) else {
            panic!("params are an object");
        };

        assert!(is_list_entry(&params, "/edges/0"));
        assert!(!is_list_entry(&params, "/base/face"));
        assert!(!is_list_entry(&params, "/edges"));
    }

    #[test]
    fn stored_references_name_their_pointer_and_the_nearest_body() {
        let edge = json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0"});
        let face = json!({"q": "f-box1/top", "fp": "plane|n=0,0,1"});
        let Value::Object(params) = json!({
            "body": "b-1",
            "edges": [edge, {"q": "not a name", "fp": "line"}],
            "plane": {"kind": "face", "body": "b-2", "face": face.clone()},
            "a/b": {"face": face},
            "label": "q"
        }) else {
            panic!("params are an object");
        };

        let found: Vec<(String, Option<String>, String)> = stored_references(&params)
            .into_iter()
            .map(|stored| {
                (
                    stored.pointer,
                    stored.body.map(|body| body.to_string()),
                    stored.reference.query.to_string(),
                )
            })
            .collect();

        let named = |pointer: &str, body: &str, query: &str| {
            (
                pointer.to_string(),
                Some(body.to_string()),
                query.to_string(),
            )
        };
        assert_eq!(
            found,
            [
                named("/edges/0", "b-1", "edge(f-box1/front|f-box1/top)"),
                named("/plane/face", "b-2", "f-box1/top"),
                named("/a~1b/face", "b-1", "f-box1/top"),
            ]
        );
    }

    #[test]
    fn references_are_read_at_a_pointer_only_from_objects_with_a_query() {
        let Value::Object(params) = json!({
            "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0"}],
            "plane": {"kind": "face", "body": "b-1"},
            "broken": {"q": "not a name", "fp": "line"}
        }) else {
            panic!("params are an object");
        };

        let edge = reference_at(&params, "/edges/0").expect("a stored reference");

        assert_eq!(edge.query.to_string(), "edge(f-box1/front|f-box1/top)");
        assert_eq!(reference_at(&params, "/plane"), None);
        assert_eq!(reference_at(&params, "/broken"), None);
        assert_eq!(reference_at(&params, "/edges/1"), None);
    }

    #[test]
    fn list_entries_are_replaced_by_every_value() {
        let mut params = json!({"edges": ["a", "b", "c"]});

        let replaced = replace(&mut params, "/edges/1", vec![json!("x"), json!("y")]);

        assert!(replaced);
        assert_eq!(params, json!({"edges": ["a", "x", "y", "c"]}));
    }

    #[test]
    fn a_member_is_replaced_by_one_value_only() {
        let mut params = json!({"base": {"face": "a"}});

        assert!(!replace(
            &mut params,
            "/base/face",
            vec![json!(1), json!(2)]
        ));
        assert!(replace(&mut params, "/base/face", vec![json!("b")]));
        assert_eq!(params, json!({"base": {"face": "b"}}));
    }

    #[test]
    fn missing_values_are_not_replaced() {
        let mut params = json!({"edges": ["a"]});

        assert!(!replace(&mut params, "/edges/3", vec![json!("x")]));
        assert!(!replace(&mut params, "/faces/0", vec![json!("x")]));
        assert_eq!(params, json!({"edges": ["a"]}));
    }

    #[test]
    fn several_replacements_keep_their_positions() {
        let mut params = json!({"edges": ["a", "b", "c"]});

        let replaced = replace_each(
            &mut params,
            vec![
                ("/edges/0".to_string(), vec![json!("x"), json!("y")]),
                ("/edges/2".to_string(), vec![json!("z")]),
            ],
        );

        assert!(replaced);
        assert_eq!(params, json!({"edges": ["x", "y", "b", "z"]}));
    }

    #[test]
    fn later_entries_of_one_list_come_first() {
        assert_eq!(later_first("/edges/10", "/edges/2"), Ordering::Less);
        assert_eq!(later_first("/edges/2", "/edges/10"), Ordering::Greater);
    }

    #[test]
    fn values_are_set_with_their_key_and_removed() {
        let mut params = json!({"direction": {"count": "2.5"}, "edges": ["a", "b"]});

        assert!(set(&mut params, "/direction/count", json!("3")));
        assert!(set(&mut params, "/reverse", json!(true)));
        assert!(remove(&mut params, "/edges/0"));
        assert!(!remove(&mut params, "/edges/5"));

        assert_eq!(
            params,
            json!({"direction": {"count": "3"}, "edges": ["b"], "reverse": true})
        );
    }

    #[test]
    fn numeric_keys_of_objects_stay_keys() {
        let mut params = json!({"map": {"7": "a"}});

        assert!(set(&mut params, "/map/7", json!("b")));
        assert_eq!(params, json!({"map": {"7": "b"}}));
    }

    #[test]
    fn the_uses_of_a_sketch_are_the_objects_that_name_it() {
        let mut params = json!({
            "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1"}]},
            "axis": {"kind": "sketch_line", "sketch": "f-sk2", "line": "e-9"},
            "steps": [{"sketch": "f-sk1", "points": ["e-2.start"]}],
            "sketch": "f-sk3"
        });
        let mut visited = Vec::new();

        visit_sketch_uses(
            &mut params,
            &Id::new("f-sk1").expect("a valid id"),
            &mut |uses| {
                visited.push(Value::Object(uses.clone()));
                Ok::<(), ()>(())
            },
        )
        .expect("the visit succeeds");

        assert_eq!(
            visited,
            [
                json!({"sketch": "f-sk1", "regions": [{"key": "e-1"}]}),
                json!({"sketch": "f-sk1", "points": ["e-2.start"]})
            ]
        );
    }

    #[test]
    fn a_visit_of_the_uses_of_a_sketch_stops_at_its_first_error() {
        let mut params = json!({"a": {"sketch": "f-sk1"}, "b": [{"sketch": "f-sk1"}]});
        let mut visits = 0;

        let visited = visit_sketch_uses(
            &mut params,
            &Id::new("f-sk1").expect("a valid id"),
            &mut |_| {
                visits += 1;
                Err("no")
            },
        );

        assert_eq!(visited, Err("no"));
        assert_eq!(visits, 1);
    }
}
