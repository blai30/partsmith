//! JSON examples copied verbatim from `docs/spec/protocol.md`, plus a few supplementary records so every command type has a request and a result.
//! The last group, `ENGINE_EXAMPLES`, holds engine-filled records: compact uses, causes, messages and repairs as the engine writes them (engine.md and the corpora), which are not spec examples of this crate.

/// Every command type of section 2.5, in the order of test 34.
pub const COMMAND_TYPES: [&str; 34] = [
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
    "apply_fix",
    "set_document_units",
    "list_uses",
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
pub const ERROR_CODES: [&str; 40] = [
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
    "stale_fix",
];

/// The message of `stale_fix` in section 4.
pub const STALE_FIX_MESSAGE: &str = "This fix no longer applies because the document changed after it was offered. Look at the problem again.";

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
    r#"{"type": "solve_sketch", "solution": {"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [10.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [10.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [10.0, 0.0]}], "messages": [{"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (10, 0) at (0, 0) is not connected to other geometry.", "field": "/profile_findings/0", "detail": "open_endpoint entity=e-1 point=0,0"}, {"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (10, 0) at (10, 0) is not connected to other geometry.", "field": "/profile_findings/1", "detail": "open_endpoint entity=e-1 point=10,0"}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}}"#,
    r#"{"type": "end_sketch_drag", "revision": 9}"#,
    r#"{"type": "list_elements", "elements": [{"name": "f-box1/top", "kind": "face", "body": "b-1", "feature": "f-box1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}, "surface": "plane", "centroid": [0.0, 0.0, 40.0], "area": 600.0, "normal": [0.0, 0.0, 1.0], "placement": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "fallback": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "adjacent": ["edge(f-box1/back|f-box1/top)", "edge(f-box1/front|f-box1/top)", "edge(f-box1/left|f-box1/top)", "edge(f-box1/right|f-box1/top)"]}]}"#,
];

/// The `internal_error` outcome of section 2.8, with the panic text in `detail`.
pub const INTERNAL_ERROR_OUTCOME: &str = r#"{"kind": "error", "code": "internal_error", "message": "Partsmith hit a bug while running apply edits. Your document is unchanged. Save a copy, and report this with the details.", "detail": "called `Option::unwrap()` on a `None` value"}"#;

/// The `transaction_open` outcome of section 2.8, with its `finish_transaction` action.
pub const TRANSACTION_OPEN_OUTCOME: &str = r#"{"kind": "error", "code": "transaction_open", "message": "Finish or cancel \"Fillet\" first.", "actions": [{"label": "Finish \"Fillet\" and save", "run": {"kind": "finish_transaction"}}]}"#;

/// The error outcomes of section 2.8.
pub const ERROR_OUTCOMES: [&str; 4] = [
    r#"{"kind": "error", "code": "edit_rejected", "message": "There is no feature with id f-x.", "edit_index": 1}"#,
    r#"{"kind": "error", "code": "expression_error", "message": "Unknown name 'wal'.", "span": {"start": 4, "end": 7}}"#,
    INTERNAL_ERROR_OUTCOME,
    TRANSACTION_OPEN_OUTCOME,
];

/// The sketch solution of the `drag_sketch` result example in section 2.5.
pub const DRAG_SOLUTION: &str = r#"{"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [20.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [20.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [20.0, 0.0]}], "messages": [{"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (20, 0) at (0, 0) is not connected to other geometry.", "field": "/profile_findings/0", "detail": "open_endpoint entity=e-1 point=0,0"}, {"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (20, 0) at (20, 0) is not connected to other geometry.", "field": "/profile_findings/1", "detail": "open_endpoint entity=e-1 point=20,0"}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}"#;

/// The `unknown_printer_profile` diagnostic example of section 2.4: a document message without field, suggestion or any of the newer fields.
pub const UNKNOWN_PRINTER_DIAGNOSTIC: &str = r#"{"severity": "warning", "code": "unknown_printer_profile", "message": "Printer profile \"my-printer\" is not installed, so the default FDM profile is used. Install it or pick another printer."}"#;

/// The `fillet_too_large` diagnostic example of section 2.4.
pub const FILLET_DIAGNOSTIC: &str = r#"{"severity": "error", "code": "fillet_too_large", "message": "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm.", "field": "/radius", "suggestion": "2.3 mm"}"#;

/// The diagnostic of reference stability corpus scenario R16 in section 2.4, with actions, a cause, a detail and a reason.
pub const R16_DIAGNOSTIC: &str = r#"{"severity": "error", "code": "references_not_found", "message": "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.", "field": "/edges", "actions": [{"label": "Use the front edge of the cut made by 'Extrude 1'", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "references_not_found", "field": "/edges", "cause": false, "index": 0, "revision": 7}}}, {"label": "Pick an edge again", "run": {"kind": "pick", "feature": "f-fil1", "path": "/edges/0"}}], "cause": {"text": "This started after you added 'Extrude 1' before this fillet.", "feature": "f-ext2", "actions": [{"label": "Suppress 'Extrude 1'", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "references_not_found", "field": "/edges", "cause": true, "index": 0, "revision": 7}}}, {"label": "Undo", "run": {"kind": "undo"}}]}, "detail": "edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing lost.unmet=true candidates=edge(f-box1/front|f-ext2/lateral/e-1):0.913978", "reason": {"kind": "removed", "by": "f-ext2", "how": {"kind": "cut_away"}}}"#;

/// The action examples of section 2.4: `apply`, `show` without and with `entity`, `undo` and `finish_transaction`.
pub const FIX_ACTIONS: [&str; 5] = [
    r#"{"label": "Use 2.3 mm", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "fillet_too_large", "field": "/radius", "cause": false, "index": 0, "revision": 3}}}"#,
    r#"{"label": "Show 'Datum plane 1'", "run": {"kind": "show", "feature": "f-dp1"}}"#,
    r#"{"label": "Show the line", "run": {"kind": "show", "feature": "f-sk1", "entity": "e-5"}}"#,
    r#"{"label": "Undo", "run": {"kind": "undo"}}"#,
    r#"{"label": "Finish \"Fillet\" and save", "run": {"kind": "finish_transaction"}}"#,
];

/// The loss reason examples of section 2.4.
pub const LOSS_REASONS: [&str; 5] = [
    r#"{"kind": "removed", "by": "f-ext2", "how": {"kind": "no_longer_made", "absence": "through"}}"#,
    r#"{"kind": "other_body", "body": "b-2", "by": "f-split1", "by_suppressed": false, "present": true}"#,
    r#"{"kind": "copy_gone", "feature": "f-lp1", "instance": {"first": 2, "second": 0}}"#,
    r#"{"kind": "entity_deleted", "feature": "f-ext1", "sketch": "f-sk1", "entity": "e-5"}"#,
    r#"{"kind": "followed", "by": "f-sk1"}"#,
];

/// The ghost examples of section 2.4: an element ghost in model coordinates and a ghost in sketch coordinates.
pub const GHOSTS: [&str; 2] = [
    r#"{"positions": 3, "kind": "lines", "frame": "world"}"#,
    r#"{"positions": 4, "kind": "lines", "frame": "sketch"}"#,
];

/// The descriptor of the box fixture's top face in section 2.4, with its `fallback`.
pub const TOP_FACE_DESCRIPTOR: &str = r#"{"name": "f-box1/top", "kind": "face", "body": "b-1", "feature": "f-box1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}, "surface": "plane", "centroid": [0.0, 0.0, 40.0], "area": 600.0, "normal": [0.0, 0.0, 1.0], "placement": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "fallback": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "adjacent": ["edge(f-box1/back|f-box1/top)", "edge(f-box1/front|f-box1/top)", "edge(f-box1/left|f-box1/top)", "edge(f-box1/right|f-box1/top)"]}"#;

/// The descriptor of the box fixture's front top edge in section 2.4, with its `axis_direction`.
pub const FRONT_TOP_EDGE_DESCRIPTOR: &str = r#"{"name": "edge(f-box1/front|f-box1/top)", "kind": "edge", "body": "b-1", "feature": "f-box1", "reference": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-15,40|l=20|adj=2,2"}, "curve": "line", "centroid": [0.0, -15.0, 40.0], "length": 20.0, "direction": [1.0, 0.0, 0.0], "axis_direction": [1.0, 0.0, 0.0], "adjacent": ["f-box1/front", "f-box1/top", "vertex(f-box1/front|f-box1/left|f-box1/top)", "vertex(f-box1/front|f-box1/right|f-box1/top)"]}"#;

/// The full reference a `pick_reference` returns for the front top edge of the right piece of a top split by a slot (section 2.4).
pub const FULL_REFERENCE: &str = r#"{"q": "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", "nb": {"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}}"#;

/// The reference with merged names of naming.md section 2.4: a top face that every copy of a linear pattern merged into.
pub const MERGED_REFERENCE: &str = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4", "merged": {"f-box1/top": ["f-box1/top@f-lp1:*"]}}"#;

/// A `solve_sketch` request whose candidate lies on the face plane of sketch.md section 2.2 that keeps its fallback in body coordinates.
pub const SOLVE_ON_FACE_IN_BODY_FRAME: &str = r#"{"type": "solve_sketch", "doc": 1, "feature": "f-sk2", "candidate": {"plane": {"kind": "face", "body": "b-1", "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}, "fallback": {"origin": [0.0, 0.0, 30.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "fallback_frame": "body"}, "entities": [], "constraints": []}}"#;

/// The status of the fillet of corpus scenario R03 in section 2.7: a message with actions, a cause with a ghost, a detail and a reason, and an element repair with candidates and a ghost.
pub const FEATURE_STATUS_R03: &str = r#"{"id": "f-fil1", "status": "warning", "messages": [{"severity": "warning", "code": "reference_missing", "message": "The top edge of 'Extrude 1' along a line of 'Sketch 1' selected for Edges is gone because that line was deleted from 'Sketch 1', so it was skipped.", "field": "/edges/0", "actions": [{"label": "Use the right top edge of 'Extrude 1'", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "reference_missing", "field": "/edges/0", "cause": false, "index": 0, "revision": 2}}}, {"label": "Use the back top edge of 'Extrude 1'", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "reference_missing", "field": "/edges/0", "cause": false, "index": 1, "revision": 2}}}, {"label": "Remove from Edges", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "reference_missing", "field": "/edges/0", "cause": false, "index": 2, "revision": 2}}}, {"label": "Pick an edge again", "run": {"kind": "pick", "feature": "f-fil1", "path": "/edges/0"}}], "cause": {"text": "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.", "feature": "f-sk1", "actions": [{"label": "Undo", "run": {"kind": "undo"}}, {"label": "Show the line", "run": {"kind": "show", "feature": "f-sk1", "entity": "e-5"}}], "ghost": {"positions": 1, "kind": "lines", "frame": "sketch"}}, "detail": "edges[0] q=edge(f-ext1/end-cap|f-ext1/lateral/e-5) resolution=missing lost.parts=f-ext1/lateral/e-5 candidates=edge(f-ext1/end-cap|f-ext1/lateral/e-2):0.701904,edge(f-ext1/end-cap|f-ext1/lateral/e-3):0.652130", "reason": {"kind": "entity_deleted", "feature": "f-ext1", "sketch": "f-sk1", "entity": "e-5"}}], "repairs": [{"kind": "element", "path": "/edges/0", "body": "b-1", "reference": {"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-5)", "fp": "line|n=0.707107,-0.707107,0|c=75,45,30|l=14.142136|adj=2,2"}, "resolution": "missing", "centroid": [75.0, 45.0, 30.0], "description": "the top edge of 'Extrude 1' along a line of 'Sketch 1'", "candidates": [{"reference": {"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-2)", "fp": "line|n=0,1,0|c=80,25,30|l=50|adj=2,2"}, "description": "the right top edge of 'Extrude 1'", "relation": "on the top of 'Extrude 1', about 21 mm from where it was", "meaning": true, "distance": 20.615528, "score": 0.701904, "element": {"body": "b-1", "mesh_id": 5, "kind": "edge", "index": 7}}, {"reference": {"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-3)", "fp": "line|n=1,0,0|c=40,50,30|l=80|adj=2,2"}, "description": "the back top edge of 'Extrude 1'", "relation": "on the top of 'Extrude 1', about 35 mm from where it was", "meaning": true, "distance": 35.355339, "score": 0.65213, "element": {"body": "b-1", "mesh_id": 5, "kind": "edge", "index": 8}}], "ghost": {"positions": 0, "kind": "lines", "frame": "world"}}]}"#;

/// The element repair of the R03 status example in section 2.7 (test 68).
pub const ELEMENT_REPAIR: &str = r#"{"kind": "element", "path": "/edges/0", "body": "b-1", "reference": {"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-5)", "fp": "line|n=0.707107,-0.707107,0|c=75,45,30|l=14.142136|adj=2,2"}, "resolution": "missing", "centroid": [75.0, 45.0, 30.0], "description": "the top edge of 'Extrude 1' along a line of 'Sketch 1'", "candidates": [{"reference": {"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-2)", "fp": "line|n=0,1,0|c=80,25,30|l=50|adj=2,2"}, "description": "the right top edge of 'Extrude 1'", "relation": "on the top of 'Extrude 1', about 21 mm from where it was", "meaning": true, "distance": 20.615528, "score": 0.701904, "element": {"body": "b-1", "mesh_id": 5, "kind": "edge", "index": 7}}, {"reference": {"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-3)", "fp": "line|n=1,0,0|c=40,50,30|l=80|adj=2,2"}, "description": "the back top edge of 'Extrude 1'", "relation": "on the top of 'Extrude 1', about 35 mm from where it was", "meaning": true, "distance": 35.355339, "score": 0.65213, "element": {"body": "b-1", "mesh_id": 5, "kind": "edge", "index": 8}}], "ghost": {"positions": 0, "kind": "lines", "frame": "world"}}"#;

/// The region repair of test 68, with two candidate sets and a ghost in buffer 2.
pub const REGION_REPAIR: &str = r#"{"kind":"region","path":"/profile/regions/0","sketch":"f-sk1","region":{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000","outline":"e-1,e-2,e-3,e-4"},"resolution":"ambiguous","description":"the region of 'Sketch 1' around (40, 25)","candidates":[{"regions":[{"key":"e-1,e-3,e-4,e-9","fp":"c=40,25|a=4000","outline":"e-1,e-9,e-3,e-4"}],"description":"the region of 'Sketch 1' around (40, 25)"},{"regions":[{"key":"e-1,e-5,e-6,e-7","fp":"c=175,10|a=1000","outline":"e-1,e-5,e-6,e-7"}],"description":"the region of 'Sketch 1' around (175, 10)"}],"ghost":{"positions":2,"kind":"lines","frame":"sketch"}}"#;

/// The `put_printer_profile` request example of section 2.5: a profile in the input form.
pub const PUT_PRINTER_PROFILE_REQUEST: &str = r#"{"type": "put_printer_profile", "profile": {"id": "my-mk4", "name": "My MK4", "technology": "fdm", "layer_height": 0.15}}"#;

/// The `list_printer_profiles` result example of section 2.5: `generic-fdm` in the written form.
pub const LIST_PRINTER_PROFILES_RESULT: &str = r#"{"type": "list_printer_profiles", "profiles": [{"id": "generic-fdm", "name": "Generic FDM 0.4 mm", "technology": "fdm", "bed": {"kind": "rectangle", "width": 256.0, "depth": 256.0, "height": 256.0}, "nozzle_diameter": 0.4, "layer_height": 0.2, "max_overhang_angle": 0.7853981633974483, "max_bridge_length": 10.0, "fit": {"press": 0.05, "snug": 0.1, "slip": 0.2, "loose": 0.4}}]}"#;

/// The `apply_fix` request example of section 2.5.
pub const APPLY_FIX_REQUEST: &str = r#"{"type": "apply_fix", "doc": 1, "fix": {"feature": "f-fil1", "code": "references_not_found", "field": "/edges", "cause": false, "index": 0, "revision": 7}}"#;

/// The `apply_fix` result example of section 2.5.
pub const APPLY_FIX_RESULT: &str = r#"{"type": "apply_fix", "revision": 8}"#;

/// The `set_document_units` request example of section 2.5.
pub const SET_DOCUMENT_UNITS_REQUEST: &str =
    r#"{"type": "set_document_units", "doc": 1, "units": "in", "keep_sizes": true}"#;

/// The `set_document_units` result example of section 2.5.
pub const SET_DOCUMENT_UNITS_RESULT: &str =
    r#"{"type": "set_document_units", "revision": 5, "kept": 4}"#;

/// The `list_uses` request example of section 2.5.
pub const LIST_USES_REQUEST: &str = r#"{"type": "list_uses", "doc": 1}"#;

/// The `list_uses` result example of section 2.5: a link, then a use of a feature and of its sketch entity.
pub const LIST_USES_RESULT: &str = r#"{"type": "list_uses", "uses": [{"user": "f-ext1", "field": "/profile/sketch", "feature": "f-sk1"}, {"user": "f-fil1", "field": "/edges/0", "feature": "f-ext1"}, {"user": "f-fil1", "field": "/edges/0", "feature": "f-ext1", "entity": "e-3"}]}"#;

/// The `document_changed` event example of section 2.7: a dirty document after an edit, with `bare_numbers` last.
pub const DOCUMENT_CHANGED_EVENT: &str = r#"{"kind": "event", "type": "document_changed", "doc": 1, "revision": 4, "document": {"format": "partsmith", "format_version": 2, "units": "mm", "parameters": [{"id": "p-h", "name": "height", "expr": "50 mm"}], "bodies": [{"id": "b-1", "name": "Block"}], "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "height"], "op": "new"}]}, "dirty": true, "undo": "Change height", "transaction_open": false, "bare_numbers": 0}"#;

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

/// The `sketch_solved` event example of section 2.7: an open line with its two finding messages.
pub const SKETCH_SOLVED_EVENT: &str = r#"{"kind": "event", "type": "sketch_solved", "doc": 1, "revision": 0, "solution": {"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [10.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [10.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [10.0, 0.0]}], "messages": [{"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (10, 0) at (0, 0) is not connected to other geometry.", "field": "/profile_findings/0", "detail": "open_endpoint entity=e-1 point=0,0"}, {"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (10, 0) at (10, 0) is not connected to other geometry.", "field": "/profile_findings/1", "detail": "open_endpoint entity=e-1 point=10,0"}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}}"#;

/// The examples of sections 2.3 and 2.7 whose top level is a message, in spec order.
pub const MESSAGE_EXAMPLES: [&str; 11] = [
    r#"{"kind": "request", "id": 7, "command": {"type": "undo", "doc": 1}}"#,
    r#"{"kind": "response", "id": 7, "result": {"kind": "ok", "type": "undo", "revision": 6, "label": "Change height"}}"#,
    r#"{"kind": "response", "id": 8, "result": {"kind": "error", "code": "nothing_to_undo", "message": "There is nothing to undo."}}"#,
    r#"{"kind": "event", "type": "document_changed", "doc": 1, "revision": 6, "document": {"format": "partsmith", "format_version": 2, "units": "mm", "parameters": [], "bodies": [], "features": []}, "dirty": false, "transaction_open": false, "bare_numbers": 0}"#,
    DOCUMENT_CHANGED_EVENT,
    r#"{"kind": "event", "type": "recomputed", "doc": 1, "revision": 4, "features": [{"id": "f-box1", "status": "ok", "messages": [], "repairs": []}], "parameters": [{"id": "p-h", "value": 50.0, "dimension": {"length": 1, "angle": 0}}], "diagnostics": []}"#,
    SKETCH_SOLVED_EVENT,
    r#"{"kind": "event", "type": "datum_plane", "doc": 1, "revision": 2, "feature": "f-dp1", "placement": {"origin": [0.0, 0.0, 10.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}}"#,
    r#"{"kind": "event", "type": "body_mesh", "doc": 1, "revision": 0, "body": "b-1", "layer": "model", "mesh_id": 1, "mesh": {"positions": 0, "normals": 1, "indices": 2, "face_ids": 3, "edge_positions": 4, "edge_ids": 5, "vertex_positions": 6, "vertex_ids": 7, "face_count": 6, "edge_count": 12, "vertex_count": 8, "bounding_box": {"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}}}"#,
    r#"{"kind": "event", "type": "body_mesh", "doc": 1, "revision": 5, "body": "b-1", "layer": "preview", "mesh_id": 9}"#,
    r#"{"kind": "event", "type": "print_health", "doc": 1, "revision": 4, "report": {"profile": "generic-fdm", "orientation": {"kind": "as_modeled"}, "summary": {"health": "ok", "errors": 0, "warnings": 0, "infos": 0, "label": "Ready to print"}, "bodies": [{"body": "b-1", "transform": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 128.0, 128.0, 0.0, 1.0], "mesh_id": 3, "overhang": 0, "report": {"profile": "generic-fdm", "rotation": {"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]}, "health": "ok", "checks": [{"check": "mesh_validity", "status": "ok", "findings": [], "omitted": 0}, {"check": "bed_fit", "status": "ok", "findings": [], "omitted": 0}, {"check": "first_layer_contact", "status": "ok", "findings": [], "omitted": 0}, {"check": "islands", "status": "ok", "findings": [], "omitted": 0}, {"check": "overhangs", "status": "ok", "findings": [], "omitted": 0}, {"check": "thin_walls", "status": "ok", "findings": [], "omitted": 0}, {"check": "small_features", "status": "ok", "findings": [], "omitted": 0}, {"check": "trapped_volumes", "status": "ok", "findings": [], "omitted": 0}]}}]}}"#,
];

/// The command examples of section 2.5 (request side), in spec order.
pub const COMMAND_EXAMPLES: [&str; 33] = [
    r#"{"type": "engine_info"}"#,
    r#"{"type": "feature_catalog"}"#,
    r#"{"type": "list_printer_profiles"}"#,
    PUT_PRINTER_PROFILE_REQUEST,
    r#"{"type": "new_document", "units": "mm"}"#,
    r#"{"type": "open_document", "text": "{\n  \"format\": \"partsmith\",\n  ...\n}\n", "assets": []}"#,
    r#"{"type": "save_document", "doc": 1}"#,
    r#"{"type": "get_document", "doc": 1}"#,
    r#"{"type": "mark_saved", "doc": 1, "revision": 4}"#,
    r#"{"type": "apply_edits", "doc": 1, "label": "Change height", "edits": [{"type": "update_parameter", "parameter": {"id": "p-h", "name": "height", "expr": "50 mm"}}]}"#,
    r#"{"type": "apply_edits", "doc": 1, "label": "Roll back", "edits": [{"type": "set_rollback", "rollback": "f-fil1"}]}"#,
    r#"{"type": "rename_parameter", "doc": 1, "parameter": "p-h", "name": "h"}"#,
    APPLY_FIX_REQUEST,
    SET_DOCUMENT_UNITS_REQUEST,
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
    LIST_USES_REQUEST,
    r#"{"type": "analyze_print", "doc": 1, "orientation": {"kind": "as_modeled"}}"#,
    r#"{"type": "export", "doc": 1, "format": {"kind": "stl", "binary": true, "per_body": false}}"#,
    r#"{"type": "export", "doc": 1, "format": {"kind": "3mf"}, "bodies": ["b-1", "b-2"], "orientation": {"kind": "as_modeled"}}"#,
    r#"{"type": "export", "doc": 1, "format": {"kind": "step"}}"#,
    r#"{"type": "import_step", "doc": 1, "data": 0, "name": "Bracket"}"#,
    r#"{"type": "export_python", "doc": 1}"#,
];

/// The result examples of section 2.5 (the `ok` value of a response), in spec order.
pub const RESULT_EXAMPLES: [&str; 27] = [
    r#"{"type": "engine_info", "protocol_version": 1, "engine_version": "0.1.0", "format_version": 2, "kernel": "OCCT 7.9.3"}"#,
    r#"{"type": "feature_catalog", "features": [{"type": "box", "manifest": {"label": "Box", "category": "create", "icon": "box", "version": 1, "fields": []}, "params_schema": {"type": "object", "properties": {"body": {"type": "string"}}, "required": ["body"]}}]}"#,
    LIST_PRINTER_PROFILES_RESULT,
    r#"{"type": "put_printer_profile"}"#,
    r#"{"type": "new_document", "doc": 1}"#,
    r#"{"type": "open_document", "doc": 2}"#,
    r#"{"type": "save_document", "text": "{\n  \"format\": \"partsmith\",\n  \"format_version\": 2,\n  \"units\": \"mm\"\n}\n", "assets": [], "revision": 0}"#,
    r#"{"type": "get_document", "revision": 3, "document": {"format": "partsmith", "format_version": 2, "units": "mm", "parameters": [{"id": "p-h", "name": "height", "expr": "40 mm"}], "bodies": [{"id": "b-1", "name": "Block"}], "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "height"], "op": "new"}]}}"#,
    r#"{"type": "mark_saved"}"#,
    r#"{"type": "apply_edits", "revision": 4}"#,
    r#"{"type": "rename_parameter", "revision": 5}"#,
    APPLY_FIX_RESULT,
    SET_DOCUMENT_UNITS_RESULT,
    r#"{"type": "begin_transaction"}"#,
    r#"{"type": "commit_transaction", "revision": 7, "recorded": true}"#,
    r#"{"type": "drag_sketch", "solution": {"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [20.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [20.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [20.0, 0.0]}], "messages": [{"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (20, 0) at (0, 0) is not connected to other geometry.", "field": "/profile_findings/0", "detail": "open_endpoint entity=e-1 point=0,0"}, {"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (20, 0) at (20, 0) is not connected to other geometry.", "field": "/profile_findings/1", "detail": "open_endpoint entity=e-1 point=20,0"}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}}"#,
    r#"{"type": "set_tessellation", "tolerance": {"chord_deviation": 0.025, "angular_deviation": 0.04363323129985824}}"#,
    r#"{"type": "evaluate", "value": 80.0, "dimension": {"length": 1, "angle": 0}}"#,
    r#"{"type": "pick_reference", "element": {"name": "f-box1/top", "kind": "face", "body": "b-1", "feature": "f-box1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}, "surface": "plane", "centroid": [0.0, 0.0, 40.0], "area": 600.0, "normal": [0.0, 0.0, 1.0], "placement": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "fallback": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "adjacent": ["edge(f-box1/back|f-box1/top)", "edge(f-box1/front|f-box1/top)", "edge(f-box1/left|f-box1/top)", "edge(f-box1/right|f-box1/top)"]}}"#,
    r#"{"type": "resolve_references", "results": [{"status": "resolved", "matches": [{"body": "b-1", "mesh_id": 12, "kind": "face", "index": 3}]}]}"#,
    r#"{"type": "measure", "volume": 24000.0, "area": 5200.0, "center_of_mass": [0.0, 0.0, 20.0], "bounding_box": {"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}, "face_count": 6, "edge_count": 12, "vertex_count": 8}"#,
    r#"{"type": "measure", "area": 600.0, "centroid": [0.0, 0.0, 40.0], "surface": "plane", "normal": [0.0, 0.0, 1.0]}"#,
    LIST_USES_RESULT,
    r#"{"type": "analyze_print", "health": "warning"}"#,
    r#"{"type": "export", "files": [{"name": "model.stl", "media_type": "model/stl", "data": 0}], "warnings": []}"#,
    r#"{"type": "import_step", "feature": "f-3fa91c0e72b4", "body": "b-0c1d2e3f4a5b"}"#,
    r#"{"type": "export_python", "code": "from partsmith import *\n\ndoc = Document(units=\"mm\")\n"}"#,
];

/// The examples of single types in sections 2.4, 2.6 and 2.7, each with the `$defs` entry it validates against.
pub const TYPE_EXAMPLES: [(&str, &str); 37] = [
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
    ("Diagnostic", UNKNOWN_PRINTER_DIAGNOSTIC),
    ("Diagnostic", FILLET_DIAGNOSTIC),
    (
        "AssetData",
        r#"{"name": "9f2c0e5b7a1d4e3f8a6b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d2e1f.step", "data": 0}"#,
    ),
    (
        "ExportFile",
        r#"{"name": "model.stl", "media_type": "model/stl", "data": 0}"#,
    ),
    ("Diagnostic", R16_DIAGNOSTIC),
    ("FixAction", FIX_ACTIONS[0]),
    ("FixAction", FIX_ACTIONS[1]),
    ("FixAction", FIX_ACTIONS[2]),
    ("FixAction", FIX_ACTIONS[3]),
    ("FixAction", FIX_ACTIONS[4]),
    ("LossReason", LOSS_REASONS[0]),
    ("LossReason", LOSS_REASONS[1]),
    ("LossReason", LOSS_REASONS[2]),
    ("LossReason", LOSS_REASONS[3]),
    ("LossReason", LOSS_REASONS[4]),
    ("Ghost", GHOSTS[0]),
    ("Ghost", GHOSTS[1]),
    ("ElementDescriptor", TOP_FACE_DESCRIPTOR),
    ("ElementDescriptor", FRONT_TOP_EDGE_DESCRIPTOR),
    ("Reference", FULL_REFERENCE),
    (
        "RegionInfo",
        r#"{"reference": {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}, "area": 4000.0, "centroid": [40.0, 25.0]}"#,
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
    ("FeatureStatus", FEATURE_STATUS_R03),
];

/// The `list_uses` result of engine.md test 143, compact: BLOCK with a fillet on the top edge along `e-3` and a sketch on the side made from `e-2`. Not a spec example of this crate.
pub const BLOCK_USES_RESULT: &str = r#"{"type":"list_uses","uses":[{"user":"f-ext1","field":"/profile/sketch","feature":"f-sk1"},{"user":"f-fil1","field":"/edges/0","feature":"f-ext1"},{"user":"f-fil1","field":"/edges/0","feature":"f-ext1","entity":"e-3"},{"user":"f-sk2","field":"/plane/face","feature":"f-ext1","entity":"e-2"}]}"#;

/// The cause of 'Sketch 2' in corpus R26 after its delete (engine.md section 3.21 and test 136), compact: a deleted feature, Keep the sketch where it was, then Undo. Not a spec example of this crate.
pub const R26_CAUSE: &str = r#"{"text":"This started after you deleted the former 'Extrude 1'.","feature":"f-ext1","actions":[{"label":"Keep the sketch where it was","run":{"kind":"apply","fix":{"feature":"f-sk2","code":"plane_fallback","field":"/plane/face","cause":true,"index":0,"revision":2}}},{"label":"Undo","run":{"kind":"undo"}}]}"#;

/// The warning of a hole whose second point was deleted from its sketch (corpus R43b, features/hole.md test 34), compact, with a cause line whose ghost is the deleted point in sketch coordinates, buffer 1 (engine.md sections 3.21 and 3.23). Not a spec example of this crate.
pub const DELETED_POINT_MESSAGE: &str = r#"{"severity":"warning","code":"point_missing","message":"This hole no longer makes the hole at a point that was deleted from 'Sketch 1', so it was skipped.","field":"/points/1","actions":[{"label":"Remove from Points","run":{"kind":"apply","fix":{"feature":"f-hole1","code":"point_missing","field":"/points/1","cause":false,"index":0,"revision":3}}},{"label":"Pick a point again","run":{"kind":"pick","feature":"f-hole1","path":"/points/1"}}],"cause":{"text":"This started after you deleted the point at (20, 10) in 'Sketch 1'.","feature":"f-sk1","actions":[{"label":"Undo","run":{"kind":"undo"}},{"label":"Show the point","run":{"kind":"show","feature":"f-sk1","entity":"e-p9"}}],"ghost":{"positions":1,"kind":"points","frame":"sketch"}}}"#;

/// The error of a datum plane on the right side of 'Extrude 1' (BLOCK) after 'Extrude 2', inserted before it, cut that face away and left no face to offer in its place, compact: features.md section 4.1 `reference_not_found` with its one action, Pick, and a cause line like R16's (engine.md section 3.21). Its repair is `LOST_FACE_REPAIR`. Not a spec example of this crate.
pub const LOST_FACE_MESSAGE: &str = r#"{"severity":"error","code":"reference_not_found","message":"The right side of 'Extrude 1' selected for Base plane was cut away by 'Extrude 2', which comes before this datum plane.","field":"/base/face","actions":[{"label":"Pick a face again","run":{"kind":"pick","feature":"f-dp1","path":"/base/face"}}],"cause":{"text":"This started after you added 'Extrude 2' before this datum plane.","feature":"f-ext2","actions":[{"label":"Suppress 'Extrude 2'","run":{"kind":"apply","fix":{"feature":"f-dp1","code":"reference_not_found","field":"/base/face","cause":true,"index":0,"revision":3}}},{"label":"Undo","run":{"kind":"undo"}}]},"detail":"base.face q=f-ext1/lateral/e-2 resolution=missing lost.parts=f-ext1/lateral/e-2","reason":{"kind":"removed","by":"f-ext2","how":{"kind":"cut_away"}}}"#;

/// The repair of `LOST_FACE_MESSAGE`, compact, with the face's ghost: triangles in model coordinates, buffer 0 (engine.md section 3.23). Not a spec example of this crate.
pub const LOST_FACE_REPAIR: &str = r#"{"kind":"element","path":"/base/face","body":"b-1","reference":{"q":"f-ext1/lateral/e-2","fp":"plane|n=1,0,0|c=80,25,15|a=1500|adj=4,4"},"resolution":"missing","centroid":[80.0,25.0,15.0],"description":"the right side of 'Extrude 1'","candidates":[],"ghost":{"positions":0,"kind":"triangles","frame":"world"}}"#;

/// The warning of an extrude whose first region now matches the two separate areas of `REGION_REPAIR` while its profile keeps another region, compact: features.md section 4.2 `region_ambiguous`, with a Use action per candidate set, then Pick (engine.md section 3.22). Not a spec example of this crate.
pub const REGION_AMBIGUOUS_MESSAGE: &str = r#"{"severity":"warning","code":"region_ambiguous","message":"The region of 'Sketch 1' selected for Profile now matches 2 separate areas, so it was skipped. Pick the one you mean.","field":"/profile/regions/0","actions":[{"label":"Use the region of 'Sketch 1' around (40, 25)","run":{"kind":"apply","fix":{"feature":"f-ext1","code":"region_ambiguous","field":"/profile/regions/0","cause":false,"index":0,"revision":2}}},{"label":"Use the region of 'Sketch 1' around (175, 10)","run":{"kind":"apply","fix":{"feature":"f-ext1","code":"region_ambiguous","field":"/profile/regions/0","cause":false,"index":1,"revision":2}}},{"label":"Pick a region again","run":{"kind":"pick","feature":"f-ext1","path":"/profile/regions/0"}}]}"#;

/// The engine-filled records, each with the `$defs` entry it validates against.
pub const ENGINE_EXAMPLES: [(&str, &str); 6] = [
    ("CommandResult", BLOCK_USES_RESULT),
    ("Cause", R26_CAUSE),
    ("Diagnostic", DELETED_POINT_MESSAGE),
    ("Diagnostic", LOST_FACE_MESSAGE),
    ("Repair", LOST_FACE_REPAIR),
    ("Diagnostic", REGION_AMBIGUOUS_MESSAGE),
];
