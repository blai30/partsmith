//! Face names as the loss diagnosis reads them (naming.md section 3.1), on partsmith-naming's
//! `ElementName::face_name` view: the feature a name belongs to and its copy groups.

use partsmith_model::Id;
use partsmith_naming::ElementName;

/// The feature that starts `name`, or for a name with `@` groups the feature of its last group,
/// as an id; None for an edge or vertex name.
pub(crate) fn maker_of(name: &ElementName) -> Option<Id> {
    Id::new(name.face_name()?.maker()).ok()
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
