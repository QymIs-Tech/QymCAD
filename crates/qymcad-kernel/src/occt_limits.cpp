// THE LIMITING CASE OF A BLEND, BUILT EXACTLY. A chamfer or a fillet whose size eats a face whole - a leg as high as
// the side it lies on, a radius as high as the step it rounds - is refused by OCCT at its size: the blend surface
// lands exactly on the far edge of the face and the algorithm meets the degenerate strip it would leave. The door
// builds it a hair smaller and takes the strip out, and the result is off the drawing by the step (measured: 7e-6 to
// 1.2e-2 mm^3 on blocks of 4e3 to 1.2e4 mm^3).
//
// Where every edge is a straight line between two planar faces meeting convexly, the blend is a prism: its section,
// across the edge, is the corner the blend takes off - a triangle for a chamfer, the corner less a circle for a
// fillet - and it runs the length of the edge. That prism, carried past both ends, is cut from the body: one boolean,
// exact to the drawing, the eaten face gone with no strip. Past the ends of an edge whose end faces bound the body
// the prism meets nothing; where it would (an edge ending against a wall), a face of the prism other than the blend's
// is left in the body, and the construction refuses: what bounds the material it took off is then not the blend alone.
#include "occt_common.hpp"
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <GC_MakeArcOfCircle.hxx>

namespace {

/// AN EDGE AND ITS TWO FACES AS THE BLEND SEES THEM: where it runs, and the directions across it - into each face, and
/// out of each face.
struct Corner {
    gp_Pnt start;
    gp_Dir along;
    double length;
    gp_Vec into_first, into_second;
    gp_Vec out_first, out_second;
    TopoDS_Face first, second;
};

/// WHETHER THE BLEND LANDS ON ITS FACES: the point `reach` off the edge, across it into `face`, at its start, its middle
/// and its end, is on the face - inside it or on its boundary. A size past the face is no limiting case but a blend
/// rolling over the edge of the next one, which OCCT refuses and this construction does not build either (measured: a
/// chamfer of 10.5 on a side 10 high came in green, the bottom of the block cut).
bool lands_on(const Corner& c, const TopoDS_Face& face, const gp_Vec& reach) {
    for (double t : {0.0, 0.5, 1.0}) {
        gp_Pnt at = c.start.Translated(gp_Vec(c.along) * (c.length * t)).Translated(reach);
        BRepClass_FaceClassifier cls(face, at, 1e-6);
        if (cls.State() == TopAbs_OUT) return false;
    }
    return true;
}

/// The outward normal of a planar face, or nothing where the face is not a plane.
bool plane_out(const TopoDS_Face& f, gp_Vec& out) {
    BRepAdaptor_Surface s(f, Standard_False);
    if (s.GetType() != GeomAbs_Plane) return false;
    gp_Dir n = s.Plane().Axis().Direction();
    if (f.Orientation() == TopAbs_REVERSED) n.Reverse();
    out = gp_Vec(n);
    return true;
}

/// The direction across `edge`, lying in `face`, that goes into the face: of the two, the one whose point a little way
/// off the middle of the edge the face holds.
bool into_face(const TopoDS_Face& face, const gp_Vec& out, const gp_Dir& along, const gp_Pnt& middle, double reach, gp_Vec& into) {
    gp_Vec across = out.Crossed(gp_Vec(along));
    if (across.Magnitude() < 1e-12) return false;
    across.Normalize();
    for (double sign : {1.0, -1.0}) {
        gp_Pnt probe = middle.Translated(across * (sign * reach));
        BRepClass_FaceClassifier cls(face, probe, 1e-7);
        if (cls.State() == TopAbs_IN) {
            into = across * sign;
            return true;
        }
    }
    return false;
}

/// The corner of `edge` in `s`, the first face chosen as the asymmetric chamfer chooses its reference - the first of
/// the two by the walk, the second with `flip`, the face named `ref_face` where it is one of them - or nothing where
/// the edge is not a straight line between two planar faces meeting convexly.
bool corner_of(const QymShape* s, const TopoDS_Edge& edge, const TopTools_IndexedDataMapOfShapeListOfShape& faces_of, int flip, uint32_t ref_face, Corner& c) {
    BRepAdaptor_Curve curve(edge);
    if (curve.GetType() != GeomAbs_Line || !faces_of.Contains(edge)) return false;
    const TopTools_ListOfShape& faces = faces_of.FindFromKey(edge);
    if (faces.Extent() != 2) return false;
    TopoDS_Face first = TopoDS::Face(faces.First()), second = TopoDS::Face(faces.Last());
    if (flip) std::swap(first, second);
    if (ref_face != 0 && s->fids.IsBound(second) && (uint32_t)s->fids.Find(second) == ref_face) std::swap(first, second);
    c.first = first;
    c.second = second;
    gp_Pnt a = curve.Value(curve.FirstParameter()), b = curve.Value(curve.LastParameter());
    c.length = a.Distance(b);
    if (c.length < 1e-6) return false;
    c.start = a;
    c.along = gp_Dir(gp_Vec(a, b));
    if (!plane_out(first, c.out_first) || !plane_out(second, c.out_second)) return false;
    gp_Pnt middle = a.Translated(gp_Vec(a, b) * 0.5);
    const double reach = std::min(1e-3, c.length * 1e-3);
    if (!into_face(first, c.out_first, c.along, middle, reach, c.into_first)) return false;
    if (!into_face(second, c.out_second, c.along, middle, reach, c.into_second)) return false;
    // convex: going into either face goes into the body beside the other
    return c.into_first.Dot(c.out_second) < -1e-9 && c.into_second.Dot(c.out_first) < -1e-9;
}

/// THE PRISM A BLEND TAKES OFF along the corner `c`: its section laid out from `near` on the first face to `far` on the
/// second - straight for a chamfer, an arc for a fillet of `radius` through `centre` - and closed outside the body, the
/// prism carried `past` beyond both ends. The face the section's blend side sweeps is given back in `surface`.
struct Section {
    gp_Vec near, far;
    bool arc;
    gp_Vec centre;
    double radius;
};

TopoDS_Shape prism_of(const Corner& c, const Section& sec, double past, TopoDS_Face& surface) {
    const gp_Pnt origin = c.start.Translated(gp_Vec(c.along) * -past);
    auto at = [&](const gp_Vec& v) { return origin.Translated(v); };
    const double out = 2.0 * (sec.near.Magnitude() + sec.far.Magnitude()) + 1.0;
    const gp_Vec p1 = sec.near, p2 = sec.near + c.out_first * out, p3 = (c.out_first + c.out_second) * out, p4 = sec.far + c.out_second * out, p5 = sec.far;
    BRepBuilderAPI_MakeWire wire;
    wire.Add(BRepBuilderAPI_MakeEdge(at(p1), at(p2)).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(at(p2), at(p3)).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(at(p3), at(p4)).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(at(p4), at(p5)).Edge());
    TopoDS_Edge side;
    if (sec.arc) {
        gp_Vec toward = (gp_Vec(0, 0, 0) - sec.centre).Normalized();
        const gp_Vec middle = sec.centre + toward * sec.radius;
        GC_MakeArcOfCircle arc(at(p5), at(middle), at(p1));
        if (!arc.IsDone()) return TopoDS_Shape();
        side = BRepBuilderAPI_MakeEdge(arc.Value()).Edge();
    } else {
        side = BRepBuilderAPI_MakeEdge(at(p5), at(p1)).Edge();
    }
    wire.Add(side);
    if (!wire.IsDone()) return TopoDS_Shape();
    // the wire joins the edges at shared vertices and may hold a copy of the one added: the prism sweeps the copy
    side = wire.Edge();
    BRepBuilderAPI_MakeFace face(wire.Wire(), Standard_True);
    if (!face.IsDone()) return TopoDS_Shape();
    BRepPrimAPI_MakePrism prism(face.Face(), gp_Vec(c.along) * (c.length + 2.0 * past));
    if (!prism.IsDone()) return TopoDS_Shape();
    const TopTools_ListOfShape& swept = prism.Generated(side);
    if (swept.Extent() != 1) return TopoDS_Shape();
    surface = TopoDS::Face(swept.First());
    return prism.Shape();
}

/// WHAT A COPY MADE OF THE BODY: the copy of each element of the body, and nothing of any other shape - the copying
/// algorithm, asked of a shape it never copied (a face of a prism), throws.
struct CopyHistory : QymHistory {
    CopyHistory(BRepBuilderAPI_Copy& c, const TopoDS_Shape& of) : copier(c) { TopExp::MapShapes(of, own); }
    const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) override { return own.Contains(s) ? copier.Modified(s) : none; }
    const TopTools_ListOfShape& Generated(const TopoDS_Shape&) override { return none; }
    bool Removed(const TopoDS_Shape&) override { return false; }
    BRepBuilderAPI_Copy& copier;
    TopTools_IndexedMapOfShape own;
    TopTools_ListOfShape none;
};

} // namespace

/// WHAT BLEND THE PRISMS MAKE: a fillet (`kind` 0) of radius `a`, or a chamfer (`kind` 1) of legs `a` on the first face
/// and `b` on the second, the first face chosen as the asymmetric chamfer chooses it (`flip`, `ref_face`).
struct QymBlendLimit {
    int kind;
    double a;
    double b;
    int flip;
    uint32_t ref_face;
};

// THE BLEND `how` ON THE EDGES `idx` OF `s`, BUILT AS THE PRISMS IT TAKES OFF. Null where an edge is not a straight line
// between planar faces meeting convexly, where two edges asked share a vertex (their corner is a blend of its own), or
// where OCCT does not cut. The surface of each blend takes the number `names[k]` its edge was handed.
extern "C" QymShape* qym_shape_blend_limit(const QymShape* s, const QymBlendLimit* how, const uint32_t* idx, size_t n, const unsigned* names) {
    if (!s || !how || n == 0) return nullptr;
    const int kind = how->kind, flip = how->flip;
    const double a = how->a, b = kind == 1 ? how->b : 0.0;
    const uint32_t ref_face = how->ref_face;
    if (a <= 0.0 || (kind == 1 && b <= 0.0)) return nullptr;
    try {
        QYM_SIGNALS;
        // ON A COPY: a boolean may move the tolerances of what it is handed, and the body in the cache shares its faces
        // with the body the door built stepped back - cut on the body itself, the strips of a rim that body leaves
        // were no longer told apart, and a pinched rim landed as one face (measured on the frame of
        // `face_is_one_island`)
        BRepBuilderAPI_Copy copier(s->shape);
        QymShape body{copier.Shape(), {}, {}, {}, {}, {}, {}};
        // the names of the copy, asked of what the body holds: the record of names may keep shapes the body no longer
        // has, and the copy is asked only of its own
        auto name_copy = [&](TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& ids, TopTools_DataMapOfShapeInteger& out) {
            TopTools_IndexedMapOfShape own;
            TopExp::MapShapes(s->shape, ty, own);
            for (int i = 1; i <= own.Extent(); ++i) {
                if (!ids.IsBound(own(i))) continue;
                for (TopTools_ListIteratorOfListOfShape c(copier.Modified(own(i))); c.More(); c.Next()) out.Bind(c.Value(), ids.Find(own(i)));
            }
        };
        name_copy(TopAbs_FACE, s->fids, body.fids);
        name_copy(TopAbs_EDGE, s->eids, body.eids);
        TopTools_IndexedDataMapOfShapeListOfShape faces_of;
        TopExp::MapShapesAndAncestors(body.shape, TopAbs_EDGE, TopAbs_FACE, faces_of);
        TopTools_ListOfShape tools;
        TopTools_ListOfShape sides; // the faces of the prisms but the blends'
        TopTools_DataMapOfShapeInteger tool_ids;
        TopTools_MapOfShape ends;
        TopoDS_Compound all;
        BRep_Builder bld;
        bld.MakeCompound(all);
        size_t found = 0;
        // each edge once: a walk of the body meets an edge once for each face it bounds
        TopTools_IndexedMapOfShape body_edges;
        TopExp::MapShapes(body.shape, TopAbs_EDGE, body_edges);
        for (int i = 1; i <= body_edges.Extent(); ++i) {
            if (!body.eids.IsBound(body_edges(i))) continue;
            const uint32_t id = (uint32_t)body.eids.Find(body_edges(i));
            size_t k = 0;
            while (k < n && idx[k] != id) ++k;
            if (k == n) continue;
            const TopoDS_Edge& edge = TopoDS::Edge(body_edges(i));
            for (TopExp_Explorer v(edge, TopAbs_VERTEX); v.More(); v.Next())
                if (!ends.Add(v.Current())) return why("blend/limit", "two edges asked meet at a corner"), nullptr;
            Corner c;
            if (!corner_of(&body, edge, faces_of, flip, ref_face, c)) return why("blend/limit", "an edge is not a straight line between planar faces meeting convexly"), nullptr;
            Section sec{};
            if (kind == 1) {
                sec.near = c.into_first * a;
                sec.far = c.into_second * b;
            } else {
                // the circle touches each face where it lies `a` from the other: `a / tan(angle / 2)` from the edge
                const double angle = c.into_first.Angle(c.into_second);
                const double touch = a / std::tan(angle / 2.0);
                sec.near = c.into_first * touch;
                sec.far = c.into_second * touch;
                sec.arc = true;
                sec.centre = sec.near - c.out_first * a;
                sec.radius = a;
            }
            if (!lands_on(c, c.first, sec.near) || !lands_on(c, c.second, sec.far))
                return why("blend/limit", "the blend reaches past a face it lies on"), nullptr;
            TopoDS_Face surface;
            TopoDS_Shape prism = prism_of(c, sec, 2.0 * (a + b) + 1.0, surface);
            if (prism.IsNull()) return why("blend/limit", "the prism of a blend was not built"), nullptr;
            if (names && names[k] != 0) tool_ids.Bind(surface, (int)names[k]);
            for (TopExp_Explorer f(prism, TopAbs_FACE); f.More(); f.Next())
                if (!f.Current().IsSame(surface)) sides.Append(f.Current());
            tools.Append(prism);
            bld.Add(all, prism);
            ++found;
        }
        if (found != n) return why("blend/limit", "an edge asked is not on the body"), nullptr;
        BRepAlgoAPI_Cut algo;
        TopTools_ListOfShape args;
        args.Append(body.shape);
        if (!qym_boolean_many(algo, args, tools)) return why("blend/limit", "the prisms were not cut from the body"), nullptr;
        TopoDS_Shape cut_out = algo.Shape();
        if (cut_out.IsNull()) return why("blend/limit", "the prisms were not cut from the body"), nullptr;
        // THE MEASURE OF THE CONSTRUCTION: what bounds the material taken off is the surface of each blend and nothing
        // else. A face of a prism left in the body - a prism carried past the end of an edge into a wall - took off what
        // the blend does not, and the construction refuses rather than land a body off its drawing (measured on the
        // frame of `face_is_one_island`: the roundings of its rim, a prism each into the walls at the corners, took off
        // 21 mm^3 more than the blend).
        TopTools_IndexedMapOfShape left;
        TopExp::MapShapes(cut_out, TopAbs_FACE, left);
        for (TopTools_ListIteratorOfListOfShape f(sides); f.More(); f.Next()) {
            if (left.Contains(f.Value())) return why("blend/limit", "a prism reached past the blend into the body"), nullptr;
            for (TopTools_ListIteratorOfListOfShape m(algo.Modified(f.Value())); m.More(); m.Next())
                if (left.Contains(m.Value())) return why("blend/limit", "a prism reached past the blend into the body"), nullptr;
        }
        // A cut splits what a blend leaves whole: the edge of a face the prism ends on in two along one line, a face in
        // two pieces in one plane. A blend gives one edge and one face there; so does the cut, merged in its plane and
        // along its line. Measured: unmerged, a rounding of the vacuum sample gave four edges an end of two edges, which
        // swapped with the other end on a second rebuild.
        ShapeUpgrade_UnifySameDomain unify(cut_out, Standard_True, Standard_True, Standard_False);
        // only what the cut touched: two faces it left as they were stay two, though they lie on one surface (measured:
        // the two quarters of a rounding of a rim 2 thick, on one axis, merged into a half round the chamfer after it
        // never reached)
        TopTools_MapOfShape kept_faces;
        for (TopExp_Explorer f(body.shape, TopAbs_FACE); f.More(); f.Next())
            if (algo.Modified(f.Current()).IsEmpty() && !algo.IsDeleted(f.Current())) kept_faces.Add(f.Current());
        for (TopAbs_ShapeEnum ty : {TopAbs_EDGE, TopAbs_VERTEX}) {
            TopTools_IndexedDataMapOfShapeListOfShape around;
            TopExp::MapShapesAndAncestors(cut_out, ty, TopAbs_FACE, around);
            for (int i = 1; i <= around.Extent(); ++i) {
                bool untouched = true;
                for (TopTools_ListIteratorOfListOfShape f(around(i)); f.More() && untouched; f.Next()) untouched = kept_faces.Contains(f.Value());
                if (untouched) unify.KeepShape(around.FindKey(i));
            }
        }
        unify.Build();
        TopoDS_Shape res = unify.Shape();
        if (res.IsNull() || !BRepCheck_Analyzer(res).IsValid()) return why("blend/limit", "the body with the prisms cut is not sound"), nullptr;
        // the surface of each blend named by its edge, as the blend names it; nothing else of a prism is left
        const QymShape cut{all, tool_ids, {}, {}, {}, {}, {}};
        CopyHistory copied(copier, s->shape);
        QymBuilderHistory cutting(algo);
        QymToolsHistory merging(unify.History());
        QymChainHistory copied_and_cut(copied, cutting);
        QymChainHistory history(copied_and_cut, merging);
        return qym_result(history, res, QymResultSpec{{QymOperand{s, QymCarry::Everything, "blend/limit"}, QymOperand{&cut, QymCarry::RecipeOnly, "blend/limit/tool"}}});
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}
