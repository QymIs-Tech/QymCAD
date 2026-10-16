#pragma once
// The C-ABI bridge to OpenCASCADE: a STEP import gives a set of BODIES (one per solid), and each body is
// a triangle mesh plus a split of those triangles by B-rep face (contiguous runs).

#include <functional>
#include <map>
#include <set>
#include <algorithm>
#include <cstdio>
#include <cstdio>
#include <TopoDS_Iterator.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <XCAFApp_Application.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <XCAFDoc_ColorTool.hxx>
#include <TDF_Tool.hxx>
#include <map>
#include <TDocStd_Document.hxx>
#include <TDataStd_Name.hxx>
#include <TDF_LabelSequence.hxx>
#include <Quantity_Color.hxx>
#include <TopLoc_Location.hxx>
#include <string>
#include <STEPControl_Writer.hxx>
#include <mutex>
#include <IGESControl_Reader.hxx>
#include <IGESControl_Writer.hxx>
#include <IGESControl_Controller.hxx>
#include <IGESData_IGESModel.hxx>
#include <IGESData_IGESEntity.hxx>
#include <IGESBasic_SingularSubfigure.hxx>
#include <IGESBasic_SubfigureDef.hxx>
#include <IGESBasic_Group.hxx>
#include <IGESBasic_Name.hxx>
#include <Interface_EntityIterator.hxx>
#include <IGESSolid_ManifoldSolid.hxx>
#include <IGESSolid_Shell.hxx>
#include <IGESSolid_Face.hxx>
#include <IGESGraph_Color.hxx>
#include <gp_GTrsf.hxx>
#include <TCollection_HAsciiString.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <Interface_Static.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <IMeshTools_Parameters.hxx>
#include <BRepTools.hxx>
#include <BinTools.hxx>
#include <BinTools_FormatVersion.hxx>
#include <BRep_Tool.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <TopExp_Explorer.hxx>
#include <TopAbs.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>
#include <TopLoc_Location.hxx>
#include <Poly_Triangulation.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeSweep.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepAlgoAPI_Defeaturing.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepAlgoAPI_Section.hxx>
#include <BRepFeat_SplitShape.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <Geom_Plane.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <Geom_BezierSurface.hxx>
#include <TColgp_Array2OfPnt.hxx>
#include <TopoDS_Shell.hxx>
#include <Precision.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <ShapeFix_Shell.hxx>
#include <BOPAlgo_Builder.hxx>
#include <NCollection_IncAllocator.hxx>
#include <BOPAlgo_BuilderFace.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <Geom2d_Curve.hxx>
#include <BRepTools_ReShape.hxx>
#include <BRepTools_Modification.hxx>
#include <BRepOffset_MakeSimpleOffset.hxx>
#include <BRepTools_Modifier.hxx>
#include <ShapeFix_ShapeTolerance.hxx>
#include <ShapeFix_FixSmallFace.hxx>
#include <Geom_Circle.hxx>
#include <BRepLib.hxx>
#include <ShapeFix_Edge.hxx>
#include <Geom_Curve.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <Geom2dConvert.hxx>
#include <BSplCLib.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <TopTools_MapOfShape.hxx>
#include <TopoDS_Compound.hxx>
#include <ShapeFix_Solid.hxx>
#include <ShapeFix_Shape.hxx>
#include <ShapeFix_Face.hxx>
#include <ShapeAnalysis_Surface.hxx>
#include <ShapeAnalysis_Curve.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_SurfaceOfRevolution.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_BSplineSurface.hxx>
#include <GeomConvert.hxx>
#include <Geom2d_Curve.hxx>
#include <functional>
#include <ShapeAnalysis_FreeBounds.hxx>
#include <TopTools_HSequenceOfShape.hxx>
#include <ShapeBuild_ReShape.hxx>
#include <ShapeUpgrade_ShapeDivideClosed.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepFill.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeTorus.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <gp_Circ.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <OSD_ThreadPool.hxx>
#include <atomic>
#include <thread>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepTools_History.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepCheck_Result.hxx>
#include <BRepCheck_ListOfStatus.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakeFilling.hxx>
#include <BRepOffsetAPI_MakePipe.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <Law_Interpol.hxx>
#include <Law_Function.hxx>
#include <TColgp_Array1OfPnt2d.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepOffsetAPI_DraftAngle.hxx>
#include <BRepAdaptor_CompCurve.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom2d_Line.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <BRepLib.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <gp_Pnt2d.hxx>
#include <TopTools_ListOfShape.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <TopTools_ListIteratorOfListOfShape.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <TopExp.hxx>
#include <TopTools_IndexedMapOfShape.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <GeomLProp_SLProps.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <TopTools_DataMapOfShapeInteger.hxx>
#include <TopTools_DataMapIteratorOfDataMapOfShapeInteger.hxx>
#include <TransferBRep.hxx>
#include <Transfer_TransientProcess.hxx>
#include <XSControl_TransferReader.hxx>
#include <XSControl_WorkSession.hxx>
#include <TopTools_ListIteratorOfListOfShape.hxx>
#include <BRepAlgoAPI_BooleanOperation.hxx>
#include <BRepBuilderAPI_MakeShape.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Wire.hxx>
#include <Geom_Curve.hxx>
#include <Standard_Failure.hxx>
#include <Standard_ErrorHandler.hxx>
#include <OSD.hxx>
#include <gp_Pnt.hxx>
#include <gp_Vec.hxx>
#include <gp_Ax1.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Dir.hxx>
#include <gp_Pln.hxx>
#include <gp_Trsf.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <GeomAbs_SurfaceType.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Cone.hxx>
#include <Bnd_Box.hxx>
#include <BRepBndLib.hxx>
#include <GProp_GProps.hxx>
#include <BRepGProp.hxx>

#include <vector>
#include <cstdint>
#include <cstring>
#include <sstream>
#include <unordered_map>
#include <unordered_set>
#include <cmath>
#include <algorithm>
#include <climits>


struct QymBody {
    std::vector<float> verts;     // x, y, z in a row
    std::vector<float> norms;     // the SURFACE's normal at every vertex, x, y, z in a row, facing out of the body: the
                                  // light of a smooth face is shaded by it rather than by an average of its triangles
    std::vector<uint32_t> tris;   // indices, three per triangle
    std::vector<uint32_t> fstart; // the first triangle of each B-rep face
    std::vector<uint32_t> fcount; // how many triangles the face has
    std::vector<uint32_t> fid;    // the face's PERSISTENT id (0 = unknown, or found by mesh detection), parallel to fstart
    std::vector<double> fanchor;  // the face's EXACT anchor, seven values per face: px, py, pz, nx, ny, nz, is_plane.
                                  // Taken from the B-rep SURFACE, not from the tessellation: a sketch on a face
                                  // then lands exactly. Otherwise bosses drifted by about 1e-5, unify would not
                                  // merge the seams, and fillets died.
};

/// EVERY BOOLEAN RUNS ON ALL THE CORES, and this is the one place that says so.
///
/// `BRepAlgoAPI_*` and `BOPAlgo_*` descend from `BOPAlgo_Options`, and its `SetRunParallel` divides the most
/// expensive phase of a boolean - intersecting the faces of the arguments - between threads. Measured before
/// the flag: a boolean takes 81 % of the rebuild of the scenario document and
/// 99 % of a plate with two hundred holes cut in one node.
///
/// The flag has to be set BEFORE the work, so a boolean written as an expression - `BRepAlgoAPI_Cut(a, b)` -
/// cannot take it at all: its constructor has already done the job. That is why every boolean here is written
/// as an object, and why `qym_boolean` below exists - so that a ninth one cannot appear without the flag.
/// WHETHER THE BOOLEANS RUN ON SEVERAL CORES, and on how many. Set from outside, read here.
///
/// It is a switch rather than a constant for two reasons. A check has to build the same document both ways and
/// compare the geometry - a parallel boolean that quietly returns something slightly different is the danger
/// this whole change carries, and it cannot be proved absent without the "off" side. And a person whose
/// machine is busy with something else needs a way to give the kernel fewer cores than it has.
extern std::atomic<bool> g_parallel;

/// The base `BOPAlgo_Options` is not accessible in every algorithm of this kernel, while the method itself is
/// - hence a template over the type rather than a reference to the base.
template <class Algo>
inline void qym_configure(Algo& algo) {
    algo.SetRunParallel(g_parallel.load() ? Standard_True : Standard_False);
}

/// ONE BOOLEAN OVER TWO SHAPES: the arguments, the flag, the work. True means there is a result to take.
///
/// The algorithm itself stays with the caller: the names of the faces are carried over through its history
/// (`carry_ids`), and a helper that returned only the shape would take that away.
template <class Algo>
bool qym_boolean(Algo& algo, const TopoDS_Shape& base, const TopoDS_Shape& tool) {
    TopTools_ListOfShape args, tools;
    args.Append(base);
    tools.Append(tool);
    algo.SetArguments(args);
    algo.SetTools(tools);
    qym_configure(algo);
    algo.Build();
    return algo.IsDone() && !algo.Shape().IsNull();
}

/// THE SAME OVER LISTS: one base and many tools in a single operation, which is how a grid of holes is cut.
template <class Algo>
bool qym_boolean_many(Algo& algo, const TopTools_ListOfShape& args, const TopTools_ListOfShape& tools) {
    algo.SetArguments(args);
    algo.SetTools(tools);
    qym_configure(algo);
    algo.Build();
    return algo.IsDone() && !algo.Shape().IsNull();
}

struct QymDoc {
    std::vector<QymBody> bodies;
    // HOW MANY FACES CAME OUT WITHOUT A TRIANGULATION. A face that did not mesh is skipped without a word and
    // leaves a hole in the shell - you look through the part into its inside. Reported with a screenshot on an
    // imported engine; measured there as 399 bodies out of 1296 with an unclosed shell.
    size_t faces_total = 0;
    size_t faces_unmeshed = 0;
    // the faces the tessellation had to mesh: those that came to it with no triangulation of their own
    size_t faces_meshed = 0;
};

struct QymShape {
    TopoDS_Shape shape;
    TopTools_DataMapOfShapeInteger fids; // the persistent ids of FACES
    TopTools_DataMapOfShapeInteger eids; // the persistent ids of EDGES
    // A FACE SPLIT: the operation cut one face into several pieces. The first keeps the source's name and
    // the kernel gives the rest a positional number — but what should name them is their ORIGIN: "piece k
    // of face N". The name is assembled on the Rust side (it is the one that knows the naming scheme), so
    // all that is recorded here is: face -> the source's name, and face -> the piece number.
    TopTools_DataMapOfShapeInteger fsplit_of;
    TopTools_DataMapOfShapeInteger fsplit_idx;
    // The ABSORPTION of names when coplanar faces merge: "the former name -> the shared face's name".
    std::vector<std::pair<unsigned, unsigned>> absorbed;
    // A FREE EDGE OF A SHEET -> THE NAME OF THE FACE BESIDE IT IN THE BODY IT WAS TAKEN FROM. A copied or offset face
    // has edges with one face only, and an edge is named after the pair of its faces: the face that stood across its
    // source edge is the second of the pair. Filled where a sheet is cut out of a body, read when its edges are named.
    TopTools_DataMapOfShapeInteger ebeside;
};

/// Note, for every edge of `taken` (a face cut out of the body `s` as `source`, the same edges in the same order), the
/// name of the face across its source edge in `s`, into `ebeside`. Nothing is noted where the order does not hold.
void note_beside(const QymShape* s, const TopoDS_Shape& source, struct QymHistory& made, TopTools_DataMapOfShapeInteger& ebeside);

// AN ASSEMBLY AS ITS FILE BUILDS IT: every subassembly and part a node - its name in UTF-8, where it stands in its
// parent, its colour where the file gives one - and every part's solids an index into the list of solids read.
#include <array>

struct QymTree {
    struct Node {
        int64_t parent = -1;
        std::string name;
        double place[12] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0};
        int64_t repeat_of = -1; // an occurrence of a product met before: the body of its first occurrence
        int64_t solid = -1;
        bool has_color = false;
        float rgb[3] = {0, 0, 0};
        std::vector<std::pair<int, std::array<float, 3>>> faces; // colours of single faces: (persistent face id, sRGB)
    };
    std::vector<Node> nodes;
};

// Read a STEP file with its structure (see the definition); the solids come out where the file puts them, in the order
// the tree meets them. `false` when the file does not read or holds no shape.
bool step_tree(const char* path, QymTree& tree, std::vector<TopoDS_Shape>& solids);
bool iges_tree(const char* path, QymTree& tree, std::vector<TopoDS_Shape>& solids);

struct QymShapeList {
    std::vector<TopoDS_Shape> shapes;
    /// FINISHED bodies with the face and edge names already carried over. Filled where the names are
    /// known already (splitting a body), and then `shapes` is not used: seeding names anew would lose the
    /// references of every fillet and chamfer in all the pieces at once.
    std::vector<QymShape*> named;
};

// -- WHAT THE PARTS OF THE BRIDGE SHARE -----------------------------------------------------------
//
// The helpers below are used from more than one of the bridge's files. Everything else stays `static` in the
// file that needs it: a name visible across the whole bridge is a name somebody has to keep in mind.

/// The bit that marks a STRUCTURAL name (one derived from the recipe) apart from a positional one.
static const int QYM_NAMED = 0x40000000;
/// A ROLE AWAITING ITS NAME: the number the bridge hands a face an operation makes, until the table of the document
/// names it when the result lands (`names::ROLE` on the Rust side).
static const int QYM_ROLE = 0x10000000;

/// A NAME BY RECIPE: a name of the document, or a role awaiting its name. Everything else is a positional number - a
/// place in the walk of a result, meaningful inside its own body only.
inline bool qym_recipe(int id) { return (id & (QYM_NAMED | QYM_ROLE)) != 0; }

void why(const char* where, const char* what);

/// The tail of a `try` that ends in a refusal: whatever kind of exception came out, its words are kept.
///
/// OCCT throws `Standard_Failure` with a message that names the actual trouble ("BRepAlgoAPI: the operands
/// are self-intersecting" and the like); 71 places in this file used to catch it as `...` and drop every word
/// of it on the floor.
/// A FAULT INSIDE OCCT IS A REFUSAL, NOT THE END OF THE PROGRAM. A rounding of R9.9999998 on the arc of an R10 rounding
/// ending in the corners of two chamfers that ate their faces reads memory it does not own inside `BRepFilletAPI` and
/// took the whole process down with it. With the handlers of `OSD` set (`qym_signals_here`) and a jump point in every
/// `try` of the bridge, such a fault comes back as a `Standard_Failure` the `catch` beside it turns into words.
void qym_signals_here();
#define QYM_SIGNALS                                                      \
    qym_signals_here();                                                  \
    OCC_CATCH_SIGNALS

#define QYM_WHY_CATCH(where)                                            \
    catch (Standard_Failure const& e) { why(where, e.GetMessageString()); } \
    catch (std::exception const& e) { why(where, e.what()); }           \
    catch (...) { why(where, nullptr); }

extern "C" void qym_why_clear();
extern "C" const char* qym_why();

/// What kind of body this is - a solid, a sheet, an empty shape - asked for from more than one part.
extern "C" int qym_shape_kind(const QymShape* s);

// -- THE DOOR OF THE BRIDGE ON THE C++ SIDE -----------------------------------------------------------------------
//
// An operation brings its algorithm and says what its result is made of; the names of its faces and edges are carried
// here, once for all of them: through the history of the algorithm, from every operand by the rule of that operand,
// with the pieces of a split face recorded, the faces it generates named by their roles, the free edges of a sheet kept
// beside the face they stood against, and whatever the history is silent about numbered for the passes after it.

/// WHAT BECAME OF AN ELEMENT in an algorithm: the one question the carrying of names asks. OCCT answers it in three
/// ways - a builder of shapes, the history of the low-level booleans, the record of a repair - and each has its
/// translation; the carrying does not know which one it is reading.
struct QymHistory {
    virtual ~QymHistory() = default;
    virtual const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) = 0;
    virtual const TopTools_ListOfShape& Generated(const TopoDS_Shape& s) = 0;
    /// the element left the result and nothing took its place
    virtual bool Removed(const TopoDS_Shape& s) = 0;
};

/// The history of a builder of shapes: every algorithm of `BRepAlgoAPI`, `BRepFilletAPI`, `BRepOffsetAPI`,
/// `BRepPrimAPI` and `BRepBuilderAPI`.
struct QymBuilderHistory : QymHistory {
    explicit QymBuilderHistory(BRepBuilderAPI_MakeShape& a) : algo(a) {}
    const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) override { return algo.Modified(s); }
    const TopTools_ListOfShape& Generated(const TopoDS_Shape& s) override { return algo.Generated(s); }
    bool Removed(const TopoDS_Shape& s) override { return algo.IsDeleted(s); }
    BRepBuilderAPI_MakeShape& algo;
};

/// The history the low-level algorithms hand over (`BRepTools_History`): the merging of a monolith
/// (`ShapeUpgrade_UnifySameDomain`) and the `BOPAlgo` builders. An algorithm that kept no history changed nothing.
struct QymToolsHistory : QymHistory {
    explicit QymToolsHistory(const Handle(BRepTools_History)& h) : hist(h) {}
    const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) override { return hist.IsNull() ? none : hist->Modified(s); }
    const TopTools_ListOfShape& Generated(const TopoDS_Shape& s) override { return hist.IsNull() ? none : hist->Generated(s); }
    bool Removed(const TopoDS_Shape& s) override { return !hist.IsNull() && hist->IsRemoved(s); }
    Handle(BRepTools_History) hist;
    TopTools_ListOfShape none;
};

/// WHAT A REPAIR IS ASKED OF AN ELEMENT: the replacement its record holds for the element itself (`Value`), or the
/// element rebuilt from the replacements of everything inside it (`Apply`).
enum class QymRepairAsk { Value, Apply };

/// The record a repair keeps (`ShapeBuild_ReShape`: `ShapeFix_*`, `ShapeUpgrade_*`): the images of an element are the
/// pieces of its type in what replaced it, none when it stayed itself; it is removed when nothing replaced it. A
/// repair generates nothing.
struct QymRepairHistory : QymHistory {
    QymRepairHistory(const Handle(BRepTools_ReShape)& c, QymRepairAsk a) : ctx(c), ask(a) {}
    /// What stands for `s` after the repair: itself when the record holds nothing for it.
    TopoDS_Shape Image(const TopoDS_Shape& s) {
        if (ctx.IsNull()) return s;
        return ask == QymRepairAsk::Apply ? ctx->Apply(s) : ctx->Value(s);
    }
    const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) override {
        images.Clear();
        const TopoDS_Shape now = Image(s);
        if (now.IsNull() || now.IsSame(s)) return images;
        for (TopExp_Explorer ex(now, s.ShapeType()); ex.More(); ex.Next()) images.Append(ex.Current());
        return images;
    }
    const TopTools_ListOfShape& Generated(const TopoDS_Shape&) override { return none; }
    bool Removed(const TopoDS_Shape& s) override { return Image(s).IsNull(); }
    Handle(BRepTools_ReShape) ctx;
    QymRepairAsk ask;
    TopTools_ListOfShape images;
    TopTools_ListOfShape none;
};

/// A BINDING CARRIED THROUGH A HISTORY: every element of type `ty` of the shapes `before`, bound to a value, hands it
/// to its images, or keeps it when the algorithm left it as it was; a removed element hands it to none. The walk goes
/// in the order of the shapes, so when two elements become one the first of them wins every time.
void carry_binding(QymHistory& history, const TopTools_ListOfShape& before, TopAbs_ShapeEnum ty, TopTools_DataMapOfShapeInteger& bound);

/// HOW THE NAMES OF AN OPERAND ARE CARRIED. The first operand of an operation - the body it works on - gives all its
/// names and its counter of positional numbers; any other gives only the names its recipe made: a positional number
/// means something only inside its own body, and "face 1" of the tool carried beside "face 1" of the base would glue
/// two faces under one name.
enum class QymCarry { Everything, RecipeOnly };

/// AN OPERAND OF AN OPERATION: its body, how its names are carried, and the word a diagnosis tells it by.
struct QymOperand {
    const QymShape* body;
    QymCarry carry;
    const char* tag;
};

/// THE COUNTERS OF POSITIONAL NUMBERS of a result in the making, faces and edges apart.
struct QymCounters {
    int faces;
    int edges;
};

/// WHAT THE RECIPE NAMES ITSELF in a result in the making - a fillet surface after its edge, the wall of a hole, a
/// moved face as itself. Called after the names of the operands are carried and before what is left gets positional
/// numbers: a name the recipe gives is then never taken by a number first.
using QymRecipe = std::function<void(QymShape& made, QymCounters& next)>;

/// The record of a modifier of shapes (`BRepTools_Modifier`): every element has one image, itself where the modification
/// left it as it was. A modifier removes and generates nothing.
struct QymModifierHistory : QymHistory {
    explicit QymModifierHistory(BRepTools_Modifier& m) : mod(m) {}
    const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) override {
        images.Clear();
        TopoDS_Shape now;
        try {
            QYM_SIGNALS;
            now = mod.ModifiedShape(s);
        } catch (...) {
            return images; // an element the modifier never met has no image of its own
        }
        if (!now.IsNull() && !now.IsSame(s)) images.Append(now);
        return images;
    }
    const TopTools_ListOfShape& Generated(const TopoDS_Shape&) override { return none; }
    bool Removed(const TopoDS_Shape&) override { return false; }
    BRepTools_Modifier& mod;
    TopTools_ListOfShape images;
    TopTools_ListOfShape none;
};

/// A CHANGE THAT KEEPS THE TOPOLOGY ONE FOR ONE AND ANSWERS NO HISTORY: the simple offset of a sheet - a modifier of the
/// geometry, every face and edge moved and none added or lost, whose `Modified` gives nothing (measured: null for every
/// face of a block's top lifted 5, both through `BRepOffsetAPI_MakeOffsetShape` and `BRepOffset_MakeSimpleOffset`). The
/// n-th element of a type in `after` is the image of the n-th in `before`; nothing is answered when the counts differ.
/// A caller holds it only after checking the images: each face of an offset gone the distance.
struct QymTwinHistory : QymHistory {
    QymTwinHistory(const TopoDS_Shape& before, const TopoDS_Shape& after) {
        for (TopAbs_ShapeEnum ty : {TopAbs_FACE, TopAbs_EDGE}) {
            TopTools_IndexedMapOfShape a, b;
            TopExp::MapShapes(before, ty, a);
            TopExp::MapShapes(after, ty, b);
            if (a.Extent() != b.Extent()) continue;
            for (int i = 1; i <= a.Extent(); ++i) twin.Bind(a(i), b(i));
        }
    }
    const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) override {
        images.Clear();
        if (twin.IsBound(s) && !twin.Find(s).IsSame(s)) images.Append(twin.Find(s));
        return images;
    }
    const TopTools_ListOfShape& Generated(const TopoDS_Shape&) override { return none; }
    bool Removed(const TopoDS_Shape&) override { return false; }
    TopTools_DataMapOfShapeShape twin;
    TopTools_ListOfShape images;
    TopTools_ListOfShape none;
};

/// WHERE AN OPERATION MOVES NOTHING, THE PLACE IS ITS HISTORY: an element of `after` lying exactly where one of `before`
/// lay - the same centre and the same length or area, to 1e-6 - is its image; for the rest `rest` answers. For a change
/// that keeps every element where it was and only re-describes some: the seam of a round face turned half a turn, whose
/// cutting and gluing rebuild the seam edge and its neighbours without saying what became of them.
struct QymSamePlaceHistory : QymHistory {
    QymSamePlaceHistory(const TopoDS_Shape& after, QymHistory& r) : rest(r) {
        for (TopAbs_ShapeEnum ty : {TopAbs_FACE, TopAbs_EDGE}) {
            TopTools_IndexedMapOfShape all;
            TopExp::MapShapes(after, ty, all);
            for (int i = 1; i <= all.Extent(); ++i) placed.push_back({ty, Key(all(i)), all(i)});
        }
    }
    struct Spot {
        TopAbs_ShapeEnum ty;
        std::array<double, 4> key;
        TopoDS_Shape shape;
    };
    static std::array<double, 4> Key(const TopoDS_Shape& s) {
        GProp_GProps g;
        if (s.ShapeType() == TopAbs_EDGE) BRepGProp::LinearProperties(s, g); else BRepGProp::SurfaceProperties(s, g);
        const gp_Pnt c = g.CentreOfMass();
        return {c.X(), c.Y(), c.Z(), g.Mass()};
    }
    /// The element of `after` lying where `s` lies; null when none does.
    TopoDS_Shape There(const TopoDS_Shape& s) {
        if (s.ShapeType() != TopAbs_FACE && s.ShapeType() != TopAbs_EDGE) return {};
        const auto k = Key(s);
        for (const Spot& p : placed) {
            if (p.ty != s.ShapeType()) continue;
            bool same = true;
            for (int j = 0; j < 4 && same; ++j) same = std::abs(p.key[j] - k[j]) < 1e-6;
            if (same) return p.shape;
        }
        return {};
    }
    const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) override {
        images.Clear();
        const TopoDS_Shape now = There(s);
        if (now.IsNull()) {
            for (TopTools_ListIteratorOfListOfShape it(rest.Modified(s)); it.More(); it.Next()) images.Append(it.Value());
        } else if (!now.IsSame(s)) {
            images.Append(now);
        }
        return images;
    }
    const TopTools_ListOfShape& Generated(const TopoDS_Shape& s) override { return rest.Generated(s); }
    bool Removed(const TopoDS_Shape& s) override { return There(s).IsNull() && rest.Removed(s); }
    QymHistory& rest;
    std::vector<Spot> placed;
    TopTools_ListOfShape images;
};

/// The record of a sewing (`BRepBuilderAPI_Sewing`): a face or an edge it replaced has that one image, one it left as
/// it was has none; one it found degenerate is removed. A sewing generates nothing.
struct QymSewingHistory : QymHistory {
    explicit QymSewingHistory(BRepBuilderAPI_Sewing& s) : sew(s) {}
    const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) override {
        images.Clear();
        if (sew.IsModified(s)) images.Append(sew.Modified(s));
        else if (sew.IsModifiedSubShape(s)) images.Append(sew.ModifiedSubShape(s));
        return images;
    }
    const TopTools_ListOfShape& Generated(const TopoDS_Shape&) override { return none; }
    bool Removed(const TopoDS_Shape& s) override { return sew.IsDegenerated(s); }
    BRepBuilderAPI_Sewing& sew;
    TopTools_ListOfShape images;
    TopTools_ListOfShape none;
};

/// TWO HISTORIES ONE AFTER THE OTHER: what the first made of an element, then what the second made of that. An element
/// the first left as it was goes into the second as itself; one left as it was by both is not modified.
struct QymChainHistory : QymHistory {
    QymChainHistory(QymHistory& a, QymHistory& b) : first(a), then(b) {}
    /// What the first made of `s`, `s` itself when it left it as it was; empty when it removed it.
    TopTools_ListOfShape Middle(const TopoDS_Shape& s) {
        TopTools_ListOfShape mid;
        for (TopTools_ListIteratorOfListOfShape it(first.Modified(s)); it.More(); it.Next()) mid.Append(it.Value());
        if (mid.IsEmpty() && !first.Removed(s)) mid.Append(s);
        return mid;
    }
    /// What the second made of `m`, `m` itself when it left it as it was.
    void Then(const TopoDS_Shape& m, TopTools_ListOfShape& into) {
        TopTools_ListOfShape now;
        for (TopTools_ListIteratorOfListOfShape it(then.Modified(m)); it.More(); it.Next()) now.Append(it.Value());
        if (now.IsEmpty() && !then.Removed(m)) now.Append(m);
        into.Append(now);
    }
    const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) override {
        images.Clear();
        TopTools_ListOfShape mid = Middle(s);
        for (TopTools_ListIteratorOfListOfShape it(mid); it.More(); it.Next()) Then(it.Value(), images);
        if (images.Extent() == 1 && images.First().IsSame(s)) images.Clear();
        return images;
    }
    const TopTools_ListOfShape& Generated(const TopoDS_Shape& s) override {
        made.Clear();
        TopTools_ListOfShape born;
        for (TopTools_ListIteratorOfListOfShape it(first.Generated(s)); it.More(); it.Next()) born.Append(it.Value());
        for (TopTools_ListIteratorOfListOfShape it(born); it.More(); it.Next()) Then(it.Value(), made);
        TopTools_ListOfShape mid = Middle(s);
        for (TopTools_ListIteratorOfListOfShape it(mid); it.More(); it.Next())
            for (TopTools_ListIteratorOfListOfShape g(then.Generated(it.Value())); g.More(); g.Next()) made.Append(g.Value());
        return made;
    }
    bool Removed(const TopoDS_Shape& s) override {
        TopTools_ListOfShape mid = Middle(s);
        for (TopTools_ListIteratorOfListOfShape it(mid); it.More(); it.Next())
            if (!then.Removed(it.Value())) return false;
        return true;
    }
    QymHistory& first;
    QymHistory& then;
    TopTools_ListOfShape images;
    TopTools_ListOfShape made;
};

/// WHO KEEPS AN IMAGE TWO ELEMENTS BECAME; the other is recorded as yielded to it either way, so a reference to it still
/// finds the element. `First`: the first carried, as an operation hands its elements on - two edges of a base and a tool
/// lying together in a boolean, the base's first; the two long sides of a strip taken out, the first side. `ByName`: in
/// the monolith, coplanar faces glued - a name beats a positional number and the smaller name wins. Measured: the name
/// rule over every operation, the strips included, made a rounding of R2 on a sample leave two of its four corners out.
enum class QymMerge { First, ByName };

/// WHAT THE RESULT OF AN OPERATION IS MADE OF.
struct QymResultSpec {
    std::vector<QymOperand> operands;
    /// The names of the faces an operation generates, by the name of the face they come from (a wall of a shell after
    /// its outer face); null where the generated face continues its source.
    const std::map<int, int>* generated = nullptr;
    /// Merge the faces a boolean leaves split along its seam into one (the monolith), recording which names yielded.
    bool unify = false;
    /// What the recipe names itself, between the names carried and the numbers handed out; empty when nothing.
    QymRecipe recipe = {};
    /// Who keeps an image two elements became.
    QymMerge merge = QymMerge::First;
};

/// THE RESULT OF AN OPERATION WITH ITS NAMES, built from `res` by `history` as `spec` says. Null when `res` is null.
QymShape* qym_result(QymHistory& history, const TopoDS_Shape& res, const QymResultSpec& spec);

int next_local(const TopTools_DataMapOfShapeInteger& m);
void fill_unnamed(const TopoDS_Shape& res, TopAbs_ShapeEnum ty, TopTools_DataMapOfShapeInteger& out, int& next);
/// Only the elements of type `ty` that stand in `res` kept in `ids`: the carrying binds a source element too, gone or not.
void keep_standing(const TopoDS_Shape& res, TopAbs_ShapeEnum ty, TopTools_DataMapOfShapeInteger& ids);
void carry_ids(BRepBuilderAPI_MakeShape& algo, const TopoDS_Shape& a, TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& aid, TopTools_DataMapOfShapeInteger& out, int& next, bool named_only = false, TopTools_DataMapOfShapeInteger* splits_of = nullptr, TopTools_DataMapOfShapeInteger* splits_idx = nullptr, const std::map<int,int>* gen_names = nullptr);
void carry_ids(QymHistory& history, const TopoDS_Shape& a, TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& aid, TopTools_DataMapOfShapeInteger& out, int& next, TopTools_DataMapOfShapeInteger* splits_of = nullptr, TopTools_DataMapOfShapeInteger* splits_idx = nullptr);
void classify_unnamed(BRepBuilderAPI_MakeShape& algo, const TopoDS_Shape& src, const QymShape* q, const char* tag, const TopTools_DataMapOfShapeInteger* src_ids = nullptr);
gp_Vec face_normal_vec(const TopoDS_Face& f);
int heal_pinched_faces(TopoDS_Shape& shape, TopTools_DataMapOfShapeInteger& fids, TopTools_DataMapOfShapeInteger& eids, TopTools_DataMapOfShapeInteger& fsplit_of, TopTools_DataMapOfShapeInteger& fsplit_idx);
TopoDS_Shape unify_monolithic(const TopoDS_Shape& in, TopTools_DataMapOfShapeInteger& fids, TopTools_DataMapOfShapeInteger& eids, std::vector<std::pair<unsigned, unsigned>>* absorbed = nullptr, const TopTools_MapOfShape* keep = nullptr);
QymDoc* doc_from_shape(const TopoDS_Shape& shape, double defl, const TopTools_DataMapOfShapeInteger& fids);
QymDoc* doc_from_shape(const TopoDS_Shape& shape, double defl);
/// A BODY NO HISTORY LEADS TO - a file read, a primitive, a sweep of a profile, a plate thickened off a face, a compound
/// put together of pieces each already named: what its recipe names keeps that name, the rest is numbered in the order
/// of the shape, stable for one recipe. Null when `s` is null. The one door of a body with no history to carry names
/// through, as `qym_result` is the one door of a body an algorithm made of others.
QymShape* qym_born(const TopoDS_Shape& s, const QymRecipe& recipe = {});
// The shells a triangle mesh makes, its faces built with their edges shared and neighbours on one plane one face: solids
// of the closed pieces where `solids`, and runs of sides along a patch's border one edge only where `border_runs`. A null
// shape where this way does not come through. `dropped` counts the triangles with no area.
TopoDS_Shape mesh_shells(const double* verts, size_t nv, const uint32_t* tris, size_t nt, bool solids, bool border_runs, size_t* dropped);
// ONE LOCK FOR OCCT'S DATA EXCHANGE, held by every reader and writer of STEP and IGES alike. They share process-wide
// state: the framework each kind sets up the first time a reader or writer of it is made, and the parameters they set
// (`xstep.cascade.unit`). Measured: an IGES reader and a STEP reader made at the same moment died in
// `MoniTool_TypedValue`, SIGSEGV, 1 run of 25. A lock of their own each kept two IGES files apart and two STEP ones,
// never one of each.
std::mutex& xstep_lock();
Handle(Law_Function) runout_law(double z0, double z1, double L, double lin, double lout);
TopoDS_Wire make_helix_wire_seg(const gp_Ax3& axes, double radius, double lead, double z0, double z1, bool left);
TopoDS_Wire thread_profile_wire(const gp_Ax3& axes, double radius, double pitch, double angle_deg, double depth, bool internal, int form, double clearance_crest, double clearance_root);

