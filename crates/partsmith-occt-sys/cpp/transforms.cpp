// Transforms, split by plane, planar section and compounds (kernel spec sections 3.4.10 to
// 3.4.13).
#include <algorithm>
#include <cmath>
#include <sstream>
#include <vector>

#include <BRepAlgoAPI_Section.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepGProp.hxx>
#include <BRep_Builder.hxx>
#include <Bnd_Box.hxx>
#include <ElSLib.hxx>
#include <GProp_GProps.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <TopoDS_Compound.hxx>
#include <gp_Pln.hxx>
#include <gp_Trsf.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

namespace {

// Element kinds whose index maps a transform preserves.
constexpr ElementKind kKinds[] = {ElementKind::Solid, ElementKind::Face, ElementKind::Edge,
                                  ElementKind::Vertex};

// True when the shape contains a solid.
bool has_solid(const TopoDS_Shape &shape) { return TopExp_Explorer(shape, TopAbs_SOLID).More(); }

// The failure message of a Boolean-family algorithm.
template <class Algorithm>
void require_no_errors(const Algorithm &algorithm, const char *name) {
  if (algorithm.HasErrors()) {
    std::ostringstream alerts;
    algorithm.DumpErrors(alerts);
    raise_not_done(std::string(name) + " failed: " + alerts.str());
  }
}

// A planar face on `plane` that covers the projection of the shape's bounding box.
TopoDS_Face covering_face(const TopoDS_Shape &shape, const gp_Pln &plane) {
  Bnd_Box box;
  BRepBndLib::Add(shape, box);
  const double half_size = std::sqrt(box.SquareExtent()) + 1.0;
  double min_x = 0.0, min_y = 0.0, min_z = 0.0, max_x = 0.0, max_y = 0.0, max_z = 0.0;
  box.Get(min_x, min_y, min_z, max_x, max_y, max_z);
  const gp_Pnt center((min_x + max_x) / 2.0, (min_y + max_y) / 2.0, (min_z + max_z) / 2.0);
  double u_center = 0.0, v_center = 0.0;
  ElSLib::Parameters(plane, center, u_center, v_center);
  BRepBuilderAPI_MakeFace maker(plane, u_center - half_size, u_center + half_size,
                                v_center - half_size, v_center + half_size);
  return maker.Face();
}

}  // namespace

OpResultData transform(const Shape &shape, rust::Slice<const double> matrix) {
  if (matrix.size() != 12) {
    raise_invalid_input("The transform needs twelve matrix entries.");
  }
  gp_Trsf transformation;
  transformation.SetValues(matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5],
                           matrix[6], matrix[7], matrix[8], matrix[9], matrix[10], matrix[11]);
  Handle(BRepTools_History) history = new BRepTools_History;
  TopoDS_Shape result;
  const bool rigid = !transformation.IsNegative() &&
                     std::abs(transformation.ScaleFactor() - 1.0) <= TopLoc_Location::ScalePrec();
  if (rigid) {
    // Rotations and translations only change the location; every index is kept.
    result = shape.get().Moved(TopLoc_Location(transformation));
    const Shape moved(result);
    for (ElementKind kind : kKinds) {
      for (int index = 1; index <= shape.map(kind).Extent(); ++index) {
        history->AddModified(shape.map(kind)(index), moved.map(kind)(index));
      }
    }
  } else {
    // Mirrors and scales transform a copy of the geometry.
    BRepBuilderAPI_Transform transformer(shape.get(), transformation, Standard_True);
    result = transformer.Shape();
    TopTools_ListOfShape arguments;
    arguments.Append(shape.get());
    merge_history(history, new BRepTools_History(arguments, transformer));
    if (has_solid(shape.get())) {
      require_solid(result, "BRepBuilderAPI_Transform");
      ensure_valid(result, history, "BRepBuilderAPI_Transform");
    }
  }
  return op_result(result, history, {shape.get()});
}

SplitData split(const Shape &shape, const Frame &plane_frame) {
  const gp_Pln plane(to_ax3(plane_frame));
  const TopoDS_Face tool = covering_face(shape.get(), plane);
  BRepAlgoAPI_Splitter splitter;
  TopTools_ListOfShape arguments;
  arguments.Append(shape.get());
  TopTools_ListOfShape tools;
  tools.Append(tool);
  splitter.SetArguments(arguments);
  splitter.SetTools(tools);
  splitter.SetNonDestructive(Standard_True);
  splitter.SetRunParallel(Standard_False);
  splitter.SetToFillHistory(Standard_True);
  splitter.Build();
  require_no_errors(splitter, "BRepAlgoAPI_Splitter");
  Handle(BRepTools_History) history = new BRepTools_History;
  merge_history(history, splitter.History());

  // Copy each piece on its own so the pieces share no sub-shapes.
  TopTools_IndexedMapOfShape pieces;
  TopExp::MapShapes(splitter.Shape(), TopAbs_SOLID, pieces);
  Handle(BRepTools_History) copies = new BRepTools_History;
  BRep_Builder builder;
  TopoDS_Compound compound;
  builder.MakeCompound(compound);
  for (int piece = 1; piece <= pieces.Extent(); ++piece) {
    BRepBuilderAPI_Copy copier(pieces(piece), Standard_True, Standard_False);
    builder.Add(compound, copier.Shape());
    TopTools_IndexedMapOfShape elements;
    TopExp::MapShapes(pieces(piece), elements);
    for (int element = 1; element <= elements.Extent(); ++element) {
      if (!BRepTools_History::IsSupportedType(elements(element))) {
        continue;
      }
      for (const TopoDS_Shape &image : copier.Modified(elements(element))) {
        copies->AddModified(elements(element), image);
      }
    }
  }
  merge_history(history, copies);
  const TopoDS_Shape result = finish_solid(compound, history, "BRepAlgoAPI_Splitter");

  std::shared_ptr<Shape> pieces_shape = make_shape(result);
  const Shape &output = *pieces_shape;
  const gp_XYZ origin = plane.Location().XYZ();
  const gp_XYZ normal = plane.Axis().Direction().XYZ();
  rust::Vec<bool> solid_positive;
  for (int solid = 1; solid <= output.solids().Extent(); ++solid) {
    GProp_GProps properties;
    BRepGProp::VolumeProperties(output.solids()(solid), properties);
    solid_positive.push_back((properties.CentreOfMass().XYZ() - origin).Dot(normal) > 0.0);
  }

  rust::Vec<InputHistoryData> inputs = history_data(history, {shape.get()}, output);
  // A face of the input lying on the plane is also an image of the tool, since the splitter
  // merges coincident faces; it was not created on the plane, so it is no section face.
  std::vector<bool> input_images(static_cast<std::size_t>(output.faces().Extent()) + 1, false);
  for (const FateData &fate : inputs[0].faces) {
    for (std::uint32_t face : fate.modified) {
      input_images[static_cast<std::size_t>(face)] = true;
    }
  }

  TopTools_IndexedDataMapOfShapeListOfShape face_solids;
  TopExp::MapShapesAndUniqueAncestors(result, TopAbs_FACE, TopAbs_SOLID, face_solids);
  std::vector<std::pair<std::uint32_t, bool>> sections;
  for (const TopoDS_Shape &image : history->Modified(tool)) {
    const int face = output.faces().FindIndex(image);
    if (face == 0 || input_images[static_cast<std::size_t>(face)]) {
      continue;
    }
    const TopTools_ListOfShape *solids = face_solids.Seek(image);
    if (solids == nullptr || solids->IsEmpty()) {
      continue;
    }
    const int solid = output.solids().FindIndex(solids->First());
    sections.emplace_back(static_cast<std::uint32_t>(face),
                          solid > 0 && solid_positive[static_cast<std::size_t>(solid) - 1]);
  }
  std::sort(sections.begin(), sections.end());
  sections.erase(std::unique(sections.begin(), sections.end()), sections.end());
  rust::Vec<SectionFaceData> section_faces;
  for (const auto &[face, positive] : sections) {
    section_faces.push_back(SectionFaceData{face, positive});
  }

  return SplitData{OpResultData{std::move(pieces_shape), std::move(inputs)},
                   std::move(solid_positive), std::move(section_faces)};
}

std::shared_ptr<Shape> section(const Shape &shape, const Frame &plane_frame) {
  BRep_Builder builder;
  TopoDS_Compound edges;
  builder.MakeCompound(edges);
  if (shape.faces().Extent() == 0) {
    return make_shape(edges);
  }
  BRepAlgoAPI_Section section(shape.get(), gp_Pln(to_ax3(plane_frame)), Standard_False);
  section.Approximation(Standard_True);
  section.SetNonDestructive(Standard_True);
  section.SetRunParallel(Standard_False);
  section.Build();
  require_no_errors(section, "BRepAlgoAPI_Section");
  TopTools_IndexedMapOfShape curves;
  TopExp::MapShapes(section.Shape(), TopAbs_EDGE, curves);
  for (int index = 1; index <= curves.Extent(); ++index) {
    builder.Add(edges, curves(index));
  }
  return make_shape(edges);
}

OpResultData compound(rust::Slice<const ShapeHandle> shapes) {
  const std::vector<TopoDS_Shape> inputs = shapes_of(shapes);
  BRep_Builder builder;
  TopoDS_Compound result;
  builder.MakeCompound(result);
  for (const TopoDS_Shape &shape : inputs) {
    builder.Add(result, shape);
  }
  return op_result(result, Handle(BRepTools_History)(), inputs);
}

}  // namespace occt
}  // namespace partsmith
