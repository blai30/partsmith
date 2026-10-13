// Extrusion and revolution with face roles (kernel spec sections 3.4.3 and 3.4.4).
#include <utility>
#include <vector>

#include <BRepPrimAPI_MakePrism.hxx>
#include <BRep_Builder.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS_Compound.hxx>
#include <gp_Ax1.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

namespace {

// Faces of a sweep paired with their roles, before validation and repair.
using Roles = std::vector<std::pair<TopoDS_Shape, SweepFaceData>>;

// Adds every face of `shape` with `role`.
void add_faces(Roles &roles, const TopoDS_Shape &shape, SweepFaceData role) {
  if (shape.IsNull()) {
    return;
  }
  for (TopExp_Explorer explorer(shape, TopAbs_FACE); explorer.More(); explorer.Next()) {
    roles.emplace_back(explorer.Current(), role);
  }
}

// Adds the lateral faces generated from every edge of the profile.
template <class Sweep>
void add_lateral_faces(Roles &roles, Sweep &sweep, const TopoDS_Shape &profile) {
  TopTools_IndexedMapOfShape edges;
  TopExp::MapShapes(profile, TopAbs_EDGE, edges);
  for (int index = 1; index <= edges.Extent(); ++index) {
    for (const TopoDS_Shape &generated : sweep.Generated(edges(index))) {
      if (generated.ShapeType() == TopAbs_FACE) {
        roles.emplace_back(generated,
                           SweepFaceData{SweepRole::Lateral, static_cast<std::uint32_t>(index)});
      }
    }
  }
}

// The caps of a sweep: the copied profile it started from and the profile moved to its end.
TopoDS_Shape caps_of(const TopoDS_Shape &first, const TopoDS_Shape &last) {
  BRep_Builder builder;
  TopoDS_Compound caps;
  builder.MakeCompound(caps);
  for (const TopoDS_Shape &cap : {first, last}) {
    if (!cap.IsNull()) {
      builder.Add(caps, cap);
    }
  }
  return caps;
}

// Normalizes, checks and repairs a swept shape, then gives every face exactly one role. The caps
// are copies of the profile, which `make_profile` checked, moved rigidly, so the check trusts
// them like the unchanged faces of an input (kernel spec section 3.5, item 4); without caps the
// whole result is checked.
SweepData sweep_data(const TopoDS_Shape &swept, const TopoDS_Shape &caps, const Roles &roles,
                     const char *algorithm) {
  CheckInputs inputs = caps.IsNull() ? CheckInputs() : CheckInputs({caps});
  inputs.settle();
  // The sweep copied the profile, so every sub-shape of the swept shape is new.
  store_plane_pcurves(swept);
  Handle(BRepTools_History) history = new BRepTools_History;
  std::shared_ptr<Shape> shape = make_shape(finish_solid(swept, history, inputs, algorithm));

  const int face_count = shape->faces().Extent();
  std::vector<bool> assigned(static_cast<std::size_t>(face_count) + 1, false);
  rust::Vec<SweepFaceData> faces;
  for (int index = 0; index < face_count; ++index) {
    faces.push_back(SweepFaceData{SweepRole::Lateral, 0});
  }
  for (const auto &[face, role] : roles) {
    TopTools_ListOfShape images = history->Modified(face);
    if (images.IsEmpty() && !history->IsRemoved(face)) {
      images.Append(face);
    }
    for (const TopoDS_Shape &image : images) {
      const int index = shape->faces().FindIndex(image);
      if (index == 0) {
        continue;
      }
      if (assigned[static_cast<std::size_t>(index)]) {
        raise_invalid_result(std::string(algorithm) + " gave face " + std::to_string(index) +
                             " two roles");
      }
      assigned[static_cast<std::size_t>(index)] = true;
      faces[static_cast<std::size_t>(index) - 1] = role;
    }
  }
  for (int index = 1; index <= face_count; ++index) {
    if (!assigned[static_cast<std::size_t>(index)]) {
      raise_invalid_result(std::string(algorithm) + " gave face " + std::to_string(index) +
                           " no role");
    }
  }
  return SweepData{std::move(shape), std::move(faces)};
}

}  // namespace

SweepData extrude(const Shape &profile, const Vec3 &direction, double start, double end) {
  const gp_Vec unit(direction.x, direction.y, direction.z);
  gp_Trsf shift;
  shift.SetTranslation(unit * start);
  const TopoDS_Shape moved = profile.get().Moved(TopLoc_Location(shift));
  // Copy so the sweep never writes to the profile's sub-shapes (kernel spec section 3.1).
  BRepPrimAPI_MakePrism prism(moved, unit * (end - start), Standard_True, Standard_True);
  if (!prism.IsDone()) {
    raise_not_done("BRepPrimAPI_MakePrism could not sweep the profile");
  }
  Roles roles;
  add_lateral_faces(roles, prism, moved);
  add_faces(roles, prism.FirstShape(), SweepFaceData{SweepRole::StartCap, 0});
  add_faces(roles, prism.LastShape(), SweepFaceData{SweepRole::EndCap, 0});
  return sweep_data(prism.Shape(), caps_of(prism.FirstShape(), prism.LastShape()), roles,
                    "BRepPrimAPI_MakePrism");
}

SweepData revolve(const Shape &profile, const Vec3 &origin, const Vec3 &direction, double angle,
                  bool full) {
  const gp_Ax1 axis(to_point(origin), to_direction(direction));
  // Copy so the sweep never writes to the profile's sub-shapes (kernel spec section 3.1).
  BRepPrimAPI_MakeRevol revolution(profile.get(), axis, full ? 2.0 * M_PI : angle, Standard_True);
  if (!revolution.IsDone() || revolution.Shape().IsNull()) {
    raise_not_done("BRepPrimAPI_MakeRevol could not revolve the profile");
  }
  Roles roles;
  add_lateral_faces(roles, revolution, profile.get());
  if (!full) {
    add_faces(roles, revolution.FirstShape(), SweepFaceData{SweepRole::StartCap, 0});
    add_faces(roles, revolution.LastShape(), SweepFaceData{SweepRole::EndCap, 0});
  }
  const TopoDS_Shape caps =
      full ? TopoDS_Shape() : caps_of(revolution.FirstShape(), revolution.LastShape());
  return sweep_data(revolution.Shape(), caps, roles, "BRepPrimAPI_MakeRevol");
}

}  // namespace occt
}  // namespace partsmith
