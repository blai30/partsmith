// Transforms, split by plane, planar section and compounds (kernel spec sections 3.4.10 to
// 3.4.13).
#include <algorithm>
#include <cmath>
#include <vector>

#include <BRepAlgoAPI_Section.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepGProp.hxx>
#include <Bnd_Box.hxx>
#include <ElSLib.hxx>
#include <GProp_GProps.hxx>
#include <TopExp.hxx>
#include <TopLoc_Location.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
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

// The solids of `split_solids` copied apart, so that the pieces share no sub-shapes, with the
// copies' history merged into `history`.
TopoDS_Shape separated_pieces(const TopoDS_Shape &split_solids,
                              const Handle(BRepTools_History) &history) {
  TopTools_IndexedMapOfShape pieces;
  TopExp::MapShapes(split_solids, TopAbs_SOLID, pieces);
  Handle(BRepTools_History) copies = new BRepTools_History;
  std::vector<TopoDS_Shape> separated;
  for (int piece = 1; piece <= pieces.Extent(); ++piece) {
    BRepBuilderAPI_Copy copier(pieces(piece), Standard_True, Standard_False);
    separated.push_back(copier.Shape());
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
  return solids_only(compound_of(separated));
}

// For each solid of `pieces`, whether its center of mass lies on the side of `plane` its normal
// points to.
rust::Vec<bool> solid_sides(const Shape &pieces, const gp_Pln &plane) {
  const gp_XYZ origin = plane.Location().XYZ();
  const gp_XYZ normal = plane.Axis().Direction().XYZ();
  rust::Vec<bool> positive;
  for (int solid = 1; solid <= pieces.solids().Extent(); ++solid) {
    GProp_GProps properties;
    BRepGProp::VolumeProperties(pieces.solids()(solid), properties);
    positive.push_back((properties.CentreOfMass().XYZ() - origin).Dot(normal) > 0.0);
  }
  return positive;
}

// The faces of `pieces` created on the split plane, ascending, each with the side of the solid it
// bounds: the images of the splitting tool, except those that are also images of a face of the
// input (`input`), since the splitter merges an input face lying on the plane with the tool.
rust::Vec<SectionFaceData> section_faces(const Shape &pieces,
                                         const TopTools_ListOfShape &tool_images,
                                         const InputHistoryData &input,
                                         const rust::Vec<bool> &solid_positive) {
  std::vector<bool> input_images(static_cast<std::size_t>(pieces.faces().Extent()) + 1, false);
  for (const FateData &fate : input.faces) {
    for (std::uint32_t face : fate.modified) {
      input_images[static_cast<std::size_t>(face)] = true;
    }
  }
  TopTools_IndexedDataMapOfShapeListOfShape face_solids;
  TopExp::MapShapesAndUniqueAncestors(pieces.get(), TopAbs_FACE, TopAbs_SOLID, face_solids);
  std::vector<std::pair<std::uint32_t, bool>> sections;
  for (const TopoDS_Shape &image : tool_images) {
    const int face = pieces.faces().FindIndex(image);
    if (face == 0 || input_images[static_cast<std::size_t>(face)]) {
      continue;
    }
    const TopTools_ListOfShape *solids = face_solids.Seek(image);
    if (solids == nullptr || solids->IsEmpty()) {
      continue;
    }
    const int solid = pieces.solids().FindIndex(solids->First());
    sections.emplace_back(static_cast<std::uint32_t>(face),
                          solid > 0 && solid_positive[static_cast<std::size_t>(solid) - 1]);
  }
  std::sort(sections.begin(), sections.end());
  sections.erase(std::unique(sections.begin(), sections.end()), sections.end());
  rust::Vec<SectionFaceData> faces;
  for (const auto &[face, positive] : sections) {
    faces.push_back(SectionFaceData{face, positive});
  }
  return faces;
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
    merge_history(history, history_of(shape.get(), transformer));
    if (has_solid(shape.get())) {
      require_solid(result, "BRepBuilderAPI_Transform");
      // Every element is a transformed copy, so the whole result is checked.
      ensure_valid(result, history, CheckInputs(), "BRepBuilderAPI_Transform");
    }
  }
  return op_result(result, history, {shape.get()});
}

SplitData split(const Shape &shape, const Frame &plane_frame) {
  const gp_Pln plane(to_ax3(plane_frame));
  const TopoDS_Face tool = covering_face(shape.get(), plane);
  // The splitter runs non-destructively; recording the input still lets the check see any face
  // it changed in place (kernel spec section 3.5).
  const CheckInputs check_inputs({shape.get()});
  BRepAlgoAPI_Splitter splitter;
  TopTools_ListOfShape arguments;
  arguments.Append(shape.get());
  TopTools_ListOfShape tools;
  tools.Append(tool);
  splitter.SetArguments(arguments);
  splitter.SetTools(tools);
  set_boolean_options(splitter);
  splitter.Build();
  require_no_errors(splitter, "BRepAlgoAPI_Splitter");
  Handle(BRepTools_History) history = new BRepTools_History;
  merge_history(history, splitter.History());

  const TopoDS_Shape split_solids = solids_only(splitter.Shape());
  require_solid(split_solids, "BRepAlgoAPI_Splitter");
  // The pieces are checked as the splitter built them, where the faces the plane does not cut
  // are still the input's own (kernel spec section 3.5); copying keeps them valid.
  const bool valid = changed_part_valid(split_solids, check_inputs);
  TopoDS_Shape result = separated_pieces(split_solids, history);
  if (!valid) {
    // Repair the separated pieces: ShapeFix_Shape merges solids that share a face.
    ensure_valid(result, history, CheckInputs(), "BRepAlgoAPI_Splitter");
    result = solids_only(result);
  }

  std::shared_ptr<Shape> pieces = make_shape(result);
  rust::Vec<bool> solid_positive = solid_sides(*pieces, plane);
  rust::Vec<InputHistoryData> inputs = history_data(history, {shape.get()}, *pieces);
  rust::Vec<SectionFaceData> sections =
      section_faces(*pieces, history->Modified(tool), inputs[0], solid_positive);
  return SplitData{OpResultData{std::move(pieces), std::move(inputs)}, std::move(solid_positive),
                   std::move(sections)};
}

std::shared_ptr<Shape> section(const Shape &shape, const Frame &plane_frame) {
  if (shape.faces().Extent() == 0) {
    return make_shape(compound_of({}));
  }
  BRepAlgoAPI_Section section(shape.get(), gp_Pln(to_ax3(plane_frame)), Standard_False);
  section.Approximation(Standard_True);
  set_boolean_options(section);
  section.Build();
  require_no_errors(section, "BRepAlgoAPI_Section");
  TopTools_IndexedMapOfShape curves;
  TopExp::MapShapes(section.Shape(), TopAbs_EDGE, curves);
  return make_shape(compound_of(std::vector<TopoDS_Shape>(curves.cbegin(), curves.cend())));
}

OpResultData compound(rust::Slice<const ShapeHandle> shapes) {
  const std::vector<TopoDS_Shape> inputs = shapes_of(shapes);
  return op_result(compound_of(inputs), Handle(BRepTools_History)(), inputs);
}

}  // namespace occt
}  // namespace partsmith
