// Fillet, chamfer and shell with history (kernel spec sections 3.4.6 to 3.4.9).
#include <algorithm>
#include <array>
#include <cmath>
#include <exception>
#include <map>
#include <unordered_map>
#include <utility>
#include <vector>

#include <BRepAdaptor_Surface.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffset_Analyse.hxx>
#include <BRepOffset_Interval.hxx>
#include <BRepOffset_MakeOffset.hxx>
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
#include <TopoDS_Compound.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>

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

// Finishes a blend: history from the algorithm and a normalized, checked result. The blend
// ran on `copy`, whose element indices are the input's and whose state `inputs` recorded before
// the blend. OCCT lists fillet and chamfer faces as generated by their edges, and corner faces as
// generated by their vertices (kernel spec section 7, open issue 4, verified by the blend tests).
template <class Blend>
OpResultData blend_result(Blend &blend, const Shape &copy, CheckInputs &inputs,
                          const char *algorithm) {
  TopTools_ListOfShape arguments;
  arguments.Append(copy.get());
  const Handle(BRepTools_History) history = new BRepTools_History(arguments, blend);
  // The result shares edges only with the copy, so the shim may store curves on planes in it.
  inputs.settle();
  store_plane_pcurves(blend.Shape());
  return op_result(finish_solid(blend.Shape(), history, inputs, algorithm), history,
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

  BRep_Builder builder;
  std::vector<Ranked<TopoDS_Solid>> solids;
  for (TopExp_Explorer solid(result, TopAbs_SOLID); solid.More(); solid.Next()) {
    std::vector<Ranked<TopoDS_Shell>> shells;
    for (TopExp_Explorer shell(solid.Current(), TopAbs_SHELL); shell.More(); shell.Next()) {
      // Each face keeps its orientation as explored, so the rebuilt shell bounds the same side.
      std::vector<Ranked<TopoDS_Shape>> faces;
      for (TopExp_Explorer face(shell.Current(), TopAbs_FACE); face.More(); face.Next()) {
        faces.emplace_back(rank_of(face.Current()), face.Current());
      }
      sort_by_rank(faces);
      TopoDS_Shell ordered;
      builder.MakeShell(ordered);
      for (const Ranked<TopoDS_Shape> &face : faces) {
        builder.Add(ordered, face.second);
      }
      ordered.Closed(shell.Current().Closed());
      shells.emplace_back(faces.empty() ? unranked : faces.front().first, ordered);
    }
    sort_by_rank(shells);
    TopoDS_Solid ordered;
    builder.MakeSolid(ordered);
    for (const Ranked<TopoDS_Shell> &shell : shells) {
      builder.Add(ordered, shell.second);
    }
    ordered.Closed(solid.Current().Closed());
    solids.emplace_back(shells.empty() ? unranked : shells.front().first, ordered);
  }
  sort_by_rank(solids);
  if (solids.size() == 1) {
    return solids.front().second;
  }
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  for (const Ranked<TopoDS_Solid> &solid : solids) {
    builder.Add(compound, solid.second);
  }
  return compound;
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

// The sort key of one curve representation: its class (the 3D curve and polygon first, then the
// curves on faces, then the continuity records, then anything else) and the ranks of its faces.
std::array<int, 3> representation_key(const Handle(BRep_CurveRepresentation) &representation,
                                      const std::vector<SurfaceRank> &ranks) {
  if (representation->IsCurve3D() || representation->IsPolygon3D()) {
    return {0, 0, 0};
  }
  if (representation->IsCurveOnSurface() || representation->IsPolygonOnSurface()) {
    return {1, surface_rank(ranks, representation->Surface(), representation->Location()), 0};
  }
  if (representation->IsRegularity()) {
    const int first = surface_rank(ranks, representation->Surface(), representation->Location());
    const int second = surface_rank(ranks, representation->Surface2(), representation->Location2());
    return {2, std::min(first, second), std::max(first, second)};
  }
  return {3, 0, 0};
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
      Handle(BRep_CurveRepresentation) stored = representation;
      const int first =
          representation->IsRegularity()
              ? surface_rank(ranks, representation->Surface(), representation->Location())
              : 0;
      const int second =
          representation->IsRegularity()
              ? surface_rank(ranks, representation->Surface2(), representation->Location2())
              : 0;
      if (first > 0 && second > 0 && second < first) {
        stored = new BRep_CurveOn2Surfaces(representation->Surface2(), representation->Surface(),
                                           representation->Location2(),
                                           representation->Location(),
                                           representation->Continuity());
      }
      keyed.emplace_back(representation_key(stored, ranks), stored);
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

// Raises StdFail_NotDone unless every face of `result` generated from a kept face of `copy`
// lies at distance `thickness` from that face's surface, measured at the middle of its
// parameter bounds (kernel spec section 3.4.8, item 4). OCCT turns the offset of a convex face
// inside out when the wall is thicker than its radius and returns a wrong solid.
void require_wall_thickness(const Shape &copy, const std::vector<bool> &removed,
                            const TopoDS_Shape &result,
                            const Handle(BRepTools_History) &history, double thickness) {
  TopTools_IndexedMapOfShape result_faces;
  TopExp::MapShapes(result, TopAbs_FACE, result_faces);
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
  // The attempt shares edges only with the copy, so the shim may store curves on planes in it.
  inputs.settle();
  store_plane_pcurves(attempt.shape);
  const TopoDS_Shape result = ordered_by_input(
      copy, finish_solid(attempt.shape, attempt.history, inputs, attempt.algorithm),
      attempt.history);
  require_material_removed(copy.solids()(1), result);
  require_wall_thickness(copy, removed, result, attempt.history, thickness);
  // Every edge of the result belongs to the copy or was built by the attempt.
  order_curve_representations(result);
  return op_result(result, attempt.history, {copy.get()});
}

// A closed hollow (kernel spec section 3.4.8, item 1). OCCT offsets a closed solid into the
// inner solid alone; the hollow solid is the input's shell plus the reversed inner shell,
// which then faces the cavity.
ShellAttempt closed_hollow(const Shape &copy, double thickness) {
  const TopoDS_Shape &solid = copy.solids()(1);
  BRepOffsetAPI_MakeOffsetShape offset;
  offset.PerformByJoin(solid, -thickness, kLinear, BRepOffset_Skin, Standard_False,
                       Standard_False, GeomAbs_Arc);
  if (!offset.IsDone()) {
    raise_not_done(std::string("BRepOffsetAPI_MakeOffsetShape failed: ") +
                   offset_error_name(offset.MakeOffset().Error()));
  }
  TopTools_ListOfShape arguments;
  arguments.Append(solid);
  const Handle(BRepTools_History) history = new BRepTools_History(arguments, offset);
  return ShellAttempt{hollow_solid(solid, offset.Shape()), history,
                      "BRepOffsetAPI_MakeOffsetShape"};
}

// An open shell by OCCT's thick solid (kernel spec section 3.4.8, item 2).
ShellAttempt thick_solid(const Shape &copy, const std::vector<bool> &removed, double thickness) {
  const TopoDS_Shape &solid = copy.solids()(1);
  TopTools_ListOfShape closing_faces;
  for (int index = 1; index <= copy.faces().Extent(); ++index) {
    if (removed[static_cast<std::size_t>(index)]) {
      closing_faces.Append(copy.faces()(index));
    }
  }
  BRepOffsetAPI_MakeThickSolid thick;
  thick.MakeThickSolidByJoin(solid, closing_faces, -thickness, kLinear, BRepOffset_Skin,
                             Standard_False, Standard_False, GeomAbs_Arc);
  if (!thick.IsDone()) {
    raise_not_done(std::string("BRepOffsetAPI_MakeThickSolid failed: ") +
                   offset_error_name(thick.MakeOffset().Error()));
  }
  TopTools_ListOfShape arguments;
  arguments.Append(solid);
  const Handle(BRepTools_History) history = new BRepTools_History(arguments, thick);
  return ShellAttempt{thick.Shape(), history, "BRepOffsetAPI_MakeThickSolid"};
}

// True when a removed face shares an edge with a kept face along which the solid is concave,
// classified with the angular tolerance of BRepOffset_MakeOffset (kernel spec section 3.4.8,
// item 3). OCCT's thick solid cannot close the wall there. Only reads `shape`.
bool borders_concave_edge(const Shape &shape, const std::vector<bool> &removed,
                          double thickness) {
  const TopoDS_Shape &solid = shape.solids()(1);
  const double angle =
      4.0 * std::asin(std::min(kLinear / (thickness / 2.0 + Precision::Confusion()), 1.0));
  const BRepOffset_Analyse analyse(solid, angle);
  TopTools_IndexedDataMapOfShapeListOfShape edge_faces;
  TopExp::MapShapesAndUniqueAncestors(solid, TopAbs_EDGE, TopAbs_FACE, edge_faces);
  for (int index = 1; index <= edge_faces.Extent(); ++index) {
    bool beside_removed = false;
    bool beside_kept = false;
    for (const TopoDS_Shape &face : edge_faces(index)) {
      const bool is_removed = removed[static_cast<std::size_t>(shape.faces().FindIndex(face))];
      beside_removed = beside_removed || is_removed;
      beside_kept = beside_kept || !is_removed;
    }
    if (!beside_removed || !beside_kept) {
      continue;
    }
    for (const BRepOffset_Interval &interval :
         analyse.Type(TopoDS::Edge(edge_faces.FindKey(index)))) {
      if (interval.Type() == ChFiDS_Concave) {
        return true;
      }
    }
  }
  return false;
}

// An open shell by cutting a cavity out of the solid (kernel spec section 3.4.8, item 3): the
// offset of the solid with the kept faces moved inward and the removed faces staying in place,
// extended until they meet, then the solid minus that cavity.
ShellAttempt cavity_cut(const Shape &copy, const std::vector<bool> &removed, double thickness) {
  const TopoDS_Shape &solid = copy.solids()(1);
  BRepOffset_MakeOffset offset;
  offset.Initialize(solid, -thickness, kLinear, BRepOffset_Skin, Standard_False, Standard_False,
                    GeomAbs_Intersection, Standard_False, Standard_False);
  for (int index = 1; index <= copy.faces().Extent(); ++index) {
    if (removed[static_cast<std::size_t>(index)]) {
      offset.SetOffsetOnFace(TopoDS::Face(copy.faces()(index)), 0.0);
    }
  }
  offset.MakeOffsetShape();
  if (!offset.IsDone()) {
    raise_not_done(std::string("BRepOffset_MakeOffset failed to build the cavity: ") +
                   offset_error_name(offset.Error()));
  }
  TopTools_IndexedMapOfShape cavities;
  TopExp::MapShapes(offset.Shape(), TopAbs_SOLID, cavities);
  if (cavities.Extent() != 1 || !BRepCheck_Analyzer(cavities(1)).IsValid() ||
      !(solids_volume(cavities(1)) > 0.0)) {
    raise_not_done("BRepOffset_MakeOffset did not build one valid cavity solid");
  }
  TopTools_ListOfShape arguments;
  arguments.Append(solid);
  // The cavity's elements are generated from the input's; the cut maps them into the result.
  const Handle(BRepTools_History) history = new BRepTools_History(arguments, offset);
  TopTools_ListOfShape tools;
  tools.Append(cavities(1));
  const TopoDS_Shape result = run_boolean(BOPAlgo_CUT, solid, tools, false, history);
  return ShellAttempt{result, history, "BRepAlgoAPI_Cut"};
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
  check_requests(shape, requests);
  // The fillet runs on a copy so it never writes to the input (kernel spec section 3.1).
  const Shape copy = copy_shape(shape);
  CheckInputs inputs({copy.get()});
  BRepFilletAPI_MakeFillet fillet(copy.get(), ChFi3d_Rational);
  for (const BlendRequest &request : requests) {
    fillet.Add(request.size, request_edge(copy, request));
  }
  check_contours(fillet, copy, requests, "BRepFilletAPI_MakeFillet");
  fillet.Build();
  if (!fillet.IsDone()) {
    raise_not_done(fillet_failure(fillet));
  }
  return blend_result(fillet, copy, inputs, "BRepFilletAPI_MakeFillet");
}

OpResultData chamfer(const Shape &shape, rust::Slice<const ChamferEdgeData> edges) {
  std::vector<BlendRequest> requests;
  for (const ChamferEdgeData &edge : edges) {
    requests.push_back(BlendRequest{edge.edge, edge.distance, edge.other_distance,
                                    edge.two_distances, edge.face});
  }
  check_requests(shape, requests);
  // The chamfer runs on a copy so it never writes to the input (kernel spec section 3.1).
  const Shape copy = copy_shape(shape);
  CheckInputs inputs({copy.get()});
  BRepFilletAPI_MakeChamfer chamfer(copy.get());
  for (const BlendRequest &request : requests) {
    if (request.on_face) {
      chamfer.Add(request.size, request.other_size, request_edge(copy, request),
                  TopoDS::Face(copy.faces()(static_cast<int>(request.face))));
    } else {
      chamfer.Add(request.size, request_edge(copy, request));
    }
  }
  check_contours(chamfer, copy, requests, "BRepFilletAPI_MakeChamfer");
  chamfer.Build();
  if (!chamfer.IsDone()) {
    // BRepFilletAPI_MakeChamfer does not expose the status of its contours.
    raise_not_done("BRepFilletAPI_MakeChamfer could not build the chamfer");
  }
  return blend_result(chamfer, copy, inputs, "BRepFilletAPI_MakeChamfer");
}

OpResultData shell(const Shape &shape, rust::Slice<const std::uint32_t> remove_faces,
                   double thickness) {
  // Every attempt runs on a copy so it never writes to the input (kernel spec section 3.1). The
  // copy has no location, since the offset fails on located blend surfaces (section 3.4.8).
  const Shape copy = copy_unlocated(shape);
  CheckInputs inputs({copy.get()});
  const std::vector<bool> removed = removed_flags(copy, remove_faces);
  if (remove_faces.empty()) {
    const ShellAttempt attempt = closed_hollow(copy, thickness);
    return checked_shell(copy, inputs, attempt, removed, thickness);
  }
  try {
    const ShellAttempt attempt = thick_solid(copy, removed, thickness);
    return checked_shell(copy, inputs, attempt, removed, thickness);
  } catch (const Standard_Failure &) {
    const std::exception_ptr first_failure = std::current_exception();
    try {
      // BRepOffset_Analyse only reads, so the input is checked before anything is copied.
      if (borders_concave_edge(shape, removed, thickness)) {
        // A fresh copy, independent of what the first attempt wrote to its own.
        const Shape fresh = copy_unlocated(shape);
        CheckInputs fresh_inputs({fresh.get()});
        const ShellAttempt attempt = cavity_cut(fresh, removed, thickness);
        return checked_shell(fresh, fresh_inputs, attempt, removed, thickness);
      }
    } catch (const Standard_Failure &) {
      // When the fallback fails too, the call reports the first attempt's failure.
    }
    std::rethrow_exception(first_failure);
  }
}

}  // namespace occt
}  // namespace partsmith
