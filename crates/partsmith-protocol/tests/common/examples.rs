//! JSON examples copied verbatim from `docs/spec/protocol.md`, plus a few supplementary records so every command type has a request and a result.

/// Every command type of section 2.5, in the order of test 34.
pub const COMMAND_TYPES: [&str; 31] = [
    "engine_info",
    "feature_catalog",
    "list_printer_profiles",
    "put_printer_profile",
    "new_document",
    "open_document",
    "save_document",
    "close_document",
    "get_document",
    "apply_edits",
    "begin_transaction",
    "set_focus",
    "commit_transaction",
    "cancel_transaction",
    "undo",
    "redo",
    "solve_sketch",
    "drag_sketch",
    "end_sketch_drag",
    "set_tessellation",
    "evaluate",
    "list_elements",
    "pick_reference",
    "resolve_references",
    "measure",
    "analyze_print",
    "export",
    "import_step",
    "export_python",
    "mark_saved",
    "rename_parameter",
];

/// Every event type of section 2.7, in the order of test 36.
pub const EVENT_TYPES: [&str; 6] = [
    "document_changed",
    "recomputed",
    "sketch_solved",
    "datum_plane",
    "body_mesh",
    "print_health",
];

/// Every error code of section 4, in table order.
pub const ERROR_CODES: [&str; 39] = [
    "malformed_frame",
    "malformed_request",
    "protocol_version_mismatch",
    "superseded",
    "internal_error",
    "unknown_document",
    "document_poisoned",
    "parse_error",
    "unsupported_format_version",
    "asset_mismatch",
    "transaction_open",
    "no_transaction",
    "label_required",
    "label_not_allowed",
    "edit_rejected",
    "invalid_params",
    "nothing_to_undo",
    "nothing_to_redo",
    "unknown_feature",
    "focus_inactive",
    "not_a_sketch",
    "unknown_point",
    "no_drag",
    "stale_pick",
    "preview_not_pickable",
    "invalid_pick",
    "unknown_body",
    "body_empty",
    "reference_unresolved",
    "invalid_measure",
    "expression_error",
    "unknown_printer_profile",
    "profile_id_reserved",
    "invalid_profile",
    "invalid_orientation",
    "nothing_to_export",
    "export_failed",
    "import_failed",
    "invalid_tolerance",
];

/// Requests for the command types that section 2.5 lists without a request example (field lists from section 2.5).
pub const SUPPLEMENTARY_COMMANDS: [&str; 5] = [
    r#"{"type": "close_document", "doc": 1}"#,
    r#"{"type": "cancel_transaction", "doc": 1}"#,
    r#"{"type": "undo", "doc": 1}"#,
    r#"{"type": "redo", "doc": 1}"#,
    r#"{"type": "end_sketch_drag", "doc": 1, "feature": "f-sk1", "commit": true}"#,
];

/// Results for the command types that section 2.5 lists without a result example (field lists from section 2.5; sketch and element values from other spec examples).
pub const SUPPLEMENTARY_RESULTS: [&str; 8] = [
    r#"{"type": "close_document"}"#,
    r#"{"type": "set_focus"}"#,
    r#"{"type": "cancel_transaction", "revision": 8}"#,
    r#"{"type": "undo", "revision": 6, "label": "Change height"}"#,
    r#"{"type": "redo", "revision": 7, "label": "Change height"}"#,
    r#"{"type": "solve_sketch", "solution": {"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [10.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [10.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [10.0, 0.0]}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}}"#,
    r#"{"type": "end_sketch_drag", "revision": 9}"#,
    r#"{"type": "list_elements", "elements": [{"name": "f-box1/top", "kind": "face", "body": "b-1", "feature": "f-box1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}, "surface": "plane", "centroid": [0.0, 0.0, 40.0], "area": 600.0, "normal": [0.0, 0.0, 1.0], "placement": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "adjacent": ["edge(f-box1/back|f-box1/top)", "edge(f-box1/front|f-box1/top)", "edge(f-box1/left|f-box1/top)", "edge(f-box1/right|f-box1/top)"]}]}"#,
];

/// The error outcomes of section 2.8.
pub const ERROR_OUTCOMES: [&str; 2] = [
    r#"{"kind": "error", "code": "edit_rejected", "message": "There is no feature with id f-x.", "edit_index": 1}"#,
    r#"{"kind": "error", "code": "expression_error", "message": "Unknown name 'wal'.", "span": {"start": 4, "end": 7}}"#,
];

/// The sketch solution of the `drag_sketch` result example in section 2.5.
pub const DRAG_SOLUTION: &str = r#"{"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [20.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [20.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [20.0, 0.0]}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}"#;

/// The `fillet_too_large` diagnostic example of section 2.4.
pub const FILLET_DIAGNOSTIC: &str = r#"{"severity": "error", "code": "fillet_too_large", "message": "Fillet radius 3 mm is too large for the selected edges. The largest radius that works is about 2.3 mm. Try 2.3 mm or less.", "field": "/radius", "suggestion": "2.3 mm"}"#;

/// The element repair of the `recomputed` event example in section 2.7.
pub const ELEMENT_REPAIR: &str = r#"{"kind": "element", "path": "/edges/1", "body": "b-1", "reference": {"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-10,-15,20|l=40|adj=2,2"}, "resolution": "ambiguous", "centroid": [-10.0, -15.0, 20.0]}"#;

/// The region repair of test 68.
pub const REGION_REPAIR: &str = r#"{"kind":"region","path":"/profile/regions/0","sketch":"f-sk1","region":{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000"},"resolution":"missing"}"#;

/// The mesh of the first `body_mesh` event example in section 2.7.
pub const BOX_MESH: &str = r#"{"positions": 0, "normals": 1, "indices": 2, "face_ids": 3, "edge_positions": 4, "edge_ids": 5, "vertex_positions": 6, "vertex_ids": 7, "face_count": 6, "edge_count": 12, "vertex_count": 8, "bounding_box": {"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}}"#;

/// Example C of section 2.2: the buffer table of the 20 x 30 x 40 box mesh.
pub const EXAMPLE_C: &str = r#"[
  {"offset": 0, "length": 288, "element_type": "f32"},
  {"offset": 288, "length": 288, "element_type": "f32"},
  {"offset": 576, "length": 144, "element_type": "u32"},
  {"offset": 720, "length": 96, "element_type": "u32"},
  {"offset": 816, "length": 288, "element_type": "f32"},
  {"offset": 1104, "length": 48, "element_type": "u32"},
  {"offset": 1152, "length": 96, "element_type": "f32"},
  {"offset": 1248, "length": 32, "element_type": "u32"}
]"#;

/// Every request example: section 2.5's, then the supplementary ones.
pub fn all_commands() -> Vec<&'static str> {
    COMMAND_EXAMPLES
        .into_iter()
        .chain(SUPPLEMENTARY_COMMANDS)
        .collect()
}

/// Every result example: section 2.5's, then the supplementary ones.
pub fn all_results() -> Vec<&'static str> {
    RESULT_EXAMPLES
        .into_iter()
        .chain(SUPPLEMENTARY_RESULTS)
        .collect()
}

/// The examples of sections 2.3 and 2.7 whose top level is a message, in spec order.
pub const MESSAGE_EXAMPLES: [&str; 11] = [
    r#"{"kind": "request", "id": 7, "command": {"type": "undo", "doc": 1}}"#,
    r#"{"kind": "response", "id": 7, "result": {"kind": "ok", "type": "undo", "revision": 6, "label": "Change height"}}"#,
    r#"{"kind": "response", "id": 8, "result": {"kind": "error", "code": "nothing_to_undo", "message": "There is nothing to undo."}}"#,
    r#"{"kind": "event", "type": "document_changed", "doc": 1, "revision": 6, "document": {"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [], "bodies": [], "features": []}, "dirty": false, "transaction_open": false}"#,
    r#"{"kind": "event", "type": "document_changed", "doc": 1, "revision": 4, "document": {"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [{"id": "p-h", "name": "height", "expr": "50 mm"}], "bodies": [{"id": "b-1", "name": "Block"}], "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "height"], "op": "new"}]}, "dirty": true, "undo": "Change height", "transaction_open": false}"#,
    r#"{"kind": "event", "type": "recomputed", "doc": 1, "revision": 4, "features": [{"id": "f-box1", "status": "ok", "messages": [], "repairs": []}, {"id": "f-fil1", "status": "warning", "messages": [{"severity": "warning", "code": "reference_ambiguous", "message": "One of the edges selected for Edges now matches 2 edges, so it was skipped. Pick the one you mean in the repair panel.", "field": "/edges/1"}], "repairs": [{"kind": "element", "path": "/edges/1", "body": "b-1", "reference": {"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-10,-15,20|l=40|adj=2,2"}, "resolution": "ambiguous", "centroid": [-10.0, -15.0, 20.0]}]}], "parameters": [{"id": "p-h", "value": 50.0, "dimension": {"length": 1, "angle": 0}}], "diagnostics": []}"#,
    r#"{"kind": "event", "type": "sketch_solved", "doc": 1, "revision": 0, "solution": {"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [10.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [10.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [10.0, 0.0]}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}}"#,
    r#"{"kind": "event", "type": "datum_plane", "doc": 1, "revision": 2, "feature": "f-dp1", "placement": {"origin": [0.0, 0.0, 10.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}}"#,
    r#"{"kind": "event", "type": "body_mesh", "doc": 1, "revision": 0, "body": "b-1", "layer": "model", "mesh_id": 1, "mesh": {"positions": 0, "normals": 1, "indices": 2, "face_ids": 3, "edge_positions": 4, "edge_ids": 5, "vertex_positions": 6, "vertex_ids": 7, "face_count": 6, "edge_count": 12, "vertex_count": 8, "bounding_box": {"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}}}"#,
    r#"{"kind": "event", "type": "body_mesh", "doc": 1, "revision": 5, "body": "b-1", "layer": "preview", "mesh_id": 9}"#,
    r#"{"kind": "event", "type": "print_health", "doc": 1, "revision": 4, "report": {"profile": "generic-fdm", "orientation": {"kind": "as_modeled"}, "summary": {"health": "ok", "errors": 0, "warnings": 0, "infos": 0, "label": "Ready to print"}, "bodies": [{"body": "b-1", "transform": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 128.0, 128.0, 0.0, 1.0], "mesh_id": 3, "overhang": 0, "report": {"profile": "generic-fdm", "rotation": {"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]}, "health": "ok", "checks": [{"check": "mesh_validity", "status": "ok", "findings": [], "omitted": 0}, {"check": "bed_fit", "status": "ok", "findings": [], "omitted": 0}, {"check": "first_layer_contact", "status": "ok", "findings": [], "omitted": 0}, {"check": "islands", "status": "ok", "findings": [], "omitted": 0}, {"check": "overhangs", "status": "ok", "findings": [], "omitted": 0}, {"check": "thin_walls", "status": "ok", "findings": [], "omitted": 0}, {"check": "small_features", "status": "ok", "findings": [], "omitted": 0}, {"check": "trapped_volumes", "status": "ok", "findings": [], "omitted": 0}]}}]}}"#,
];

/// The command examples of section 2.5 (request side), in spec order.
pub const COMMAND_EXAMPLES: [&str; 30] = [
    r#"{"type": "engine_info"}"#,
    r#"{"type": "feature_catalog"}"#,
    r#"{"type": "list_printer_profiles"}"#,
    r#"{"type": "put_printer_profile", "profile": {"id": "my-mk4", "name": "My MK4", "technology": "fdm", "layer_height": 0.15}}"#,
    r#"{"type": "new_document", "units": "mm"}"#,
    r#"{"type": "open_document", "text": "{\n  \"format\": \"partsmith\",\n  ...\n}\n", "assets": []}"#,
    r#"{"type": "save_document", "doc": 1}"#,
    r#"{"type": "get_document", "doc": 1}"#,
    r#"{"type": "mark_saved", "doc": 1, "revision": 4}"#,
    r#"{"type": "apply_edits", "doc": 1, "label": "Change height", "edits": [{"type": "update_parameter", "parameter": {"id": "p-h", "name": "height", "expr": "50 mm"}}]}"#,
    r#"{"type": "apply_edits", "doc": 1, "label": "Roll back", "edits": [{"type": "set_rollback", "rollback": "f-fil1"}]}"#,
    r#"{"type": "rename_parameter", "doc": 1, "parameter": "p-h", "name": "h"}"#,
    r#"{"type": "begin_transaction", "doc": 1, "label": "Fillet"}"#,
    r#"{"type": "set_focus", "doc": 1, "feature": "f-fil1"}"#,
    r#"{"type": "commit_transaction", "doc": 1}"#,
    r#"{"type": "solve_sketch", "doc": 1, "feature": "f-sk1"}"#,
    r#"{"type": "drag_sketch", "doc": 1, "feature": "f-sk1", "point": "e-1.end", "to": [20.0, 5.0]}"#,
    r#"{"type": "set_tessellation", "doc": 1, "quality": "fine"}"#,
    r#"{"type": "evaluate", "doc": 1, "expr": "2 * height"}"#,
    r#"{"type": "list_elements", "doc": 1, "body": "b-1", "kind": "face"}"#,
    r#"{"type": "pick_reference", "doc": 1, "pick": {"mesh_id": 12, "kind": "face", "index": 3}}"#,
    r#"{"type": "resolve_references", "doc": 1, "references": [{"body": "b-1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}]}"#,
    r#"{"type": "measure", "doc": 1, "targets": [{"kind": "body", "body": "b-1"}]}"#,
    r#"{"type": "measure", "doc": 1, "targets": [{"kind": "element", "body": "b-1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}]}"#,
    r#"{"type": "analyze_print", "doc": 1, "orientation": {"kind": "as_modeled"}}"#,
    r#"{"type": "export", "doc": 1, "format": {"kind": "stl", "binary": true, "per_body": false}}"#,
    r#"{"type": "export", "doc": 1, "format": {"kind": "3mf"}, "bodies": ["b-1", "b-2"], "orientation": {"kind": "as_modeled"}}"#,
    r#"{"type": "export", "doc": 1, "format": {"kind": "step"}}"#,
    r#"{"type": "import_step", "doc": 1, "data": 0, "name": "Bracket"}"#,
    r#"{"type": "export_python", "doc": 1}"#,
];

/// The result examples of section 2.5 (the `ok` value of a response), in spec order.
pub const RESULT_EXAMPLES: [&str; 24] = [
    r#"{"type": "engine_info", "protocol_version": 1, "engine_version": "0.1.0", "format_version": 1, "kernel": "OCCT 7.9.3"}"#,
    r#"{"type": "feature_catalog", "features": [{"type": "box", "manifest": {"label": "Box", "category": "create", "icon": "box", "version": 1, "fields": []}, "params_schema": {"type": "object", "properties": {"body": {"type": "string"}}, "required": ["body"]}}]}"#,
    r#"{"type": "list_printer_profiles", "profiles": [{"id": "generic-fdm", "name": "Generic FDM 0.4 mm", "technology": "fdm", "bed": {"kind": "rectangle", "width": 256.0, "depth": 256.0, "height": 256.0}, "nozzle_diameter": 0.4, "layer_height": 0.2, "max_overhang_angle": 0.7853981633974483, "max_bridge_length": 10.0, "fit": {"press": 0.05, "snug": 0.1, "slip": 0.2, "loose": 0.4}}]}"#,
    r#"{"type": "put_printer_profile"}"#,
    r#"{"type": "new_document", "doc": 1}"#,
    r#"{"type": "open_document", "doc": 2}"#,
    r#"{"type": "save_document", "text": "{\n  \"format\": \"partsmith\",\n  \"format_version\": 1,\n  \"units\": \"mm\"\n}\n", "assets": [], "revision": 0}"#,
    r#"{"type": "get_document", "revision": 3, "document": {"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [{"id": "p-h", "name": "height", "expr": "40 mm"}], "bodies": [{"id": "b-1", "name": "Block"}], "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "height"], "op": "new"}]}}"#,
    r#"{"type": "mark_saved"}"#,
    r#"{"type": "apply_edits", "revision": 4}"#,
    r#"{"type": "rename_parameter", "revision": 5}"#,
    r#"{"type": "begin_transaction"}"#,
    r#"{"type": "commit_transaction", "revision": 7, "recorded": true}"#,
    r#"{"type": "drag_sketch", "solution": {"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [20.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [20.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [20.0, 0.0]}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}}"#,
    r#"{"type": "set_tessellation", "tolerance": {"chord_deviation": 0.025, "angular_deviation": 0.04363323129985824}}"#,
    r#"{"type": "evaluate", "value": 80.0, "dimension": {"length": 1, "angle": 0}}"#,
    r#"{"type": "pick_reference", "element": {"name": "f-box1/top", "kind": "face", "body": "b-1", "feature": "f-box1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}, "surface": "plane", "centroid": [0.0, 0.0, 40.0], "area": 600.0, "normal": [0.0, 0.0, 1.0], "placement": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "adjacent": ["edge(f-box1/back|f-box1/top)", "edge(f-box1/front|f-box1/top)", "edge(f-box1/left|f-box1/top)", "edge(f-box1/right|f-box1/top)"]}}"#,
    r#"{"type": "resolve_references", "results": [{"status": "resolved", "matches": [{"body": "b-1", "mesh_id": 12, "kind": "face", "index": 3}]}]}"#,
    r#"{"type": "measure", "volume": 24000.0, "area": 5200.0, "center_of_mass": [0.0, 0.0, 20.0], "bounding_box": {"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}, "face_count": 6, "edge_count": 12, "vertex_count": 8}"#,
    r#"{"type": "measure", "area": 600.0, "centroid": [0.0, 0.0, 40.0], "surface": "plane", "normal": [0.0, 0.0, 1.0]}"#,
    r#"{"type": "analyze_print", "health": "warning"}"#,
    r#"{"type": "export", "files": [{"name": "model.stl", "media_type": "model/stl", "data": 0}], "warnings": []}"#,
    r#"{"type": "import_step", "feature": "f-3fa91c0e72b4", "body": "b-0c1d2e3f4a5b"}"#,
    r#"{"type": "export_python", "code": "from partsmith import *\n\ndoc = Document(units=\"mm\")\n"}"#,
];

/// The examples of single types in sections 2.4 and 2.6, each with the `$defs` entry it validates against.
pub const TYPE_EXAMPLES: [(&str, &str); 22] = [
    (
        "Placement",
        r#"{"origin": [0.0, 0.0, 10.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}"#,
    ),
    (
        "BoundingBox",
        r#"{"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}"#,
    ),
    (
        "Axis",
        r#"{"origin": [0.0, 0.0, 0.0], "direction": [0.0, 0.0, 1.0]}"#,
    ),
    ("Pick", r#"{"mesh_id": 12, "kind": "face", "index": 3}"#),
    (
        "MeshElement",
        r#"{"body": "b-1", "mesh_id": 12, "kind": "edge", "index": 4}"#,
    ),
    (
        "ElementRef",
        r#"{"body": "b-1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}"#,
    ),
    ("Orientation", r#"{"kind": "as_modeled"}"#),
    (
        "Orientation",
        r#"{"kind": "matrix", "rows": [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]}"#,
    ),
    (
        "Orientation",
        r#"{"kind": "face_down", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}"#,
    ),
    ("TessellationQuality", r#""standard""#),
    (
        "TessellationTolerance",
        r#"{"chord_deviation": 0.05, "angular_deviation": 0.08726646259971647}"#,
    ),
    (
        "Diagnostic",
        r#"{"severity": "warning", "code": "unknown_printer_profile", "message": "Printer profile \"my-printer\" is not installed, so the default FDM profile is used. Install it or pick another printer."}"#,
    ),
    (
        "Diagnostic",
        r#"{"severity": "error", "code": "fillet_too_large", "message": "Fillet radius 3 mm is too large for the selected edges. The largest radius that works is about 2.3 mm. Try 2.3 mm or less.", "field": "/radius", "suggestion": "2.3 mm"}"#,
    ),
    (
        "AssetData",
        r#"{"name": "9f2c0e5b7a1d4e3f8a6b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d2e1f.step", "data": 0}"#,
    ),
    (
        "ExportFile",
        r#"{"name": "model.stl", "media_type": "model/stl", "data": 0}"#,
    ),
    (
        "ElementDescriptor",
        r#"{"name": "f-box1/top", "kind": "face", "body": "b-1", "feature": "f-box1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}, "surface": "plane", "centroid": [0.0, 0.0, 40.0], "area": 600.0, "normal": [0.0, 0.0, 1.0], "placement": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "adjacent": ["edge(f-box1/back|f-box1/top)", "edge(f-box1/front|f-box1/top)", "edge(f-box1/left|f-box1/top)", "edge(f-box1/right|f-box1/top)"]}"#,
    ),
    (
        "ElementDescriptor",
        r#"{"name": "edge(f-box1/front|f-box1/top)", "kind": "edge", "body": "b-1", "feature": "f-box1", "reference": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-15,40|l=20|adj=2,2"}, "curve": "line", "centroid": [0.0, -15.0, 40.0], "length": 20.0, "direction": [1.0, 0.0, 0.0], "adjacent": ["f-box1/front", "f-box1/top", "vertex(f-box1/front|f-box1/left|f-box1/top)", "vertex(f-box1/front|f-box1/right|f-box1/top)"]}"#,
    ),
    (
        "RegionInfo",
        r#"{"reference": {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}, "area": 4000.0, "centroid": [40.0, 25.0]}"#,
    ),
    (
        "ConstraintState",
        r#"{"id": "c-3", "state": "satisfied", "value": 10.0}"#,
    ),
    (
        "ConstraintState",
        r#"{"id": "c-2", "state": "conflicting"}"#,
    ),
    ("SolvedPoint", r#"{"point": "e-1.end", "at": [20.0, 0.0]}"#),
    (
        "SketchDisplay",
        r#"{"region_positions": 0, "region_indices": 1, "region_ids": 2}"#,
    ),
];
