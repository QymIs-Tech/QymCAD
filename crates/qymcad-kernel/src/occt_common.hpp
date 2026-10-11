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
};

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

void why(const char* where, const char* what);

/// The tail of a `try` that ends in a refusal: whatever kind of exception came out, its words are kept.
///
/// OCCT throws `Standard_Failure` with a message that names the actual trouble ("BRepAlgoAPI: the operands
/// are self-intersecting" and the like); 71 places in this file used to catch it as `...` and drop every word
/// of it on the floor.
#define QYM_WHY_CATCH(where)                                            \
    catch (Standard_Failure const& e) { why(where, e.GetMessageString()); } \
    catch (std::exception const& e) { why(where, e.what()); }           \
    catch (...) { why(where, nullptr); }

extern "C" void qym_why_clear();
extern "C" const char* qym_why();

/// What kind of body this is - a solid, a sheet, an empty shape - asked for from more than one part.
extern "C" int qym_shape_kind(const QymShape* s);

int next_local(const TopTools_DataMapOfShapeInteger& m);
void seed_ids(const TopoDS_Shape& s, TopAbs_ShapeEnum ty, TopTools_DataMapOfShapeInteger& ids);
void fill_unnamed(const TopoDS_Shape& res, TopAbs_ShapeEnum ty, TopTools_DataMapOfShapeInteger& out, int& next);
void carry_ids(BRepBuilderAPI_MakeShape& algo, const TopoDS_Shape& a, TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& aid, TopTools_DataMapOfShapeInteger& out, int& next, bool named_only = false, TopTools_DataMapOfShapeInteger* splits_of = nullptr, TopTools_DataMapOfShapeInteger* splits_idx = nullptr, const std::map<int,int>* gen_names = nullptr);
void propagate_ids(BRepBuilderAPI_MakeShape& algo, const TopoDS_Shape& a, TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& aid, const TopoDS_Shape& res, TopTools_DataMapOfShapeInteger& out, TopTools_DataMapOfShapeInteger* splits_of = nullptr, TopTools_DataMapOfShapeInteger* splits_idx = nullptr, const std::map<int,int>* gen_names = nullptr);
void copy_ids_by_order(const TopoDS_Shape& src, TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& sid, const TopoDS_Shape& dst, TopTools_DataMapOfShapeInteger& out);
void classify_unnamed(BRepBuilderAPI_MakeShape& algo, const TopoDS_Shape& src, const QymShape* q, const char* tag, const TopTools_DataMapOfShapeInteger* src_ids = nullptr);
gp_Vec face_normal_vec(const TopoDS_Face& f);
int heal_pinched_faces(TopoDS_Shape& shape, TopTools_DataMapOfShapeInteger& fids, TopTools_DataMapOfShapeInteger& eids, TopTools_DataMapOfShapeInteger& fsplit_of, TopTools_DataMapOfShapeInteger& fsplit_idx);
TopoDS_Shape unify_monolithic(const TopoDS_Shape& in, TopTools_DataMapOfShapeInteger& fids, TopTools_DataMapOfShapeInteger& eids, std::vector<std::pair<unsigned, unsigned>>* absorbed = nullptr, const TopTools_MapOfShape* keep = nullptr);
QymDoc* doc_from_shape(const TopoDS_Shape& shape, double defl, const TopTools_DataMapOfShapeInteger& fids);
QymDoc* doc_from_shape(const TopoDS_Shape& shape, double defl);
QymShape* seeded(const TopoDS_Shape& s);
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

