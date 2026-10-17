//! The public view of a face name taken apart (naming.md sections 2.2 and 3.1): its feature, role,
//! generator and own suffix groups, read with the scanner `Shape::of` uses.

use crate::name::Instance;
use crate::roles;
use crate::shape::{FaceParts, GeneratorParts, instance_groups};

/// A face name taken apart: `<feature>/<role>[/<generator>]` followed by its own `#n` and `@`
/// groups, borrowed from the name (`ElementName::face_name`). Groups after a face-name generator
/// belong to that generator (section 3.1 rule 6), so such a name has no own groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceName<'n> {
    /// The feature id that starts the name.
    feature: &'n str,
    /// The role.
    role: &'n str,
    /// What the face was made from, if the name says.
    generator: Option<FaceGenerator<'n>>,
    /// The name's own `#n` and `@` groups, as written.
    groups: &'n str,
}

/// What a face was made from, as written in its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceGenerator<'n> {
    /// A sketch entity id or point reference, for example `e-5` or `e-1.center`.
    Sketch(&'n str),
    /// A face name, such as the face a shell's inner face or rim was named after, with its suffix
    /// groups.
    Face(&'n str),
    /// An edge or vertex name, such as the edge a fillet face rounds.
    List(&'n str),
}

/// One of a face name's own `@<feature>:<instance>` groups: a copy made by a pattern or a mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyGroup<'n> {
    /// The pattern or mirror feature that made the copy.
    pub feature: &'n str,
    /// The instance number.
    pub instance: Instance,
    /// The group as written, such as `@f-lp1:2.1`.
    pub text: &'n str,
}

impl<'n> FaceName<'n> {
    /// Takes apart the structurally valid face name `text`.
    pub(crate) fn of(text: &'n str) -> FaceName<'n> {
        let parts = FaceParts::of(text);
        let generator_start = parts.role.end + 1;
        let generator = match parts.generator {
            GeneratorParts::Nothing => None,
            GeneratorParts::Sketch => Some(FaceGenerator::Sketch(
                &text[generator_start..parts.groups_start],
            )),
            GeneratorParts::Face => Some(FaceGenerator::Face(&text[generator_start..])),
            GeneratorParts::List { .. } => Some(FaceGenerator::List(
                &text[generator_start..parts.groups_start],
            )),
        };
        FaceName {
            feature: &text[..parts.feature_end],
            role: &text[parts.role],
            generator,
            groups: &text[parts.groups_start..],
        }
    }

    /// The feature id that starts the name.
    pub fn feature(&self) -> &'n str {
        self.feature
    }

    /// The role, for example `lateral`, `top` or `fillet`.
    pub fn role(&self) -> &'n str {
        self.role
    }

    /// What the face was made from, if the name says.
    pub fn generator(&self) -> Option<FaceGenerator<'n>> {
        self.generator
    }

    /// The sketch generator, when the face was made from a sketch entity or point.
    pub fn sketch_generator(&self) -> Option<&'n str> {
        match self.generator {
            Some(FaceGenerator::Sketch(generator)) => Some(generator),
            _ => None,
        }
    }

    /// The name's own `#n` and `@` groups, as written: empty after a face-name generator, whose
    /// groups belong to it. The name is `<feature>/<role>[/<generator>]` followed by them.
    pub fn groups(&self) -> &'n str {
        self.groups
    }

    /// True for a fallback name: role `face`, with no meaning beyond geometric order (section
    /// 3.6.14).
    pub fn is_fallback(&self) -> bool {
        self.role == roles::FACE
    }

    /// True when the name has a `#n` group of its own, so it names a piece of a split face.
    pub fn has_own_pieces(&self) -> bool {
        self.groups.contains('#')
    }

    /// The name's own `@<feature>:<instance>` groups, in written order.
    pub fn copies(&self) -> Vec<CopyGroup<'n>> {
        self.copy_groups().collect()
    }

    /// The feature the name belongs to: the pattern or mirror of its last own `@` group, else the
    /// feature that starts it.
    pub fn maker(&self) -> &'n str {
        self.copy_groups()
            .last()
            .map_or(self.feature, |copy| copy.feature)
    }

    /// The name's own `@` groups taken apart.
    fn copy_groups(&self) -> impl Iterator<Item = CopyGroup<'n>> {
        instance_groups(self.groups).map(|text| {
            let (feature, number) = text[1..]
                .split_once(':')
                .expect("an instance group has a feature and an instance");
            let index = |digits: &str| -> u32 {
                digits
                    .parse()
                    .expect("an instance index of a valid name fits a u32")
            };
            let instance = match number.split_once('.') {
                Some((first, second)) => Instance {
                    first: index(first),
                    second: index(second),
                },
                None => Instance {
                    first: index(number),
                    second: 0,
                },
            };
            CopyGroup {
                feature,
                instance,
                text,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::name;

    fn copy(first: u32, second: u32) -> Instance {
        Instance { first, second }
    }

    fn feature_and_instance<'n>(name: &FaceName<'n>) -> Vec<(&'n str, Instance)> {
        name.copies()
            .iter()
            .map(|copy| (copy.feature, copy.instance))
            .collect()
    }

    fn copy_text(name: &FaceName<'_>) -> String {
        name.copies().iter().map(|copy| copy.text).collect()
    }

    #[test]
    fn a_role_name_has_no_generator() {
        let top = name("f-box1/top");
        let name = top.face_name().expect("a face name");

        assert_eq!(name.feature(), "f-box1");
        assert_eq!(name.role(), "top");
        assert_eq!(name.generator(), None);
        assert!(!name.has_own_pieces());
    }

    #[test]
    fn a_sketch_generator_ends_at_the_suffix_groups() {
        let lateral = name("f-ext1/lateral/e-1#2@f-lp1:2.1");
        let name = lateral.face_name().expect("a face name");

        assert_eq!(name.generator(), Some(FaceGenerator::Sketch("e-1")));
        assert!(name.has_own_pieces());
        assert_eq!(
            name.copies(),
            [CopyGroup {
                feature: "f-lp1",
                instance: copy(2, 1),
                text: "@f-lp1:2.1"
            }]
        );
        assert_eq!(copy_text(&name), "@f-lp1:2.1");
    }

    #[test]
    fn an_edge_generator_ends_at_its_parenthesis() {
        let fillet = name("f-fil1/fillet/edge(f-box1/front|f-box1/top)@f-mir1:1");
        let name = fillet.face_name().expect("a face name");

        assert_eq!(
            name.generator(),
            Some(FaceGenerator::List("edge(f-box1/front|f-box1/top)"))
        );
        assert_eq!(name.copies().len(), 1);
    }

    #[test]
    fn a_face_generator_runs_to_the_end() {
        let rim = name("f-sh1/rim/f-ext1/end-cap#2");
        let name = rim.face_name().expect("a face name");

        assert_eq!(
            name.generator(),
            Some(FaceGenerator::Face("f-ext1/end-cap#2"))
        );
        assert!(!name.has_own_pieces());
    }

    #[test]
    fn a_sketch_generator_ends_at_the_groups() {
        let lateral = name("f-ext1/lateral/e-1#2@f-lp1:2");
        let parts = lateral.face_name().expect("a face name");

        assert_eq!(parts.feature(), "f-ext1");
        assert_eq!(parts.role(), "lateral");
        assert_eq!(parts.sketch_generator(), Some("e-1"));
        assert_eq!(feature_and_instance(&parts), [("f-lp1", copy(2, 0))]);
        assert_eq!(parts.maker(), "f-lp1");
    }

    #[test]
    fn groups_after_a_face_generator_belong_to_the_generator() {
        let inner = name("f-sh1/inner/f-box1/top@f-lp1:2");
        let parts = inner.face_name().expect("a face name");

        assert_eq!(
            parts.generator(),
            Some(FaceGenerator::Face("f-box1/top@f-lp1:2"))
        );
        assert!(parts.copies().is_empty());
        assert_eq!(parts.maker(), "f-sh1");
    }

    #[test]
    fn groups_after_an_edge_generator_belong_to_the_name() {
        let blend = name("f-cha1/chamfer/edge(f-box1/front|f-box1/top)@f-lp1:1.2");
        let parts = blend.face_name().expect("a face name");

        assert_eq!(
            parts.generator(),
            Some(FaceGenerator::List("edge(f-box1/front|f-box1/top)"))
        );
        assert_eq!(feature_and_instance(&parts), [("f-lp1", copy(1, 2))]);
    }

    #[test]
    fn a_name_is_its_parts_followed_by_its_own_groups() {
        let lateral = name("f-hole1/bore/e-5.start#2@f-lp1:2");
        let fillet = name("f-fil1/fillet/edge(f-box1/front|f-box1/top)@f-mir1:1");
        let rim = name("f-sh1/rim/f-ext1/end-cap#2");

        assert_eq!(
            lateral.face_name().expect("a face name").groups(),
            "#2@f-lp1:2"
        );
        assert_eq!(
            fillet.face_name().expect("a face name").groups(),
            "@f-mir1:1"
        );
        assert_eq!(rim.face_name().expect("a face name").groups(), "");
        assert_eq!(
            name("f-box1/top")
                .face_name()
                .expect("a face name")
                .groups(),
            ""
        );
    }

    #[test]
    fn fallback_names_have_the_face_role() {
        let face = name("f-imp1/face#3");
        let top = name("f-box1/top");

        assert!(face.face_name().expect("a face name").is_fallback());
        assert!(!top.face_name().expect("a face name").is_fallback());
        assert_eq!(name("edge(f-box1/front|f-box1/top)").face_name(), None);
    }

    #[test]
    fn a_copy_group_is_written_with_both_indices() {
        let copied = name("f-box1/top@f-lp1:2.1");
        let parts = copied.face_name().expect("a face name");

        assert_eq!(
            parts.copies().last().map(|copy| copy.text),
            Some("@f-lp1:2.1")
        );
    }
}
