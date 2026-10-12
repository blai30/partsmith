//! Field paths inside params and the fields named in messages (features.md section 3.2).

use partsmith_model::FeatureManifest;

/// A params field named in a message: its path and its manifest label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldRef {
    /// Field path, for example "extent.expr".
    pub path: String,
    /// Manifest label, for example "Distance". Falls back to the path when the manifest has no hint.
    pub label: String,
}

impl FieldRef {
    /// The field at `path` with its label from `manifest` (section 3.2): the hint with this exact
    /// path, else the hint with this path with every index written `[*]`, else the longest hint
    /// whose path is a prefix of this one followed by `.` or `[`, else the path itself.
    pub fn new(path: &str, manifest: &FeatureManifest) -> FieldRef {
        let any_index = with_any_index(path);
        let exact = manifest
            .fields
            .iter()
            .find(|hint| hint.path == path)
            .or_else(|| manifest.fields.iter().find(|hint| hint.path == any_index));
        let label = exact
            .or_else(|| {
                manifest
                    .fields
                    .iter()
                    .filter(|hint| {
                        is_parent_path(&hint.path, path) || is_parent_path(&hint.path, &any_index)
                    })
                    .max_by_key(|hint| hint.path.len())
            })
            .map_or(path, |hint| hint.label.as_str());
        FieldRef {
            path: path.to_string(),
            label: label.to_string(),
        }
    }

    /// The path as a JSON Pointer into the feature record, for example "edges[1]" gives "/edges/1".
    pub fn pointer(&self) -> String {
        pointer(&self.path)
    }
}

/// One step of a field path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Segment<'a> {
    /// A params key.
    Name(&'a str),
    /// A 0-based array index.
    Index(usize),
    /// `[*]`: any index (manifest hints only).
    AnyIndex,
}

/// The steps of `path` by the grammar `name ( "." name | "[" index "]" )*`, where a hint may also
/// use `[*]`. None when the path does not follow the grammar.
pub(crate) fn segments(path: &str) -> Option<Vec<Segment<'_>>> {
    let mut steps = Vec::new();
    let mut rest = path;
    let mut expect_name = true;
    while !rest.is_empty() || expect_name {
        if expect_name {
            let end = rest.find(['.', '[', ']']).unwrap_or(rest.len());
            if end == 0 {
                return None;
            }
            steps.push(Segment::Name(&rest[..end]));
            rest = &rest[end..];
            expect_name = false;
        } else if let Some(after) = rest.strip_prefix('.') {
            rest = after;
            expect_name = true;
        } else {
            let after = rest.strip_prefix('[')?;
            let end = after.find(']')?;
            let index = &after[..end];
            steps.push(if index == "*" {
                Segment::AnyIndex
            } else if !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit()) {
                Segment::Index(index.parse().ok()?)
            } else {
                return None;
            });
            rest = &after[end + 1..];
        }
    }
    Some(steps)
}

/// The JSON Pointer of a field path: `.name` and `[index]` become `/name` and `/index`, with `~`
/// and `/` inside names escaped as RFC 6901 requires. The empty path is the empty pointer.
pub(crate) fn pointer(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut pointer = String::with_capacity(path.len() + 1);
    for step in path.split(['.', '[']) {
        pointer.push('/');
        let step = step.strip_suffix(']').unwrap_or(step);
        pointer.push_str(&step.replace('~', "~0").replace('/', "~1"));
    }
    pointer
}

/// `path` with every `[<digits>]` written `[*]`.
fn with_any_index(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut rest = path;
    while let Some(start) = rest.find('[') {
        result.push_str(&rest[..=start]);
        let after = &rest[start + 1..];
        match after.find(']') {
            Some(end) if end > 0 && after[..end].bytes().all(|byte| byte.is_ascii_digit()) => {
                result.push('*');
                rest = &after[end..];
            }
            _ => rest = after,
        }
    }
    result.push_str(rest);
    result
}

/// True when `parent` is a proper prefix of `path` ending at a `.` or `[` of `path`.
fn is_parent_path(parent: &str, path: &str) -> bool {
    path.strip_prefix(parent)
        .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
}

#[cfg(test)]
mod tests {
    use partsmith_model::{Category, FieldHint};

    use super::*;

    fn manifest(paths: &[(&str, &str)]) -> FeatureManifest {
        FeatureManifest {
            label: "Test".to_string(),
            category: Category::Create,
            icon: "test".to_string(),
            version: 1,
            fields: paths
                .iter()
                .map(|(path, label)| FieldHint {
                    path: path.to_string(),
                    label: label.to_string(),
                    essential: false,
                    unit: None,
                    selection: None,
                    options: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn pointer_of_a_plain_name() {
        assert_eq!(pointer("body"), "/body");
    }

    #[test]
    fn pointer_of_nested_indices() {
        assert_eq!(pointer("a[1][2].b"), "/a/1/2/b");
    }

    #[test]
    fn pointer_escapes_tilde_and_slash() {
        assert_eq!(pointer("a~b.c/d"), "/a~0b/c~1d");
    }

    #[test]
    fn pointer_of_the_empty_path_is_empty() {
        assert_eq!(pointer(""), "");
    }

    #[test]
    fn segments_of_a_full_path() {
        assert_eq!(
            segments("constraints[*].expr"),
            Some(vec![
                Segment::Name("constraints"),
                Segment::AnyIndex,
                Segment::Name("expr")
            ])
        );
        assert_eq!(
            segments("size[2]"),
            Some(vec![Segment::Name("size"), Segment::Index(2)])
        );
    }

    #[test]
    fn segments_reject_malformed_paths() {
        for path in ["", ".a", "a.", "a[", "a[]", "a[x]", "a]b", "a..b", "[0]"] {
            assert_eq!(segments(path), None, "path {path:?}");
        }
    }

    #[test]
    fn any_index_replaces_only_numeric_indices() {
        assert_eq!(with_any_index("a[12].b[3]"), "a[*].b[*]");
        assert_eq!(with_any_index("a[x]"), "a[x]");
    }

    #[test]
    fn exact_hint_wins_over_a_prefix() {
        let manifest = manifest(&[("extent", "Extent"), ("extent.expr", "Distance")]);

        assert_eq!(FieldRef::new("extent.expr", &manifest).label, "Distance");
        assert_eq!(FieldRef::new("extent.kind", &manifest).label, "Extent");
    }

    #[test]
    fn prefix_under_an_any_index_hint() {
        let manifest = manifest(&[("points[*]", "Point")]);

        assert_eq!(FieldRef::new("points[3].x", &manifest).label, "Point");
    }

    #[test]
    fn unrelated_path_keeps_its_own_text() {
        let manifest = manifest(&[("body", "Body")]);

        assert_eq!(FieldRef::new("bodyx", &manifest).label, "bodyx");
    }
}
