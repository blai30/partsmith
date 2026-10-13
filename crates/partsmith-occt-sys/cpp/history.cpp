// History normalization, copies, validity and repair (kernel spec sections 3.3 and 3.5).
#include <algorithm>
#include <utility>

#include <BRepBuilderAPI_Copy.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepCheck_Edge.hxx>
#include <BRepCheck_Face.hxx>
#include <BRepCheck_IndexedDataMapOfShapeResult.hxx>
#include <BRepCheck_ListOfStatus.hxx>
#include <BRepCheck_Result.hxx>
#include <BRepCheck_Shell.hxx>
#include <BRepCheck_Solid.hxx>
#include <BRepCheck_Vertex.hxx>
#include <BRepCheck_Wire.hxx>
#include <BRepLib.hxx>
#include <BRep_Builder.hxx>
#include <BRep_CurveRepresentation.hxx>
#include <BRep_GCurve.hxx>
#include <BRep_PointRepresentation.hxx>
#include <BRep_TEdge.hxx>
#include <BRep_TFace.hxx>
#include <BRep_TVertex.hxx>
#include <NCollection_Map.hxx>
#include <ShapeBuild_ReShape.hxx>
#include <ShapeFix_Shape.hxx>
#include <ShapeUpgrade_RemoveLocations.hxx>
#include <StdFail_NotDone.hxx>
#include <Standard_NullObject.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_MapOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {
namespace detail {

gp_Pnt to_point(const Vec3 &point) { return gp_Pnt(point.x, point.y, point.z); }

gp_Dir to_direction(const Vec3 &direction) {
  return gp_Dir(direction.x, direction.y, direction.z);
}

gp_Ax2 to_ax2(const Frame &frame) {
  return gp_Ax2(to_point(frame.origin), to_direction(frame.normal), to_direction(frame.x_axis));
}

gp_Ax3 to_ax3(const Frame &frame) { return gp_Ax3(to_ax2(frame)); }

Vec3 to_vec3(const gp_XYZ &coordinates) {
  return Vec3{coordinates.X(), coordinates.Y(), coordinates.Z()};
}

TopAbs_ShapeEnum shape_enum(ElementKind kind) {
  switch (kind) {
    case ElementKind::Solid:
      return TopAbs_SOLID;
    case ElementKind::Face:
      return TopAbs_FACE;
    case ElementKind::Edge:
      return TopAbs_EDGE;
    case ElementKind::Vertex:
      return TopAbs_VERTEX;
  }
  raise_invalid_input("Unknown element kind.");
}

std::shared_ptr<Shape> make_shape(const TopoDS_Shape &shape) {
  // Freeze the root like every sub-shape inside a container already is. TopoDS_Builder::Add
  // freezes each shape it adds, so a later compound holding this shape cannot change it (or
  // its BRep bytes); only roots no other Shape holds are still free here.
  TopoDS_Shape root = shape;
  if (root.Free()) {
    root.Free(Standard_False);
  }
  return std::make_shared<Shape>(root);
}

std::vector<TopoDS_Shape> shapes_of(rust::Slice<const ShapeHandle> handles) {
  std::vector<TopoDS_Shape> shapes;
  shapes.reserve(handles.size());
  for (const ShapeHandle &handle : handles) {
    if (!handle.shape) {
      Standard_NullObject::Raise("a shape handle is null");
    }
    shapes.push_back(handle.shape->get());
  }
  return shapes;
}

void raise_invalid_input(const std::string &message) {
  throw Partsmith_InvalidInput(message.c_str());
}

void raise_invalid_result(const std::string &message) {
  throw Partsmith_InvalidResult(message.c_str());
}

void raise_empty_result(const std::string &message) {
  throw Partsmith_EmptyResult(message.c_str());
}

void raise_read_error(const std::string &message) { throw Partsmith_ReadError(message.c_str()); }

void raise_not_done(const std::string &message) { throw StdFail_NotDone(message.c_str()); }

void require_solid(const TopoDS_Shape &shape, const char *algorithm) {
  if (!TopExp_Explorer(shape, TopAbs_SOLID).More()) {
    raise_empty_result(std::string(algorithm) + " produced no solid");
  }
}

TopoDS_Shape solids_only(const TopoDS_Shape &shape) {
  TopTools_IndexedMapOfShape solids;
  TopExp::MapShapes(shape, TopAbs_SOLID, solids);
  if (solids.Extent() == 1) {
    return solids(1);
  }
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  for (int index = 1; index <= solids.Extent(); ++index) {
    builder.Add(compound, solids(index));
  }
  return compound;
}

namespace {

// Raises StdFail_NotDone unless `copy` has as many elements of each kind as `shape`.
void require_same_counts(const Shape &shape, const Shape &copy, const char *algorithm) {
  for (ElementKind kind :
       {ElementKind::Solid, ElementKind::Face, ElementKind::Edge, ElementKind::Vertex}) {
    if (copy.map(kind).Extent() != shape.map(kind).Extent()) {
      raise_not_done(std::string(algorithm) + " changed the element counts");
    }
  }
}

}  // namespace

Shape copy_shape(const Shape &shape) {
  BRepBuilderAPI_Copy copier(shape.get(), Standard_True, Standard_False);
  Shape copy(copier.Shape());
  require_same_counts(shape, copy, "BRepBuilderAPI_Copy");
  return copy;
}

Shape copy_elements(const Shape &shape) {
  BRepBuilderAPI_Copy copier(shape.get(), Standard_False, Standard_False);
  Shape copy(copier.Shape());
  require_same_counts(shape, copy, "BRepBuilderAPI_Copy");
  return copy;
}

namespace {

// True when `shape` or one of its sub-shapes has a location, or a face of it a located surface.
bool has_location(const TopoDS_Shape &shape) {
  if (!shape.Location().IsIdentity()) {
    return true;
  }
  if (const Handle(BRep_TFace) face = Handle(BRep_TFace)::DownCast(shape.TShape());
      !face.IsNull() && !face->Location().IsIdentity()) {
    return true;
  }
  for (TopoDS_Iterator child(shape, Standard_False, Standard_False); child.More(); child.Next()) {
    if (has_location(child.Value())) {
      return true;
    }
  }
  return false;
}

}  // namespace

Shape copy_unlocated(const Shape &shape) {
  Shape copy = copy_shape(shape);
  if (!has_location(copy.get())) {
    return copy;
  }
  // Rebuilds every located element with its location applied to its geometry.
  ShapeUpgrade_RemoveLocations remover;
  remover.Remove(copy.get());
  Shape unlocated(remover.GetResult());
  require_same_counts(shape, unlocated, "ShapeUpgrade_RemoveLocations");
  return unlocated;
}

bool ElementState::operator==(const ElementState &other) const {
  if (parts != other.parts || values != other.values ||
      locations.size() != other.locations.size()) {
    return false;
  }
  for (std::size_t position = 0; position < locations.size(); ++position) {
    if (!locations[position].IsEqual(other.locations[position])) {
      return false;
    }
  }
  return true;
}

namespace {

// The state of a face as the validity check reads it (see ElementState): the face's own data,
// then each of its edges as the face sees it (tolerance, flags, the 3D curve and the curves on
// this face's surface, each with its range), then each of its vertices (point, tolerance and
// point representations). A curve an algorithm adds to an edge for another face changes nothing
// here.
ElementState face_state(const TopoDS_Face &face) {
  ElementState state;
  const Handle(BRep_TFace) own = Handle(BRep_TFace)::DownCast(face.TShape());
  if (!own.IsNull()) {
    state.parts = {own->Surface().get(), own->ActiveTriangulation().get()};
    state.values = {own->Tolerance(), own->NaturalRestriction() ? 1.0 : 0.0};
    state.locations = {own->Location()};
  }
  TopLoc_Location face_location;
  const Handle(Geom_Surface) &surface = BRep_Tool::Surface(face, face_location);
  for (TopExp_Explorer edges(face, TopAbs_EDGE); edges.More(); edges.Next()) {
    const Handle(BRep_TEdge) edge = Handle(BRep_TEdge)::DownCast(edges.Current().TShape());
    if (edge.IsNull()) {
      continue;
    }
    state.values.insert(state.values.end(),
                        {edge->Tolerance(), edge->SameParameter() ? 1.0 : 0.0,
                         edge->SameRange() ? 1.0 : 0.0, edge->Degenerated() ? 1.0 : 0.0});
    const TopLoc_Location on_face = face_location.Predivided(edges.Current().Location());
    for (const Handle(BRep_CurveRepresentation) &representation : edge->Curves()) {
      const bool on_this_face = representation->IsCurveOnSurface(surface, on_face);
      if (!representation->IsCurve3D() && !on_this_face) {
        continue;
      }
      state.parts.push_back(representation.get());
      state.locations.push_back(representation->Location());
      if (representation->IsCurve3D()) {
        state.parts.push_back(representation->Curve3D().get());
      } else {
        state.parts.push_back(representation->PCurve().get());
        if (representation->IsCurveOnClosedSurface()) {
          state.parts.push_back(representation->PCurve2().get());
        }
      }
      if (const Handle(BRep_GCurve) curve = Handle(BRep_GCurve)::DownCast(representation);
          !curve.IsNull()) {
        state.values.push_back(curve->First());
        state.values.push_back(curve->Last());
      }
    }
  }
  for (TopExp_Explorer vertices(face, TopAbs_VERTEX); vertices.More(); vertices.Next()) {
    const Handle(BRep_TVertex) vertex =
        Handle(BRep_TVertex)::DownCast(vertices.Current().TShape());
    if (vertex.IsNull()) {
      continue;
    }
    const gp_Pnt &point = vertex->Pnt();
    state.values.insert(state.values.end(),
                        {vertex->Tolerance(), point.X(), point.Y(), point.Z()});
    for (const Handle(BRep_PointRepresentation) &representation : vertex->Points()) {
      state.parts.push_back(representation.get());
      state.locations.push_back(representation->Location());
      state.values.push_back(representation->Parameter());
      if (representation->IsPointOnSurface()) {
        state.values.push_back(representation->Parameter2());
      }
    }
  }
  return state;
}

}  // namespace

CheckInputs::CheckInputs(std::vector<TopoDS_Shape> shapes) : shapes_(std::move(shapes)) {
  TopTools_IndexedMapOfShape faces;
  for (const TopoDS_Shape &shape : shapes_) {
    TopExp::MapShapes(shape, TopAbs_FACE, faces);
  }
  states_.reserve(static_cast<std::size_t>(faces.Extent()));
  for (int index = 1; index <= faces.Extent(); ++index) {
    states_.emplace_back(faces(index), face_state(TopoDS::Face(faces(index))));
  }
}

void CheckInputs::settle() { settled_ = compare(); }

TopTools_MapOfShape CheckInputs::altered_faces() const {
  return settled_.has_value() ? *settled_ : compare();
}

TopTools_MapOfShape CheckInputs::compare() const {
  TopTools_MapOfShape faces;
  for (const auto &[face, state] : states_) {
    if (!(face_state(TopoDS::Face(face)) == state)) {
      faces.Add(face);
    }
  }
  return faces;
}

void store_plane_pcurves(const TopoDS_Shape &shape) {
  for (TopExp_Explorer faces(shape, TopAbs_FACE); faces.More(); faces.Next()) {
    const TopoDS_Face &face = TopoDS::Face(faces.Current());
    for (TopExp_Explorer edges(face, TopAbs_EDGE); edges.More(); edges.Next()) {
      BRepLib::BuildPCurveForEdgeOnPlane(TopoDS::Edge(edges.Current()), face);
    }
  }
}

void merge_history(const Handle(BRepTools_History) &history,
                   const Handle(BRepTools_History) &next) {
  if (!next.IsNull()) {
    history->Merge(next);
  }
}

namespace {

// The checks of BRepCheck_Analyzer, with its default settings, limited to what an operation
// produced or changed (kernel spec section 3.5): every face of the result that is not an
// unchanged face of an input (by IsSame, and not changed in place), with its wires, edges and
// vertices, each edge and vertex also in every face and edge of the result that contains it, and
// every shell and solid that is not an unchanged shell or solid of an input. The rest of the
// result is the inputs' own unchanged elements.
class ChangeCheck {
 public:
  ChangeCheck(const TopoDS_Shape &shape, const CheckInputs &inputs) : shape_(shape) {
    const TopTools_MapOfShape altered = inputs.altered_faces();
    for (const TopoDS_Shape &input : inputs.shapes()) {
      add_unchanged(input, TopAbs_FACE, altered, input_faces_);
      add_unchanged(input, TopAbs_SHELL, altered, input_shells_);
      add_unchanged(input, TopAbs_SOLID, altered, input_solids_);
    }
    for (TopExp_Explorer face(shape, TopAbs_FACE); face.More(); face.Next()) {
      if (input_faces_.Contains(face.Current())) {
        keeps_input_faces_ = true;
      } else {
        TopExp::MapShapes(face.Current(), TopAbs_EDGE, changed_edges_);
        TopExp::MapShapes(face.Current(), TopAbs_VERTEX, changed_vertices_);
      }
    }
  }

  // True when some face of the result is a face of an input.
  bool keeps_input_faces() const { return keeps_input_faces_; }

  // Runs the checks; true when every one passed.
  bool run() {
    visit(shape_);
    return valid();
  }

 private:
  // Adds the sub-shapes of `input` of `type` that hold no face changed in place.
  static void add_unchanged(const TopoDS_Shape &input, TopAbs_ShapeEnum type,
                            const TopTools_MapOfShape &altered,
                            TopTools_IndexedMapOfShape &unchanged) {
    TopTools_IndexedMapOfShape elements;
    TopExp::MapShapes(input, type, elements);
    for (int index = 1; index <= elements.Extent(); ++index) {
      bool holds_altered = false;
      for (TopExp_Explorer face(elements(index), TopAbs_FACE); !altered.IsEmpty() && face.More();
           face.Next()) {
        holds_altered = holds_altered || altered.Contains(face.Current());
      }
      if (!holds_altered) {
        unchanged.Add(elements(index));
      }
    }
  }

  // True when every recorded status is BRepCheck_NoError.
  bool valid() const {
    for (int index = 1; index <= results_.Extent(); ++index) {
      const Handle(BRepCheck_Result) &result = results_(index);
      if (!all_clear(result->Status())) {
        return false;
      }
      for (result->InitContextIterator(); result->MoreShapeInContext();
           result->NextShapeInContext()) {
        if (!all_clear(result->StatusOnShape())) {
          return false;
        }
      }
    }
    return true;
  }

  static bool all_clear(const BRepCheck_ListOfStatus &statuses) {
    for (const BRepCheck_Status status : statuses) {
      if (status != BRepCheck_NoError) {
        return false;
      }
    }
    return true;
  }

  // The result of one occurrence of an element (orientation included, as in the analyzer),
  // created with its minimum checks on first use.
  const Handle(BRepCheck_Result) &result(const TopoDS_Shape &element) {
    if (const Handle(BRepCheck_Result) *found = results_.Seek(element)) {
      return *found;
    }
    Handle(BRepCheck_Result) created;
    switch (element.ShapeType()) {
      case TopAbs_VERTEX:
        created = new BRepCheck_Vertex(TopoDS::Vertex(element));
        break;
      case TopAbs_EDGE: {
        const Handle(BRepCheck_Edge) edge = new BRepCheck_Edge(TopoDS::Edge(element));
        edge->GeometricControls(Standard_True);
        edge->SetExactMethod(Standard_False);
        created = edge;
        break;
      }
      case TopAbs_WIRE: {
        const Handle(BRepCheck_Wire) wire = new BRepCheck_Wire(TopoDS::Wire(element));
        wire->GeometricControls(Standard_True);
        created = wire;
        break;
      }
      case TopAbs_FACE: {
        const Handle(BRepCheck_Face) face = new BRepCheck_Face(TopoDS::Face(element));
        face->GeometricControls(Standard_True);
        created = face;
        break;
      }
      case TopAbs_SHELL:
        created = new BRepCheck_Shell(TopoDS::Shell(element));
        break;
      case TopAbs_SOLID:
        created = new BRepCheck_Solid(TopoDS::Solid(element));
        break;
      default:
        raise_not_done("the validity check met an unexpected sub-shape type");
    }
    return results_(results_.Add(element, created));
  }

  // Checks each element occurrence once, parents before children like the analyzer.
  void visit(const TopoDS_Shape &shape) {
    if (!visited_.Add(shape)) {
      return;
    }
    switch (shape.ShapeType()) {
      case TopAbs_SOLID:
        if (!input_solids_.Contains(shape)) {
          result(shape);
          for (TopoDS_Iterator shell(shape); shell.More(); shell.Next()) {
            if (shell.Value().ShapeType() == TopAbs_SHELL) {
              result(shell.Value())->InContext(shape);
            }
          }
        }
        break;
      case TopAbs_SHELL:
        if (!input_shells_.Contains(shape)) {
          result(shape);
        }
        break;
      case TopAbs_FACE:
        if (input_faces_.Contains(shape)) {
          // Nothing below an unchanged face changed except along its border.
          check_border_of_unchanged_face(shape);
          return;
        }
        check_changed_face(TopoDS::Face(shape));
        break;
      case TopAbs_EDGE:
        check_edge(shape);
        break;
      case TopAbs_VERTEX:
        if (changed_vertices_.Contains(shape)) {
          result(shape);
        }
        break;
      default:
        break;
    }
    for (TopoDS_Iterator child(shape); child.More(); child.Next()) {
      visit(child.Value());
    }
  }

  // The analyzer's checks of a face: its vertices, edges and wires in its context, then the
  // orientation of its wires when they allow it.
  void check_changed_face(const TopoDS_Face &face) {
    result(face);
    TopTools_MapOfShape done;
    for (TopExp_Explorer vertex(face, TopAbs_VERTEX); vertex.More(); vertex.Next()) {
      if (done.Add(vertex.Current())) {
        result(vertex.Current())->InContext(face);
      }
    }
    bool wires_checkable = true;
    done.Clear();
    for (TopExp_Explorer edge(face, TopAbs_EDGE); edge.More(); edge.Next()) {
      if (!done.Add(edge.Current())) {
        continue;
      }
      const Handle(BRepCheck_Result) &edge_result = result(edge.Current());
      edge_result->InContext(face);
      if (wires_checkable && edge_result->IsStatusOnShape(face)) {
        for (const BRepCheck_Status status : edge_result->StatusOnShape(face)) {
          if (status == BRepCheck_NoCurveOnSurface || status == BRepCheck_InvalidCurveOnSurface ||
              status == BRepCheck_InvalidRange ||
              status == BRepCheck_InvalidCurveOnClosedSurface) {
            wires_checkable = false;
          }
        }
      }
    }
    bool wires_oriented = wires_checkable;
    for (TopExp_Explorer wire(face, TopAbs_WIRE); wire.More(); wire.Next()) {
      const Handle(BRepCheck_Result) &wire_result = result(wire.Current());
      wire_result->InContext(face);
      if (wires_oriented && wire_result->IsStatusOnShape(face) &&
          !all_clear(wire_result->StatusOnShape(face))) {
        wires_oriented = false;
      }
    }
    const Handle(BRepCheck_Face) face_result = Handle(BRepCheck_Face)::DownCast(result(face));
    if (wires_oriented) {
      face_result->OrientationOfWires(Standard_True);
    } else {
      face_result->SetUnorientable();
    }
  }

  // An unchanged face shares edges and vertices with changed faces along the border of what
  // the operation changed; those are checked in its context and in its edges' too.
  void check_border_of_unchanged_face(const TopoDS_Shape &face) {
    TopTools_MapOfShape done;
    for (TopExp_Explorer vertex(face, TopAbs_VERTEX); vertex.More(); vertex.Next()) {
      if (changed_vertices_.Contains(vertex.Current()) && done.Add(vertex.Current())) {
        result(vertex.Current())->InContext(face);
      }
    }
    for (TopExp_Explorer edge(face, TopAbs_EDGE); edge.More(); edge.Next()) {
      if (changed_edges_.Contains(edge.Current()) && done.Add(edge.Current())) {
        result(edge.Current())->InContext(face);
      }
      if (visited_.Add(edge.Current())) {
        check_edge(edge.Current());
      }
    }
  }

  // The analyzer's checks of an edge (its triangulation polygons and its vertices in its
  // context), for edges of changed faces; vertices of changed faces are also checked in the
  // context of every other edge that contains them.
  void check_edge(const TopoDS_Shape &edge) {
    const bool changed = changed_edges_.Contains(edge);
    if (changed) {
      const Handle(BRepCheck_Edge) edge_result = Handle(BRepCheck_Edge)::DownCast(result(edge));
      const BRepCheck_Status status = edge_result->CheckPolygonOnTriangulation(TopoDS::Edge(edge));
      if (status != BRepCheck_NoError) {
        edge_result->SetStatus(status);
      }
    }
    TopTools_MapOfShape done;
    for (TopExp_Explorer vertex(edge, TopAbs_VERTEX); vertex.More(); vertex.Next()) {
      if ((changed || changed_vertices_.Contains(vertex.Current())) && done.Add(vertex.Current())) {
        result(vertex.Current())->InContext(edge);
      }
    }
  }

  const TopoDS_Shape shape_;
  // Unchanged faces, shells and solids of the inputs (IsSame semantics).
  TopTools_IndexedMapOfShape input_faces_;
  TopTools_IndexedMapOfShape input_shells_;
  TopTools_IndexedMapOfShape input_solids_;
  // Edges and vertices of changed faces (IsSame semantics).
  TopTools_IndexedMapOfShape changed_edges_;
  TopTools_IndexedMapOfShape changed_vertices_;
  // Occurrences already visited (orientation included).
  NCollection_Map<TopoDS_Shape> visited_;
  BRepCheck_IndexedDataMapOfShapeResult results_;
  bool keeps_input_faces_ = false;
};

}  // namespace

bool changed_part_valid(const TopoDS_Shape &shape, const CheckInputs &inputs) {
  if (inputs.shapes().empty()) {
    return BRepCheck_Analyzer(shape).IsValid();
  }
  ChangeCheck check(shape, inputs);
  if (!check.keeps_input_faces()) {
    return BRepCheck_Analyzer(shape).IsValid();
  }
  try {
    return check.run();
  } catch (const Standard_Failure &) {
    // The analyzer records a check that throws as a failure.
    return false;
  }
}

void ensure_valid(TopoDS_Shape &shape, const Handle(BRepTools_History) &history,
                  const CheckInputs &inputs, const char *algorithm) {
  if (changed_part_valid(shape, inputs)) {
    return;
  }
  initialize();
  // Repair a copy: the result may share sub-shapes with the operation's inputs.
  BRepBuilderAPI_Copy copier(shape, Standard_True, Standard_False);
  TopTools_ListOfShape arguments;
  arguments.Append(shape);
  const Handle(BRepTools_History) copy_history = new BRepTools_History(arguments, copier);
  ShapeFix_Shape fixer(copier.Shape());
  fixer.SetPrecision(kLinear);
  fixer.Perform();
  const TopoDS_Shape fixed = fixer.Shape();
  const bool lost_solids = TopExp_Explorer(shape, TopAbs_SOLID).More() &&
                           !TopExp_Explorer(fixed, TopAbs_SOLID).More();
  if (lost_solids || !BRepCheck_Analyzer(fixed).IsValid()) {
    raise_invalid_result(std::string(algorithm) +
                         " produced a shape that stays invalid after ShapeFix_Shape");
  }
  merge_history(history, copy_history);
  merge_history(history, fixer.Context()->History());
  shape = fixed;
}

TopoDS_Shape finish_solid(const TopoDS_Shape &shape, const Handle(BRepTools_History) &history,
                          const CheckInputs &inputs, const char *algorithm) {
  TopoDS_Shape result = solids_only(shape);
  require_solid(result, algorithm);
  ensure_valid(result, history, inputs, algorithm);
  return solids_only(result);
}

namespace {

// The rank of a sub-shape type in history lists (faces, then edges, then vertices), or -1.
int element_rank(TopAbs_ShapeEnum type) {
  switch (type) {
    case TopAbs_FACE:
      return 0;
    case TopAbs_EDGE:
      return 1;
    case TopAbs_VERTEX:
      return 2;
    default:
      return -1;
  }
}

constexpr ElementKind kRankKinds[] = {ElementKind::Face, ElementKind::Edge, ElementKind::Vertex};

// The fate of one input element (kernel spec section 3.3, items 1 to 4).
FateData fate_of(const TopoDS_Shape &element, const TopTools_IndexedMapOfShape &same_kind,
                 const Handle(BRepTools_History) &history, const Shape &result) {
  std::vector<std::uint32_t> modified;
  const int own = same_kind.FindIndex(element);
  if (own > 0) {
    modified.push_back(static_cast<std::uint32_t>(own));
  } else if (!history.IsNull()) {
    for (const TopoDS_Shape &image : history->Modified(element)) {
      const int index = same_kind.FindIndex(image);
      if (index > 0) {
        modified.push_back(static_cast<std::uint32_t>(index));
      }
    }
  }
  std::sort(modified.begin(), modified.end());
  modified.erase(std::unique(modified.begin(), modified.end()), modified.end());

  std::vector<std::pair<int, std::uint32_t>> generated;
  if (!history.IsNull()) {
    const TopTools_IndexedMapOfShape *maps[] = {&result.faces(), &result.edges(),
                                                &result.vertices()};
    for (const TopoDS_Shape &shape : history->Generated(element)) {
      const int rank = element_rank(shape.ShapeType());
      if (rank < 0) {
        continue;
      }
      const int index = maps[rank]->FindIndex(shape);
      if (index > 0) {
        generated.emplace_back(rank, static_cast<std::uint32_t>(index));
      }
    }
  }
  std::sort(generated.begin(), generated.end());
  generated.erase(std::unique(generated.begin(), generated.end()), generated.end());

  FateData fate;
  for (std::uint32_t index : modified) {
    fate.modified.push_back(index);
  }
  for (const auto &[rank, index] : generated) {
    fate.generated.push_back(SubShapeData{kRankKinds[rank], index});
  }
  return fate;
}

// The fates of every input element of one type, in the input's index order.
rust::Vec<FateData> fates_of(const TopoDS_Shape &input, TopAbs_ShapeEnum type,
                             const TopTools_IndexedMapOfShape &same_kind,
                             const Handle(BRepTools_History) &history, const Shape &result) {
  TopTools_IndexedMapOfShape elements;
  TopExp::MapShapes(input, type, elements);
  rust::Vec<FateData> fates;
  fates.reserve(static_cast<std::size_t>(elements.Extent()));
  for (int index = 1; index <= elements.Extent(); ++index) {
    fates.push_back(fate_of(elements(index), same_kind, history, result));
  }
  return fates;
}

}  // namespace

rust::Vec<InputHistoryData> history_data(const Handle(BRepTools_History) &history,
                                         const std::vector<TopoDS_Shape> &inputs,
                                         const Shape &result) {
  rust::Vec<InputHistoryData> data;
  data.reserve(inputs.size());
  for (const TopoDS_Shape &input : inputs) {
    data.push_back(InputHistoryData{
        fates_of(input, TopAbs_FACE, result.faces(), history, result),
        fates_of(input, TopAbs_EDGE, result.edges(), history, result),
        fates_of(input, TopAbs_VERTEX, result.vertices(), history, result),
    });
  }
  return data;
}

OpResultData op_result(const TopoDS_Shape &result, const Handle(BRepTools_History) &history,
                       const std::vector<TopoDS_Shape> &inputs) {
  std::shared_ptr<Shape> shape = make_shape(result);
  rust::Vec<InputHistoryData> data = history_data(history, inputs, *shape);
  return OpResultData{std::move(shape), std::move(data)};
}

}  // namespace detail
}  // namespace occt
}  // namespace partsmith
