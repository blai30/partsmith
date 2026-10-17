// Fillet, chamfer and shell with history (kernel spec sections 3.4.6 to 3.4.9).
#include <algorithm>
#include <array>
#include <cmath>
#include <exception>
#include <map>
#include <mutex>
#include <optional>
#include <unordered_map>
#include <utility>
#include <vector>

#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Defeaturing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepLib.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffset_Analyse.hxx>
#include <BRepOffset_Interval.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_CurveOn2Surfaces.hxx>
#include <BRep_CurveRepresentation.hxx>
#include <BRep_ListOfCurveRepresentation.hxx>
#include <BRep_TEdge.hxx>
#include <BRep_Tool.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_Surface.hxx>
#include <Precision.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <gp.hxx>
#include <gp_Pln.hxx>
#include <gp_Torus.hxx>
#include <gp_Vec.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

namespace {

// One requested blend: a fillet radius (as both sizes) or two chamfer distances, the first
// measured on `face` when `on_face` is set.
struct BlendRequest {
  std::uint32_t edge;
  double size;
  double other_size;
  bool on_face;
  std::uint32_t face;
};

// Raises Partsmith_InvalidInput for a seam or degenerated edge, which cannot be blended, and for
// a chamfer side face that does not touch its edge.
void check_requests(const Shape &shape, const std::vector<BlendRequest> &requests) {
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndUniqueAncestors(shape.get(), TopAbs_EDGE, TopAbs_FACE, edge_faces);
  for (const BlendRequest &request : requests) {
    const TopoDS_Edge &edge = TopoDS::Edge(shape.edges()(static_cast<int>(request.edge)));
    const TopTools_ListOfShape *faces = edge_faces.Seek(edge);
    bool seam = false;
    bool adjacent = false;
    if (faces != nullptr) {
      for (const TopoDS_Shape &face : *faces) {
        seam = seam || BRep_Tool::IsClosed(edge, TopoDS::Face(face));
        adjacent = adjacent ||
                   (request.on_face && face.IsSame(shape.faces()(static_cast<int>(request.face))));
      }
    }
    if (seam || BRep_Tool::Degenerated(edge)) {
      raise_invalid_input("Edge " + std::to_string(request.edge) +
                          " is a seam or a degenerated edge and cannot be rounded or beveled.");
    }
    if (request.on_face && !adjacent) {
      raise_invalid_input("Face " + std::to_string(request.face) + " is not next to edge " +
                          std::to_string(request.edge) + ", so it cannot set the chamfer side.");
    }
  }
}

// The copy's edge of a request.
const TopoDS_Edge &request_edge(const Shape &copy, const BlendRequest &request) {
  return TopoDS::Edge(copy.edges()(static_cast<int>(request.edge)));
}

// Checks the contours OCCT built from the requests: every edge is in one, and edges of one
// tangent contour have the same size.
template <class Blend>
void check_contours(const Blend &blend, const Shape &copy,
                    const std::vector<BlendRequest> &requests, const char *algorithm) {
  std::map<int, const BlendRequest *> first_in_contour;
  for (const BlendRequest &request : requests) {
    const int contour = blend.Contour(request_edge(copy, request));
    if (contour == 0) {
      raise_not_done(std::string(algorithm) + " cannot blend edge " +
                     std::to_string(request.edge) + " between its faces");
    }
    const auto [entry, inserted] = first_in_contour.emplace(contour, &request);
    const BlendRequest &first = *entry->second;
    if (!inserted && (std::abs(first.size - request.size) > kLinear ||
                      std::abs(first.other_size - request.other_size) > kLinear)) {
      raise_invalid_input("Edges " + std::to_string(first.edge) + " and " +
                          std::to_string(request.edge) +
                          " are tangent to each other, so they must use the same size.");
    }
  }
}

// The name of a fillet contour status.
const char *status_name(ChFiDS_ErrorStatus status) {
  switch (status) {
    case ChFiDS_Ok:
      return "ChFiDS_Ok";
    case ChFiDS_Error:
      return "ChFiDS_Error";
    case ChFiDS_WalkingFailure:
      return "ChFiDS_WalkingFailure";
    case ChFiDS_StartsolFailure:
      return "ChFiDS_StartsolFailure";
    case ChFiDS_TwistedSurface:
      return "ChFiDS_TwistedSurface";
  }
  return "ChFiDS_Error";
}

// The failure message of a fillet that is not done, naming each faulty contour's status.
std::string fillet_failure(const BRepFilletAPI_MakeFillet &fillet) {
  std::string message = "BRepFilletAPI_MakeFillet failed";
  for (int position = 1; position <= fillet.NbFaultyContours(); ++position) {
    const int contour = fillet.FaultyContour(position);
    message += "; contour " + std::to_string(contour) + ": " +
               status_name(fillet.StripeStatus(contour));
  }
  return message;
}

// Serializes fillet and chamfer builds (kernel spec section 3.10, item 5). OCCT's blend builder
// assembles every result with TopOpeBRepBuild, which keeps state between calls in file-level
// statics: `STATIC_SOLIDINDEX` in TopOpeBRepBuild_Builder.cxx tells `SplitShapes` which solid to
// classify an edge against, so concurrent blends classified edges against each other's solids.
std::mutex &blend_mutex() {
  static std::mutex mutex;
  return mutex;
}

// A blend as OCCT built it, with its history.
struct BuiltBlend {
  TopoDS_Shape shape;
  Handle(BRepTools_History) history;
};

// Builds a blend on `copy`: adds each request with `add`, checks the contours and builds, raising
// StdFail_NotDone with the message `failure` gives when the blend is not done. Everything that
// touches OCCT's blend builder, from creating it to reading its history, holds `blend_mutex`.
// OCCT lists fillet and chamfer faces as generated by their edges, and corner faces as generated
// by their vertices (kernel spec section 7, open issue 4, verified by the blend tests).
template <class Blend, class Add, class Failure>
BuiltBlend build_blend(const Shape &copy, const std::vector<BlendRequest> &requests,
                       const char *algorithm, Add add, Failure failure) {
  const std::lock_guard<std::mutex> lock(blend_mutex());
  Blend blend(copy.get());
  for (const BlendRequest &request : requests) {
    add(blend, copy, request);
  }
  check_contours(blend, copy, requests, algorithm);
  blend.Build();
  if (!blend.IsDone()) {
    raise_not_done(failure(blend));
  }
  return BuiltBlend{blend.Shape(), history_of(copy.get(), blend)};
}

// Fillets or chamfers edges of a copy of `shape`, so the input is never written to (kernel spec
// section 3.1): checks the requests, builds the blend (`build_blend`) and returns a normalized,
// checked result. The copy's element indices are the input's, and `inputs` records its state
// before the blend.
template <class Blend, class Add, class Failure>
OpResultData run_blend(const Shape &shape, const std::vector<BlendRequest> &requests,
                       const char *algorithm, Add add, Failure failure) {
  check_requests(shape, requests);
  const Shape copy = copy_shape(shape);
  CheckInputs inputs({copy.get()});
  const BuiltBlend built = build_blend<Blend>(copy, requests, algorithm, add, failure);
  // The result shares edges only with the copy.
  store_result_pcurves(inputs, built.shape);
  return op_result(finish_solid(built.shape, built.history, inputs, algorithm), built.history,
                   {copy.get()});
}

// The name of an offset error.
const char *offset_error_name(BRepOffset_Error error) {
  switch (error) {
    case BRepOffset_NoError:
      return "BRepOffset_NoError";
    case BRepOffset_UnknownError:
      return "BRepOffset_UnknownError";
    case BRepOffset_BadNormalsOnGeometry:
      return "BRepOffset_BadNormalsOnGeometry";
    case BRepOffset_C0Geometry:
      return "BRepOffset_C0Geometry";
    case BRepOffset_NullOffset:
      return "BRepOffset_NullOffset";
    case BRepOffset_NotConnectedShell:
      return "BRepOffset_NotConnectedShell";
    case BRepOffset_CannotTrimEdges:
      return "BRepOffset_CannotTrimEdges";
    case BRepOffset_CannotFuseVertices:
      return "BRepOffset_CannotFuseVertices";
    case BRepOffset_CannotExtentEdge:
      return "BRepOffset_CannotExtentEdge";
    case BRepOffset_UserBreak:
      return "BRepOffset_UserBreak";
    case BRepOffset_MixedConnectivity:
      return "BRepOffset_MixedConnectivity";
  }
  return "BRepOffset_UnknownError";
}

// A solid bounded by the outer shells of `solid` and the reversed shells of `inner`.
TopoDS_Shape hollow_solid(const TopoDS_Shape &solid, const TopoDS_Shape &inner) {
  BRep_Builder builder;
  TopoDS_Solid hollow;
  builder.MakeSolid(hollow);
  for (TopExp_Explorer shells(solid, TopAbs_SHELL); shells.More(); shells.Next()) {
    builder.Add(hollow, shells.Current());
  }
  for (TopExp_Explorer shells(inner, TopAbs_SHELL); shells.More(); shells.Next()) {
    builder.Add(hollow, shells.Current().Reversed());
  }
  return hollow;
}

// A shape with the rank of its first face, for sorting.
template <class Part>
using Ranked = std::pair<int, Part>;

// Sorts by rank, keeping the order of equal ranks.
template <class Part>
void sort_by_rank(std::vector<Ranked<Part>> &parts) {
  std::stable_sort(parts.begin(), parts.end(),
                   [](const Ranked<Part> &first, const Ranked<Part> &second) {
                     return first.first < second.first;
                   });
}

// Makes an empty shell or solid.
void make_empty(const BRep_Builder &builder, TopoDS_Shell &shell) { builder.MakeShell(shell); }
void make_empty(const BRep_Builder &builder, TopoDS_Solid &solid) { builder.MakeSolid(solid); }

// A new shell or solid holding `parts` sorted by rank, closed like `original`, with the rank of
// its first part, or `unranked` when it holds none.
template <class Container, class Part>
Ranked<Container> ranked_container(std::vector<Ranked<Part>> &parts,
                                   const TopoDS_Shape &original, int unranked) {
  sort_by_rank(parts);
  const BRep_Builder builder;
  Container container;
  make_empty(builder, container);
  for (const Ranked<Part> &part : parts) {
    builder.Add(container, part.second);
  }
  container.Closed(original.Closed());
  return {parts.empty() ? unranked : parts.front().first, container};
}

// The shell result rebuilt with its solids, shells and faces in an order that depends only on
// the input (kernel spec sections 3.4.8 and 3.11). A face's rank is its position among the
// faces that the faces of `copy` are, were modified into or generated, then those its edges
// and then its vertices generated, each kind in index order; shells and solids follow their
// first face. OCCT's offset assembles shells from hash maps keyed by shape addresses, so its
// own order, and with it every element index and the last bits of the volume, changed from run
// to run. Faces, edges and vertices are kept, so the history still applies.
TopoDS_Shape ordered_by_input(const Shape &copy, const TopoDS_Shape &result,
                              const Handle(BRepTools_History) &history) {
  TopTools_IndexedMapOfShape ranked;
  const auto add_faces = [&ranked](const TopTools_ListOfShape &images) {
    for (const TopoDS_Shape &image : images) {
      if (image.ShapeType() == TopAbs_FACE) {
        ranked.Add(image);
      }
    }
  };
  for (int index = 1; index <= copy.faces().Extent(); ++index) {
    ranked.Add(copy.faces()(index));
    add_faces(history->Modified(copy.faces()(index)));
    add_faces(history->Generated(copy.faces()(index)));
  }
  for (ElementKind kind : {ElementKind::Edge, ElementKind::Vertex}) {
    for (int index = 1; index <= copy.map(kind).Extent(); ++index) {
      add_faces(history->Generated(copy.map(kind)(index)));
    }
  }
  // A face that no input element accounts for (none is expected) comes after the others.
  const int unranked = ranked.Extent() + 1;
  const auto rank_of = [&ranked, unranked](const TopoDS_Shape &face) {
    const int rank = ranked.FindIndex(face);
    return rank > 0 ? rank : unranked;
  };

  std::vector<Ranked<TopoDS_Solid>> solids;
  for (TopExp_Explorer solid(result, TopAbs_SOLID); solid.More(); solid.Next()) {
    std::vector<Ranked<TopoDS_Shell>> shells;
    for (TopExp_Explorer shell(solid.Current(), TopAbs_SHELL); shell.More(); shell.Next()) {
      // Each face keeps its orientation as explored, so the rebuilt shell bounds the same side.
      std::vector<Ranked<TopoDS_Shape>> faces;
      for (TopExp_Explorer face(shell.Current(), TopAbs_FACE); face.More(); face.Next()) {
        faces.emplace_back(rank_of(face.Current()), face.Current());
      }
      shells.push_back(ranked_container<TopoDS_Shell>(faces, shell.Current(), unranked));
    }
    solids.push_back(ranked_container<TopoDS_Solid>(shells, solid.Current(), unranked));
  }
  sort_by_rank(solids);
  std::vector<TopoDS_Shape> ordered;
  ordered.reserve(solids.size());
  for (const Ranked<TopoDS_Solid> &solid : solids) {
    ordered.push_back(solid.second);
  }
  return single_or_compound(ordered);
}

// One (surface, location) pair an edge's curve representations can refer to, with the index of
// the first face of the shape on it. The location is the face's surface location relative to the
// edge's own, as OCCT stores it in the representations (BRep_Tool::CurveOnSurface).
struct SurfaceRank {
  const Geom_Surface *surface;
  TopLoc_Location location;
  int rank;
};

// The rank of a representation's surface and location among `ranks`, or 0 when no face of the
// shape is on it.
int surface_rank(const std::vector<SurfaceRank> &ranks, const Handle(Geom_Surface) &surface,
                 const TopLoc_Location &location) {
  for (const SurfaceRank &entry : ranks) {
    if (entry.surface == surface.get() && entry.location.IsEqual(location)) {
      return entry.rank;
    }
  }
  return 0;
}

// One curve representation as it is stored, with its sort key: its class (the 3D curve and
// polygon first, then the curves on faces, then the continuity records, then anything else) and
// the ranks of its faces. A continuity record is stored naming its lower face first.
std::pair<std::array<int, 3>, Handle(BRep_CurveRepresentation)> keyed_representation(
    const Handle(BRep_CurveRepresentation) &representation,
    const std::vector<SurfaceRank> &ranks) {
  if (representation->IsCurve3D() || representation->IsPolygon3D()) {
    return {{0, 0, 0}, representation};
  }
  if (representation->IsCurveOnSurface() || representation->IsPolygonOnSurface()) {
    return {{1, surface_rank(ranks, representation->Surface(), representation->Location()), 0},
            representation};
  }
  if (representation->IsRegularity()) {
    const int first = surface_rank(ranks, representation->Surface(), representation->Location());
    const int second = surface_rank(ranks, representation->Surface2(), representation->Location2());
    const std::array<int, 3> key{2, std::min(first, second), std::max(first, second)};
    if (first > 0 && second > 0 && second < first) {
      return {key, new BRep_CurveOn2Surfaces(representation->Surface2(), representation->Surface(),
                                             representation->Location2(),
                                             representation->Location(),
                                             representation->Continuity())};
    }
    return {key, representation};
  }
  return {{3, 0, 0}, representation};
}

// Stores every edge's curve representations (kernel spec section 3.4.8, item 5) in an order that
// depends only on the face order of `shape`: the 3D curve, then the curves on faces by the index
// of the first face on their surface and location, then the continuity records by the indices of
// their two faces, each record naming its lower face first. OCCT's offset adds them in the order
// of maps keyed by shape addresses, which changed the BRep bytes from run to run; they mean the
// same in any order. Curves on surfaces no face of `shape` lies on are leftovers of the offset's
// intermediate faces and are removed first. Only for shapes the operation built itself: it
// writes to the edges.
void order_curve_representations(const TopoDS_Shape &shape) {
  BRepTools::RemoveUnusedPCurves(shape);
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(shape, TopAbs_FACE, faces);
  // Looked up by address only; the order comes from the face indices.
  std::unordered_map<const TopoDS_TShape *, std::vector<SurfaceRank>> edge_ranks;
  for (int index = 1; index <= faces.Extent(); ++index) {
    const TopoDS_Face &face = TopoDS::Face(faces(index));
    TopLoc_Location face_location;
    const Handle(Geom_Surface) &surface = BRep_Tool::Surface(face, face_location);
    for (TopExp_Explorer edge(face, TopAbs_EDGE); edge.More(); edge.Next()) {
      const TopLoc_Location location = face_location.Predivided(edge.Current().Location());
      std::vector<SurfaceRank> &ranks = edge_ranks[edge.Current().TShape().get()];
      if (surface_rank(ranks, surface, location) == 0) {
        ranks.push_back(SurfaceRank{surface.get(), location, index});
      }
    }
  }
  TopTools_IndexedMapOfShape edges;
  TopExp::MapShapes(shape, TopAbs_EDGE, edges);
  for (int index = 1; index <= edges.Extent(); ++index) {
    const Handle(BRep_TEdge) edge = Handle(BRep_TEdge)::DownCast(edges(index).TShape());
    if (edge.IsNull()) {
      continue;
    }
    const std::vector<SurfaceRank> &ranks = edge_ranks[edge.get()];
    std::vector<std::pair<std::array<int, 3>, Handle(BRep_CurveRepresentation)>> keyed;
    for (const Handle(BRep_CurveRepresentation) &representation : edge->Curves()) {
      keyed.push_back(keyed_representation(representation, ranks));
    }
    std::stable_sort(keyed.begin(), keyed.end(), [](const auto &first, const auto &second) {
      return first.first < second.first;
    });
    BRep_ListOfCurveRepresentation &list = edge->ChangeCurves();
    list.Clear();
    for (const auto &[key, representation] : keyed) {
      list.Append(representation);
    }
  }
}

// Raises StdFail_NotDone unless hollowing left a positive volume smaller than the original.
// When the wall does not fit, the offset can return the original solid unchanged.
void require_material_removed(const TopoDS_Shape &original, const TopoDS_Shape &hollowed) {
  const double before = solids_volume(original);
  const double after = solids_volume(hollowed);
  if (!(after > 0.0 && after < before * (1.0 - 1e-9))) {
    raise_not_done("the shell did not hollow the solid");
  }
}

// A face's surface, without a rectangular trim, so points beyond the face still project.
Handle(Geom_Surface) untrimmed_surface(const TopoDS_Face &face) {
  Handle(Geom_Surface) surface = BRep_Tool::Surface(face);
  if (surface.IsNull()) {
    raise_not_done("a face of the shell's input has no surface");
  }
  while (surface->IsKind(STANDARD_TYPE(Geom_RectangularTrimmedSurface))) {
    surface = Handle(Geom_RectangularTrimmedSurface)::DownCast(surface)->BasisSurface();
  }
  return surface;
}

// Raises StdFail_NotDone unless every face of the result, whose faces `result_faces` holds,
// generated from a kept face of `copy` lies at distance `thickness` from that face's surface,
// measured at the middle of its parameter bounds (kernel spec section 3.4.8, item 4). OCCT turns
// the offset of a convex face inside out when the wall is thicker than its radius and returns a
// wrong solid.
void require_wall_thickness(const Shape &copy, const std::vector<bool> &removed,
                            const TopTools_IndexedMapOfShape &result_faces,
                            const Handle(BRepTools_History) &history, double thickness) {
  const double tolerance = 1e-4 * std::max(1.0, thickness);
  for (int index = 1; index <= copy.faces().Extent(); ++index) {
    if (removed[static_cast<std::size_t>(index)]) {
      continue;
    }
    const TopoDS_Face &kept = TopoDS::Face(copy.faces()(index));
    const Handle(Geom_Surface) surface = untrimmed_surface(kept);
    for (const TopoDS_Shape &generated : history->Generated(kept)) {
      if (generated.ShapeType() != TopAbs_FACE || !result_faces.Contains(generated)) {
        continue;
      }
      const TopoDS_Face &inner = TopoDS::Face(generated);
      double u_min = 0.0, u_max = 0.0, v_min = 0.0, v_max = 0.0;
      BRepTools::UVBounds(inner, u_min, u_max, v_min, v_max);
      const gp_Pnt point = BRepAdaptor_Surface(inner, Standard_False)
                               .Value(0.5 * (u_min + u_max), 0.5 * (v_min + v_max));
      GeomAPI_ProjectPointOnSurf projection(point, surface);
      if (projection.NbPoints() == 0 ||
          std::abs(projection.LowerDistance() - thickness) > tolerance) {
        raise_not_done("the shell's inner face generated from face " + std::to_string(index) +
                       " is not at the wall thickness from it");
      }
    }
  }
}

// The area of a face, also of one whose wires run the wrong way, as in the reversed copy OCCT's
// thick solid leaves of a removed face it could not open.
double face_area(const TopoDS_Face &face) {
  return std::abs(face_mass(face, BRepAdaptor_Surface(face, Standard_False)).area);
}

// Raises StdFail_NotDone unless each face of `copy` flagged in `removed` lost area in the result,
// whose faces `result_faces` holds: none of its images there, itself included, is as large as it
// (kernel spec section 3.4.8, item 4). Where a removed face meets kept faces tangentially, OCCT's
// thick solid keeps that face and a reversed copy of it, closing the wall instead of opening it.
void require_openings(const Shape &copy, const std::vector<bool> &removed,
                      const TopTools_IndexedMapOfShape &result_faces,
                      const Handle(BRepTools_History) &history) {
  for (int index = 1; index <= copy.faces().Extent(); ++index) {
    if (!removed[static_cast<std::size_t>(index)]) {
      continue;
    }
    const TopoDS_Face &face = TopoDS::Face(copy.faces()(index));
    const double area = face_area(face);
    TopTools_ListOfShape images = history->Modified(face);
    images.Append(face);
    for (const TopoDS_Shape &image : images) {
      if (result_faces.Contains(image) && face_area(TopoDS::Face(image)) >= area * (1.0 - 1e-9)) {
        raise_not_done("the shell did not open face " + std::to_string(index));
      }
    }
  }
}

// One way of shelling: its result and the history relating the copy to it.
struct ShellAttempt {
  TopoDS_Shape shape;
  Handle(BRepTools_History) history;
  // The algorithm named in failures.
  const char *algorithm;
};

// Checks an attempt on `copy`, whose state `inputs` recorded before the attempt (kernel spec
// section 3.4.8, item 4), and returns its result.
OpResultData checked_shell(const Shape &copy, CheckInputs &inputs, const ShellAttempt &attempt,
                           const std::vector<bool> &removed, double thickness) {
  // The attempt shares edges only with the copy.
  store_result_pcurves(inputs, attempt.shape);
  const TopoDS_Shape result = ordered_by_input(
      copy, finish_solid(attempt.shape, attempt.history, inputs, attempt.algorithm),
      attempt.history);
  require_material_removed(copy.solids()(1), result);
  TopTools_IndexedMapOfShape result_faces;
  TopExp::MapShapes(result, TopAbs_FACE, result_faces);
  require_openings(copy, removed, result_faces, attempt.history);
  require_wall_thickness(copy, removed, result_faces, attempt.history, thickness);
  // Every edge of the result belongs to the copy or was built by the attempt.
  order_curve_representations(result);
  return op_result(result, attempt.history, {copy.get()});
}

// The angle under which BRepOffset_MakeOffset treats the faces at an edge as tangent for a wall of
// `thickness` (kernel spec section 3.4.8, item 3).
double offset_angle(double thickness) {
  return 4.0 * std::asin(std::min(kLinear / (thickness / 2.0 + Precision::Confusion()), 1.0));
}

// The concavity of the edges of a solid as OCCT's offset classifies them: BRepOffset_Analyse with
// the angular tolerance of BRepOffset_MakeOffset (kernel spec section 3.4.8, items 3 and 6). Only
// reads the shape.
class EdgeConcavity {
 public:
  // The edges of `solid`, whose faces `faces` indexes for the flags the queries take.
  EdgeConcavity(const TopoDS_Shape &solid, const TopTools_IndexedMapOfShape &faces,
                double thickness)
      : faces_(faces), analyse_(solid, offset_angle(thickness)) {
    TopExp::MapShapesAndUniqueAncestors(solid, TopAbs_EDGE, TopAbs_FACE, edge_faces_);
  }

  // The edges of the solid of `shape`, flagged by its face indices.
  EdgeConcavity(const Shape &shape, double thickness)
      : EdgeConcavity(shape.solids()(1), shape.faces(), thickness) {}

  // True when an edge of the concavity `type` lies between a face flagged in `first` and another
  // face flagged in `second` (flags indexed by face index).
  bool between(const std::vector<bool> &first, const std::vector<bool> &second,
               ChFiDS_TypeOfConcavity type) const {
    bool found = false;
    for_each_edge_between(first, second, [&](const BRepOffset_ListOfInterval &intervals) {
      found = found || has_interval(intervals, type);
    });
    return found;
  }

  // True when edges lie between faces flagged in `first` and other faces flagged in `second`,
  // and each of them is of the concavity `type` along its whole length.
  bool only_between(const std::vector<bool> &first, const std::vector<bool> &second,
                    ChFiDS_TypeOfConcavity type) const {
    bool any = false;
    bool only = true;
    for_each_edge_between(first, second, [&](const BRepOffset_ListOfInterval &intervals) {
      any = true;
      for (const BRepOffset_Interval &interval : intervals) {
        only = only && interval.Type() == type;
      }
    });
    return any && only;
  }

 private:
  // True when one of `intervals` is of the concavity `type`.
  static bool has_interval(const BRepOffset_ListOfInterval &intervals,
                           ChFiDS_TypeOfConcavity type) {
    for (const BRepOffset_Interval &interval : intervals) {
      if (interval.Type() == type) {
        return true;
      }
    }
    return false;
  }

  // Calls `visit` with the concavity intervals of each edge between a face flagged in `first`
  // and another face flagged in `second`.
  template <class Visit>
  void for_each_edge_between(const std::vector<bool> &first, const std::vector<bool> &second,
                             Visit visit) const {
    for (int index = 1; index <= edge_faces_.Extent(); ++index) {
      const TopTools_ListOfShape &faces = edge_faces_(index);
      if (faces.Extent() != 2) {
        continue;
      }
      const auto one = static_cast<std::size_t>(faces_.FindIndex(faces.First()));
      const auto other = static_cast<std::size_t>(faces_.FindIndex(faces.Last()));
      if ((first[one] && second[other]) || (first[other] && second[one])) {
        visit(analyse_.Type(TopoDS::Edge(edge_faces_.FindKey(index))));
      }
    }
  }

  const TopTools_IndexedMapOfShape &faces_;
  BRepOffset_Analyse analyse_;
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces_;
};

// Flags indexed by face index (entry 0 unused): true for the faces not flagged in `removed`.
std::vector<bool> kept_flags(const std::vector<bool> &removed) {
  std::vector<bool> kept(removed.size());
  for (std::size_t position = 0; position < removed.size(); ++position) {
    kept[position] = !removed[position];
  }
  return kept;
}

// True when a removed face shares an edge with a kept face along which the solid is concave
// (kernel spec section 3.4.8, item 3). OCCT's thick solid cannot close the wall there. Only reads
// `shape`.
bool borders_concave_edge(const Shape &shape, const std::vector<bool> &removed,
                          double thickness) {
  return EdgeConcavity(shape, thickness).between(removed, kept_flags(removed), ChFiDS_Concave);
}

// The radius of `face` when it curves away from the material with a constant radius, such as the
// round of a fillet on a convex edge: a cylinder or a sphere whose outward normal points away from
// its axis or center, or a torus whose outward normal points away from the circle in the middle
// of its tube (its minor radius). Nothing for any other face.
std::optional<double> convex_radius(const TopoDS_Face &face) {
  const BRepAdaptor_Surface surface(face, Standard_False);
  const GeomAbs_SurfaceType type = surface.GetType();
  if (type != GeomAbs_Cylinder && type != GeomAbs_Sphere && type != GeomAbs_Torus) {
    return std::nullopt;
  }
  double u_min = 0.0, u_max = 0.0, v_min = 0.0, v_max = 0.0;
  BRepTools::UVBounds(face, u_min, u_max, v_min, v_max);
  gp_Pnt point;
  gp_Vec u_tangent;
  gp_Vec v_tangent;
  surface.D1(0.5 * (u_min + u_max), 0.5 * (v_min + v_max), point, u_tangent, v_tangent);
  gp_Vec outward = u_tangent.Crossed(v_tangent);
  if (face.Orientation() == TopAbs_REVERSED) {
    outward.Reverse();
  }
  // The center of the face's curvature across it, at distance `radius` from the face.
  gp_Pnt center;
  double radius = 0.0;
  switch (type) {
    case GeomAbs_Cylinder: {
      const gp_Ax1 axis = surface.Cylinder().Axis();
      const gp_Vec along(axis.Direction());
      center = axis.Location().Translated(along * gp_Vec(axis.Location(), point).Dot(along));
      radius = surface.Cylinder().Radius();
      break;
    }
    case GeomAbs_Sphere:
      center = surface.Sphere().Location();
      radius = surface.Sphere().Radius();
      break;
    case GeomAbs_Torus: {
      const gp_Torus torus = surface.Torus();
      const gp_Vec along(torus.Axis().Direction());
      const gp_Vec from_center(torus.Axis().Location(), point);
      const gp_Vec across = from_center - along * from_center.Dot(along);
      if (across.Magnitude() <= gp::Resolution()) {
        return std::nullopt;
      }
      center = torus.Axis().Location().Translated(across.Normalized() * torus.MajorRadius());
      radius = torus.MinorRadius();
      break;
    }
    default:
      return std::nullopt;
  }
  if (!(gp_Vec(center, point).Dot(outward) > 0.0)) {
    return std::nullopt;
  }
  return radius;
}

// How close a wall may come to the radius of a convex face, on either side, before OCCT's offset
// of that face fails (kernel spec section 3.4.8, item 6), measured for radii from 0.5 mm to 10 mm.
// Just under the radius, the join offset fails on closed hollows once the offset face is thinner
// than about 0.0007 mm, and merges that face's vertices on open shells. Just over it, the
// complete intersection fails while the offset radius lies within about 0.002 mm of zero. A sharp
// inner corner where a round thinner than this belongs is below what any printer resolves.
constexpr double kVanishingMargin = 0.01;

// Flags indexed by face index (entry 0 unused): true for the kept faces whose inner offset
// vanishes at `thickness` or would be thinner than kVanishingMargin, convex faces with a radius
// below the wall plus that margin (kernel spec section 3.4.8, item 6).
std::vector<bool> vanishing_flags(const Shape &copy, const std::vector<bool> &removed,
                                  double thickness) {
  std::vector<bool> vanishing(removed.size(), false);
  for (int index = 1; index <= copy.faces().Extent(); ++index) {
    const auto position = static_cast<std::size_t>(index);
    if (removed[position]) {
      continue;
    }
    const std::optional<double> radius = convex_radius(TopoDS::Face(copy.faces()(index)));
    vanishing[position] = radius && *radius - thickness < kVanishingMargin;
  }
  return vanishing;
}

// How a shell takes out its cavity past the kept faces whose inner offset vanishes (kernel spec
// section 3.4.8, item 6).
enum class VanishingPath {
  // OCCT's complete intersection drops them from the cavity of item 3 (item 6.1).
  CompleteIntersection,
  // The join offset of the solid without them, which keeps the arc joins at concave edges between
  // kept faces that the complete intersection makes sharp, and the complete intersection when it
  // fails (item 6.3).
  ArcJoins,
  // Only the solid without them works, where the complete intersection fails (item 6.2).
  WithoutThem,
};

// The path a shell takes past the faces flagged in `vanishing`: without them at a wall within
// kVanishingMargin of the radius of one of them, at a sphere among them and at a removed face that
// meets a kept face tangentially, where the complete intersection fails; with arc joins at a
// concave edge between kept faces; else the complete intersection. Only reads `shape`.
VanishingPath vanishing_path(const Shape &shape, const std::vector<bool> &removed,
                             const std::vector<bool> &vanishing, double thickness) {
  for (int index = 1; index <= shape.faces().Extent(); ++index) {
    if (!vanishing[static_cast<std::size_t>(index)]) {
      continue;
    }
    const TopoDS_Face &face = TopoDS::Face(shape.faces()(index));
    const std::optional<double> radius = convex_radius(face);
    if (BRepAdaptor_Surface(face, Standard_False).GetType() == GeomAbs_Sphere ||
        (radius && std::abs(thickness - *radius) < kVanishingMargin)) {
      return VanishingPath::WithoutThem;
    }
  }
  const EdgeConcavity concavity(shape, thickness);
  const std::vector<bool> kept = kept_flags(removed);
  if (concavity.between(removed, kept, ChFiDS_Tangential)) {
    return VanishingPath::WithoutThem;
  }
  return concavity.between(kept, kept, ChFiDS_Concave) ? VanishingPath::ArcJoins
                                                       : VanishingPath::CompleteIntersection;
}

// Which offset faces OCCT intersects when it builds a cavity: those that share an edge, or every
// pair (complete intersection), which drops the faces whose offset vanishes and extends their
// neighbors until they meet (kernel spec section 3.4.8, item 6.1).
enum class FaceIntersection { Local, Complete };

// The solid whose inner offset bounds a shell's cavity, with the history relating the solid of the
// copy the shell works on to it. Each step of an attempt merges its history into `history`, which
// then relates the copy to the attempt's result.
struct CavitySource {
  TopoDS_Shape solid;
  Handle(BRepTools_History) history;
};

// The solid of `copy` itself, with an empty history.
CavitySource solid_of(const Shape &copy) {
  return CavitySource{copy.solids()(1), new BRepTools_History};
}

// The solid of `copy` without its faces flagged in `faces`, its other faces extended until they
// meet where those were (BRepAlgoAPI_Defeaturing, kernel spec section 3.4.8, item 6.2). Raises
// StdFail_NotDone when a flagged face cannot be removed, such as the side of a cylinder, which no
// neighbor can close.
CavitySource without_faces(const Shape &copy, const std::vector<bool> &faces) {
  const TopoDS_Shape &solid = copy.solids()(1);
  BRepAlgoAPI_Defeaturing defeaturing;
  defeaturing.SetShape(solid);
  for (int index = 1; index <= copy.faces().Extent(); ++index) {
    if (faces[static_cast<std::size_t>(index)]) {
      defeaturing.AddFaceToRemove(copy.faces()(index));
    }
  }
  defeaturing.SetRunParallel(Standard_False);
  defeaturing.SetToFillHistory(Standard_True);
  defeaturing.Build();
  require_no_errors(defeaturing, "BRepAlgoAPI_Defeaturing");
  const Handle(BRepTools_History) history = history_of(solid, defeaturing);
  for (int index = 1; index <= copy.faces().Extent(); ++index) {
    if (faces[static_cast<std::size_t>(index)] && !history->IsRemoved(copy.faces()(index))) {
      raise_not_done("BRepAlgoAPI_Defeaturing could not remove face " + std::to_string(index));
    }
  }
  if (!has_solid(defeaturing.Shape())) {
    raise_not_done("BRepAlgoAPI_Defeaturing left no solid");
  }
  return CavitySource{defeaturing.Shape(), history};
}

// Appends to `images` the faces `face` became in the solid a history leads to: its modified
// faces, or the face itself when the history leaves it unchanged.
void append_images(const Handle(BRepTools_History) &history, const TopoDS_Shape &face,
                   TopTools_ListOfShape &images) {
  const TopTools_ListOfShape &modified = history->Modified(face);
  if (modified.IsEmpty()) {
    images.Append(face);
  }
  for (const TopoDS_Shape &image : modified) {
    images.Append(image);
  }
}

// The faces of `source` that the faces of `copy` flagged in `removed` became.
TopTools_ListOfShape removed_images(const Shape &copy, const std::vector<bool> &removed,
                                    const CavitySource &source) {
  TopTools_ListOfShape images;
  for (int index = 1; index <= copy.faces().Extent(); ++index) {
    if (removed[static_cast<std::size_t>(index)]) {
      append_images(source.history, copy.faces()(index), images);
    }
  }
  return images;
}

// True when the faces `openings` of the solid of `source` meet its other faces at edges, and only
// tangentially (kernel spec section 3.4.8, item 7), where OCCT's thick solid keeps those faces and
// a reversed copy of each instead of opening them. Only reads that solid.
bool opens_tangentially(const CavitySource &source, const TopTools_ListOfShape &openings,
                        double thickness) {
  TopTools_IndexedMapOfShape faces;
  TopExp::MapShapes(source.solid, TopAbs_FACE, faces);
  std::vector<bool> opening(static_cast<std::size_t>(faces.Extent()) + 1, false);
  for (const TopoDS_Shape &face : openings) {
    opening[static_cast<std::size_t>(faces.FindIndex(face))] = true;
  }
  return EdgeConcavity(source.solid, faces, thickness)
      .only_between(opening, kept_flags(opening), ChFiDS_Tangential);
}

// The inner offset of the solid of `source` by `thickness` with arc joins, the shell bounding the
// cavity of a closed hollow (kernel spec section 3.4.8, items 1, 6.2 and 7). OCCT offsets a closed
// solid into the inner solid alone. Merges the offset's history into the source's.
TopoDS_Shape inner_offset(const CavitySource &source, double thickness) {
  BRepOffsetAPI_MakeOffsetShape offset;
  offset.PerformByJoin(source.solid, -thickness, kLinear, BRepOffset_Skin, Standard_False,
                       Standard_False, GeomAbs_Arc);
  if (!offset.IsDone()) {
    raise_not_done(std::string("BRepOffsetAPI_MakeOffsetShape failed: ") +
                   offset_error_name(offset.MakeOffset().Error()));
  }
  merge_history(source.history, history_of(source.solid, offset));
  return offset.Shape();
}

// A closed hollow (kernel spec section 3.4.8, items 1 and 6.2): the solid of `copy` with the
// reversed inner shell of the inner offset of `source`, which then faces the cavity.
ShellAttempt closed_hollow(const Shape &copy, const CavitySource &source, double thickness) {
  return ShellAttempt{hollow_solid(copy.solids()(1), inner_offset(source, thickness)),
                      source.history, "BRepOffsetAPI_MakeOffsetShape"};
}

// OCCT's thick solid of `source` with arc joins and the faces of `openings` as closing faces
// (kernel spec section 3.4.8, item 2). Merges its history into the source's.
TopoDS_Shape thick_solid(const CavitySource &source, const TopTools_ListOfShape &openings,
                         double thickness) {
  BRepOffsetAPI_MakeThickSolid thick;
  thick.MakeThickSolidByJoin(source.solid, openings, -thickness, kLinear, BRepOffset_Skin,
                             Standard_False, Standard_False, GeomAbs_Arc);
  if (!thick.IsDone()) {
    raise_not_done(std::string("BRepOffsetAPI_MakeThickSolid failed: ") +
                   offset_error_name(thick.MakeOffset().Error()));
  }
  merge_history(source.history, history_of(source.solid, thick));
  return thick.Shape();
}

// The solid of `copy` minus `cavity`, by BRepAlgoAPI_Cut with the settings of kernel spec section
// 3.4.5 and without unification, which may hold several solids when the opening cuts a part of the
// body loose (section 3.1, item 7). Merges the cut's history into `history`.
ShellAttempt cut_cavity(const Shape &copy, const TopoDS_Shape &cavity,
                        const Handle(BRepTools_History) &history) {
  TopTools_ListOfShape tools;
  tools.Append(cavity);
  return ShellAttempt{run_boolean(BOPAlgo_CUT, copy.solids()(1), tools, false, history), history,
                      "BRepAlgoAPI_Cut"};
}

// The outward normal of a planar face. Raises StdFail_NotDone for any other face: a shell opens a
// face that meets kept faces tangentially only when it is planar (kernel spec section 3.4.8,
// item 7).
gp_Dir planar_normal(const TopoDS_Face &face) {
  const BRepAdaptor_Surface surface(face, Standard_False);
  if (surface.GetType() != GeomAbs_Plane) {
    raise_not_done("the shell cannot open a curved face that meets kept faces tangentially");
  }
  const gp_Pln plane = surface.Plane();
  gp_Dir normal = plane.Axis().Direction();
  // The surface normal of a plane with an indirect frame points against its axis.
  if (!plane.Position().Direct()) {
    normal.Reverse();
  }
  if (face.Orientation() == TopAbs_REVERSED) {
    normal.Reverse();
  }
  return normal;
}

// The face other than `face` in `faces`, the faces of an edge, or a null shape when there is none.
TopoDS_Shape other_face(const TopTools_ListOfShape &faces, const TopoDS_Shape &face) {
  for (const TopoDS_Shape &candidate : faces) {
    if (!candidate.IsSame(face)) {
      return candidate;
    }
  }
  return TopoDS_Shape();
}

// The input elements that generate the faces ending the wall of a tangent opening (kernel spec
// section 3.4.9, item 3): the edge of `copy` between the removed face and the kept face whose inner
// face borders the wall end's edge in the inner offset, or the removed face itself when they share
// no single edge.
class WallEndOrigins {
 public:
  WallEndOrigins(const Shape &copy, const std::vector<bool> &removed, const TopoDS_Shape &inner,
                 const Handle(BRepTools_History) &history)
      : copy_(copy), removed_(removed), history_(history) {
    TopExp::MapShapesAndUniqueAncestors(inner, TopAbs_EDGE, TopAbs_FACE, inner_edge_faces_);
    TopExp::MapShapesAndUniqueAncestors(copy.solids()(1), TopAbs_EDGE, TopAbs_FACE,
                                        copy_edge_faces_);
  }

  // The element that generates the wall end over `edge` of `inner_face`, the face the removed face
  // `face` generated in the inner offset.
  TopoDS_Shape of(const TopoDS_Shape &face, const TopoDS_Shape &inner_face,
                  const TopoDS_Shape &edge) const {
    const TopoDS_Shape kept =
        kept_origin(other_face(inner_edge_faces_.FindFromKey(edge), inner_face));
    const TopoDS_Shape along = kept.IsNull() ? TopoDS_Shape() : shared_edge(face, kept);
    return along.IsNull() ? face : along;
  }

 private:
  // The kept face of the copy that generated `inner_face`, or a null shape.
  TopoDS_Shape kept_origin(const TopoDS_Shape &inner_face) const {
    for (int index = 1; index <= copy_.faces().Extent(); ++index) {
      if (removed_[static_cast<std::size_t>(index)]) {
        continue;
      }
      for (const TopoDS_Shape &generated : history_->Generated(copy_.faces()(index))) {
        if (generated.IsSame(inner_face)) {
          return copy_.faces()(index);
        }
      }
    }
    return TopoDS_Shape();
  }

  // The single edge of the copy that faces `first` and `second` share, or a null shape when they
  // share none or several.
  TopoDS_Shape shared_edge(const TopoDS_Shape &first, const TopoDS_Shape &second) const {
    TopTools_IndexedMapOfShape shared;
    for (TopExp_Explorer edge(first, TopAbs_EDGE); edge.More(); edge.Next()) {
      if (other_face(copy_edge_faces_.FindFromKey(edge.Current()), first).IsSame(second)) {
        shared.Add(edge.Current());
      }
    }
    return shared.Extent() == 1 ? shared(1) : TopoDS_Shape();
  }

  const Shape &copy_;
  const std::vector<bool> &removed_;
  Handle(BRepTools_History) history_;
  TopTools_IndexedDataMapOfShapeListOfShape inner_edge_faces_;
  TopTools_IndexedDataMapOfShapeListOfShape copy_edge_faces_;
};

// An open shell whose removed faces meet the kept faces only tangentially (kernel spec section
// 3.4.8, item 7): the closed hollow of `copy` with the inner offset of `source` (item 1 or 6.2)
// minus, for each removed face, the prism that sweeps each face it generated in that offset back
// to it along its normal. The wall then ends in a face normal to the removed face along each edge
// where it met a kept face: the prism side over each edge of the inner face, generated by the input
// element `WallEndOrigins` names.
ShellAttempt tangent_opening_shell(const Shape &copy, const std::vector<bool> &removed,
                                   const CavitySource &source, double thickness) {
  const TopoDS_Shape inner = inner_offset(source, thickness);
  const WallEndOrigins origins(copy, removed, inner, source.history);
  TopTools_ListOfShape openings;
  for (int index = 1; index <= copy.faces().Extent(); ++index) {
    if (!removed[static_cast<std::size_t>(index)]) {
      continue;
    }
    const TopoDS_Shape &face = copy.faces()(index);
    const gp_Vec sweep = gp_Vec(planar_normal(TopoDS::Face(face))) * thickness;
    // A copy: recording the wall ends adds to the history's maps.
    const TopTools_ListOfShape generated = source.history->Generated(face);
    for (const TopoDS_Shape &inner_face : generated) {
      if (inner_face.ShapeType() != TopAbs_FACE) {
        continue;
      }
      BRepPrimAPI_MakePrism prism(inner_face, sweep);
      if (!prism.IsDone()) {
        raise_not_done("BRepPrimAPI_MakePrism could not sweep an opening");
      }
      for (TopExp_Explorer edge(inner_face, TopAbs_EDGE); edge.More(); edge.Next()) {
        const TopoDS_Shape origin = origins.of(face, inner_face, edge.Current());
        for (const TopoDS_Shape &end : prism.Generated(edge.Current())) {
          source.history->AddGenerated(origin, end);
        }
      }
      openings.Append(prism.Shape());
    }
  }
  return ShellAttempt{run_boolean(BOPAlgo_CUT, hollow_solid(copy.solids()(1), inner), openings,
                                  false, source.history),
                      source.history, "BRepAlgoAPI_Cut"};
}

// A shell by the join offset of `source` with arc joins (kernel spec section 3.4.8, items 1, 2,
// 6.2 and 7): a closed hollow without removed faces; the closed hollow's cavity opened by prisms
// where the removed faces meet the kept faces only tangentially; else OCCT's thick solid, which is
// the shell itself when `source` is the solid of `copy`. When `source` is that solid without its
// vanishing faces, the thick solid cut from the source solid leaves the cavity, which is then cut
// from the solid of `copy` (`cut_cavity`), so concave edges keep their arc joins.
ShellAttempt join_offset_shell(const Shape &copy, const std::vector<bool> &removed,
                               const CavitySource &source, double thickness) {
  const TopTools_ListOfShape openings = removed_images(copy, removed, source);
  if (openings.IsEmpty()) {
    return closed_hollow(copy, source, thickness);
  }
  if (opens_tangentially(source, openings, thickness)) {
    return tangent_opening_shell(copy, removed, source, thickness);
  }
  TopTools_ListOfShape wall;
  wall.Append(thick_solid(source, openings, thickness));
  if (source.solid.IsSame(copy.solids()(1))) {
    return ShellAttempt{wall.First(), source.history, "BRepOffsetAPI_MakeThickSolid"};
  }
  return cut_cavity(copy, run_boolean(BOPAlgo_CUT, source.solid, wall, false, source.history),
                    source.history);
}

// The cavity of a shell: the inner offset of `source` by `thickness`, with the faces `openings`
// staying in place and the offsets of adjacent faces extended until they meet (join
// GeomAbs_Intersection), as one valid solid with positive volume. The complete intersection
// returns the cavity as a closed shell, which becomes that solid. Merges the offset's history
// into `history`.
TopoDS_Shape offset_cavity(const TopoDS_Shape &source, const TopTools_ListOfShape &openings,
                           double thickness, FaceIntersection intersection,
                           const Handle(BRepTools_History) &history) {
  BRepOffset_MakeOffset offset;
  offset.Initialize(source, -thickness, kLinear, BRepOffset_Skin,
                    intersection == FaceIntersection::Complete, Standard_False,
                    GeomAbs_Intersection, Standard_False, Standard_False);
  for (const TopoDS_Shape &opening : openings) {
    offset.SetOffsetOnFace(TopoDS::Face(opening), 0.0);
  }
  offset.MakeOffsetShape();
  if (!offset.IsDone()) {
    raise_not_done(std::string("BRepOffset_MakeOffset failed to build the cavity: ") +
                   offset_error_name(offset.Error()));
  }
  TopTools_IndexedMapOfShape shells;
  TopExp::MapShapes(offset.Shape(), TopAbs_SHELL, shells);
  TopoDS_Solid cavity;
  if (shells.Extent() == 1) {
    const BRep_Builder builder;
    builder.MakeSolid(cavity);
    builder.Add(cavity, shells(1));
  }
  if (shells.Extent() != 1 || !BRepLib::OrientClosedSolid(cavity) ||
      !BRepCheck_Analyzer(cavity).IsValid() || !(solids_volume(cavity) > 0.0)) {
    raise_not_done("BRepOffset_MakeOffset did not build one valid cavity solid");
  }
  merge_history(history, history_of(source, offset));
  return cavity;
}

// A shell made by taking a cavity out of the solid of `copy` (kernel spec section 3.4.8, items 3
// and 6). The cavity is the offset of `source` with the removed faces staying in place
// (`offset_cavity`). A closed hollow is the solid's shells and the cavity's reversed shell; an
// open shell is the solid minus the cavity (`cut_cavity`). An input element's `generated` list
// holds the images in the result of the cavity elements made from it (section 3.4.9).
ShellAttempt cavity_shell(const Shape &copy, const std::vector<bool> &removed,
                          const CavitySource &source, double thickness,
                          FaceIntersection intersection) {
  const TopTools_ListOfShape openings = removed_images(copy, removed, source);
  const TopoDS_Shape cavity =
      offset_cavity(source.solid, openings, thickness, intersection, source.history);
  if (openings.IsEmpty()) {
    return ShellAttempt{hollow_solid(copy.solids()(1), cavity), source.history,
                        "BRepOffset_MakeOffset"};
  }
  return cut_cavity(copy, cavity, source.history);
}

// Checks the attempt `first` makes on `copy`, whose state `inputs` recorded before it. When that
// fails and `fallback_applies()`, checks the attempt `fallback` makes on a fresh copy of `shape`,
// independent of what the first attempt wrote to its own; when the fallback fails as well, the
// call reports the first failure (kernel spec section 3.4.8, items 3 and 6).
template <class First, class Applies, class Fallback>
OpResultData checked_with_fallback(const Shape &shape, const Shape &copy, CheckInputs &inputs,
                                   const std::vector<bool> &removed, double thickness,
                                   First first, Applies fallback_applies, Fallback fallback) {
  try {
    return checked_shell(copy, inputs, first(copy), removed, thickness);
  } catch (const Standard_Failure &) {
    const std::exception_ptr first_failure = std::current_exception();
    try {
      if (fallback_applies()) {
        const Shape fresh = copy_unlocated(shape);
        CheckInputs fresh_inputs({fresh.get()});
        return checked_shell(fresh, fresh_inputs, fallback(fresh), removed, thickness);
      }
    } catch (const Standard_Failure &) {
      // When the fallback fails too, the call reports the first attempt's failure.
    }
    std::rethrow_exception(first_failure);
  }
}

// Flags indexed by face index (entry 0 unused): true for the faces to remove.
std::vector<bool> removed_flags(const Shape &shape, rust::Slice<const std::uint32_t> faces) {
  std::vector<bool> removed(static_cast<std::size_t>(shape.faces().Extent()) + 1, false);
  for (std::uint32_t face : faces) {
    removed.at(face) = true;
  }
  return removed;
}

}  // namespace

OpResultData fillet(const Shape &shape, rust::Slice<const FilletEdgeData> edges) {
  std::vector<BlendRequest> requests;
  for (const FilletEdgeData &edge : edges) {
    requests.push_back(BlendRequest{edge.edge, edge.radius, edge.radius, false, 0});
  }
  // BRepFilletAPI_MakeFillet builds rational fillet surfaces (ChFi3d_Rational) by default.
  return run_blend<BRepFilletAPI_MakeFillet>(
      shape, requests, "BRepFilletAPI_MakeFillet",
      [](BRepFilletAPI_MakeFillet &fillet, const Shape &copy, const BlendRequest &request) {
        fillet.Add(request.size, request_edge(copy, request));
      },
      fillet_failure);
}

OpResultData chamfer(const Shape &shape, rust::Slice<const ChamferEdgeData> edges) {
  std::vector<BlendRequest> requests;
  for (const ChamferEdgeData &edge : edges) {
    requests.push_back(BlendRequest{edge.edge, edge.distance, edge.other_distance,
                                    edge.two_distances, edge.face});
  }
  return run_blend<BRepFilletAPI_MakeChamfer>(
      shape, requests, "BRepFilletAPI_MakeChamfer",
      [](BRepFilletAPI_MakeChamfer &chamfer, const Shape &copy, const BlendRequest &request) {
        if (request.on_face) {
          chamfer.Add(request.size, request.other_size, request_edge(copy, request),
                      TopoDS::Face(copy.faces()(static_cast<int>(request.face))));
        } else {
          chamfer.Add(request.size, request_edge(copy, request));
        }
      },
      [](const BRepFilletAPI_MakeChamfer &) {
        // BRepFilletAPI_MakeChamfer does not expose the status of its contours.
        return std::string("BRepFilletAPI_MakeChamfer could not build the chamfer");
      });
}

OpResultData shell(const Shape &shape, rust::Slice<const std::uint32_t> remove_faces,
                   double thickness) {
  // Every attempt runs on a copy so it never writes to the input (kernel spec section 3.1). The
  // copy has no location, since the offset fails on located blend surfaces (section 3.4.8).
  const Shape copy = copy_unlocated(shape);
  CheckInputs inputs({copy.get()});
  const std::vector<bool> removed = removed_flags(copy, remove_faces);
  const std::vector<bool> vanishing = vanishing_flags(copy, removed, thickness);
  const bool any_vanishing = std::find(vanishing.begin(), vanishing.end(), true) != vanishing.end();
  // OCCT's join offset cannot take a convex face to radius zero, below it or close to it (section
  // 3.4.8, item 6). BRepOffset_Analyse only reads, so it runs on the input.
  if (any_vanishing) {
    switch (vanishing_path(shape, removed, vanishing, thickness)) {
      case VanishingPath::CompleteIntersection:
        return checked_shell(
            copy, inputs,
            cavity_shell(copy, removed, solid_of(copy), thickness, FaceIntersection::Complete),
            removed, thickness);
      case VanishingPath::ArcJoins:
        return checked_with_fallback(
            shape, copy, inputs, removed, thickness,
            [&](const Shape &on) {
              return join_offset_shell(on, removed, without_faces(on, vanishing), thickness);
            },
            [] { return true; },
            [&](const Shape &on) {
              return cavity_shell(on, removed, solid_of(on), thickness,
                                  FaceIntersection::Complete);
            });
      case VanishingPath::WithoutThem:
        break;
    }
  }
  // Items 1 to 3, on the solid without its vanishing faces when it has any (item 6.2).
  const auto source = [&](const Shape &on) {
    return any_vanishing ? without_faces(on, vanishing) : solid_of(on);
  };
  return checked_with_fallback(
      shape, copy, inputs, removed, thickness,
      [&](const Shape &on) { return join_offset_shell(on, removed, source(on), thickness); },
      [&] { return !remove_faces.empty() && borders_concave_edge(shape, removed, thickness); },
      [&](const Shape &on) {
        return cavity_shell(on, removed, source(on), thickness, FaceIntersection::Local);
      });
}

}  // namespace occt
}  // namespace partsmith
