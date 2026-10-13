//! Acceptance tests for name sets, families and the order independence of resolving (spec section
//! 6, tests 118 and 140; spec sections 2.7 and 3.9).

mod common;
mod oracle;

use common::{
    BOX_FACE_NAMES, ScopeElement, box_infos, box_map, build_with, edge_info, element_set,
    element_set_of_map, face_info, map_with_faces, merged, name, nested_alias_scope, nested_merged,
    nested_merged_scope, new_source, renumbered_pieces_infos, scope_element,
};
use oracle::{Face, Flat, MergedChoice, World, element_name, face, scenario};
use partsmith_naming::{
    ElementKind, ElementName, FaceSource, Merged, Motion, NameSet, NamingMap, Successions,
    TopologyInfo, resolve,
};
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// The names of a family, as text.
fn texts(family: Vec<&ElementName>) -> Vec<&str> {
    family.into_iter().map(ElementName::as_str).collect()
}

/// A flat oracle name with a head and split numbers.
fn flat(head: &'static str, splits: &[u32]) -> Face {
    Face::Flat(Flat {
        head,
        groups: splits
            .iter()
            .map(|split| oracle::Group::Split(*split))
            .collect(),
    })
}

#[test]
fn t140_family_of_a_renumbered_piece_holds_every_piece() {
    let scope = element_set(renumbered_pieces_infos());

    let family = scope.names().family(&name("f-b/top#2"), &Merged::default());

    assert_eq!(texts(family), ["f-b/top#1", "f-b/top#2#1", "f-b/top#2#2"]);
}

#[test]
fn t140_touches_reads_the_adjacency() {
    let scope = element_set(renumbered_pieces_infos());

    assert!(
        scope
            .names()
            .touches(&name("f-b/top#2#2"), &name("f-b/right"))
    );
    assert!(
        !scope
            .names()
            .touches(&name("f-b/top#1"), &name("f-b/right"))
    );
}

#[test]
fn t140_family_of_a_nested_name_follows_aliases_of_its_terms() {
    let scope = nested_alias_scope();

    let family = scope.names().family(
        &name("f-cha1/chamfer/edge(f-box1/front|f-box1/top)"),
        &Merged::default(),
    );

    assert_eq!(texts(family), ["f-cha1/chamfer/edge(f-a1/front|f-a1/top)"]);
}

#[test]
fn t140_family_of_a_nested_name_uses_merged_names_of_its_terms() {
    let scope = nested_merged_scope();

    let family = scope.names().family(
        &name("f-cha1/chamfer/edge(f-a1/front|f-a1/top)"),
        &nested_merged(),
    );

    assert_eq!(
        texts(family),
        ["f-cha1/chamfer/edge(f-box1/front|f-box1/top)"]
    );
}

#[test]
fn t140_family_of_a_nested_name_without_merged_names_is_empty() {
    let scope = nested_merged_scope();

    let family = scope.names().family(
        &name("f-cha1/chamfer/edge(f-a1/front|f-a1/top)"),
        &Merged::default(),
    );

    assert!(family.is_empty(), "{family:?}");
}

#[test]
fn t140_box_name_set_agrees_with_the_box_element_set() {
    let from_map = NameSet::new(&box_map());
    let scope = element_set(box_infos());
    let from_set = scope.names();

    for part in BOX_FACE_NAMES {
        assert_eq!(
            texts(from_map.family(&name(part), &Merged::default())),
            texts(from_set.family(&name(part), &Merged::default())),
            "{part}"
        );
        for anchor in BOX_FACE_NAMES {
            assert_eq!(
                from_map.touches(&name(part), &name(anchor)),
                from_set.touches(&name(part), &name(anchor)),
                "{part} and {anchor}"
            );
        }
    }
}

#[test]
fn t140_box_top_touches_its_sides_not_its_bottom() {
    let from_map = NameSet::new(&box_map());

    assert!(from_map.touches(&name("f-box1/top"), &name("f-box1/front")));
    assert!(!from_map.touches(&name("f-box1/top"), &name("f-box1/bottom")));
}

#[test]
fn t140_box_family_of_a_face_is_the_face() {
    let from_map = NameSet::new(&box_map());

    let family = from_map.family(&name("f-box1/top"), &Merged::default());

    assert_eq!(texts(family), ["f-box1/top"]);
}

#[test]
fn t140_name_set_keeps_the_map_aliases_and_successions() {
    let inherited = map_with_faces(&["f-ext1/end-cap", "f-ext2/end-cap", "f-ext1/lateral/e-9"]);
    let map = build_with(
        vec![
            common::from_sources(&["f-ext2/end-cap", "f-ext1/end-cap"]),
            common::from_sources(&["f-ext1/lateral/e-9"]),
        ],
        &common::face_row_topology(2),
        &[&inherited],
        &common::successions(&[("f-ext1/lateral/e-3", &["f-ext1/lateral/e-9"])]),
        &Motion::IDENTITY,
    );

    let from_map = NameSet::new(&map);

    assert_eq!(from_map.aliases(), map.aliases());
    assert_eq!(from_map.successions(), map.successions());
}

#[test]
fn t140_family_follows_a_merge_alias() {
    let scope = common::element_set_with(
        vec![scope_element("f-ext1/end-cap")],
        common::aliases(&[("f-ext2/end-cap", &["f-ext1/end-cap"])]),
        Successions::default(),
    );

    let family = scope
        .names()
        .family(&name("f-ext2/end-cap"), &Merged::default());

    assert_eq!(texts(family), ["f-ext1/end-cap"]);
}

#[test]
fn t140_family_of_a_missing_part_is_empty() {
    let scope = element_set(box_infos());

    let family = scope
        .names()
        .family(&name("f-gone/top"), &Merged::default());

    assert!(family.is_empty(), "{family:?}");
}

#[test]
fn t140_touches_finds_the_anchor_family_through_an_alias() {
    let scope = common::element_set_with(
        vec![scope_element("f-b/top#2").with_adjacency(&["f-b/back", "f-j/right"])],
        common::aliases(&[("f-b/right", &["f-j/right"])]),
        Successions::default(),
    );

    assert!(
        scope
            .names()
            .touches(&name("f-b/top#2"), &name("f-b/right"))
    );
}

#[test]
fn t140_merged_piece_stands_for_its_further_pieces_never_its_siblings() {
    let scope = element_set(vec![scope_element("f-a/x#1#1"), scope_element("f-a/x#2#1")]);

    let piece_two = scope
        .names()
        .family(&name("f-q/q"), &merged(&[("f-q/q", &["f-a/x#2"])]));
    let piece_one = scope
        .names()
        .family(&name("f-q/q"), &merged(&[("f-q/q", &["f-a/x#1"])]));

    assert_eq!(texts(piece_two), ["f-a/x#2#1"]);
    assert_eq!(texts(piece_one), ["f-a/x#1#1"]);
}

#[test]
fn t140_piece_of_a_copy_extends_the_copy() {
    let scope = element_set(vec![
        scope_element("f-a/x#1@f-p:1"),
        scope_element("f-a/x@f-p:1#2"),
        scope_element("f-a/x@f-p:2"),
    ]);

    let family = scope
        .names()
        .family(&name("f-a/x@f-p:1"), &Merged::default());

    assert_eq!(texts(family), ["f-a/x#1@f-p:1", "f-a/x@f-p:1#2"]);
}

#[test]
fn oracle_agrees_with_the_renumbered_piece_family() {
    let world = World {
        faces: [
            flat("f-b/top", &[1]),
            flat("f-b/top", &[2, 1]),
            flat("f-b/top", &[2, 2]),
            flat("f-b/right", &[]),
        ]
        .into(),
        ..World::default()
    };

    assert_eq!(
        world.family(&flat("f-b/top", &[2]), &[]),
        ["f-b/top#1", "f-b/top#2#1", "f-b/top#2#2"]
    );
}

#[test]
fn oracle_agrees_with_merged_pieces() {
    let world = World {
        faces: [flat("f-a/x", &[1, 1]), flat("f-a/x", &[2, 1])].into(),
        ..World::default()
    };

    assert_eq!(
        world.family(
            &flat("f-q/q", &[]),
            &[MergedChoice::Face(flat("f-a/x", &[2]))]
        ),
        ["f-a/x#2#1"]
    );
}

#[test]
fn oracle_agrees_with_a_kept_piece_alias() {
    let world = World {
        faces: [flat("f-ext1/end-cap", &[1]), flat("f-box2/top", &[])].into(),
        aliases: BTreeMap::from([(flat("f-ext1/end-cap", &[2]), vec![flat("f-box2/top", &[])])]),
        ..World::default()
    };

    assert_eq!(
        world.family(&flat("f-ext1/end-cap", &[]), &[]),
        ["f-box2/top", "f-ext1/end-cap#1"]
    );
    assert_eq!(
        world.family(&flat("f-ext1/end-cap", &[2]), &[]),
        ["f-box2/top"]
    );
}

#[test]
fn oracle_agrees_with_copies() {
    let copy = |instance| {
        Face::Flat(Flat {
            head: "f-box1/top",
            groups: vec![oracle::Group::Instance(instance)],
        })
    };
    let world = World {
        faces: [flat("f-box1/top", &[]), copy(1), copy(2)].into(),
        ..World::default()
    };

    assert_eq!(
        world.family(
            &flat("f-box1/top", &[]),
            &[MergedChoice::Copies(Flat {
                head: "f-box1/top",
                groups: Vec::new(),
            })]
        ),
        ["f-box1/top", "f-box1/top@f-p:1", "f-box1/top@f-p:2"]
    );
}

#[test]
fn oracle_agrees_with_nested_aliases() {
    let chamfer = |first: &'static str, second: &'static str| Face::Chamfer {
        terms: vec![
            Flat {
                head: first,
                groups: Vec::new(),
            },
            Flat {
                head: second,
                groups: Vec::new(),
            },
        ],
        groups: Vec::new(),
    };
    let world = World {
        faces: [chamfer("f-z/front", "f-z/top")].into(),
        aliases: BTreeMap::from([
            (flat("f-a/front", &[]), vec![flat("f-z/front", &[])]),
            (flat("f-a/top", &[]), vec![flat("f-z/top", &[])]),
        ]),
        ..World::default()
    };

    assert_eq!(
        world.family(&chamfer("f-a/front", "f-a/top"), &[]),
        ["f-c/chamfer/edge(f-z/front|f-z/top)"]
    );
}

/// One output face of a random map: new, or from inherited faces picked modulo their count.
#[derive(Debug, Clone)]
enum RandomSource {
    /// A new face with this name.
    New(Face),
    /// Faces of the inherited map.
    From(Vec<usize>),
}

/// A random build: an inherited map's faces, output faces, edges between output faces (picked
/// modulo their count) and added successions.
#[derive(Debug, Clone)]
struct RandomBuild {
    /// Faces of the inherited map.
    inherited: Vec<Face>,
    /// Output faces.
    sources: Vec<RandomSource>,
    /// Edges as pairs of output faces.
    edges: Vec<(usize, usize)>,
    /// Added successions.
    successions: Vec<(Face, Vec<Face>)>,
}

impl RandomBuild {
    /// The inherited map: its faces, each named once.
    fn inherited_map(&self) -> (NamingMap, Vec<String>) {
        let texts: Vec<String> = self
            .inherited
            .iter()
            .map(Face::text)
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect();
        let borrowed: Vec<&str> = texts.iter().map(String::as_str).collect();
        (map_with_faces(&borrowed), texts)
    }

    /// The topology: output faces in a row, and one edge per distinct pair of faces.
    fn topology(&self) -> TopologyInfo {
        let face_count = self.sources.len();
        let pairs: Vec<(usize, usize)> = self
            .edges
            .iter()
            .map(|(first, second)| (first % face_count, second % face_count))
            .filter(|(first, second)| first != second)
            .map(|(first, second)| (first.min(second), first.max(second)))
            .collect();
        let index = |position: usize| u32::try_from(position + 1).expect("small counts");
        let faces = (0..face_count)
            .map(|face| {
                let edges: Vec<u32> = pairs
                    .iter()
                    .enumerate()
                    .filter(|(_, (first, second))| *first == face || *second == face)
                    .map(|(edge, _)| index(edge))
                    .collect();
                face_info(&edges, &format!("plane|n=0,0,1|c={face},0,0|a=1"))
            })
            .collect();
        let edges = pairs
            .iter()
            .enumerate()
            .map(|(edge, (first, second))| {
                edge_info(
                    &[index(*first), index(*second)],
                    &[],
                    &format!("line|n=1,0,0|c={edge},1,0|l=1"),
                )
            })
            .collect();
        TopologyInfo {
            faces,
            edges,
            vertices: Vec::new(),
        }
    }

    /// The added successions: distinct keys, each list distinct in its order.
    fn added_successions(&self) -> Successions {
        let mut entries = BTreeMap::new();
        for (key, targets) in &self.successions {
            let mut distinct: Vec<ElementName> = Vec::new();
            for target in targets {
                let target_name = element_name(&target.text());
                if !distinct.contains(&target_name) {
                    distinct.push(target_name);
                }
            }
            entries.entry(element_name(&key.text())).or_insert(distinct);
        }
        Successions(entries)
    }

    /// Builds the map and returns it with its topology.
    fn build(&self) -> (NamingMap, TopologyInfo) {
        let (inherited, inherited_texts) = self.inherited_map();
        let sources = self
            .sources
            .iter()
            .map(|source| match source {
                RandomSource::New(face) => new_source(&face.text()),
                RandomSource::From(picks) => FaceSource::From(
                    picks
                        .iter()
                        .map(|pick| {
                            (
                                0,
                                element_name(&inherited_texts[pick % inherited_texts.len()]),
                            )
                        })
                        .collect(),
                ),
            })
            .collect();
        let topology = self.topology();
        let map = build_with(
            sources,
            &topology,
            &[&inherited],
            &self.added_successions(),
            &Motion::IDENTITY,
        );
        (map, topology)
    }
}

/// A random build.
fn random_build() -> impl Strategy<Value = RandomBuild> {
    (
        prop::collection::vec(face(), 1..=5),
        prop::collection::vec(
            prop_oneof![
                face().prop_map(RandomSource::New),
                prop::collection::vec(0..8_usize, 1..=3).prop_map(RandomSource::From),
            ],
            1..=6,
        ),
        prop::collection::vec((0..16_usize, 0..16_usize), 0..=8),
        prop::collection::vec((face(), prop::collection::vec(face(), 1..=3)), 0..=2),
    )
        .prop_map(|(inherited, sources, edges, successions)| RandomBuild {
            inherited,
            sources,
            edges,
            successions,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn t118_name_indexes_give_the_families_of_the_definition(scenario in scenario()) {
        let set = scenario.element_set(false);

        for (part, choices) in scenario.parts() {
            let expected = scenario.world.family(&part, &choices);
            let merged_names = oracle::Scenario::merged(&part, &choices);

            let family = set.names().family(&element_name(&part.text()), &merged_names);

            prop_assert_eq!(
                texts(family),
                expected.iter().map(String::as_str).collect::<Vec<&str>>(),
                "family of {} with {:?}", part.text(), choices
            );
        }
    }

    #[test]
    fn t118_resolving_does_not_depend_on_the_order_of_the_scope(scenario in scenario()) {
        let forward = scenario.element_set(false);
        let backward = scenario.element_set(true);

        for reference in scenario.references() {
            prop_assert_eq!(
                resolve(&reference, &forward),
                resolve(&reference, &backward),
                "{:?}", reference
            );
        }
    }

    #[test]
    fn t140_name_set_of_a_map_agrees_with_its_element_set(build in random_build()) {
        let (map, topology) = build.build();
        let from_map = NameSet::new(&map);
        let scope = element_set_of_map(&map, &topology);
        let from_set = scope.names();
        let faces: Vec<ElementName> = map
            .names(ElementKind::Face)
            .map(|(_, face_name)| face_name.clone())
            .collect();
        let parts: Vec<ElementName> = faces
            .iter()
            .cloned()
            .chain(map.aliases().0.keys().cloned())
            .chain(map.successions().0.keys().cloned())
            .collect();

        for part in &parts {
            prop_assert_eq!(
                texts(from_map.family(part, &Merged::default())),
                texts(from_set.family(part, &Merged::default())),
                "family of {}", part
            );
        }
        for face_name in &faces {
            for anchor in &parts {
                prop_assert_eq!(
                    from_map.touches(face_name, anchor),
                    from_set.touches(face_name, anchor),
                    "{} touching {}", face_name, anchor
                );
            }
        }
    }
}
