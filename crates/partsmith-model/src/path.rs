//! Locations inside JSON values, for error messages (file-format.md section 3.7).

use std::fmt;

/// A location inside a JSON value: a chain of borrowed segments from the root, so nothing is
/// allocated until an error needs the text. Displayed as object keys joined by `.` with array
/// indices as `[i]`; the root is the empty string.
#[derive(Clone, Copy, Debug)]
pub(crate) struct JsonPath<'a> {
    /// The path this one extends, or `None` for the root.
    parent: Option<&'a JsonPath<'a>>,
    /// The last step.
    segment: Segment<'a>,
}

/// One step of a path.
#[derive(Clone, Copy, Debug)]
enum Segment<'a> {
    /// The root itself.
    Root,
    /// An object member.
    Key(&'a str),
    /// An array element.
    Index(usize),
}

impl<'a> JsonPath<'a> {
    /// The root of a value.
    pub(crate) const ROOT: JsonPath<'static> = JsonPath {
        parent: None,
        segment: Segment::Root,
    };

    /// The member `key` of the object at this path.
    pub(crate) fn key(&'a self, key: &'a str) -> JsonPath<'a> {
        JsonPath {
            parent: Some(self),
            segment: Segment::Key(key),
        }
    }

    /// The element `index` of the array at this path.
    pub(crate) fn index(&'a self, index: usize) -> JsonPath<'a> {
        JsonPath {
            parent: Some(self),
            segment: Segment::Index(index),
        }
    }

    /// True for the root.
    pub(crate) fn is_root(&self) -> bool {
        self.parent.is_none()
    }
}

impl fmt::Display for JsonPath<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut segments = Vec::new();
        let mut current = Some(self);
        while let Some(path) = current {
            segments.push(path.segment);
            current = path.parent;
        }
        let mut wrote_segment = false;
        for segment in segments.into_iter().rev() {
            match segment {
                Segment::Root => {}
                Segment::Key(key) => {
                    if wrote_segment {
                        formatter.write_str(".")?;
                    }
                    formatter.write_str(key)?;
                }
                Segment::Index(index) => write!(formatter, "[{index}]")?,
            }
            wrote_segment = !matches!(segment, Segment::Root);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_is_empty() {
        assert_eq!(JsonPath::ROOT.to_string(), "");
        assert!(JsonPath::ROOT.is_root());
    }

    #[test]
    fn keys_join_with_dots_and_indices_use_brackets() {
        let root = JsonPath::ROOT;
        let bodies = root.key("bodies");
        let first = bodies.index(0);
        let color = first.key("color");

        assert_eq!(color.to_string(), "bodies[0].color");
        assert!(!color.is_root());
    }

    #[test]
    fn an_index_at_the_root() {
        let root = JsonPath::ROOT;
        let element = root.index(2);
        let key = element.key("x");

        assert_eq!(element.to_string(), "[2]");
        assert_eq!(key.to_string(), "[2].x");
    }
}
