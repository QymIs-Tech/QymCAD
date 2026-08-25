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
#include <STEPControl_Writer.hxx>
#include <Interface_Static.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
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
#include <ShapeBuild_ReShape.hxx>
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
    std::vector<uint32_t> tris;   // indices, three per triangle
    std::vector<uint32_t> fstart; // the first triangle of each B-rep face
    std::vector<uint32_t> fcount; // how many triangles the face has
    std::vector<uint32_t> fid;    // the face's PERSISTENT id (0 = unknown, or found by mesh detection), parallel to fstart
    std::vector<double> fanchor;  // the face's EXACT anchor, seven values per face: px, py, pz, nx, ny, nz, is_plane.
                                  // Taken from the B-rep SURFACE, not from the tessellation: a sketch on a face
                                  // then lands exactly. Otherwise bosses drifted by about 1e-5, unify would not
                                  // merge the seams, and fillets died.
};

struct QymDoc {
    std::vector<QymBody> bodies;
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
TopoDS_Shape unify_monolithic(const TopoDS_Shape& in, TopTools_DataMapOfShapeInteger& fids, TopTools_DataMapOfShapeInteger& eids, std::vector<std::pair<unsigned, unsigned>>* absorbed = nullptr);
QymDoc* doc_from_shape(const TopoDS_Shape& shape, double defl, const TopTools_DataMapOfShapeInteger& fids);
QymDoc* doc_from_shape(const TopoDS_Shape& shape, double defl);
QymShape* seeded(const TopoDS_Shape& s);
Handle(Law_Function) runout_law(double z0, double z1, double L, double lin, double lout);
TopoDS_Wire make_helix_wire_seg(const gp_Ax3& axes, double radius, double lead, double z0, double z1, bool left);
TopoDS_Wire thread_profile_wire(const gp_Ax3& axes, double radius, double pitch, double angle_deg, double depth, bool internal, int form, double clearance_crest, double clearance_root);

