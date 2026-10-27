//! The comparison of engine.md test 154: the header of a response frame of the web core and that of the native session, as JSON, by the rules of engine.md section 3.18.
//!
//! Objects have the same keys in the same order and arrays the same length; strings, booleans, `null` and numbers written as integers are equal; any other two numbers agree within `max(1e-9 * |native|, 1e-9)`, an arc's `start_angle` and `end_angle` modulo 2π. In a `list_elements` result numbers agree within `max(1e-9 * |native|, 1e-8)`. In the `print_health` event numbers agree within `max(1e-6 * |native|, 1e-6)`, the tolerance of delivery.md section 2.8, and the findings' `triangles` are left out. Of each entry of the header's `buffers` only `element_type` is compared.

use std::f64::consts::TAU;

use serde_json::{Map, Number, Value, json};

/// How far a number that is not an integer may be from the native one: `max(relative * |native|, floor)`.
#[derive(Clone, Copy, Debug)]
struct Tolerance {
    /// The bound as a fraction of the native number.
    relative: f64,
    /// The absolute bound near zero.
    floor: f64,
}

impl Tolerance {
    /// The largest difference allowed from `native`.
    fn bound(self, native: f64) -> f64 {
        (self.relative * native.abs()).max(self.floor)
    }
}

/// The tolerance of numbers that are not integers (engine.md section 3.18).
const TOLERANCE: Tolerance = Tolerance {
    relative: 1e-9,
    floor: 1e-9,
};

/// The tolerance of the geometry of element descriptors: a floor ten times higher, since OCCT builds some faces, such as fillets, to a tolerance of its own, which moves their coordinates near zero by up to 1.6e-9 (engine.md section 3.18 and open issue 18).
const ELEMENT_TOLERANCE: Tolerance = Tolerance {
    relative: 1e-9,
    floor: 1e-8,
};

/// The tolerance of the numbers of a print report (delivery.md section 2.8).
const PRINT_TOLERANCE: Tolerance = Tolerance {
    relative: 1e-6,
    floor: 1e-6,
};

/// One place where the two headers differ, as a failure line of test 154 writes it.
#[derive(Debug, PartialEq)]
pub struct Difference {
    /// JSON Pointer into the frame header.
    pub pointer: String,
    /// What the web core's header holds there: compact JSON, `missing`, `array of {n}` or `keys {keys}`.
    pub wasm: String,
    /// What the native header holds there, written the same way.
    pub native: String,
}

/// Every place where `wasm`, the header of the web core's response frame, differs from `native`, the native session's, in the order of the native header.
pub fn compare_headers(wasm: &Value, native: &Value) -> Vec<Difference> {
    let mut comparison = Comparison::default();
    comparison.values("", wasm, native, Scope::Frame);
    comparison.differences
}

/// The rules that apply at a place of the header.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Scope {
    /// Numbers within [`TOLERANCE`].
    Frame,
    /// The `print_health` event: numbers within [`PRINT_TOLERANCE`], and every finding's `triangles` left out.
    PrintHealth,
    /// A `list_elements` response: numbers within [`ELEMENT_TOLERANCE`].
    ElementList,
    /// An entry of the header's `buffers`: `offset` and `length` left out.
    BufferEntry,
}

impl Scope {
    /// The tolerance of numbers that are not integers.
    fn tolerance(self) -> Tolerance {
        match self {
            Scope::Frame | Scope::BufferEntry => TOLERANCE,
            Scope::ElementList => ELEMENT_TOLERANCE,
            Scope::PrintHealth => PRINT_TOLERANCE,
        }
    }

    /// Whether the value of `key` is left out of the comparison here.
    fn leaves_out(self, key: &str) -> bool {
        match self {
            Scope::Frame | Scope::ElementList => false,
            Scope::PrintHealth => key == "triangles",
            Scope::BufferEntry => key == "offset" || key == "length",
        }
    }

    /// The scope of `native`, the native value at `pointer`, a child of a value in this scope.
    fn of_child(self, pointer: &str, native: &Value) -> Scope {
        let parent = pointer.rsplit_once('/').map_or("", |(parent, _)| parent);
        match (self, parent) {
            (Scope::Frame, "/buffers") => Scope::BufferEntry,
            (Scope::Frame, "/messages") if native["type"] == "print_health" => Scope::PrintHealth,
            (Scope::Frame, "/messages") if native["result"]["type"] == "list_elements" => {
                Scope::ElementList
            }
            _ => self,
        }
    }
}

/// The differences found so far.
#[derive(Default)]
struct Comparison {
    differences: Vec<Difference>,
}

impl Comparison {
    /// Compares the two values at `pointer`.
    fn values(&mut self, pointer: &str, wasm: &Value, native: &Value, scope: Scope) {
        match (wasm, native) {
            (Value::Object(wasm_object), Value::Object(native_object)) => {
                self.objects(pointer, wasm_object, native_object, scope);
            }
            (Value::Array(wasm_items), Value::Array(native_items)) => {
                self.arrays(pointer, wasm_items, native_items, scope);
            }
            (Value::Number(wasm_number), Value::Number(native_number)) => {
                if !numbers_agree(wasm_number, native_number, scope.tolerance()) {
                    self.differ_in_values(pointer, wasm, native);
                }
            }
            // Strings, booleans, `null`, and two values of different types.
            _ => {
                if wasm != native {
                    self.differ_in_values(pointer, wasm, native);
                }
            }
        }
    }

    /// Compares two objects: their keys, in order, then the values of the keys both hold.
    fn objects(
        &mut self,
        pointer: &str,
        wasm: &Map<String, Value>,
        native: &Map<String, Value>,
        scope: Scope,
    ) {
        let wasm_keys: Vec<&String> = wasm.keys().collect();
        let native_keys: Vec<&String> = native.keys().collect();
        for key in native.keys().filter(|key| !wasm.contains_key(*key)) {
            self.differ(
                &child_pointer(pointer, key),
                "missing",
                compact(&native[key]),
            );
        }
        for key in wasm.keys().filter(|key| !native.contains_key(*key)) {
            self.differ(&child_pointer(pointer, key), compact(&wasm[key]), "missing");
        }
        if wasm_keys.len() == native_keys.len()
            && wasm_keys != native_keys
            && wasm_keys.iter().all(|key| native.contains_key(*key))
        {
            self.differ(pointer, keys_text(&wasm_keys), keys_text(&native_keys));
        }
        let arc = native.get("type").and_then(Value::as_str) == Some("arc");
        for (key, native_value) in native {
            let Some(wasm_value) = wasm.get(key) else {
                continue;
            };
            if scope.leaves_out(key) {
                continue;
            }
            let child = child_pointer(pointer, key);
            if arc && is_angle(key) {
                self.angles(&child, wasm_value, native_value, scope);
            } else {
                self.values(
                    &child,
                    wasm_value,
                    native_value,
                    scope.of_child(&child, native_value),
                );
            }
        }
    }

    /// Compares two arrays: their lengths, then their items in order when the lengths agree.
    fn arrays(&mut self, pointer: &str, wasm: &[Value], native: &[Value], scope: Scope) {
        if wasm.len() != native.len() {
            self.differ(
                pointer,
                format!("array of {}", wasm.len()),
                format!("array of {}", native.len()),
            );
            return;
        }
        for (index, (wasm_item, native_item)) in wasm.iter().zip(native).enumerate() {
            let child = child_pointer(pointer, &index.to_string());
            self.values(
                &child,
                wasm_item,
                native_item,
                scope.of_child(&child, native_item),
            );
        }
    }

    /// Compares an arc's start or end angle, which may also differ by whole turns (sketch.md section 3.12).
    fn angles(&mut self, pointer: &str, wasm: &Value, native: &Value, scope: Scope) {
        match (wasm.as_f64(), native.as_f64()) {
            (Some(wasm_angle), Some(native_angle)) => {
                if !angles_agree(wasm_angle, native_angle, scope.tolerance()) {
                    self.differ_in_values(pointer, wasm, native);
                }
            }
            _ => self.values(pointer, wasm, native, scope),
        }
    }

    /// Records that the values at `pointer` differ, each written as compact JSON.
    fn differ_in_values(&mut self, pointer: &str, wasm: &Value, native: &Value) {
        self.differ(pointer, compact(wasm), compact(native));
    }

    /// Records a difference at `pointer`.
    fn differ(&mut self, pointer: &str, wasm: impl Into<String>, native: impl Into<String>) {
        self.differences.push(Difference {
            pointer: pointer.to_owned(),
            wasm: wasm.into(),
            native: native.into(),
        });
    }
}

/// Whether two numbers agree: equal when both are written as integers (no fraction or exponent), else within `tolerance` of the native one.
fn numbers_agree(wasm: &Number, native: &Number, tolerance: Tolerance) -> bool {
    if !wasm.is_f64() && !native.is_f64() {
        return wasm == native;
    }
    match (wasm.as_f64(), native.as_f64()) {
        (Some(wasm_value), Some(native_value)) => {
            (wasm_value - native_value).abs() <= tolerance.bound(native_value)
        }
        _ => false,
    }
}

/// Whether two angles agree within `tolerance` after taking away whole turns.
fn angles_agree(wasm: f64, native: f64, tolerance: Tolerance) -> bool {
    let difference = wasm - native;
    let residual = difference - (difference / TAU).round() * TAU;
    residual.abs() <= tolerance.bound(native)
}

/// Whether `key` is an arc angle that whole turns may change.
fn is_angle(key: &str) -> bool {
    key == "start_angle" || key == "end_angle"
}

/// The JSON Pointer of `key` below `pointer` (RFC 6901: `~` becomes `~0` and `/` becomes `~1`).
fn child_pointer(pointer: &str, key: &str) -> String {
    format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1"))
}

/// A value as compact JSON.
fn compact(value: &Value) -> String {
    value.to_string()
}

/// An object's keys in order, as a failure line writes them.
fn keys_text(keys: &[&String]) -> String {
    format!(
        "keys {}",
        Value::from(keys.iter().map(|key| key.as_str()).collect::<Vec<_>>())
    )
}

// Tests of the comparison rules.

/// The one difference between `wasm` and `native`, failing when there is not exactly one.
#[track_caller]
fn only_difference(wasm: &Value, native: &Value) -> Difference {
    let mut differences = compare_headers(wasm, native);
    assert_eq!(differences.len(), 1, "{differences:?}");
    differences.remove(0)
}

/// A difference at `pointer` with the two texts a failure line writes.
fn difference(pointer: &str, wasm: &str, native: &str) -> Difference {
    Difference {
        pointer: pointer.to_owned(),
        wasm: wasm.to_owned(),
        native: native.to_owned(),
    }
}

/// A header whose one message is `message`.
fn header_with(message: Value) -> Value {
    json!({"protocol_version": 1, "messages": [message], "buffers": []})
}

#[test]
fn equal_headers_have_no_difference() {
    let header = header_with(
        json!({"kind": "response", "id": 3, "result": {"kind": "ok", "volume": 24000.0}}),
    );

    assert_eq!(compare_headers(&header, &header.clone()), []);
}

#[test]
fn strings_must_be_equal() {
    let found = only_difference(&json!({"code": "ok"}), &json!({"code": "warning"}));

    assert_eq!(found, difference("/code", "\"ok\"", "\"warning\""));
}

#[test]
fn booleans_and_null_must_be_equal() {
    assert_eq!(
        compare_headers(
            &json!({"ok": true, "doc": null}),
            &json!({"ok": false, "doc": 1})
        ),
        [
            difference("/ok", "true", "false"),
            difference("/doc", "null", "1")
        ]
    );
}

#[test]
fn integers_must_be_equal() {
    let found = only_difference(&json!({"mesh_id": 4}), &json!({"mesh_id": 3}));

    assert_eq!(found, difference("/mesh_id", "4", "3"));
}

#[test]
fn other_numbers_agree_within_1e_9_relative() {
    let native = json!({"volume": 24000.0});

    assert_eq!(
        compare_headers(&json!({"volume": 24000.0 + 2e-5}), &native),
        []
    );
    assert_eq!(
        only_difference(&json!({"volume": 24000.0 + 3e-5}), &native).pointer,
        "/volume"
    );
}

#[test]
fn numbers_near_zero_agree_within_1e_9() {
    let native = json!({"x": 6.2e-11});

    assert_eq!(compare_headers(&json!({"x": -6.2e-11}), &native), []);
    assert_eq!(
        only_difference(&json!({"x": 1.1e-9}), &native).pointer,
        "/x"
    );
}

#[test]
fn an_integer_and_a_fraction_agree_within_the_tolerance() {
    assert_eq!(compare_headers(&json!({"x": 1.0}), &json!({"x": 1})), []);
}

#[test]
fn a_number_and_a_string_differ_in_type() {
    let found = only_difference(&json!({"value": "5"}), &json!({"value": 5.0}));

    assert_eq!(found, difference("/value", "\"5\"", "5.0"));
}

#[test]
fn a_key_the_web_core_lacks_is_missing_there() {
    let found = only_difference(&json!({"id": 1}), &json!({"id": 1, "detail": "kernel"}));

    assert_eq!(found, difference("/detail", "missing", "\"kernel\""));
}

#[test]
fn a_key_only_the_web_core_holds_is_missing_natively() {
    let found = only_difference(&json!({"id": 1, "cause": {}}), &json!({"id": 1}));

    assert_eq!(found, difference("/cause", "{}", "missing"));
}

#[test]
fn the_same_keys_in_another_order_name_both_orders() {
    let wasm: Value = serde_json::from_str(r#"{"result": {"b": 1, "a": 2}}"#).unwrap();
    let native: Value = serde_json::from_str(r#"{"result": {"a": 2, "b": 1}}"#).unwrap();

    let found = only_difference(&wasm, &native);

    assert_eq!(
        found,
        difference("/result", r#"keys ["b","a"]"#, r#"keys ["a","b"]"#)
    );
}

#[test]
fn arrays_of_other_lengths_differ_once_without_their_items() {
    let found = only_difference(&json!({"faces": [1, 2, 3]}), &json!({"faces": [4, 5]}));

    assert_eq!(found, difference("/faces", "array of 3", "array of 2"));
}

#[test]
fn items_of_arrays_are_compared_in_order() {
    assert_eq!(
        compare_headers(&json!({"faces": [1, 2]}), &json!({"faces": [2, 1]})),
        [
            difference("/faces/0", "1", "2"),
            difference("/faces/1", "2", "1")
        ]
    );
}

#[test]
fn pointers_escape_tilde_and_slash() {
    let found = only_difference(&json!({"a/b~c": 1}), &json!({"a/b~c": 2}));

    assert_eq!(found.pointer, "/a~1b~0c");
}

#[test]
fn arc_angles_agree_modulo_a_turn() {
    let native =
        json!({"type": "arc", "start_angle": 5.0 * std::f64::consts::PI / 3.0, "end_angle": 1.0});
    let wasm = json!({"type": "arc", "start_angle": -std::f64::consts::PI / 3.0, "end_angle": 1.0 - 2.0 * TAU});

    assert_eq!(compare_headers(&wasm, &native), []);
}

#[test]
fn arc_angles_half_a_turn_apart_differ() {
    let native = json!({"type": "arc", "start_angle": 0.5, "end_angle": 1.0});
    let wasm = json!({"type": "arc", "start_angle": 0.5 + TAU / 2.0, "end_angle": 1.0});

    assert_eq!(only_difference(&wasm, &native).pointer, "/start_angle");
}

#[test]
fn angles_of_other_objects_are_compared_without_turns() {
    let native = json!({"type": "line", "start_angle": 0.5});
    let wasm = json!({"type": "line", "start_angle": 0.5 + TAU});

    assert_eq!(only_difference(&wasm, &native).pointer, "/start_angle");
}

/// A `print_health` event holding one finding with `value` and `triangles`.
fn print_health(value: f64, triangles: Value) -> Value {
    header_with(json!({
        "kind": "event",
        "type": "print_health",
        "doc": 1,
        "report": {"bodies": [{"mesh_id": 3, "report": {"checks": [{"findings": [
            {"value": value, "triangles": triangles, "faces": [0, 1]}
        ]}]}}]}
    }))
}

#[test]
fn print_report_numbers_agree_within_1e_6_relative() {
    let native = print_health(0.6, json!([]));

    assert_eq!(
        compare_headers(&print_health(0.6 + 5e-7, json!([])), &native),
        []
    );
    assert_eq!(
        only_difference(&print_health(0.6 + 2e-6, json!([])), &native).pointer,
        "/messages/0/report/bodies/0/report/checks/0/findings/0/value"
    );
}

#[test]
fn print_report_findings_leave_out_their_triangles() {
    let wasm = print_health(0.6, json!([{"start": 4, "count": 2}]));
    let native = print_health(
        0.6,
        json!([{"start": 7, "count": 2}, {"start": 12, "count": 1}]),
    );

    assert_eq!(compare_headers(&wasm, &native), []);
}

#[test]
fn print_report_integers_must_be_equal() {
    let native = print_health(0.6, json!([]));
    let mut wasm = native.clone();
    wasm["messages"][0]["report"]["bodies"][0]["mesh_id"] = json!(4);

    assert_eq!(
        only_difference(&wasm, &native),
        difference("/messages/0/report/bodies/0/mesh_id", "4", "3")
    );
}

#[test]
fn numbers_outside_the_print_report_keep_1e_9() {
    let native = header_with(json!({"kind": "response", "id": 2, "result": {"area": 0.6}}));
    let wasm = header_with(json!({"kind": "response", "id": 2, "result": {"area": 0.6 + 5e-7}}));

    assert_eq!(
        only_difference(&wasm, &native).pointer,
        "/messages/0/result/area"
    );
}

#[test]
fn triangles_outside_the_print_report_are_compared() {
    let native = header_with(json!({"kind": "event", "type": "body_mesh", "triangles": 2}));
    let wasm = header_with(json!({"kind": "event", "type": "body_mesh", "triangles": 3}));

    assert_eq!(
        only_difference(&wasm, &native).pointer,
        "/messages/0/triangles"
    );
}

#[test]
fn buffer_entries_compare_their_element_types_only() {
    let native =
        json!({"messages": [], "buffers": [{"offset": 0, "length": 288, "element_type": "f32"}]});
    let wasm =
        json!({"messages": [], "buffers": [{"offset": 8, "length": 296, "element_type": "f32"}]});

    assert_eq!(compare_headers(&wasm, &native), []);
}

#[test]
fn buffer_entries_of_another_element_type_differ() {
    let native = json!({"buffers": [{"offset": 0, "length": 288, "element_type": "f32"}]});
    let wasm = json!({"buffers": [{"offset": 0, "length": 288, "element_type": "u32"}]});

    assert_eq!(
        only_difference(&wasm, &native),
        difference("/buffers/0/element_type", "\"u32\"", "\"f32\"")
    );
}

#[test]
fn buffer_tables_of_other_lengths_differ() {
    let entry = json!({"offset": 0, "length": 8, "element_type": "u8"});

    let found = only_difference(
        &json!({"buffers": [entry.clone(), entry.clone()]}),
        &json!({"buffers": [entry]}),
    );

    assert_eq!(found, difference("/buffers", "array of 2", "array of 1"));
}

#[test]
fn offsets_and_lengths_outside_the_buffer_table_are_compared() {
    let found = only_difference(
        &json!({"result": {"length": 21.0}}),
        &json!({"result": {"length": 20.0}}),
    );

    assert_eq!(found.pointer, "/result/length");
}

/// The area of the face descriptor of most [`element_list`] tests.
const ELEMENT_AREA: f64 = 58.265482442415;

/// A `list_elements` response holding one face descriptor with `centroid` and `area`.
fn element_list(name: &str, centroid: [f64; 3], area: f64) -> Value {
    header_with(json!({
        "kind": "response",
        "id": 4,
        "result": {"kind": "ok", "type": "list_elements", "elements": [
            {"name": name, "kind": "face", "centroid": centroid, "area": area}
        ]}
    }))
}

#[test]
fn element_coordinates_near_zero_agree_within_1e_8() {
    // The largest gaps of open issue 18: the centroid of the fillet face of `fillet/pipe-tee` and of `fillet/top-loop`.
    let native = element_list(
        "f-fil1/round",
        [-0.00023728924978883015, -1.2472724759996512e-9, 9.2],
        ELEMENT_AREA,
    );

    let wasm = element_list(
        "f-fil1/round",
        [-0.00023729083668833084, 3.047509815814846e-13, 9.2],
        ELEMENT_AREA,
    );

    assert_eq!(compare_headers(&wasm, &native), []);
}

#[test]
fn element_coordinates_near_zero_past_1e_8_differ() {
    let native = element_list("f-box1/top", [-14.3, 0.0, 9.2], ELEMENT_AREA);

    let wasm = element_list("f-box1/top", [-14.3, 1.1e-8, 9.2], ELEMENT_AREA);

    assert_eq!(
        only_difference(&wasm, &native).pointer,
        "/messages/0/result/elements/0/centroid/1"
    );
}

#[test]
fn element_geometry_agrees_within_1e_9_relative() {
    let native = element_list("f-box1/top", [-14.3, 0.0, 9.2], ELEMENT_AREA);

    let wasm = element_list("f-box1/top", [-14.3, 0.0, 9.2], ELEMENT_AREA + 5e-8);

    assert_eq!(compare_headers(&wasm, &native), []);
}

#[test]
fn element_geometry_past_1e_9_relative_differs() {
    let native = element_list("f-box1/top", [-14.3, 0.0, 9.2], ELEMENT_AREA);

    // 1e-7 relative: within a print report's 1e-6, but not within the 1e-9 of element geometry.
    let wasm = element_list("f-box1/top", [-14.3, 0.0, 9.2], ELEMENT_AREA * (1.0 + 1e-7));

    assert_eq!(
        only_difference(&wasm, &native).pointer,
        "/messages/0/result/elements/0/area"
    );
}

#[test]
fn element_names_must_be_equal() {
    let native = element_list("f-box1/top", [0.0; 3], ELEMENT_AREA);

    let wasm = element_list("f-box1/bottom", [0.0; 3], ELEMENT_AREA);

    assert_eq!(
        only_difference(&wasm, &native),
        difference(
            "/messages/0/result/elements/0/name",
            "\"f-box1/bottom\"",
            "\"f-box1/top\""
        )
    );
}
