//! Element tables, display meshes and their session cache (engine.md sections 3.8 and 3.17).

use std::collections::BTreeMap;
use std::mem::size_of;
use std::sync::{Arc, OnceLock};

use partsmith_features::BodyState;
use partsmith_mesh::{
    BoundingBox, EdgePolyline, Mesh, RenderBuffers, TessellationTolerance, VertexMarker,
};
use partsmith_naming::{ElementKind, ElementName};
use partsmith_occt_sys::{KernelError, Shape, SubShapeKind, Topology};
use partsmith_protocol::ElementDescriptor;

use crate::error::EngineError;
use crate::state::StateKey;

/// An internal error for a kernel failure on a shape the engine already holds.
pub(crate) fn kernel_failure(error: KernelError) -> EngineError {
    EngineError::internal(error.to_string())
}

/// One displayed element: its name and its 1-based kernel index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableEntry {
    /// The element name.
    pub(crate) name: ElementName,
    /// The kernel index.
    pub(crate) index: u32,
}

/// The displayed elements of one body state (section 3.8): faces, edges without seams, and the
/// vertices of those edges, each sorted by name, repeated names by kernel index. An element's
/// display id is its position.
#[derive(Debug)]
pub(crate) struct ElementTable {
    /// Displayed faces.
    faces: Vec<TableEntry>,
    /// Displayed edges.
    edges: Vec<TableEntry>,
    /// Displayed vertices.
    vertices: Vec<TableEntry>,
    /// Display id of each face by kernel index minus one.
    face_ids: Vec<Option<u32>>,
    /// Display id of each edge by kernel index minus one.
    edge_ids: Vec<Option<u32>>,
    /// Display id of each vertex by kernel index minus one.
    vertex_ids: Vec<Option<u32>>,
    /// Descriptors in table order, computed on first use.
    descriptors: OnceLock<Result<Arc<[ElementDescriptor]>, EngineError>>,
}

impl ElementTable {
    /// The table of a body state with a shape.
    pub(crate) fn build(state: &BodyState) -> Result<ElementTable, EngineError> {
        let shape = state
            .shape()
            .ok_or_else(|| EngineError::internal("an element table needs a shape"))?;
        let topology = shape.topology().map_err(kernel_failure)?;
        let naming = state.naming();
        let consistent = [
            (ElementKind::Face, topology.faces.len()),
            (ElementKind::Edge, topology.edges.len()),
            (ElementKind::Vertex, topology.vertices.len()),
        ]
        .iter()
        .all(|&(kind, count)| naming.count(kind) as usize == count);
        if !consistent {
            return Err(EngineError::internal(
                "a naming map and its shape have different element counts",
            ));
        }
        let entries = |kind: ElementKind, shown: &dyn Fn(u32) -> bool| {
            let mut entries: Vec<TableEntry> = naming
                .names(kind)
                .filter(|&(index, _)| shown(index))
                .map(|(index, name)| TableEntry {
                    name: name.clone(),
                    index,
                })
                .collect();
            entries.sort_by(|first, second| {
                first
                    .name
                    .cmp(&second.name)
                    .then(first.index.cmp(&second.index))
            });
            entries
        };
        let faces = entries(ElementKind::Face, &|_| true);
        let edges = entries(ElementKind::Edge, &|index| !is_seam(&topology, index));
        let edge_ids = display_ids(&edges, topology.edges.len());
        let vertices = entries(ElementKind::Vertex, &|index| {
            slot(index)
                .and_then(|position| topology.vertices.get(position))
                .is_some_and(|vertex| {
                    vertex.edges.iter().any(|&edge| {
                        slot(edge)
                            .and_then(|position| edge_ids.get(position))
                            .is_some_and(Option::is_some)
                    })
                })
        });
        Ok(ElementTable {
            face_ids: display_ids(&faces, topology.faces.len()),
            edge_ids,
            vertex_ids: display_ids(&vertices, topology.vertices.len()),
            faces,
            edges,
            vertices,
            descriptors: OnceLock::new(),
        })
    }

    /// The displayed elements of `kind` in display id order.
    pub(crate) fn entries(&self, kind: ElementKind) -> &[TableEntry] {
        match kind {
            ElementKind::Face => &self.faces,
            ElementKind::Edge => &self.edges,
            ElementKind::Vertex => &self.vertices,
        }
    }

    /// Number of displayed elements of `kind`.
    pub(crate) fn count(&self, kind: ElementKind) -> u32 {
        u32::try_from(self.entries(kind).len()).unwrap_or(u32::MAX)
    }

    /// The display id of the element with kernel index `index`, if it is displayed.
    pub(crate) fn display_id(&self, kind: ElementKind, index: u32) -> Option<u32> {
        let ids = match kind {
            ElementKind::Face => &self.face_ids,
            ElementKind::Edge => &self.edge_ids,
            ElementKind::Vertex => &self.vertex_ids,
        };
        ids.get(slot(index)?).copied().flatten()
    }

    /// The display id of the displayed element with this name.
    pub(crate) fn find(&self, name: &ElementName) -> Option<u32> {
        let position = self
            .entries(name.kind())
            .binary_search_by(|entry| entry.name.cmp(name))
            .ok()?;
        u32::try_from(position).ok()
    }

    /// The descriptors in table order (faces, edges, vertices), computed once by `describe`.
    pub(crate) fn descriptors(
        &self,
        describe: impl FnOnce(&ElementTable) -> Result<Vec<ElementDescriptor>, EngineError>,
    ) -> Result<Arc<[ElementDescriptor]>, EngineError> {
        self.descriptors
            .get_or_init(|| describe(self).map(Arc::from))
            .clone()
    }

    /// A rough size in bytes, for the cache budget.
    fn size(&self) -> usize {
        let names: usize = [&self.faces, &self.edges, &self.vertices]
            .iter()
            .flat_map(|entries| entries.iter())
            .map(|entry| entry.name.as_str().len() + size_of::<TableEntry>())
            .sum();
        let ids = self.face_ids.len() + self.edge_ids.len() + self.vertex_ids.len();
        names + ids * size_of::<Option<u32>>()
    }
}

/// True when edge `index` is a seam (it bounds the same face on both sides).
fn is_seam(topology: &Topology, index: u32) -> bool {
    slot(index)
        .and_then(|position| topology.edges.get(position))
        .is_none_or(|edge| edge.seam)
}

/// The position of a 1-based kernel index in a per-element list.
pub(crate) fn slot(index: u32) -> Option<usize> {
    usize::try_from(index.checked_sub(1)?).ok()
}

/// For each kernel index minus one, the position of its entry, if any.
fn display_ids(entries: &[TableEntry], count: usize) -> Vec<Option<u32>> {
    let mut ids = vec![None; count];
    for (display_id, entry) in (0_u32..).zip(entries) {
        if let Some(id) = slot(entry.index).and_then(|position| ids.get_mut(position)) {
            *id = Some(display_id);
        }
    }
    ids
}

/// A body state tessellated for display: its mesh with display ids, the render buffers and the
/// exact bounding box.
#[derive(Debug)]
pub(crate) struct DisplayMesh {
    /// The element table the display ids refer to.
    pub(crate) table: Arc<ElementTable>,
    /// The mesh with display face, edge and vertex ids.
    pub(crate) mesh: Mesh,
    /// The protocol buffers.
    pub(crate) buffers: RenderBuffers,
    /// The kernel's exact bounding box of the shape.
    pub(crate) bounding_box: BoundingBox,
}

impl DisplayMesh {
    /// Tessellates `shape` with `tolerance` and maps its kernel ids to the display ids of
    /// `table`: seam polylines are dropped, and every displayed vertex gets one marker.
    pub(crate) fn build(
        shape: &Shape,
        table: Arc<ElementTable>,
        tolerance: &TessellationTolerance,
    ) -> Result<DisplayMesh, EngineError> {
        let kernel = shape.tessellate(tolerance).map_err(kernel_failure)?;
        let face_ids = kernel
            .face_ids()
            .iter()
            .map(|&face| table.display_id(ElementKind::Face, face))
            .collect::<Option<Vec<u32>>>()
            .ok_or_else(|| {
                EngineError::internal("a tessellated face is not in the element table")
            })?;
        let edges: Vec<EdgePolyline> = kernel
            .edges()
            .iter()
            .filter_map(|edge| {
                Some(EdgePolyline {
                    edge_id: table.display_id(ElementKind::Edge, edge.edge_id)?,
                    points: edge.points.clone(),
                })
            })
            .collect();
        let mut markers: Vec<VertexMarker> = kernel
            .vertex_markers()
            .iter()
            .filter_map(|marker| {
                Some(VertexMarker {
                    vertex_id: table.display_id(ElementKind::Vertex, marker.vertex_id)?,
                    position: marker.position,
                })
            })
            .collect();
        let mut marked = vec![false; table.entries(ElementKind::Vertex).len()];
        for marker in &markers {
            if let Some(slot) = marked.get_mut(marker.vertex_id as usize) {
                *slot = true;
            }
        }
        for ((display_id, entry), marked) in (0_u32..)
            .zip(table.entries(ElementKind::Vertex))
            .zip(marked)
        {
            if !marked {
                let vertex = shape
                    .element(SubShapeKind::Vertex, entry.index)
                    .map_err(kernel_failure)?;
                markers.push(VertexMarker {
                    vertex_id: display_id,
                    position: [vertex.centroid.x, vertex.centroid.y, vertex.centroid.z],
                });
            }
        }
        markers.sort_by_key(|marker| marker.vertex_id);
        let mesh = Mesh::new(
            kernel.positions().to_vec(),
            kernel.triangles().to_vec(),
            face_ids,
            kernel.normals().map(<[[f64; 3]]>::to_vec),
            edges,
            markers,
        )
        .map_err(|error| EngineError::internal(error.to_string()))?;
        let buffers = mesh.render_buffers();
        let bounding_box = shape.bounding_box().map_err(kernel_failure)?;
        Ok(DisplayMesh {
            table,
            mesh,
            buffers,
            bounding_box,
        })
    }

    /// A rough size in bytes, for the cache budget.
    fn size(&self) -> usize {
        let buffers = &self.buffers;
        let floats = buffers.positions.len()
            + buffers.normals.len()
            + buffers.edge_positions.len()
            + buffers.vertex_positions.len();
        let integers = buffers.indices.len()
            + buffers.face_ids.len()
            + buffers.edge_ids.len()
            + buffers.vertex_ids.len()
            + buffers.source_vertices.len();
        let edge_points: usize = self.mesh.edges().iter().map(|edge| edge.points.len()).sum();
        let points = 2 * self.mesh.vertex_count() + edge_points + self.mesh.vertex_markers().len();
        let triangles = self.mesh.triangle_count();
        floats * size_of::<f32>()
            + integers * size_of::<u32>()
            + points * size_of::<[f64; 3]>()
            + triangles * (size_of::<[u32; 3]>() + size_of::<u32>())
    }
}

/// Identifies a display mesh: the body state and the tolerance, compared bit for bit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MeshKey {
    /// The body state.
    pub(crate) state: StateKey,
    /// `chord_deviation` bits.
    chord: u64,
    /// `angular_deviation` bits.
    angle: u64,
}

impl MeshKey {
    /// The key of `state` tessellated with `tolerance`.
    pub(crate) fn new(state: StateKey, tolerance: &TessellationTolerance) -> MeshKey {
        MeshKey {
            state,
            chord: tolerance.chord_deviation.to_bits(),
            angle: tolerance.angular_deviation.to_bits(),
        }
    }

    /// The tolerance the key was made with.
    pub(crate) fn tolerance(&self) -> TessellationTolerance {
        TessellationTolerance {
            chord_deviation: f64::from_bits(self.chord),
            angular_deviation: f64::from_bits(self.angle),
        }
    }
}

/// One cached value with its size and the tick of its last use.
#[derive(Debug)]
struct Cached<T> {
    /// The value; documents hold clones of the `Arc` of what they display.
    value: Arc<T>,
    /// Its size in bytes.
    size: usize,
    /// The cache tick of its last use.
    used: u64,
}

/// The session's element tables and display meshes, kept under a byte budget. Values that an
/// open document holds (its displayed meshes) are pinned: only the cache owns the others, and
/// those are evicted least recently used first. Hits and misses never change any output.
#[derive(Debug, Default)]
pub(crate) struct DisplayCache {
    /// Soft upper bound in bytes.
    budget: usize,
    /// Increases on every use.
    tick: u64,
    /// Element tables by body state.
    tables: BTreeMap<StateKey, Cached<ElementTable>>,
    /// Display meshes by body state and tolerance.
    meshes: BTreeMap<MeshKey, Cached<DisplayMesh>>,
}

impl DisplayCache {
    /// An empty cache with a byte budget.
    pub(crate) fn new(budget: usize) -> DisplayCache {
        DisplayCache {
            budget,
            ..DisplayCache::default()
        }
    }

    /// The element table of a body state with geometry.
    pub(crate) fn table(
        &mut self,
        key: &StateKey,
        state: &BodyState,
    ) -> Result<Arc<ElementTable>, EngineError> {
        self.tick += 1;
        if let Some(cached) = self.tables.get_mut(key) {
            cached.used = self.tick;
            return Ok(Arc::clone(&cached.value));
        }
        let table = Arc::new(ElementTable::build(state)?);
        self.tables.insert(
            key.clone(),
            Cached {
                size: table.size(),
                value: Arc::clone(&table),
                used: self.tick,
            },
        );
        // The caller holds the new table, so trimming keeps it.
        self.trim();
        Ok(table)
    }

    /// The display mesh of a body state with geometry at the key's tolerance.
    pub(crate) fn mesh(
        &mut self,
        key: &MeshKey,
        state: &BodyState,
    ) -> Result<Arc<DisplayMesh>, EngineError> {
        self.tick += 1;
        if let Some(cached) = self.meshes.get_mut(key) {
            cached.used = self.tick;
            return Ok(Arc::clone(&cached.value));
        }
        let shape = state
            .shape()
            .ok_or_else(|| EngineError::internal("a display mesh needs a shape"))?;
        let table = self.table(&key.state, state)?;
        let mesh = Arc::new(DisplayMesh::build(shape, table, &key.tolerance())?);
        self.meshes.insert(
            key.clone(),
            Cached {
                size: mesh.size(),
                value: Arc::clone(&mesh),
                used: self.tick,
            },
        );
        // The caller holds the new mesh, so trimming keeps it.
        self.trim();
        Ok(mesh)
    }

    /// Evicts unpinned values, least recently used first, until the cache fits its budget. Every
    /// insert trims, so typed callers stay within the budget too; `execute` and
    /// `close_document` trim again once they have unpinned what they replaced.
    pub(crate) fn trim(&mut self) {
        let mut total = self.size();
        if total <= self.budget {
            return;
        }
        // Meshes go first: a pinned mesh pins its table, so tables become evictable once their
        // meshes are gone.
        let mut meshes: Vec<(u64, MeshKey)> = self
            .meshes
            .iter()
            .filter(|(_, cached)| Arc::strong_count(&cached.value) == 1)
            .map(|(key, cached)| (cached.used, key.clone()))
            .collect();
        meshes.sort();
        for (_, key) in meshes {
            if total <= self.budget {
                return;
            }
            if let Some(cached) = self.meshes.remove(&key) {
                total -= cached.size;
            }
        }
        let mut tables: Vec<(u64, StateKey)> = self
            .tables
            .iter()
            .filter(|(_, cached)| Arc::strong_count(&cached.value) == 1)
            .map(|(key, cached)| (cached.used, key.clone()))
            .collect();
        tables.sort();
        for (_, key) in tables {
            if total <= self.budget {
                return;
            }
            if let Some(cached) = self.tables.remove(&key) {
                total -= cached.size;
            }
        }
    }

    /// Total size of the cached values in bytes.
    fn size(&self) -> usize {
        let tables: usize = self.tables.values().map(|cached| cached.size).sum();
        let meshes: usize = self.meshes.values().map(|cached| cached.size).sum();
        tables + meshes
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::Id;
    use partsmith_recompute::CacheKey;

    use super::*;

    fn state_key(byte: u8) -> StateKey {
        StateKey {
            feature: CacheKey([byte; 32]),
            body: Id::new("b-1").expect("a valid id"),
        }
    }

    fn entry(name: &str, index: u32) -> TableEntry {
        TableEntry {
            name: ElementName::parse(name).expect("a valid name"),
            index,
        }
    }

    fn table(faces: Vec<TableEntry>) -> ElementTable {
        ElementTable {
            face_ids: display_ids(&faces, 3),
            faces,
            edges: Vec::new(),
            vertices: Vec::new(),
            edge_ids: Vec::new(),
            vertex_ids: Vec::new(),
            descriptors: OnceLock::new(),
        }
    }

    #[test]
    fn display_ids_follow_table_positions() {
        let faces = vec![entry("f-1/a", 3), entry("f-1/b", 1)];

        let ids = display_ids(&faces, 3);

        assert_eq!(ids, [Some(1), None, Some(0)]);
    }

    #[test]
    fn display_ids_of_kernel_indices() {
        let table = table(vec![entry("f-1/a", 3), entry("f-1/b", 1)]);

        assert_eq!(table.display_id(ElementKind::Face, 3), Some(0));
        assert_eq!(table.display_id(ElementKind::Face, 2), None);
        assert_eq!(table.display_id(ElementKind::Face, 0), None);
        assert_eq!(table.display_id(ElementKind::Face, 9), None);
    }

    #[test]
    fn names_find_their_display_id() {
        let table = table(vec![entry("f-1/a", 3), entry("f-1/b", 1)]);

        let name = ElementName::parse("f-1/b").expect("a valid name");
        let missing = ElementName::parse("f-1/c").expect("a valid name");

        assert_eq!(table.find(&name), Some(1));
        assert_eq!(table.find(&missing), None);
        assert_eq!(table.count(ElementKind::Face), 2);
        assert_eq!(table.count(ElementKind::Edge), 0);
    }

    #[test]
    fn descriptors_are_computed_once() {
        let table = table(Vec::new());
        let mut calls = 0;

        let first = table.descriptors(|_| {
            calls += 1;
            Ok(Vec::new())
        });
        let second = table.descriptors(|_| unreachable!("descriptors are cached"));

        assert_eq!(calls, 1);
        assert_eq!(first, second);
    }

    #[test]
    fn mesh_keys_keep_their_tolerance() {
        let tolerance = TessellationTolerance {
            chord_deviation: 0.05,
            angular_deviation: 0.1,
        };

        let key = MeshKey::new(state_key(1), &tolerance);

        assert_eq!(key.tolerance(), tolerance);
        assert_ne!(
            key,
            MeshKey::new(
                state_key(1),
                &TessellationTolerance {
                    chord_deviation: 0.025,
                    ..tolerance
                }
            )
        );
    }

    #[test]
    fn trimming_keeps_pinned_tables_and_drops_the_rest() {
        let mut cache = DisplayCache::new(0);
        let pinned = Arc::new(table(Vec::new()));
        for (byte, value) in [(1, Arc::clone(&pinned)), (2, Arc::new(table(Vec::new())))] {
            cache.tables.insert(
                state_key(byte),
                Cached {
                    value,
                    size: 100,
                    used: u64::from(byte),
                },
            );
        }

        cache.trim();

        assert!(cache.tables.contains_key(&state_key(1)));
        assert!(!cache.tables.contains_key(&state_key(2)));
    }

    /// A session with display `display`, no cache budget and `fixture` open.
    fn session_without_budget(
        display: bool,
        fixture: &str,
    ) -> (crate::Session, partsmith_protocol::DocHandle) {
        let mut session = crate::Session::new(crate::SessionOptions {
            display,
            cache_budget: 0,
            ..crate::SessionOptions::headless()
        });
        let doc = session
            .open_document(fixture, Vec::new())
            .expect("the fixture opens")
            .doc;
        (session, doc)
    }

    #[test]
    fn typed_queries_keep_only_what_the_latest_query_used() {
        let (mut session, doc) =
            session_without_budget(false, include_str!("../tests/fixtures/two-bodies.ptsm"));
        let body = |text: &str| Id::new(text).expect("a valid id");

        for id in ["b-1", "b-2"] {
            session
                .list_elements(doc, &body(id), None)
                .expect("the body has elements");
        }
        let tables = session.cache.tables.len();
        session
            .export(
                doc,
                &crate::ExportRequest {
                    format: partsmith_protocol::ExportFormat::Stl {
                        binary: true,
                        per_body: false,
                    },
                    bodies: None,
                    tolerance: None,
                    orientation: None,
                },
            )
            .expect("both bodies export");

        assert_eq!(tables, 1);
        assert_eq!(session.cache.meshes.len(), 1);
    }

    #[test]
    fn closing_a_document_evicts_what_it_displayed() {
        let (mut session, doc) =
            session_without_budget(true, include_str!("../tests/fixtures/box.ptsm"));
        let mut output = crate::events::Output::default();
        session
            .collect_events(doc, &mut output)
            .expect("the box displays");
        assert_eq!(session.cache.meshes.len(), 1, "a displayed mesh is pinned");

        session.close_document(doc).expect("the document closes");

        assert!(session.cache.meshes.is_empty());
        assert!(session.cache.tables.is_empty());
    }

    #[test]
    fn trimming_within_the_budget_keeps_everything() {
        let mut cache = DisplayCache::new(1000);
        cache.tables.insert(
            state_key(1),
            Cached {
                value: Arc::new(table(Vec::new())),
                size: 100,
                used: 1,
            },
        );

        cache.trim();

        assert_eq!(cache.tables.len(), 1);
    }
}
