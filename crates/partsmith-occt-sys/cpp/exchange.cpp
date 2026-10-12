// STEP and binary BRep exchange (kernel spec section 3.8).
#include <cstring>
#include <mutex>
#include <sstream>
#include <string>

#include <APIHeaderSection_MakeHeader.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRep_Tool.hxx>
#include <BinTools.hxx>
#include <BinTools_ShapeSetBase.hxx>
#include <DESTEP_Parameters.hxx>
#include <Interface_HArray1OfHAsciiString.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Message_Printer.hxx>
#include <Quantity_Color.hxx>
#include <STEPCAFControl_Controller.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <ShapeFix_Shape.hxx>
#include <ShapeFix_Solid.hxx>
#include <Standard_NullObject.hxx>
#include <StepData_StepModel.hxx>
#include <TCollection_AsciiString.hxx>
#include <TCollection_ExtendedString.hxx>
#include <TCollection_HAsciiString.hxx>
#include <TDF_Label.hxx>
#include <TDF_LabelSequence.hxx>
#include <TDataStd_Name.hxx>
#include <TDocStd_Document.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <XCAFDoc_ColorTool.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>

#include "partsmith-occt-sys/cpp/internal.h"

namespace partsmith {
namespace occt {

using namespace detail;

void detail::initialize() {
  static std::once_flag once;
  std::call_once(once, [] {
    Message::DefaultMessenger()->RemovePrinters(STANDARD_TYPE(Message_Printer));
    STEPCAFControl_Controller::Init();
  });
}

namespace {

// Serializes STEP reading and writing: OCCT's data exchange layer keeps process-wide state
// (kernel spec section 3.10, item 4).
std::mutex &exchange_mutex() {
  static std::mutex mutex;
  return mutex;
}

// An XCAF document whose length unit is the millimeter.
Handle(TDocStd_Document) new_document() {
  Handle(TDocStd_Document) document = new TDocStd_Document("BinXCAF");
  XCAFDoc_DocumentTool::SetLengthUnit(document, 1.0, UnitsMethods_LengthUnit_Millimeter);
  return document;
}

// A copy of the shape with its own location applied to the geometry, so each body is written
// as one product without an assembly level.
TopoDS_Shape located_copy(const TopoDS_Shape &shape) {
  BRepBuilderAPI_Transform transformer(shape.Located(TopLoc_Location()),
                                       shape.Location().Transformation(), Standard_True);
  return transformer.Shape();
}

// A one-entry list of header strings.
Handle(Interface_HArray1OfHAsciiString) one_string(const char *value) {
  Handle(Interface_HArray1OfHAsciiString) values = new Interface_HArray1OfHAsciiString(1, 1);
  values->SetValue(1, new TCollection_HAsciiString(value));
  return values;
}

// Sets the deterministic header of kernel spec section 3.8.1, item 3.
void set_header(const Handle(StepData_StepModel) & model) {
  APIHeaderSection_MakeHeader header(model);
  header.SetName(new TCollection_HAsciiString("partsmith"));
  header.SetTimeStamp(new TCollection_HAsciiString("1970-01-01T00:00:00"));
  header.SetAuthor(one_string(""));
  header.SetOrganization(one_string(""));
  header.SetOriginatingSystem(new TCollection_HAsciiString("Partsmith"));
  header.SetAuthorisation(new TCollection_HAsciiString(""));
}

// The name attribute of a label as trimmed UTF-8, or empty.
std::string label_name(const TDF_Label &label) {
  Handle(TDataStd_Name) attribute;
  if (!label.FindAttribute(TDataStd_Name::GetID(), attribute)) {
    return std::string();
  }
  TCollection_AsciiString name(attribute->Get());
  name.LeftAdjust();
  name.RightAdjust();
  return std::string(name.ToCString());
}

// The bodies of a STEP import being collected.
class StepCollector {
 public:
  explicit StepCollector(const Handle(TDocStd_Document) & document)
      : shapes_(XCAFDoc_DocumentTool::ShapeTool(document->Main())),
        colors_(XCAFDoc_DocumentTool::ColorTool(document->Main())) {}

  StepImportData finish() {
    if (skipped_ > 0) {
      data_.warnings.push_back(rust::String("Skipped " + std::to_string(skipped_) +
                                            " shapes that are not closed solids."));
    }
    if (data_.bodies.empty()) {
      raise_empty_result("the STEP file contains no solids");
    }
    return std::move(data_);
  }

  // Walks a free shape depth first, applying assembly locations.
  void walk(const TDF_Label &label, const TopLoc_Location &location) {
    if (shapes_->IsAssembly(label)) {
      TDF_LabelSequence components;
      XCAFDoc_ShapeTool::GetComponents(label, components);
      for (int position = 1; position <= components.Length(); ++position) {
        const TDF_Label &component = components.Value(position);
        TDF_Label referred;
        if (XCAFDoc_ShapeTool::GetReferredShape(component, referred)) {
          walk(referred, location * XCAFDoc_ShapeTool::GetLocation(component));
        }
      }
      return;
    }
    add_part(label, location);
  }

 private:
  // The surface color, else the generic color, of a shape or label.
  template <class Key>
  bool find_color(const Key &key, Quantity_Color &color) const {
    return colors_->GetColor(key, XCAFDoc_ColorSurf, color) ||
           colors_->GetColor(key, XCAFDoc_ColorGen, color);
  }

  // Adds every solid of a part, turning closed shells into solids and counting the rest.
  void add_part(const TDF_Label &label, const TopLoc_Location &location) {
    const TopoDS_Shape part = XCAFDoc_ShapeTool::GetShape(label);
    const std::string name = label_name(label);
    Quantity_Color part_color;
    const bool part_colored = find_color(label, part_color);
    for (TopExp_Explorer solids(part, TopAbs_SOLID); solids.More(); solids.Next()) {
      Quantity_Color color = part_color;
      const bool colored = find_color(solids.Current(), color) || part_colored;
      add_body(solids.Current(), location, name, colored ? &color : nullptr);
    }
    for (TopExp_Explorer shells(part, TopAbs_SHELL, TopAbs_SOLID); shells.More(); shells.Next()) {
      const TopoDS_Shell &shell = TopoDS::Shell(shells.Current());
      if (!BRep_Tool::IsClosed(shell)) {
        ++skipped_;
        continue;
      }
      BRepBuilderAPI_MakeSolid maker(shell);
      ShapeFix_Solid fixer(maker.Solid());
      fixer.Perform();
      add_body(fixer.Solid(), location, name, part_colored ? &part_color : nullptr);
    }
    const TopAbs_ShapeEnum loose[][2] = {{TopAbs_FACE, TopAbs_SHELL},
                                         {TopAbs_WIRE, TopAbs_FACE},
                                         {TopAbs_EDGE, TopAbs_WIRE},
                                         {TopAbs_VERTEX, TopAbs_EDGE}};
    for (const auto &[type, owner] : loose) {
      for (TopExp_Explorer explorer(part, type, owner); explorer.More(); explorer.Next()) {
        ++skipped_;
      }
    }
  }

  // Repairs a solid and adds it as a body (kernel spec section 3.8.1, read item 3).
  void add_body(const TopoDS_Shape &solid, const TopLoc_Location &location,
                const std::string &name, const Quantity_Color *color) {
    ShapeFix_Shape fixer(solid);
    fixer.SetPrecision(kLinear);
    fixer.Perform();
    for (TopExp_Explorer solids(fixer.Shape(), TopAbs_SOLID); solids.More(); solids.Next()) {
      const TopoDS_Shape body = solids.Current().Moved(location);
      if (!BRepCheck_Analyzer(body).IsValid()) {
        data_.warnings.push_back(rust::String("Solid " + std::to_string(data_.bodies.size() + 1) +
                                              " is not valid after repair; booleans with it "
                                              "may fail."));
      }
      double red = 0.0, green = 0.0, blue = 0.0;
      if (color != nullptr) {
        color->Values(red, green, blue, Quantity_TOC_sRGB);
      }
      data_.bodies.push_back(
          StepBodyData{make_shape(body), rust::String(name), color != nullptr, red, green, blue});
    }
  }

  Handle(XCAFDoc_ShapeTool) shapes_;
  Handle(XCAFDoc_ColorTool) colors_;
  StepImportData data_;
  std::size_t skipped_ = 0;
};

// The first bytes of every stream `shape_to_brep` writes: BinTools_ShapeSet::Write starts with
// an empty line and the line naming format version 4.
std::string brep_header() {
  return std::string("\n") +
         BinTools_ShapeSetBase::THE_ASCII_VERSIONS[BinTools_FormatVersion_VERSION_4] + "\n";
}

}  // namespace

std::unique_ptr<std::string> shape_to_brep(const Shape &shape) {
  std::ostringstream stream(std::ios::out | std::ios::binary);
  BinTools::Write(shape.get(), stream, Standard_False, Standard_False,
                  BinTools_FormatVersion_VERSION_4);
  if (!stream) {
    raise_not_done("BinTools::Write could not write the shape");
  }
  return std::make_unique<std::string>(stream.str());
}

std::shared_ptr<Shape> shape_from_brep(rust::Slice<const std::uint8_t> bytes) {
  // BinTools::Read prints to standard output, past the messenger, when it finds no version
  // line, so bytes `shape_to_brep` did not write are rejected first (kernel spec section 3.9,
  // item 7).
  const std::string header = brep_header();
  if (bytes.size() < header.size() ||
      std::memcmp(bytes.data(), header.data(), header.size()) != 0) {
    raise_read_error("the bytes do not start with the BRep format version 4 header");
  }
  std::istringstream stream(
      std::string(reinterpret_cast<const char *>(bytes.data()), bytes.size()),
      std::ios::in | std::ios::binary);
  TopoDS_Shape shape;
  try {
    BinTools::Read(shape, stream);
  } catch (const Standard_Failure &failure) {
    raise_read_error(std::string("BinTools::Read failed: ") + failure.GetMessageString());
  }
  if (shape.IsNull() || stream.bad()) {
    raise_read_error("BinTools::Read found no shape");
  }
  return make_shape(shape);
}

std::unique_ptr<std::string> write_step(rust::Slice<const StepBodyData> bodies) {
  initialize();
  const std::lock_guard<std::mutex> lock(exchange_mutex());
  const Handle(TDocStd_Document) document = new_document();
  const Handle(XCAFDoc_ShapeTool) shapes = XCAFDoc_DocumentTool::ShapeTool(document->Main());
  const Handle(XCAFDoc_ColorTool) colors = XCAFDoc_DocumentTool::ColorTool(document->Main());
  for (const StepBodyData &body : bodies) {
    if (!body.shape) {
      Standard_NullObject::Raise("a STEP body has no shape");
    }
    // A copy, so writing never touches the body's shape.
    const TDF_Label label = shapes->AddShape(located_copy(body.shape->get()), Standard_False);
    const std::string name(body.name);
    TDataStd_Name::Set(label, TCollection_ExtendedString(name.c_str(), Standard_True));
    if (body.has_color) {
      colors->SetColor(label, Quantity_Color(body.red, body.green, body.blue, Quantity_TOC_sRGB),
                       XCAFDoc_ColorSurf);
    }
  }

  DESTEP_Parameters parameters;
  parameters.WriteSchema = DESTEP_Parameters::WriteMode_StepSchema_AP214IS;
  parameters.WriteUnit = UnitsMethods_LengthUnit_Millimeter;
  parameters.WriteName = true;
  parameters.WriteColor = true;
  STEPCAFControl_Writer writer;
  writer.SetNameMode(Standard_True);
  writer.SetColorMode(Standard_True);
  if (!writer.Transfer(document, parameters, STEPControl_AsIs)) {
    raise_not_done("STEPCAFControl_Writer could not transfer the bodies");
  }
  set_header(writer.ChangeWriter().Model());
  std::ostringstream stream;
  if (writer.WriteStream(stream) != IFSelect_RetDone) {
    raise_not_done("STEPCAFControl_Writer could not write the stream");
  }
  return std::make_unique<std::string>(stream.str());
}

StepImportData read_step(rust::Slice<const std::uint8_t> bytes) {
  initialize();
  const std::lock_guard<std::mutex> lock(exchange_mutex());
  std::istringstream stream(
      std::string(reinterpret_cast<const char *>(bytes.data()), bytes.size()));
  DESTEP_Parameters parameters;
  parameters.ReadName = true;
  parameters.ReadColor = true;
  STEPCAFControl_Reader reader;
  reader.SetNameMode(Standard_True);
  reader.SetColorMode(Standard_True);
  if (reader.ChangeReader().ReadStream("partsmith", parameters, stream) != IFSelect_RetDone) {
    raise_read_error("STEPCAFControl_Reader could not parse the bytes");
  }
  if (reader.NbRootsForTransfer() == 0) {
    raise_empty_result("the STEP file has nothing to transfer");
  }
  const Handle(TDocStd_Document) document = new_document();
  if (!reader.Transfer(document)) {
    raise_not_done("STEPCAFControl_Reader could not transfer the file");
  }
  StepCollector collector(document);
  TDF_LabelSequence roots;
  XCAFDoc_DocumentTool::ShapeTool(document->Main())->GetFreeShapes(roots);
  for (int position = 1; position <= roots.Length(); ++position) {
    collector.walk(roots.Value(position), TopLoc_Location());
  }
  return collector.finish();
}

}  // namespace occt
}  // namespace partsmith
