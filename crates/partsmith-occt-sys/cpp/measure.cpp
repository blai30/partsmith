// Measurement: volume, area, center of mass and the area and centroid of faces (kernel spec
// section 3.6, items 1 to 3).
#include <cmath>
#include <optional>

#include <BRepAdaptor_Surface.hxx>
#include <BRepGProp.hxx>
#include <BRepGProp_Domain.hxx>
#include <BRepGProp_Face.hxx>
#include <BRepGProp_Vinert.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Geom2d_Circle.hxx>
#include <Geom2d_Curve.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Circ2d.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Pln.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

namespace {

// Integrals over a region of a plane's parameter plane (x, y), accumulated along its boundary
// by Green's theorem with the fields (-y / 2, x / 2), (0, x^2 / 2) and (-y^2 / 2, 0): `area`
// integrates 1, `first_x` integrates x and `first_y` integrates y. Every boundary piece uses
// the same fields, so the pieces of all loops add up to the exact integrals.
struct PlaneIntegrals {
  double area = 0.0;
  double first_x = 0.0;
  double first_y = 0.0;

  // Adds the segment from `start` to `end`.
  void add_segment(const gp_XY &start, const gp_XY &end) {
    const double x0 = start.X(), y0 = start.Y(), x1 = end.X(), y1 = end.Y();
    area += 0.5 * (x0 * y1 - x1 * y0);
    first_x += (y1 - y0) * (x0 * x0 + x0 * x1 + x1 * x1) / 6.0;
    first_y -= (x1 - x0) * (y0 * y0 + y0 * y1 + y1 * y1) / 6.0;
  }

  // Adds the arc of the circle with `center` and `radius` from `start` to `end` through
  // `middle`, turning by `angle` (counterclockwise when positive): its chord, plus the loop of
  // the arc and the chord back, which bounds a circular segment with closed-form integrals.
  void add_arc(const gp_XY &start, const gp_XY &end, const gp_XY &middle, const gp_XY &center,
               double radius, double angle) {
    add_segment(start, end);
    const double turn = std::abs(angle);
    const double sign = angle < 0.0 ? -1.0 : 1.0;
    const double segment_area = 0.5 * radius * radius * (turn - std::sin(turn));
    const double half_sine = std::sin(0.5 * turn);
    // The segment's first moment about the center, along the bisector toward `middle`.
    const double offset = 2.0 / 3.0 * radius * radius * radius * half_sine * half_sine * half_sine;
    const gp_XY bisector = (middle - center) / radius;
    area += sign * segment_area;
    first_x += sign * (segment_area * center.X() + offset * bisector.X());
    first_y += sign * (segment_area * center.Y() + offset * bisector.Y());
  }
};

// Integrals over a region of a cylinder's parameter plane (u, v), accumulated along its
// boundary by Green's theorem with the fields (0, u), (0, sin u), (0, -cos u) and
// (-v^2 / 2, 0): `area` integrates 1, `cosine` cos u, `sine` sin u and `height` v.
struct CylinderIntegrals {
  double area = 0.0;
  double cosine = 0.0;
  double sine = 0.0;
  double height = 0.0;

  // Adds the segment from `start` to `end`, both relative to `reference`; `height` stays
  // relative to the reference's v.
  void add_segment(const gp_XY &start, const gp_XY &end, const gp_XY &reference) {
    const double u_span = end.X() - start.X();
    const double v_span = end.Y() - start.Y();
    const double mean_u = reference.X() + 0.5 * (start.X() + end.X());
    const double half = 0.5 * u_span;
    // sin(half) / half, with its series where the quotient loses precision.
    const double sinc = std::abs(half) < 1e-4 ? 1.0 - half * half / 6.0 : std::sin(half) / half;
    area += v_span * 0.5 * (start.X() + end.X());
    cosine += v_span * std::sin(mean_u) * sinc;
    sine -= v_span * std::cos(mean_u) * sinc;
    const double v0 = start.Y(), v1 = end.Y();
    height -= u_span * (v0 * v0 + v0 * v1 + v1 * v1) / 6.0;
  }
};

// A parameter-plane curve without its trimming wrappers, which keep the parametrization.
Handle(Geom2d_Curve) basis_curve(Handle(Geom2d_Curve) curve) {
  while (!curve.IsNull() && curve->IsKind(STANDARD_TYPE(Geom2d_TrimmedCurve))) {
    curve = Handle(Geom2d_TrimmedCurve)::DownCast(curve)->BasisCurve();
  }
  return curve;
}

// The boundary integrals of a face on a plane bounded by lines and circular arcs in its
// parameter plane, or on a cylinder bounded by lines, over the forward face and relative to its
// first boundary point `reference` (the integrals above).
struct AnalyticFace {
  bool planar;
  gp_XY reference;
  PlaneIntegrals plane;
  CylinderIntegrals cylinder;
};

// The point of `plane` at parameters (x, y).
gp_XYZ plane_point(const gp_Pln &plane, double x, double y) {
  return plane.Location().XYZ() + plane.XAxis().Direction().XYZ() * x +
         plane.YAxis().Direction().XYZ() * y;
}

// The integrals of a face on a plane (`planar`) or cylinder from the curves of its boundary in
// the parameter plane; nothing when a curve is neither a line nor, on a plane, a circle, or the
// face has no boundary.
std::optional<AnalyticFace> boundary_integrals(const TopoDS_Face &face, bool planar) {
  // The forward face has its material on the left of its oriented boundary.
  const TopoDS_Face forward = TopoDS::Face(face.Oriented(TopAbs_FORWARD));
  bool has_reference = false;
  // Coordinates are taken relative to the first boundary point to keep the sums accurate.
  gp_XY reference(0.0, 0.0);
  PlaneIntegrals plane_integrals;
  CylinderIntegrals cylinder_integrals;
  for (TopExp_Explorer edges(forward, TopAbs_EDGE); edges.More(); edges.Next()) {
    const TopoDS_Edge &edge = TopoDS::Edge(edges.Current());
    const TopAbs_Orientation orientation = edge.Orientation();
    // Internal and external edges bound nothing.
    if (orientation != TopAbs_FORWARD && orientation != TopAbs_REVERSED) {
      continue;
    }
    double first = 0.0, last = 0.0;
    const Handle(Geom2d_Curve) curve =
        basis_curve(BRep_Tool::CurveOnSurface(edge, forward, first, last));
    if (curve.IsNull()) {
      return std::nullopt;
    }
    const bool reversed = orientation == TopAbs_REVERSED;
    gp_XY start = curve->Value(reversed ? last : first).XY();
    gp_XY end = curve->Value(reversed ? first : last).XY();
    if (!has_reference) {
      has_reference = true;
      reference = start;
    }
    start -= reference;
    end -= reference;
    if (curve->IsKind(STANDARD_TYPE(Geom2d_Line))) {
      if (planar) {
        plane_integrals.add_segment(start, end);
      } else {
        cylinder_integrals.add_segment(start, end, reference);
      }
    } else if (planar && curve->IsKind(STANDARD_TYPE(Geom2d_Circle))) {
      const gp_Circ2d circle = Handle(Geom2d_Circle)::DownCast(curve)->Circ2d();
      const gp_Ax22d &axes = circle.Position();
      const double sense = axes.XDirection().Crossed(axes.YDirection()) < 0.0 ? -1.0 : 1.0;
      const double angle = (last - first) * sense * (reversed ? -1.0 : 1.0);
      const gp_XY middle = curve->Value(0.5 * (first + last)).XY() - reference;
      plane_integrals.add_arc(start, end, middle, circle.Location().XY() - reference,
                              circle.Radius(), angle);
    } else {
      return std::nullopt;
    }
  }
  if (!has_reference) {
    return std::nullopt;
  }
  return AnalyticFace{planar, reference, plane_integrals, cylinder_integrals};
}

// True when the face's own surface has the parametrization the integrals assume at their
// reference point, and the integrated area is positive.
bool fits_surface(const AnalyticFace &integrals, const BRepAdaptor_Surface &surface) {
  const gp_XY &reference = integrals.reference;
  gp_XYZ expected;
  double area = 0.0;
  if (integrals.planar) {
    expected = plane_point(surface.Plane(), reference.X(), reference.Y());
    area = integrals.plane.area;
  } else {
    const gp_Cylinder cylinder = surface.Cylinder();
    const gp_Ax3 &frame = cylinder.Position();
    const double radius = cylinder.Radius();
    expected = frame.Location().XYZ() +
               frame.XDirection().XYZ() * (radius * std::cos(reference.X())) +
               frame.YDirection().XYZ() * (radius * std::sin(reference.X())) +
               frame.Direction().XYZ() * reference.Y();
    area = integrals.cylinder.area;
  }
  const gp_XYZ surface_point = surface.Value(reference.X(), reference.Y()).XYZ();
  if ((surface_point - expected).Modulus() > kLinear) {
    return false;
  }
  return area > 0.0;
}

// The boundary integrals of a face on a plane or cylinder bounded as `AnalyticFace` says, checked
// against the face's own surface; nothing for any other face. BRepGProp integrates numerically
// with 18 x 18 points per boundary edge, which made element queries of faces with many edges
// slow (kernel spec section 5).
std::optional<AnalyticFace> analytic_face(const TopoDS_Face &face,
                                          const BRepAdaptor_Surface &surface) {
  const GeomAbs_SurfaceType type = surface.GetType();
  // A scaled location would scale the parameter plane as well.
  if ((type != GeomAbs_Plane && type != GeomAbs_Cylinder) || face.NbChildren() == 0 ||
      std::abs(std::abs(surface.Trsf().ScaleFactor()) - 1.0) > 1e-12) {
    return std::nullopt;
  }
  std::optional<AnalyticFace> integrals = boundary_integrals(face, type == GeomAbs_Plane);
  if (!integrals || !fits_surface(*integrals, surface)) {
    return std::nullopt;
  }
  return integrals;
}

// The exact area and centroid of a face from its `analytic_face` integrals.
FaceMass mass_of(const AnalyticFace &integrals, const BRepAdaptor_Surface &surface) {
  const gp_XY &reference = integrals.reference;
  if (integrals.planar) {
    const PlaneIntegrals &plane_integrals = integrals.plane;
    const double x = reference.X() + plane_integrals.first_x / plane_integrals.area;
    const double y = reference.Y() + plane_integrals.first_y / plane_integrals.area;
    return FaceMass{plane_integrals.area, gp_Pnt(plane_point(surface.Plane(), x, y))};
  }
  const CylinderIntegrals &cylinder_integrals = integrals.cylinder;
  const gp_Cylinder cylinder = surface.Cylinder();
  const gp_Ax3 &frame = cylinder.Position();
  const double radius = cylinder.Radius();
  const gp_XYZ z_axis = frame.Direction().XYZ();
  // The area element is radius du dv, and a point is origin + radius (cos u x + sin u y) + v z.
  const gp_XYZ moment = frame.XDirection().XYZ() * (radius * cylinder_integrals.cosine) +
                        frame.YDirection().XYZ() * (radius * cylinder_integrals.sine) +
                        z_axis * cylinder_integrals.height;
  return FaceMass{radius * cylinder_integrals.area,
                  gp_Pnt(frame.Location().XYZ() + z_axis * reference.Y() +
                         moment / cylinder_integrals.area)};
}

// The face's term of the divergence theorem for the volume of the solid it bounds, exactly for
// a face that `analytic_face` integrates: one third of the integral of (x - origin) . n over the
// face, with n its normal out of the material (the surface normal, reversed for a reversed face).
// Nothing for any other face.
std::optional<double> analytic_volume_term(const TopoDS_Face &face,
                                           const BRepAdaptor_Surface &surface,
                                           const gp_XYZ &origin) {
  const std::optional<AnalyticFace> integrals = analytic_face(face, surface);
  if (!integrals) {
    return std::nullopt;
  }
  // A mirroring location turns the parameter plane over against the surface normal.
  if (surface.Trsf().IsNegative()) {
    return std::nullopt;
  }
  const FaceMass mass = mass_of(*integrals, surface);
  const double orientation = face.Orientation() == TopAbs_REVERSED ? -1.0 : 1.0;
  if (surface.GetType() == GeomAbs_Plane) {
    // The normal is constant, so the integral is the normal times the centroid times the area.
    const gp_Ax3 frame = surface.Plane().Position();
    const gp_XYZ normal = frame.XDirection().XYZ().Crossed(frame.YDirection().XYZ());
    return orientation * normal.Dot(mass.centroid.XYZ() - origin) * mass.area / 3.0;
  }
  // On the cylinder the outward radial direction r(u) = cos u x + sin u y is the surface normal
  // of a right-handed frame, and (x - origin) . r(u) = (axis origin - origin) . r(u) + radius,
  // whose integral over the face is the radial part of its first moment plus radius times area.
  const gp_Cylinder cylinder = surface.Cylinder();
  const gp_Ax3 &frame = cylinder.Position();
  const double radius = cylinder.Radius();
  const double handedness = frame.Direct() ? 1.0 : -1.0;
  const gp_XYZ offset = frame.Location().XYZ() - origin;
  const double radial_moment =
      radius * (offset.Dot(frame.XDirection().XYZ()) * integrals->cylinder.cosine +
                offset.Dot(frame.YDirection().XYZ()) * integrals->cylinder.sine);
  return orientation * handedness * (radial_moment + radius * mass.area) / 3.0;
}

// The volume of one solid by the divergence theorem (kernel spec section 3.6, item 1): the sum
// over its faces of one third of the integral of (x - p) . n, with p the mean of its vertices
// (the reference point BRepGProp uses), exact for planes and cylinders (`analytic_volume_term`)
// and integrated by BRepGProp_Vinert, as BRepGProp::VolumeProperties does, for other faces.
double solid_volume(const TopoDS_Shape &solid) {
  gp_XYZ origin(0.0, 0.0, 0.0);
  int vertices = 0;
  for (TopExp_Explorer vertex(solid, TopAbs_VERTEX); vertex.More(); vertex.Next()) {
    origin += BRep_Tool::Pnt(TopoDS::Vertex(vertex.Current())).XYZ();
    ++vertices;
  }
  if (vertices > 0) {
    origin /= vertices;
  }
  BRepGProp_Face properties;
  BRepGProp_Domain domain;
  BRepGProp_Vinert integrator;
  integrator.SetLocation(gp_Pnt(origin));
  double volume = 0.0;
  for (TopExp_Explorer faces(solid, TopAbs_FACE); faces.More(); faces.Next()) {
    const TopoDS_Face &face = TopoDS::Face(faces.Current());
    const TopAbs_Orientation orientation = face.Orientation();
    TopLoc_Location location;
    if ((orientation != TopAbs_FORWARD && orientation != TopAbs_REVERSED) ||
        BRep_Tool::Surface(face, location).IsNull()) {
      continue;
    }
    const BRepAdaptor_Surface surface(face, Standard_False);
    if (const std::optional<double> term = analytic_volume_term(face, surface, origin)) {
      volume += *term;
      continue;
    }
    properties.Load(face);
    if (face.NbChildren() == 0) {
      integrator.Perform(properties);
    } else {
      domain.Init(face);
      integrator.Perform(properties, domain);
    }
    volume += integrator.Mass();
  }
  return volume;
}

// The volume of the solids of an index map, in index order.
double volume_of(const TopTools_IndexedMapOfShape &solids) {
  double volume = 0.0;
  for (int index = 1; index <= solids.Extent(); ++index) {
    volume += solid_volume(solids(index));
  }
  return volume;
}

// The smallest volume (mm^3) with a center of mass: below it, GProp_GProps::Add keeps no centroid
// and reports the origin of its frame instead (kernel spec section 3.6, item 1).
constexpr double kSmallestCenteredVolume = 1e-20;

}  // namespace

FaceMass detail::face_mass(const TopoDS_Face &face, const BRepAdaptor_Surface &surface) {
  if (const std::optional<AnalyticFace> integrals = analytic_face(face, surface)) {
    return mass_of(*integrals, surface);
  }
  GProp_GProps properties;
  BRepGProp::SurfaceProperties(face, properties);
  return FaceMass{properties.Mass(), properties.CentreOfMass()};
}

double detail::solids_volume(const TopoDS_Shape &shape) {
  TopTools_IndexedMapOfShape solids;
  TopExp::MapShapes(shape, TopAbs_SOLID, solids);
  return volume_of(solids);
}

double shape_volume(const Shape &shape) { return volume_of(shape.solids()); }

double shape_area(const Shape &shape) {
  double area = 0.0;
  for (int index = 1; index <= shape.faces().Extent(); ++index) {
    const TopoDS_Face &face = TopoDS::Face(shape.faces()(index));
    area += face_mass(face, BRepAdaptor_Surface(face, Standard_False)).area;
  }
  return area;
}

Vec3 shape_center_of_mass(const Shape &shape) {
  // Volume properties summed over the solids.
  GProp_GProps properties;
  for (int index = 1; index <= shape.solids().Extent(); ++index) {
    GProp_GProps solid_properties;
    BRepGProp::VolumeProperties(shape.solids()(index), solid_properties);
    properties.Add(solid_properties);
  }
  const double volume = properties.Mass();
  if (!std::isfinite(volume) || std::abs(volume) < kSmallestCenteredVolume) {
    raise_invalid_input("This shape has no volume, so it has no center of mass.");
  }
  return to_vec3(properties.CentreOfMass().XYZ());
}

}  // namespace occt
}  // namespace partsmith
