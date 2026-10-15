//! Face names taken apart for descriptions (naming.md section 3.1): the feature, the role, the generator, the split
//! pieces and the instance copies.

/// A face name taken apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FaceName<'n> {
    /// The whole name.
    pub(super) text: &'n str,
    /// The feature id that starts it.
    pub(super) feature: &'n str,
    /// Its role, for example `lateral`, `top` or `fillet`.
    pub(super) role: &'n str,
    /// Its generator, if any.
    pub(super) generator: Option<GeneratorText<'n>>,
    /// True when it has a `#n` group of its own.
    pub(super) piece: bool,
    /// Its `@<feature>:<instance>` groups, in written order.
    pub(super) copies: Vec<CopyGroup<'n>>,
}

/// The generator of a face name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GeneratorText<'n> {
    /// A sketch entity id or point reference, for example `e-5` or `e-1.center`.
    Sketch(&'n str),
    /// An element name, for example `edge(f-box1/front|f-box1/top)` or `f-box1/bottom`.
    Element(&'n str),
}

/// One `@<feature>:<instance>` group: a copy made by a pattern or a mirror.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CopyGroup<'n> {
    /// The pattern or mirror feature.
    pub(super) feature: &'n str,
    /// The first instance index.
    pub(super) first: u32,
    /// The second instance index of a grid copy.
    pub(super) second: Option<u32>,
}

impl<'n> FaceName<'n> {
    /// The parts of a valid face name; None for text that is not one.
    pub(super) fn parse(text: &'n str) -> Option<FaceName<'n>> {
        let (feature, rest) = text.split_once('/')?;
        let role_end = rest.find(['/', '#', '@']).unwrap_or(rest.len());
        let role = &rest[..role_end];
        let mut name = FaceName {
            text,
            feature,
            role,
            generator: None,
            piece: false,
            copies: Vec::new(),
        };
        let mut suffix = &rest[role_end..];
        if let Some(generator) = suffix.strip_prefix('/') {
            if generator.starts_with("edge(") || generator.starts_with("vertex(") {
                let end = closing_parenthesis(generator)?;
                name.generator = Some(GeneratorText::Element(&generator[..=end]));
                suffix = &generator[end + 1..];
            } else {
                let sketch_end = generator.find(['#', '@']).unwrap_or(generator.len());
                if generator[..sketch_end].contains('/') {
                    // A face-name generator runs to the end of the name, its suffix groups included.
                    name.generator = Some(GeneratorText::Element(generator));
                    suffix = "";
                } else {
                    name.generator = Some(GeneratorText::Sketch(&generator[..sketch_end]));
                    suffix = &generator[sketch_end..];
                }
            }
        }
        name.read_suffixes(suffix)?;
        Some(name)
    }

    /// Reads `#n` and `@<feature>:<instance>` groups.
    fn read_suffixes(&mut self, mut suffix: &'n str) -> Option<()> {
        while !suffix.is_empty() {
            let end = suffix[1..]
                .find(['#', '@'])
                .map_or(suffix.len(), |end| end + 1);
            let group = &suffix[..end];
            if group.starts_with('#') {
                self.piece = true;
            } else {
                let (feature, instance) = group.strip_prefix('@')?.split_once(':')?;
                let (first, second) = match instance.split_once('.') {
                    Some((first, second)) => (first.parse().ok()?, Some(second.parse().ok()?)),
                    None => (instance.parse().ok()?, None),
                };
                self.copies.push(CopyGroup {
                    feature,
                    first,
                    second,
                });
            }
            suffix = &suffix[end..];
        }
        Some(())
    }

    /// The `@` groups as written, so faces of one copy can be compared.
    pub(super) fn copy_text(&self) -> String {
        self.copies
            .iter()
            .map(|copy| match copy.second {
                Some(second) => format!("@{}:{}.{second}", copy.feature, copy.first),
                None => format!("@{}:{}", copy.feature, copy.first),
            })
            .collect()
    }
}

/// The name with every `#n` group removed, wherever it stands (naming.md section 3.1, "Piece-free name").
pub(super) fn piece_free(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '#' {
            while characters.next_if(char::is_ascii_digit).is_some() {}
        } else {
            result.push(character);
        }
    }
    result
}

/// The position of the parenthesis that closes the first one in `text`.
fn closing_parenthesis(text: &str) -> Option<usize> {
    let mut depth = 0_usize;
    for (position, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(position);
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

    #[test]
    fn a_role_name_has_no_generator() {
        let name = FaceName::parse("f-box1/top").expect("a face name");

        assert_eq!(name.feature, "f-box1");
        assert_eq!(name.role, "top");
        assert_eq!(name.generator, None);
        assert!(!name.piece);
    }

    #[test]
    fn a_sketch_generator_ends_at_the_suffix_groups() {
        let name = FaceName::parse("f-ext1/lateral/e-1#2@f-lp1:2.1").expect("a face name");

        assert_eq!(name.generator, Some(GeneratorText::Sketch("e-1")));
        assert!(name.piece);
        assert_eq!(
            name.copies,
            [CopyGroup {
                feature: "f-lp1",
                first: 2,
                second: Some(1)
            }]
        );
        assert_eq!(name.copy_text(), "@f-lp1:2.1");
    }

    #[test]
    fn an_edge_generator_ends_at_its_parenthesis() {
        let name = FaceName::parse("f-fil1/fillet/edge(f-box1/front|f-box1/top)@f-mir1:1")
            .expect("a face name");

        assert_eq!(
            name.generator,
            Some(GeneratorText::Element("edge(f-box1/front|f-box1/top)"))
        );
        assert_eq!(name.copies.len(), 1);
    }

    #[test]
    fn a_face_generator_runs_to_the_end() {
        let name = FaceName::parse("f-sh1/rim/f-ext1/end-cap#2").expect("a face name");

        assert_eq!(
            name.generator,
            Some(GeneratorText::Element("f-ext1/end-cap#2"))
        );
        assert!(!name.piece);
    }

    #[test]
    fn piece_free_names_drop_every_number() {
        assert_eq!(
            piece_free("f-ext1/end-cap#1@f-lp1:1#2"),
            "f-ext1/end-cap@f-lp1:1"
        );
        assert_eq!(piece_free("f-box1/top"), "f-box1/top");
    }
}
