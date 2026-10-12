// Planar faces from chained, oriented sketch curves (kernel spec section 3.4.2).
#include <algorithm>
#include <vector>

#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRep_Builder.hxx>
#include <ElCLib.hxx>
#include <ElSLib.hxx>
#include <Geom_Circle.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Vertex.hxx>
#include <TopoDS_Wire.hxx>
#include <gp_Circ.hxx>
#include <gp_Pln.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

namespace {

const char *const kInvalidRegion =
    "The profile region crosses itself or has a hole outside its outer boundary.";

// Absolute slack added to computed vertex tolerances: far above the rounding of curve
// evaluation at part scale and far below the modeling tolerance.
constexpr double kEvaluationMargin = 1e-9;

// The circle of an arc or circle curve, in the plane's frame.
gp_Circ circle_of(const gp_Ax3 &axes, const ProfileCurveData &curve) {
  const gp_Pnt center = ElSLib::PlaneValue(curve.center_x, curve.center_y, axes);
  return gp_Circ(gp_Ax2(center, axes.Direction(), axes.XDirection()), curve.radius);
}

// The parameter range of the counterclockwise arc of `circle` from `from` to `to`.
std::pair<double, double> arc_range(const gp_Circ &circle, const gp_Pnt &from, const gp_Pnt &to) {
  const double start = ElCLib::Parameter(circle, from);
  double end = ElCLib::Parameter(circle, to);
  if (end <= start) {
    end += 2.0 * M_PI;
  }
  return {start, end};
}

// Builds the edge of one curve between its junction vertices, oriented along the chain.
TopoDS_Edge make_edge(const gp_Ax3 &axes, const ProfileCurveData &curve, const TopoDS_Vertex &start,
                      const TopoDS_Vertex &end, const gp_Pnt &start_point,
                      const gp_Pnt &end_point) {
  if (curve.kind == CurveType::Line) {
    BRepBuilderAPI_MakeEdge maker(start, end);
    if (!maker.IsDone()) {
      raise_not_done("BRepBuilderAPI_MakeEdge failed for a line with error " +
                     std::to_string(maker.Error()));
    }
    return maker.Edge();
  }
  const gp_Circ circle = circle_of(axes, curve);
  const Handle(Geom_Circle) geometry = new Geom_Circle(circle);
  // The edge always runs counterclockwise; a clockwise curve uses it reversed.
  const TopoDS_Vertex &first = curve.clockwise ? end : start;
  const TopoDS_Vertex &last = curve.clockwise ? start : end;
  std::pair<double, double> range{0.0, 2.0 * M_PI};
  if (curve.kind == CurveType::Arc) {
    range = curve.clockwise ? arc_range(circle, end_point, start_point)
                            : arc_range(circle, start_point, end_point);
  }
  BRepBuilderAPI_MakeEdge maker(geometry, first, last, range.first, range.second);
  if (!maker.IsDone()) {
    raise_not_done("BRepBuilderAPI_MakeEdge failed for an arc with error " +
                   std::to_string(maker.Error()));
  }
  TopoDS_Edge edge = maker.Edge();
  if (curve.clockwise) {
    edge.Reverse();
  }
  return edge;
}

// The distance from a junction point to the end of an arc or circle curve that meets it.
double end_deviation(const gp_Circ &circle, const gp_Pnt &point) {
  return ElCLib::Value(ElCLib::Parameter(circle, point), circle).Distance(point);
}

// Builds the closed wire of one loop: curves [begin, end) of `curves`. Each curve's edge is
// appended to `curve_edges`.
TopoDS_Wire make_loop(const gp_Ax3 &axes, rust::Slice<const ProfileCurveData> curves,
                      std::size_t begin, std::size_t end, std::vector<TopoDS_Edge> &curve_edges) {
  const std::size_t count = end - begin;
  std::vector<gp_Pnt> points;
  points.reserve(count);
  for (std::size_t position = begin; position < end; ++position) {
    points.push_back(ElSLib::PlaneValue(curves[position].start_x, curves[position].start_y, axes));
  }
  // Each junction vertex covers the distance to every curve end meeting it.
  std::vector<double> deviations(count, 0.0);
  for (std::size_t position = 0; position < count; ++position) {
    const ProfileCurveData &curve = curves[begin + position];
    if (curve.kind == CurveType::Line) {
      continue;
    }
    const gp_Circ circle = circle_of(axes, curve);
    const std::size_t next = (position + 1) % count;
    deviations[position] = std::max(deviations[position], end_deviation(circle, points[position]));
    deviations[next] = std::max(deviations[next], end_deviation(circle, points[next]));
  }
  BRep_Builder builder;
  std::vector<TopoDS_Vertex> vertices(count);
  for (std::size_t position = 0; position < count; ++position) {
    const double tolerance = std::max(kLinear, deviations[position] + kEvaluationMargin);
    builder.MakeVertex(vertices[position], points[position], tolerance);
  }
  TopoDS_Wire wire;
  builder.MakeWire(wire);
  for (std::size_t position = 0; position < count; ++position) {
    const std::size_t next = (position + 1) % count;
    const TopoDS_Edge edge = make_edge(axes, curves[begin + position], vertices[position],
                                       vertices[next], points[position], points[next]);
    curve_edges.push_back(edge);
    builder.Add(wire, edge);
  }
  wire.Closed(Standard_True);
  return wire;
}

// Builds the face of one region from its outer wire and hole wires.
TopoDS_Face make_region(const gp_Pln &plane, const std::vector<TopoDS_Wire> &wires) {
  BRepBuilderAPI_MakeFace maker(plane, wires.front(), Standard_True);
  for (std::size_t position = 1; position < wires.size(); ++position) {
    maker.Add(wires[position]);
  }
  if (!maker.IsDone()) {
    raise_invalid_input(kInvalidRegion);
  }
  const TopoDS_Face face = maker.Face();
  if (!BRepCheck_Analyzer(face).IsValid()) {
    raise_invalid_input(kInvalidRegion);
  }
  return face;
}

}  // namespace

ProfileData make_profile(const Frame &frame, rust::Slice<const ProfileCurveData> curves) {
  const gp_Ax3 axes = to_ax3(frame);
  const gp_Pln plane(axes);
  std::vector<TopoDS_Edge> curve_edges;
  curve_edges.reserve(curves.size());
  std::vector<TopoDS_Face> faces;
  std::size_t begin = 0;
  while (begin < curves.size()) {
    const std::uint32_t region = curves[begin].region;
    std::vector<TopoDS_Wire> wires;
    while (begin < curves.size() && curves[begin].region == region) {
      const std::uint32_t loop = curves[begin].loop_index;
      std::size_t end = begin;
      while (end < curves.size() && curves[end].region == region &&
             curves[end].loop_index == loop) {
        ++end;
      }
      wires.push_back(make_loop(axes, curves, begin, end, curve_edges));
      begin = end;
    }
    faces.push_back(make_region(plane, wires));
  }
  if (faces.empty()) {
    raise_invalid_input("The profile has no regions.");
  }

  TopoDS_Shape result = faces.front();
  if (faces.size() > 1) {
    BRep_Builder builder;
    TopoDS_Compound compound;
    builder.MakeCompound(compound);
    for (const TopoDS_Face &face : faces) {
      builder.Add(compound, face);
    }
    result = compound;
  }
  std::shared_ptr<Shape> shape = make_shape(result);
  rust::Vec<std::uint32_t> edges;
  edges.reserve(curve_edges.size());
  for (const TopoDS_Edge &edge : curve_edges) {
    edges.push_back(static_cast<std::uint32_t>(shape->edges().FindIndex(edge)));
  }
  return ProfileData{std::move(shape), std::move(edges)};
}

}  // namespace occt
}  // namespace partsmith
