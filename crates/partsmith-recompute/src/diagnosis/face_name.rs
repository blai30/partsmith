//! Face names as the loss diagnosis reads them (naming.md section 3.1), on partsmith-naming's
//! `ElementName::face_name` view: the feature a name belongs to and its copy groups.

use partsmith_model::Id;
use partsmith_naming::{ElementName, FaceName};

/// The parts of a face name, or of a direct-generator name after its `edge:` or `vertex:` prefix
/// (recompute.md section 3.10); None for a face-rule edge or vertex name.
pub(crate) fn own_parts(name: &ElementName) -> Option<FaceName<'_>> {
    name.face_name().or_else(|| name.direct_parts())
}

/// The feature that starts `name`, or for a name with `@` groups the feature of its last group,
/// as an id, read from [`own_parts`]; None for a face-rule edge or vertex name.
pub(crate) fn maker_of(name: &ElementName) -> Option<Id> {
    Id::new(own_parts(name)?.maker()).ok()
}

/// True when the face name `text` has the `@<feature>:<instance>` group `group` (written
/// `@<feature>:<instance>`), wherever it stands.
pub(crate) fn has_copy_group(text: &str, group: &str) -> bool {
    text.match_indices(group).any(|(start, _)| {
        !text[start + group.len()..]
            .starts_with(|character: char| character.is_ascii_digit() || character == '.')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{id, name};

    #[test]
    fn makers_are_the_feature_of_the_last_copy_group() {
        assert_eq!(maker_of(&name("f-box1/top")), Some(id("f-box1")));
        assert_eq!(
            maker_of(&name("f-box1/top@f-m1:1@f-lp1:2")),
            Some(id("f-lp1"))
        );
    }

    #[test]
    fn makers_of_direct_generator_names_are_read_from_their_body() {
        // recompute.md section 3.10, direct-generator queries: the feature of the last `@` group,
        // else the feature after the `edge:` or `vertex:` prefix.
        assert_eq!(
            maker_of(&name("edge:f-hel1/helix@f-lp1:2")),
            Some(id("f-lp1"))
        );
        assert_eq!(maker_of(&name("edge:f-hel1/helix")), Some(id("f-hel1")));
        assert_eq!(maker_of(&name("vertex:f-hel1/start")), Some(id("f-hel1")));
    }

    #[test]
    fn face_rule_edge_names_have_no_maker() {
        assert_eq!(maker_of(&name("edge(f-box1/front|f-box1/top)")), None);
    }

    #[test]
    fn copy_groups_match_whole_instances_only() {
        let copied = name("f-box1/top@f-lp1:2");
        let copies = copied.face_name().expect("a face name").copies();
        let group = copies.last().expect("a copy group").text;

        assert_eq!(group, "@f-lp1:2");
        assert!(has_copy_group("f-box1/top@f-lp1:2", group));
        assert!(has_copy_group("f-box1/top@f-lp1:2#1", group));
        assert!(!has_copy_group("f-box1/top@f-lp1:20", group));
        assert!(!has_copy_group("f-box1/top@f-lp1:2.1", group));
    }
}
