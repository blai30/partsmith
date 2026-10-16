//! Face names taken apart as the loss diagnosis reads them (naming.md section 3.1): the feature that
//! starts a name, its role, its generator and its own `@` groups.

use partsmith_model::Id;
use partsmith_naming::{ElementKind, ElementName, Instance};

/// A face name taken apart: `<feature>/<role>[/<generator>]` followed by its own `#n` and `@` groups.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FaceName<'n> {
    /// The feature id that starts the name.
    pub(crate) feature: &'n str,
    /// The role, for example `lateral`, `top` or `fillet`.
    pub(crate) role: &'n str,
    /// What the face was made from, if the name says.
    pub(crate) generator: Option<Generator<'n>>,
    /// The name's own `@<feature>:<instance>` groups, in written order. Groups after a face-name
    /// generator belong to that generator (naming.md section 3.1 rule 6).
    pub(crate) copies: Vec<(&'n str, Instance)>,
}

/// What a face was made from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Generator<'n> {
    /// A sketch entity id or point reference, for example `e-5` or `e-1.center`.
    Sketch(&'n str),
    /// A face name, such as the face a shell's inner face or rim was named after.
    Face(&'n str),
    /// An edge or vertex name, such as the edge a fillet face rounds.
    List(&'n str),
}

impl<'n> FaceName<'n> {
    /// The parts of a face name; None for an edge or vertex name.
    pub(crate) fn of(name: &'n ElementName) -> Option<FaceName<'n>> {
        if name.kind() != ElementKind::Face {
            return None;
        }
        FaceName::parse(name.as_str())
    }

    /// The parts of the face name `text`; None when it is not one.
    fn parse(text: &'n str) -> Option<FaceName<'n>> {
        let (feature, rest) = text.split_once('/')?;
        let role_end = rest.find(['/', '#', '@']).unwrap_or(rest.len());
        let mut name = FaceName {
            feature,
            role: &rest[..role_end],
            generator: None,
            copies: Vec::new(),
        };
        let mut suffix = &rest[role_end..];
        if let Some(generator) = suffix.strip_prefix('/') {
            if generator.starts_with("edge(") || generator.starts_with("vertex(") {
                let end = closing_parenthesis(generator)?;
                name.generator = Some(Generator::List(&generator[..=end]));
                suffix = &generator[end + 1..];
            } else {
                let sketch_end = generator.find(['#', '@']).unwrap_or(generator.len());
                if generator[..sketch_end].contains('/') {
                    name.generator = Some(Generator::Face(generator));
                    suffix = "";
                } else {
                    name.generator = Some(Generator::Sketch(&generator[..sketch_end]));
                    suffix = &generator[sketch_end..];
                }
            }
        }
        name.copies = copy_groups(suffix)?;
        Some(name)
    }

    /// The feature the name belongs to (section 3.10, "The lost name"): the feature of its last
    /// `@` group, else the feature that starts it.
    pub(crate) fn maker(&self) -> &'n str {
        self.copies
            .last()
            .map_or(self.feature, |(feature, _)| *feature)
    }

    /// The sketch generator, when the face was made from a sketch entity or point.
    pub(crate) fn sketch_generator(&self) -> Option<&'n str> {
        match self.generator {
            Some(Generator::Sketch(generator)) => Some(generator),
            _ => None,
        }
    }

    /// True for a fallback name (`<feature>/face`, numbered in geometric order).
    pub(crate) fn is_fallback(&self) -> bool {
        self.role == "face" && self.generator.is_none()
    }
}

/// The feature that starts `name`, or for a name with `@` groups the feature of its last group,
/// as an id; None for an edge or vertex name.
pub(crate) fn maker_of(name: &ElementName) -> Option<Id> {
    Id::new(FaceName::of(name)?.maker()).ok()
}

/// The parts of a query: the face itself, or the face terms of an edge or vertex.
pub(crate) fn parts_of(query: &ElementName) -> Vec<ElementName> {
    match query.kind() {
        ElementKind::Face => vec![query.clone()],
        ElementKind::Edge | ElementKind::Vertex => query.face_terms(),
    }
}

/// True when the face name `text` has the `@<feature>:<instance>` group `group` (written
/// `@<feature>:<instance>`), wherever it stands.
pub(crate) fn has_copy_group(text: &str, group: &str) -> bool {
    text.match_indices(group).any(|(start, _)| {
        !text[start + group.len()..]
            .starts_with(|character: char| character.is_ascii_digit() || character == '.')
    })
}

/// The `@` group text of a copy: `@f-lp1:2`, `@f-lp1:2.1`.
pub(crate) fn copy_group(feature: &Id, instance: Instance) -> String {
    match instance.second {
        0 => format!("@{feature}:{}", instance.first),
        second => format!("@{feature}:{}.{second}", instance.first),
    }
}

/// The `@` groups of a suffix made of `#n` and `@<feature>:<instance>` groups; None when the
/// suffix is not made of them.
fn copy_groups(mut suffix: &str) -> Option<Vec<(&str, Instance)>> {
    let mut copies = Vec::new();
    while !suffix.is_empty() {
        let end = suffix[1..]
            .find(['#', '@'])
            .map_or(suffix.len(), |end| end + 1);
        let group = &suffix[..end];
        if let Some(copy) = group.strip_prefix('@') {
            let (feature, instance) = copy.split_once(':')?;
            let (first, second) = match instance.split_once('.') {
                Some((first, second)) => (first.parse().ok()?, second.parse().ok()?),
                None => (instance.parse().ok()?, 0),
            };
            copies.push((feature, Instance { first, second }));
        } else if !group.starts_with('#') {
            return None;
        }
        suffix = &suffix[end..];
    }
    Some(copies)
}

/// The byte offset of the parenthesis that closes the one `text`'s first `(` opens.
fn closing_parenthesis(text: &str) -> Option<usize> {
    let mut depth = 0_usize;
    for (offset, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("a valid name")
    }

    fn copy(first: u32, second: u32) -> Instance {
        Instance { first, second }
    }

    #[test]
    fn a_sketch_generator_ends_at_the_groups() {
        let lateral = name("f-ext1/lateral/e-1#2@f-lp1:2");
        let parts = FaceName::of(&lateral).expect("a face name");

        assert_eq!(parts.feature, "f-ext1");
        assert_eq!(parts.role, "lateral");
        assert_eq!(parts.sketch_generator(), Some("e-1"));
        assert_eq!(parts.copies, [("f-lp1", copy(2, 0))]);
        assert_eq!(parts.maker(), "f-lp1");
    }

    #[test]
    fn groups_after_a_face_generator_belong_to_the_generator() {
        let inner = name("f-sh1/inner/f-box1/top@f-lp1:2");
        let parts = FaceName::of(&inner).expect("a face name");

        assert_eq!(parts.generator, Some(Generator::Face("f-box1/top@f-lp1:2")));
        assert!(parts.copies.is_empty());
        assert_eq!(parts.maker(), "f-sh1");
    }

    #[test]
    fn groups_after_an_edge_generator_belong_to_the_name() {
        let blend = name("f-cha1/chamfer/edge(f-box1/front|f-box1/top)@f-lp1:1.2");
        let parts = FaceName::of(&blend).expect("a face name");

        assert_eq!(
            parts.generator,
            Some(Generator::List("edge(f-box1/front|f-box1/top)"))
        );
        assert_eq!(parts.copies, [("f-lp1", copy(1, 2))]);
    }

    #[test]
    fn fallback_names_have_the_face_role_and_no_generator() {
        let face = name("f-imp1/face#3");

        assert!(FaceName::of(&face).expect("a face name").is_fallback());
        assert!(
            !FaceName::of(&name("f-box1/top"))
                .expect("a face name")
                .is_fallback()
        );
        assert_eq!(FaceName::of(&name("edge(f-box1/front|f-box1/top)")), None);
    }

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
        let group = copy_group(&id("f-lp1"), copy(2, 0));

        assert_eq!(group, "@f-lp1:2");
        assert!(has_copy_group("f-box1/top@f-lp1:2", &group));
        assert!(has_copy_group("f-box1/top@f-lp1:2#1", &group));
        assert!(!has_copy_group("f-box1/top@f-lp1:20", &group));
        assert!(!has_copy_group("f-box1/top@f-lp1:2.1", &group));
        assert_eq!(copy_group(&id("f-lp1"), copy(2, 1)), "@f-lp1:2.1");
    }

    #[test]
    fn the_parts_of_a_query_are_its_face_or_its_face_terms() {
        assert_eq!(parts_of(&name("f-box1/top")), [name("f-box1/top")]);
        assert_eq!(
            parts_of(&name("edge(f-box1/front|f-box1/top)")),
            [name("f-box1/front"), name("f-box1/top")]
        );
    }

    fn id(text: &str) -> Id {
        Id::new(text).expect("a valid id")
    }
}
