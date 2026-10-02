// A LIVE SOLID INTO A FILE AND BACK, plus what is asked ABOUT a body: its edges, its faces, its blends,
// its holes, its section, STEP.
#include "occt_common.hpp"

// -- A LIVE SOLID INTO A FILE AND BACK -----------------------------------------------------------
//
// Reported behaviour: making a cut means waiting a couple of minutes for a rebuild. A measurement showed that
// an edit in an already assembled project costs a second, but the FIRST operation after opening a file
// rebuilds the whole timeline — the bundle holds meshes and faces while not one body has a live B-rep, so the
// kernel builds everything anew. Professional CAD systems do not work that way: they put THE SOLID ITSELF into
// the file.
//
// FACE NAMES DO NOT GO INTO A BREP FILE. They live in the `fids` and `eids` maps, keyed by TopoDS_Shape, and
// the BRep format knows nothing about them. So they are written out alongside by hand: the index from
// `TopExp::MapShapes` -> the name. The traversal is deterministic and survives a write-read round trip, so the
// map is restored exactly on load. If it were not, fillets and chamfers would land on the wrong edges — the
// worst possible defect, which is why there is a test of its own for it.
//
// Blob format: "QYMB" | ver:u32 | nf:u32 | ne:u32 | fids[nf] | fsplit_of[nf] | fsplit_idx[nf] |
//              eids[ne] | the BRep body
extern "C" unsigned char* qym_shape_to_brep(const QymShape* q, size_t* out_len) {
    if (!q || q->shape.IsNull() || !out_len) return nullptr;
    // BINARY BREP, AND NO TRIANGULATION. On a real document the text format came to 7.4 MB of raw solids, and
    // the mesh is not needed in the blob at all — the meshes sit in the bundle separately and the model is
    // drawn from those.
    std::ostringstream os(std::ios::binary);
    BinTools::Write(q->shape, os, Standard_False, Standard_False, BinTools_FormatVersion_CURRENT);
    const std::string brep = os.str();
    TopTools_IndexedMapOfShape fm, em;
    TopExp::MapShapes(q->shape, TopAbs_FACE, fm);
    TopExp::MapShapes(q->shape, TopAbs_EDGE, em);
    const uint32_t nf = static_cast<uint32_t>(fm.Extent());
    const uint32_t ne = static_cast<uint32_t>(em.Extent());
    std::vector<uint32_t> head;
    head.reserve(4 + 3 * nf + ne);
    head.push_back(2u); // version 2: binary BRep without triangulation
    head.push_back(nf);
    head.push_back(ne);
    auto put = [&](const TopTools_DataMapOfShapeInteger& m, const TopTools_IndexedMapOfShape& idx, uint32_t n) {
        for (uint32_t i = 1; i <= n; ++i) {
            const TopoDS_Shape& sh = idx(i);
            head.push_back(m.IsBound(sh) ? static_cast<uint32_t>(m.Find(sh)) : 0u);
        }
    };
    put(q->fids, fm, nf);
    put(q->fsplit_of, fm, nf);
    put(q->fsplit_idx, fm, nf);
    put(q->eids, em, ne);
    const size_t magic = 4;
    const size_t total = magic + head.size() * 4 + brep.size();
    unsigned char* out = static_cast<unsigned char*>(malloc(total));
    if (!out) return nullptr;
    memcpy(out, "QYMB", magic);
    memcpy(out + magic, head.data(), head.size() * 4);
    memcpy(out + magic + head.size() * 4, brep.data(), brep.size());
    *out_len = total;
    return out;
}

extern "C" void qym_bytes_free(unsigned char* p) { free(p); }

extern "C" QymShape* qym_shape_from_brep(const unsigned char* data, size_t len) {
    const size_t magic = 4;
    if (!data || len < magic + 12 || memcmp(data, "QYMB", magic) != 0) return nullptr;
    uint32_t head3[3];
    memcpy(head3, data + magic, 12);
    if (head3[0] != 2u) return nullptr; // a foreign version; do not guess silently
    const uint32_t nf = head3[1], ne = head3[2];
    const size_t table = static_cast<size_t>(3) * nf + ne;
    const size_t off = magic + 12 + table * 4;
    if (len < off) return nullptr;
    std::vector<uint32_t> ids(table);
    if (table) memcpy(ids.data(), data + magic + 12, table * 4);
    std::string brep(reinterpret_cast<const char*>(data + off), len - off);
    std::istringstream is(brep, std::ios::binary);
    TopoDS_Shape shape;
    try {
        BinTools::Read(shape, is);
    } catch (...) {
        return nullptr;
    }
    if (shape.IsNull()) return nullptr;
    QymShape* q = new QymShape();
    q->shape = shape;
    TopTools_IndexedMapOfShape fm, em;
    TopExp::MapShapes(shape, TopAbs_FACE, fm);
    TopExp::MapShapes(shape, TopAbs_EDGE, em);
    // THE FACE COUNT HAS TO MATCH. If it does not, either the order or the shape itself is different, and
    // names must not be handed out by index: they would silently land in the wrong places. Better to return the
    // solid WITHOUT names, so that references honestly fail to resolve, than to have them resolve wrongly.
    const bool ok = static_cast<uint32_t>(fm.Extent()) == nf && static_cast<uint32_t>(em.Extent()) == ne;
    if (ok) {
        for (uint32_t i = 1; i <= nf; ++i) {
            if (ids[i - 1]) q->fids.Bind(fm(i), static_cast<int>(ids[i - 1]));
            if (ids[nf + i - 1]) q->fsplit_of.Bind(fm(i), static_cast<int>(ids[nf + i - 1]));
            if (ids[2 * nf + i - 1]) q->fsplit_idx.Bind(fm(i), static_cast<int>(ids[2 * nf + i - 1]));
        }
        for (uint32_t i = 1; i <= ne; ++i) {
            if (ids[3 * nf + i - 1]) q->eids.Bind(em(i), static_cast<int>(ids[3 * nf + i - 1]));
        }
    }
    return q;
}

extern "C" void qym_shape_free(QymShape* s) { delete s; }

// A rigid transformation of a shape by a 3x4 row-major matrix (the X, Y and N axes plus the origin).
extern "C" QymShape* qym_shape_transform(const QymShape* s, const double* m) {
    if (!s) return nullptr;
    try {
        gp_Trsf t;
        t.SetValues(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]);
        BRepBuilderAPI_Transform tr(s->shape, t, Standard_True);
        if (!tr.IsDone()) return nullptr;
        QymShape* q = new QymShape{tr.Shape(), {}, {}, {}, {}};
        copy_ids_by_order(s->shape, TopAbs_FACE, s->fids, q->shape, q->fids); // a transform keeps topology 1:1
        copy_ids_by_order(s->shape, TopAbs_EDGE, s->eids, q->shape, q->eids);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// A STEPPED hole: the tool is the main cylinder (dia by depth) plus either a counterbore (dia2 by depth2) or a
// countersink cone (dia2 down to dia over depth2), built along -Z (into the solid), placed by `pl`
// (local -> world) and cut out of `s`. kind: 0 plain, 1 counterbore, 2 countersink. The base's faces keep their
// persistent ids.
// The hole's tool at THE ORIGIN along -Z: a cylinder, plus a counterbore cylinder or a countersink cone. Shared
// between the single-hole and many-holes paths.
static TopoDS_Shape make_hole_tool(int kind, double dia, double depth, double dia2, double depth2) {
    gp_Ax2 down(gp_Pnt(0.0, 0.0, 0.0), gp_Dir(0.0, 0.0, -1.0)); // the tool's axis: -Z, into the solid
    TopoDS_Shape tool = BRepPrimAPI_MakeCylinder(down, dia * 0.5, depth).Shape();
    if (kind == 1 && dia2 > dia && depth2 > 0.0) {
        TopoDS_Shape cb = BRepPrimAPI_MakeCylinder(down, dia2 * 0.5, depth2).Shape(); // the counterbore
        BRepAlgoAPI_Fuse fuse;
        if (qym_boolean(fuse, tool, cb)) tool = fuse.Shape();
    } else if (kind == 2 && dia2 > dia && depth2 > 0.0) {
        TopoDS_Shape cs = BRepPrimAPI_MakeCone(down, dia2 * 0.5, dia * 0.5, depth2).Shape(); // the countersink cone
        BRepAlgoAPI_Fuse fuse;
        if (qym_boolean(fuse, tool, cs)) tool = fuse.Shape();
    }
    return tool;
}

// THE NAME OF THE HOLE'S WALL: what is referenced on a hole tool is exactly the cylindrical wall of the main
// drilling (an edge fillet, a sketch, a coaxial mate all go on it). That is the one given a structural name;
// the counterbore and countersink steps stay positional — nothing references them, and how many there are
// depends on the kind of hole.
static void name_bore_faces(const TopoDS_Shape& tool, double dia, unsigned name, TopTools_DataMapOfShapeInteger& out,
                            const unsigned* extra, size_t nextra) {
    if (tool.IsNull()) return;
    // THE WHOLE HOLE TOOL IS NAMED, NOT JUST ONE WALL.
    //
    // A drill is not just a cylinder: it has the cone of its point, and a counterbore has a cylinder of its
    // own and an annular bottom. Only the wall of the wanted diameter used to be named, and the remaining faces
    // went into the solid untouched and unnamed (measured: 1 such face per hole, which patterns then
    // multiplied). The kind of surface is part of the RECIPE here: the drill is built here, so what is what is
    // known. `extra`: [0] the point's cone, [1] any other cylinder (the counterbore), [2] a plane (the
    // counterbore's bottom).
    for (TopExp_Explorer ex(tool, TopAbs_FACE); ex.More(); ex.Next()) {
        if (out.IsBound(ex.Current())) continue;
        BRepAdaptor_Surface ad(TopoDS::Face(ex.Current()));
        const int t = (int)ad.GetType();
        unsigned pick = 0;
        if (t == GeomAbs_Cylinder && name != 0 && std::abs(ad.Cylinder().Radius() - dia * 0.5) <= 1e-6) {
            pick = name;
        } else if (extra && nextra >= 3) {
            if (t == GeomAbs_Cone) pick = extra[0];
            else if (t == GeomAbs_Cylinder) pick = extra[1];
            else if (t == GeomAbs_Plane) pick = extra[2];
        }
        if (pick != 0) out.Bind(ex.Current(), (int)pick);
    }
}

extern "C" QymShape* qym_shape_hole_stepped(const QymShape* s, int kind, const double* pl, double dia, double depth, double dia2, double depth2, unsigned bore,
                                 const unsigned* extra_names, size_t n_extra) {
    if (!s || dia <= 0.0 || depth <= 0.0 || !pl) return nullptr;
    try {
        TopoDS_Shape tool = make_hole_tool(kind, dia, depth, dia2, depth2);
        gp_Trsf t;
        t.SetValues(pl[0], pl[1], pl[2], pl[3], pl[4], pl[5], pl[6], pl[7], pl[8], pl[9], pl[10], pl[11]);
        tool = BRepBuilderAPI_Transform(tool, t, Standard_True).Shape();
        TopTools_DataMapOfShapeInteger tool_ids;
        name_bore_faces(tool, dia, bore, tool_ids, extra_names, n_extra);
        BRepAlgoAPI_Cut algo;
        qym_boolean(algo, s->shape, tool);
        TopoDS_Shape res = algo.Shape();
        if (res.IsNull()) return nullptr;
        QymShape* q = new QymShape{res, {}, {}, {}, {}};
        int nf = next_local(s->fids), ne = next_local(s->eids);
        carry_ids(algo, s->shape, TopAbs_FACE, s->fids, q->fids, nf);
        carry_ids(algo, tool, TopAbs_FACE, tool_ids, q->fids, nf, true); // the hole wall's name comes from the recipe
        carry_ids(algo, s->shape, TopAbs_EDGE, s->eids, q->eids, ne);
        fill_unnamed(res, TopAbs_FACE, q->fids, nf);
        fill_unnamed(res, TopAbs_EDGE, q->eids, ne);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// MANY holes at once, at a sketch's points. `pls` holds n_holes placement matrices (12 doubles each,
// row-major), each putting a copy of the tool at its own point and orientation. All the tools are unioned, so
// the solid takes ONE cut.
extern "C" QymShape* qym_shape_holes_stepped(const QymShape* s, int kind, const double* pls, size_t n_holes, double dia, double depth, double dia2, double depth2, const unsigned* bores,
                                  const unsigned* extra_names, size_t n_extra) {
    if (!s || dia <= 0.0 || depth <= 0.0 || !pls || n_holes == 0) return nullptr;
    try {
        TopoDS_Shape base_tool = make_hole_tool(kind, dia, depth, dia2, depth2);
        TopoDS_Shape all_tools;
        TopTools_DataMapOfShapeInteger tool_ids; // the wall names of EVERY hole, surviving through the unions
        for (size_t i = 0; i < n_holes; ++i) {
            const double* pl = pls + i * 12;
            gp_Trsf t;
            t.SetValues(pl[0], pl[1], pl[2], pl[3], pl[4], pl[5], pl[6], pl[7], pl[8], pl[9], pl[10], pl[11]);
            TopoDS_Shape one = BRepBuilderAPI_Transform(base_tool, t, Standard_True).Shape();
            TopTools_DataMapOfShapeInteger one_ids;
            name_bore_faces(one, dia, bores ? bores[i] : 0, one_ids, extra_names, n_extra);
            if (all_tools.IsNull()) {
                all_tools = one;
                tool_ids = one_ids;
                continue;
            }
            BRepAlgoAPI_Fuse fu;
            qym_boolean(fu, all_tools, one);
            TopoDS_Shape merged = fu.Shape();
            if (merged.IsNull()) return nullptr;
            TopTools_DataMapOfShapeInteger out;
            int next = 1;
            carry_ids(fu, all_tools, TopAbs_FACE, tool_ids, out, next, true);
            carry_ids(fu, one, TopAbs_FACE, one_ids, out, next, true);
            all_tools = merged;
            tool_ids = out;
        }
        if (all_tools.IsNull()) return nullptr;
        BRepAlgoAPI_Cut algo;
        qym_boolean(algo, s->shape, all_tools);
        TopoDS_Shape res = algo.Shape();
        if (res.IsNull()) return nullptr;
        QymShape* q = new QymShape{res, {}, {}, {}, {}};
        int nf = next_local(s->fids), ne = next_local(s->eids);
        carry_ids(algo, s->shape, TopAbs_FACE, s->fids, q->fids, nf);
        carry_ids(algo, all_tools, TopAbs_FACE, tool_ids, q->fids, nf, true);
        carry_ids(algo, s->shape, TopAbs_EDGE, s->eids, q->eids, ne);
        fill_unnamed(res, TopAbs_FACE, q->fids, nf);
        fill_unnamed(res, TopAbs_EDGE, q->eids, ne);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// Mirror a solid about a plane through the origin (plane: 0 = XY, 1 = XZ, 2 = YZ) into a new shape.
extern "C" QymShape* qym_shape_mirror(const QymShape* s, int plane) {
    if (!s) return nullptr;
    try {
        gp_Dir n = (plane == 1) ? gp_Dir(0, 1, 0) : (plane == 2) ? gp_Dir(1, 0, 0) : gp_Dir(0, 0, 1);
        gp_Trsf t;
        t.SetMirror(gp_Ax2(gp_Pnt(0, 0, 0), n)); // a mirror about the plane whose normal is n
        BRepBuilderAPI_Transform tr(s->shape, t, Standard_True);
        if (!tr.IsDone()) return nullptr;
        QymShape* q = new QymShape{tr.Shape(), {}, {}, {}, {}};
        copy_ids_by_order(s->shape, TopAbs_FACE, s->fids, q->shape, q->fids);
        copy_ids_by_order(s->shape, TopAbs_EDGE, s->eids, q->shape, q->eids);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// Mirror a solid about an ARBITRARY plane (origin[3] plus normal[3]) — a datum or a face.
extern "C" QymShape* qym_shape_mirror_plane(const QymShape* s, const double* origin, const double* normal) {
    if (!s) return nullptr;
    try {
        gp_Dir n(normal[0], normal[1], normal[2]);
        gp_Trsf t;
        t.SetMirror(gp_Ax2(gp_Pnt(origin[0], origin[1], origin[2]), n));
        BRepBuilderAPI_Transform tr(s->shape, t, Standard_True);
        if (!tr.IsDone()) return nullptr;

        QymShape* q = new QymShape{tr.Shape(), {}, {}, {}, {}};
        copy_ids_by_order(s->shape, TopAbs_FACE, s->fids, q->shape, q->fids);
        copy_ids_by_order(s->shape, TopAbs_EDGE, s->eids, q->shape, q->eids);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// Fillet ALL the solid's edges with radius r into a new shape (or nullptr on failure).
// THE COMMONEST REFUSAL OF EVERY EDGE OPERATION: the names recorded by a fillet or a chamfer no longer match
// any edge of this body, because a feature above them in the timeline was edited. The counts are the answer:
// "none of the 4 asked for, the body has 26 named edges" tells a stale name from a body that carries none.
// The same answer for a face named by a feature above in the timeline.
static void why_no_named_faces(const char* where, const QymShape* s, uint32_t asked) {
    size_t named = 0;
    for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next())
        if (s->fids.IsBound(ex.Current())) ++named;
    char msg[192];
    snprintf(msg, sizeof(msg), "the face named %u is not in this body (it has %zu named faces of its own)", asked, named);
    why(where, msg);
}

static void why_no_named_edges(const char* where, const QymShape* s, size_t asked) {
    size_t named = 0;
    for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next())
        if (s->eids.IsBound(ex.Current())) ++named;
    char msg[192];
    snprintf(msg, sizeof(msg), "not one of the %zu named edges asked for is in this body (it has %zu named edges of its own)", asked, named);
    why(where, msg);
}

// AN EDGE A BLEND CAN RUN ALONG: not the point of a pole (no line at all), not the seam of a round face (the face meets
// itself there, nothing to round), and with a face on either side. Handed to the kernel, the bevel of every edge of a
// ball cut by a quarter crashed the program outright (SIGSEGV in the corner of three bevels at the pole).
static TopTools_IndexedDataMapOfShapeListOfShape faces_of_edges(const TopoDS_Shape& body) {
    TopTools_IndexedDataMapOfShapeListOfShape e2f;
    TopExp::MapShapesAndAncestors(body, TopAbs_EDGE, TopAbs_FACE, e2f);
    return e2f;
}
static bool bladeable(const TopoDS_Edge& e, const TopTools_IndexedDataMapOfShapeListOfShape& e2f) {
    if (BRep_Tool::Degenerated(e)) return false;
    if (!e2f.Contains(e)) return false;
    const TopTools_ListOfShape& fl = e2f.FindFromKey(e);
    if (fl.Extent() != 2) return false;
    // closed on one of its faces and bounding another is still an edge: the cut that lies along a seam leaves one so,
    // and that is what the moving of seams is for
    return !fl.First().IsSame(fl.Last());
}

// AN EDGE ALONG A SEAM: closed on a round face - the face comes round to it from both sides - and bounding another face
// too. The cut that lies on the seam of a round face leaves its edge so; the kernel handed one crashes outright in the
// corner of three bevels at a pole (SIGSEGV), and elsewhere fails, so its seam is moved BEFORE the blend is tried.
static bool along_a_seam(const QymShape* s, const uint32_t* idx, size_t n) {
    const TopTools_IndexedDataMapOfShapeListOfShape e2f = faces_of_edges(s->shape);
    for (int i = 1; i <= e2f.Extent(); ++i) {
        const TopTools_ListOfShape& fl = e2f.FindFromIndex(i);
        if (fl.Extent() != 2 || fl.First().IsSame(fl.Last()) || !s->eids.IsBound(e2f.FindKey(i))) continue;
        const TopoDS_Edge e = TopoDS::Edge(e2f.FindKey(i));
        if (!BRep_Tool::IsClosed(e, TopoDS::Face(fl.First())) && !BRep_Tool::IsClosed(e, TopoDS::Face(fl.Last()))) continue;
        const uint32_t id = (uint32_t)s->eids.Find(e);
        for (size_t k = 0; k < n; ++k) if (idx[k] == id) return true;
    }
    return false;
}
static QymShape* off_seams(const QymShape* s, const uint32_t* idx, size_t n, const std::function<QymShape*(const QymShape*)>& op);
// The blend tried on the body with its seams moved first; when that cannot be done the edge is refused rather than
// handed to the kernel as it is.
static QymShape* off_seams_first(const QymShape* s, const uint32_t* idx, size_t n, const std::function<QymShape*(const QymShape*)>& op) {
    if (QymShape* q = off_seams(s, idx, n, op)) return q;
    why("blend/seam", "an edge lies along the seam of a round face, and the seam could not be moved off it");
    return nullptr;
}

// declared here, defined beside the rounding of named edges: the blends of every edge meet the same seams
static QymShape* off_seams(const QymShape* s, const uint32_t* idx, size_t n, const std::function<QymShape*(const QymShape*)>& op);
static std::vector<uint32_t> every_edge(const QymShape* s);

extern "C" QymShape* qym_shape_fillet_all(const QymShape* s, double r) {
    if (!s) return why("fillet/asked", "there is no body to round"), nullptr;
    if (r <= 0.0) return why("fillet/asked", "the radius is zero"), nullptr;
    try {
        const std::vector<uint32_t> every = every_edge(s);
        if (along_a_seam(s, every.data(), every.size())) return off_seams_first(s, every.data(), every.size(), [&](const QymShape* m) { return qym_shape_fillet_all(m, r); });
        BRepFilletAPI_MakeFillet mk(s->shape);
        TopTools_IndexedMapOfShape edges;
        TopExp::MapShapes(s->shape, TopAbs_EDGE, edges);
        if (edges.Extent() == 0) return why("fillet/edges", "the body has no edges at all"), nullptr;
        const TopTools_IndexedDataMapOfShapeListOfShape e2f = faces_of_edges(s->shape);
        for (int i = 1; i <= edges.Extent(); ++i)
            if (bladeable(TopoDS::Edge(edges(i)), e2f)) mk.Add(r, TopoDS::Edge(edges(i)));
        mk.Build();
        if (!mk.IsDone() || !BRepCheck_Analyzer(mk.Shape()).IsValid()) {
            const std::vector<uint32_t> all = every_edge(s);
            if (QymShape* q = off_seams(s, all.data(), all.size(), [&](const QymShape* m) { return qym_shape_fillet_all(m, r); })) return q;
            return why("fillet/build", "the kernel could not round every edge of the body at this radius"), nullptr;
        }
        QymShape* q = new QymShape{mk.Shape(), {}, {}, {}, {}};
        propagate_ids(mk, s->shape, TopAbs_FACE, s->fids, q->shape, q->fids); // faces keep their ids; the fillets are new
        propagate_ids(mk, s->shape, TopAbs_EDGE, s->eids, q->shape, q->eids);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// Chamfer ALL the solid's edges by distance d into a new shape.
extern "C" QymShape* qym_shape_chamfer_all(const QymShape* s, double d) {
    if (!s) return why("chamfer/asked", "there is no body to bevel"), nullptr;
    if (d <= 0.0) return why("chamfer/asked", "the setback is zero"), nullptr;
    try {
        const std::vector<uint32_t> every = every_edge(s);
        if (along_a_seam(s, every.data(), every.size())) return off_seams_first(s, every.data(), every.size(), [&](const QymShape* m) { return qym_shape_chamfer_all(m, d); });
        BRepFilletAPI_MakeChamfer mk(s->shape);
        TopTools_IndexedMapOfShape edges;
        TopExp::MapShapes(s->shape, TopAbs_EDGE, edges);
        if (edges.Extent() == 0) return why("chamfer/edges", "the body has no edges at all"), nullptr;
        const TopTools_IndexedDataMapOfShapeListOfShape e2f = faces_of_edges(s->shape);
        for (int i = 1; i <= edges.Extent(); ++i)
            if (bladeable(TopoDS::Edge(edges(i)), e2f)) mk.Add(d, TopoDS::Edge(edges(i)));
        mk.Build();
        if (!mk.IsDone() || !BRepCheck_Analyzer(mk.Shape()).IsValid()) {
            const std::vector<uint32_t> all = every_edge(s);
            if (QymShape* q = off_seams(s, all.data(), all.size(), [&](const QymShape* m) { return qym_shape_chamfer_all(m, d); })) return q;
            return why("chamfer/build", "the kernel could not bevel every edge of the body at this setback"), nullptr;
        }
        QymShape* q = new QymShape{mk.Shape(), {}, {}, {}, {}};
        propagate_ids(mk, s->shape, TopAbs_FACE, s->fids, q->shape, q->fids);
        propagate_ids(mk, s->shape, TopAbs_EDGE, s->eids, q->shape, q->eids);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// Shell: remove the faces with the PERSISTENT ids `ids` (not positional indices, so it is stable across
// rebuilds) and make walls, giving a hollow solid. `offset` is SIGNED: negative puts the thickness INWARDS
// (the default), positive OUTWARDS.
extern "C" QymShape* qym_shape_shell(const QymShape* s, double offset, const uint32_t* ids, size_t n, const uint32_t* gfrom, const uint32_t* gto, size_t gn) {
    if (!s || (offset > -1e-9 && offset < 1e-9) || n == 0) return why("shell/asked", "no body, a zero offset, or no face to open"), nullptr;
    try {
        std::map<int,int> walls;
        for (size_t i = 0; i < gn; ++i) walls[(int)gfrom[i]] = (int)gto[i];

        // ONE SET OF ATTEMPTS, APPLICABLE TO ANY SHAPE. It is called twice: on the solid as it is and on its
        // UNIFIED copy. The unification is needed because splitting faces (a legitimate operation) leaves
        // coplanar pieces in the solid, and the kernel's offset does not converge on such a solid; for a wall
        // those seams mean nothing. In the unified copy the faces to remove are found BY POSITION: the merged
        // face takes the name of one of the pieces, so the name of the face to remove may not survive in it,
        // whereas its position always will.
        auto attempt = [&](const TopoDS_Shape& shape, TopTools_DataMapOfShapeInteger& fids,
                           TopTools_DataMapOfShapeInteger& eids, const TopTools_ListOfShape& toRemove) -> QymShape* {
            if (toRemove.IsEmpty()) return why("shell/faces", "not one of the named faces is in this body"), nullptr;
            BRepOffsetAPI_MakeThickSolid mk;
            mk.MakeThickSolidByJoin(shape, toRemove, offset, 1.0e-3);
            mk.Build();
            if (!mk.IsDone()) { // extend the faces to their intersection instead of rounding the joins
                mk = BRepOffsetAPI_MakeThickSolid();
                mk.MakeThickSolidByJoin(shape, toRemove, offset, 1.0e-3, BRepOffset_Skin, Standard_True, Standard_False, GeomAbs_Intersection);
                mk.Build();
            }
            if (!mk.IsDone()) { // a looser tolerance, and the internal edges removed
                mk = BRepOffsetAPI_MakeThickSolid();
                mk.MakeThickSolidByJoin(shape, toRemove, offset, 1.0e-2, BRepOffset_Skin, Standard_True, Standard_False, GeomAbs_Intersection, Standard_True);
                mk.Build();
            }
            if (mk.IsDone() && !mk.Shape().IsNull()) {
                QymShape* q = new QymShape{mk.Shape(), {}, {}, {}, {}};
                propagate_ids(mk, shape, TopAbs_FACE, fids, q->shape, q->fids, nullptr, nullptr, walls.empty() ? nullptr : &walls);
                propagate_ids(mk, shape, TopAbs_EDGE, eids, q->shape, q->eids);
                return q;
            }
            // BY HAND: a shrunk copy of the solid is subtracted from the solid, and the removed faces are
            // opened up with a prism. This handles the case the ready-made algorithm always gives up on — a
            // shell after a BLIND hole, where the face being removed carries the pocket's rim.
            BRepOffsetAPI_MakeOffsetShape mko;
            mko.PerformByJoin(shape, offset, 1.0e-3, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Intersection);
            if (!mko.IsDone()) return why("shell/offset", "the kernel could not offset the faces inwards"), nullptr;
            TopoDS_Shape shrunk = mko.Shape();
            if (shrunk.IsNull()) return why("shell/offset", "the kernel reported an offset and returned nothing"), nullptr;
            if (shrunk.ShapeType() == TopAbs_SHELL) { // the offset returns a SHELL, which a boolean will not take
                ShapeFix_Solid sfs;
                TopoDS_Shape made = sfs.SolidFromShell(TopoDS::Shell(shrunk));
                if (made.IsNull()) return why("shell/offset", "the offset shell did not close into a solid"), nullptr;
                shrunk = made;
            } else if (shrunk.ShapeType() != TopAbs_SOLID && shrunk.ShapeType() != TopAbs_COMPSOLID && shrunk.ShapeType() != TopAbs_COMPOUND) {
                return why("shell/offset", "the offset came back as neither a solid nor a shell, so nothing can be cut with it"), nullptr;
            }
            BRepAlgoAPI_Cut wall;
            qym_boolean(wall, shape, shrunk);
            if (!wall.IsDone()) return why("shell/wall", "the shrunk copy could not be cut out of the body to leave a wall"), nullptr;
            TopoDS_Shape res = wall.Shape();
            double depth = std::abs(offset) * 2.0; // certainly through the wall: beyond it is already empty
            for (TopTools_ListIteratorOfListOfShape it(toRemove); it.More(); it.Next()) {
                const TopoDS_Face& f = TopoDS::Face(it.Value());
                gp_Vec nv = face_normal_vec(f);
                if (nv.Magnitude() < 1e-9) return why("shell/opening", "a face to open has no normal, so there is no direction to open it in"), nullptr;
                nv.Normalize();
                BRepPrimAPI_MakePrism pr(f, nv * (-depth));
                if (!pr.IsDone()) return why("shell/opening", "the face to open could not be extruded through the wall"), nullptr;
                BRepAlgoAPI_Cut open;
                qym_boolean(open, res, pr.Shape());
                if (!open.IsDone()) return why("shell/opening", "the opening could not be cut out of the walled body"), nullptr;
                res = open.Shape();
            }
            if (res.IsNull()) return why("shell/result", "the body left after opening it is empty"), nullptr;
            QymShape* q = new QymShape{res, {}, {}, {}, {}};
            propagate_ids(wall, shape, TopAbs_FACE, fids, q->shape, q->fids, nullptr, nullptr, walls.empty() ? nullptr : &walls);
            propagate_ids(wall, shape, TopAbs_EDGE, eids, q->shape, q->eids);
            return q;
        };

        TopTools_ListOfShape toRemove;
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            uint32_t fid = s->fids.IsBound(ex.Current()) ? static_cast<uint32_t>(s->fids.Find(ex.Current())) : 0u;
            if (fid == 0) continue;
            for (size_t k = 0; k < n; ++k) {
                if (ids[k] == fid) { toRemove.Append(ex.Current()); break; }
            }
        }
        if (toRemove.IsEmpty()) {
            // THE COMMONEST REFUSAL IN DAILY WORK: the name of the face to open was minted by a feature above
            // in the timeline, that feature was edited, and the name no longer belongs to any face of this
            // body. The counts are part of the answer: "asked for 1, the body has 6 named faces" tells the
            // difference between a stale name and a body that carries no names at all.
            size_t named = 0;
            for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next())
                if (s->fids.IsBound(ex.Current())) ++named;
            char msg[192];
            snprintf(msg, sizeof(msg), "not one of the %zu named faces asked for is in this body (it has %zu named faces of its own)", n, named);
            return why("shell/faces", msg), nullptr;
        }

        TopTools_DataMapOfShapeInteger f0 = s->fids, e0 = s->eids;
        if (QymShape* q = attempt(s->shape, f0, e0, toRemove)) return q;

        // A SOLID MADE OF COPIES IS SEVERAL SOLIDS. A pattern or a mirror gives a compound of separate solids;
        // OCCT cannot offset them all at once and crashes inside itself. What is expected is the obvious
        // thing: the shell goes onto EVERY body, and the face being removed onto the body that owns it.
        {
            int nsolids = 0;
            for (TopExp_Explorer ex(s->shape, TopAbs_SOLID); ex.More(); ex.Next()) ++nsolids;
            if (nsolids > 1) {
                BRep_Builder bb;
                TopoDS_Compound out;
                bb.MakeCompound(out);
                bool any = false, all = true;
                TopTools_DataMapOfShapeInteger parts_f, parts_e; // the names handed out inside the pieces
                // THE ORDER OF THE PIECES IS SET BY NAMES, NOT BY THE TRAVERSAL. Each piece was shelled from
                // a COPY of one and the same maps and numbered what was new from the same starting point: the
                // walls of three copies got the same numbers (measured: 72 edges, 48 distinct names, and every
                // collision in a DIFFERENT place, with x differing by the pattern's step). Numbering them one
                // after another is not enough: the wall's name would then depend on where the piece came in the
                // traversal, which is exactly the instability this whole analysis exists to prevent. The
                // footing is the piece's OWN INHERITED names: across a pattern's copies they differ and do not
                // overlap (measured: 0..5, 6..11, 12..17), so "the smallest of one's own" sets an order that
                // survives a rebuild.
                std::vector<TopoDS_Shape> solids;
                for (TopExp_Explorer ex(s->shape, TopAbs_SOLID); ex.More(); ex.Next())
                    solids.push_back(ex.Current());
                std::vector<size_t> order(solids.size());
                for (size_t i = 0; i < order.size(); ++i) order[i] = i;
                std::vector<int> key(solids.size(), INT_MAX);
                for (size_t i = 0; i < solids.size(); ++i) {
                    for (TopExp_Explorer fx(solids[i], TopAbs_FACE); fx.More(); fx.Next()) {
                        if (!s->fids.IsBound(fx.Current())) continue;
                        const int v = s->fids.Find(fx.Current());
                        if (v < key[i]) key[i] = v;
                    }
                }
                std::stable_sort(order.begin(), order.end(),
                                 [&](size_t a, size_t b) { return key[a] < key[b]; });
                std::vector<TopoDS_Shape> done(solids.size());
                TopTools_DataMapOfShapeInteger fp = s->fids, ep = s->eids;
                for (size_t oi = 0; oi < order.size() && all; ++oi) {
                    const TopoDS_Shape& sol = solids[order[oi]];
                    TopTools_ListOfShape mine;
                    for (TopExp_Explorer fx(sol, TopAbs_FACE); fx.More(); fx.Next()) {
                        for (TopTools_ListIteratorOfListOfShape it(toRemove); it.More(); it.Next()) {
                            if (it.Value().IsSame(fx.Current())) mine.Append(fx.Current());
                        }
                    }
                    if (mine.IsEmpty()) { // nothing to remove on this solid, so it goes through as it is
                        done[order[oi]] = sol;
                        continue;
                    }
                    TopTools_DataMapOfShapeInteger fq = fp, eq = ep;
                    QymShape* part = attempt(sol, fq, eq, mine);
                    if (!part) { all = false; break; }
                    // THE PIECE'S NAMES ARE TAKEN OVER. Inside `attempt` the walls have already been named by
                    // recipe (measured: a piece has 11 faces and all 11 are named), but the map went away
                    // together with `part` while the result was assembled from the solid's ORIGINAL names plus
                    // a positional fill-in — that is, every new face, which is exactly the walls, was left with
                    // a number. On a shell after a pattern that lost 5 faces out of 23.
                    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(part->fids); it.More(); it.Next())
                        parts_f.Bind(it.Key(), it.Value());
                    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(part->eids); it.More(); it.Next())
                        parts_e.Bind(it.Key(), it.Value());
                    // THE NEXT PIECE NUMBERS AFTER THIS ONE, not from the same start. The piece's map is
                    // ADDED to the accumulated one rather than replacing it: only the piece's own shapes are
                    // bound in it.
                    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(part->fids); it.More(); it.Next())
                        fp.Bind(it.Key(), it.Value());
                    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(part->eids); it.More(); it.Next())
                        ep.Bind(it.Key(), it.Value());
                    done[order[oi]] = part->shape;
                    delete part;
                    any = true;
                }
                if (any && all)
                    for (size_t i = 0; i < done.size(); ++i)
                        if (!done[i].IsNull()) bb.Add(out, done[i]);
                if (any && all) {
                    QymShape* q = new QymShape{out, {}, {}, {}, {}};
                    // FIRST the solid's names, and OVER them the names handed out inside the pieces: a face
                    // that came through unchanged has the same name, while a wall has one only here.
                    q->fids = s->fids;
                    q->eids = s->eids;
                    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(parts_f); it.More(); it.Next())
                        q->fids.Bind(it.Key(), it.Value());
                    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(parts_e); it.More(); it.Next())
                        q->eids.Bind(it.Key(), it.Value());
                    int next = next_local(q->fids);
                    fill_unnamed(out, TopAbs_FACE, q->fids, next);
                    int nexte = next_local(q->eids);
                    fill_unnamed(out, TopAbs_EDGE, q->eids, nexte);
                    return q;
                }
            }
        }

        TopTools_DataMapOfShapeInteger fu = s->fids, eu = s->eids;
        TopoDS_Shape uni = unify_monolithic(s->shape, fu, eu);
        if (uni.IsNull() || uni.IsSame(s->shape)) return why("shell/join", "the walls did not join into one body"), nullptr;
        TopTools_ListOfShape rem2;
        TopTools_MapOfShape taken;
        for (TopTools_ListIteratorOfListOfShape it(toRemove); it.More(); it.Next()) {
            GProp_GProps gp;
            BRepGProp::SurfaceProperties(it.Value(), gp);
            TopoDS_Vertex v = BRepBuilderAPI_MakeVertex(gp.CentreOfMass());
            for (TopExp_Explorer ex(uni, TopAbs_FACE); ex.More(); ex.Next()) {
                if (taken.Contains(ex.Current())) continue;
                BRepExtrema_DistShapeShape d(v, ex.Current());
                if (d.IsDone() && d.Value() < 1.0e-6) { rem2.Append(ex.Current()); taken.Add(ex.Current()); break; }
            }
        }
        if (QymShape* q = attempt(uni, fu, eu, rem2)) return q;
        if (getenv("QYM_SHELL_DUMP")) { // a breakdown of a solid no attempt manages to shell
            int nso = 0, nsh = 0, nf = 0, ne = 0;
            for (TopExp_Explorer ex(s->shape, TopAbs_SOLID); ex.More(); ex.Next()) ++nso;
            for (TopExp_Explorer ex(s->shape, TopAbs_SHELL); ex.More(); ex.Next()) ++nsh;
            for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) ++nf;
            for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next()) ++ne;
            fprintf(stderr, "QYMDUMP shell failed: offset %g, solids %d, shells %d, faces %d, edges %d, removing %d\n",
                    offset, nso, nsh, nf, ne, toRemove.Extent());
            for (TopTools_ListIteratorOfListOfShape it(toRemove); it.More(); it.Next()) {
                Handle(Geom_Surface) su = BRep_Tool::Surface(TopoDS::Face(it.Value()));
                GProp_GProps gp;
                BRepGProp::SurfaceProperties(it.Value(), gp);
                fprintf(stderr, "QYMDUMP   face to remove: %s, area %.2f\n", su.IsNull() ? "no surface" : su->DynamicType()->Name(), gp.Mass());
            }
        }
        return nullptr;
    } QYM_WHY_CATCH("shell")
    if (getenv("QYM_SHELL_DUMP")) fprintf(stderr, "QYMDUMP shell: %s\n", qym_why() ? qym_why() : "unknown exception");
    return nullptr;
}

// A CENTRED shell: a wall of thickness `t` centred on the original surface. The solid is grown by +t/2
// outwards (MakeOffsetShape) and then hollowed by -t inwards (the open faces `ids` are mapped through
// Generated).
// A WALL CENTRED ON THE SURFACE: half the thickness outward, half inward, the open faces open where they were. It is
// made of the two halves the shell already builds right - a wall of t / 2 outward and a wall of t / 2 inward from the
// same surface - fused along that surface. Grown whole by t / 2 and then hollowed by t, as it was, the open face went
// up by t / 2 with the rest and the hollowing took nothing out: a block 40 x 30 x 10 opened on top came out 16055.5
// mm^3, more than the 12000 of the block itself, with no hollow and a wall standing 1 mm above the open face.
extern "C" QymShape* qym_shape_shell_center(const QymShape* s, double t, const uint32_t* ids, size_t n) {
    if (!s || t < 1e-9 || n == 0) return nullptr;
    try {
        std::unique_ptr<QymShape> out(qym_shape_shell(s, t * 0.5, ids, n, nullptr, nullptr, 0));
        if (!out) return nullptr;
        std::unique_ptr<QymShape> in(qym_shape_shell(s, -t * 0.5, ids, n, nullptr, nullptr, 0));
        if (!in) return nullptr;
        BRepAlgoAPI_Fuse fuse;
        if (!qym_boolean(fuse, out->shape, in->shape)) return why("shell/center", "the two halves of the wall did not join"), nullptr;
        QymShape* q = new QymShape{fuse.Shape(), {}, {}, {}, {}};
        // the names of either half go on: the faces of the part as the outward half moved them, its walls as the
        // inward half named them
        propagate_ids(fuse, out->shape, TopAbs_FACE, out->fids, q->shape, q->fids);
        propagate_ids(fuse, in->shape, TopAbs_FACE, in->fids, q->shape, q->fids);
        propagate_ids(fuse, out->shape, TopAbs_EDGE, out->eids, q->shape, q->eids);
        propagate_ids(fuse, in->shape, TopAbs_EDGE, in->eids, q->shape, q->eids);
        // the surface the halves met on is glued away: the side of a wall is one face, not two
        q->shape = unify_monolithic(q->shape, q->fids, q->eids, &q->absorbed);
        int nf = next_local(q->fids), ne = next_local(q->eids);
        fill_unnamed(q->shape, TopAbs_FACE, q->fids, nf);
        fill_unnamed(q->shape, TopAbs_EDGE, q->eids, ne);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// The axis of a CYLINDRICAL or CONICAL face by its persistent id, for picking the axis of a circular pattern
// by click. Returns 1 plus the axis's origin and direction (in the solid's LOCAL coordinate system), or 0 if
// the face was not found or is not round (a plane or a spline).
extern "C" int qym_shape_face_axis(const QymShape* s, uint32_t face_id, double* origin, double* dir) {
    if (!s || face_id == 0 || !origin || !dir) return 0;
    try {
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            uint32_t fid = s->fids.IsBound(ex.Current()) ? static_cast<uint32_t>(s->fids.Find(ex.Current())) : 0u;
            if (fid != face_id) continue;
            BRepAdaptor_Surface surf(TopoDS::Face(ex.Current()));
            gp_Ax1 ax;
            if (surf.GetType() == GeomAbs_Cylinder) {
                ax = surf.Cylinder().Axis();
            } else if (surf.GetType() == GeomAbs_Cone) {
                ax = surf.Cone().Axis();
            } else {
                return 0; // the face is not round, so there is no axis
            }
            gp_Pnt o = ax.Location();
            gp_Dir d = ax.Direction();
            origin[0] = o.X(); origin[1] = o.Y(); origin[2] = o.Z();
            dir[0] = d.X(); dir[1] = d.Y(); dir[2] = d.Z();
            return 1;
        }
        return 0;
    } catch (...) { return 0; }
}

// The persistent ids of the EDGES belonging to face `face_id`, for selecting all of a face's edges to fillet
// or chamfer. Writes up to `cap` ids into `out` and returns how many. 0 means the face was not found or has no
// edges with ids.
extern "C" size_t qym_shape_face_edge_ids(const QymShape* s, uint32_t face_id, uint32_t* out, size_t cap) {
    if (!s || face_id == 0 || !out || cap == 0) return 0;
    size_t cnt = 0;
    try {
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            uint32_t fid = s->fids.IsBound(ex.Current()) ? static_cast<uint32_t>(s->fids.Find(ex.Current())) : 0u;
            if (fid != face_id) continue;
            for (TopExp_Explorer ee(ex.Current(), TopAbs_EDGE); ee.More() && cnt < cap; ee.Next()) {
                uint32_t eid = s->eids.IsBound(ee.Current()) ? static_cast<uint32_t>(s->eids.Find(ee.Current())) : 0u;
                if (eid == 0) continue;
                bool dup = false; // a face traverses an edge once, but deduplicate just in case
                for (size_t k = 0; k < cnt; ++k) if (out[k] == eid) { dup = true; break; }
                if (!dup) out[cnt++] = eid;
            }
            break; // the face has been found
        }
    } catch (...) { return 0; }
    return cnt;
}

// The solid's edges as polylines, for drawing and picking. The order is that of TopExp::MapShapes(EDGE).
// For ROUND edges (a circle or an arc) the centre, axis and radius are kept as well: the concentric anchor of
// holes.
struct QymEdges {
    std::vector<std::vector<float>> polys; // each one is x,y,z in a row
    std::vector<uint32_t> ids;             // the edge's PERSISTENT id, parallel to polys
    std::vector<double> circles;           // 7 doubles per edge: cx,cy,cz, ax,ay,az, radius (radius 0 = not round)
    std::vector<uint8_t> smooth;           // a SMOOTH edge (a tangent junction of faces, dihedral about 180 deg):
                                           // there is nothing to fillet or chamfer, and the kernel honestly
                                           // fails (the fillet's boundary)
    std::vector<double> refs;              // 3 doubles per edge: THE NEIGHBOURING FACE'S NORMAL at the middle of
                                           // the edge, the connector's secondary axis (zero = no neighbouring
                                           // face)
};
extern "C" QymEdges* qym_shape_edges(const QymShape* s) {
    if (!s) return nullptr;
    QymEdges* e = new QymEdges();
    TopTools_IndexedMapOfShape edges;
    // edge -> its faces, for detecting SMOOTHNESS (both faces' normals at the middle of the edge point the
    // same way)
    TopTools_IndexedDataMapOfShapeListOfShape e2f;
    // THE SOLIDS ARE WALKED ONE BY ONE, and that is the whole of this function being different from the obvious
    // one-liner. Asking the whole shape for its edges with `TopExp::MapShapes(shape, TopAbs_EDGE, ...)` stops at
    // the FIRST solid: the explorer reaches the solids of a compound and refuses to go inside them, because a
    // solid may appear in more than one shell and would then be walked twice. So a body made of several pieces -
    // and every extrude of several disjoint contours produces exactly that - answered with the edges of one of
    // them and none at all of the rest.
    //
    // Reported: a cross of five rectangles with three of them extruded into one another. Twelve edges were
    // listed - exactly one rectangle - and the other pieces had none: not in the list of elements, not under the
    // cursor, not in the details. The same twelve went to picking, drawing and anchoring, because all of them
    // read this one place. Measured on the body itself: two solids of twelve edges each, and this answer of
    // twelve.
    //
    // A SHELL OR A SHEET has no solid, so the shells are taken next, and a shape with neither is walked as it
    // stands - an edge of a wire is a real edge of a sketch face.
    TopTools_IndexedMapOfShape parts;
    TopExp::MapShapes(s->shape, TopAbs_SOLID, parts);
    if (parts.Extent() == 0) TopExp::MapShapes(s->shape, TopAbs_SHELL, parts);
    if (parts.Extent() == 0) parts.Add(s->shape);
    for (int part = 1; part <= parts.Extent(); ++part) {
        TopExp::MapShapes(parts(part), TopAbs_EDGE, edges);
        TopExp::MapShapesAndAncestors(parts(part), TopAbs_EDGE, TopAbs_FACE, e2f);
    }
    for (int i = 1; i <= edges.Extent(); ++i) {
        std::vector<float> pts;
        Standard_Real f = 0, l = 0;
        Handle(Geom_Curve) c = BRep_Tool::Curve(TopoDS::Edge(edges(i)), f, l);
        if (!c.IsNull()) {
            const int n = 24;
            for (int k = 0; k <= n; ++k) {
                gp_Pnt p = c->Value(f + (l - f) * k / n);
                pts.push_back(static_cast<float>(p.X()));
                pts.push_back(static_cast<float>(p.Y()));
                pts.push_back(static_cast<float>(p.Z()));
            }
        }
        // is the edge round? the centre, axis and radius come from the analytic curve (a circle or an arc)
        double circ[7] = {0, 0, 0, 0, 0, 0, 0};
        try {
            BRepAdaptor_Curve ac(TopoDS::Edge(edges(i)));
            if (ac.GetType() == GeomAbs_Circle) {
                gp_Circ g = ac.Circle();
                gp_Pnt o = g.Location();
                gp_Dir d = g.Axis().Direction();
                circ[0] = o.X(); circ[1] = o.Y(); circ[2] = o.Z();
                circ[3] = d.X(); circ[4] = d.Y(); circ[5] = d.Z();
                circ[6] = g.Radius();
            }
        } catch (...) {}
        for (int k = 0; k < 7; ++k) e->circles.push_back(circ[k]);
        // smoothness: an angle of less than about 1.5 deg between the two faces' normals at the middle of the
        // edge means a tangent junction
        uint8_t sm = 0;
        try {
            const TopoDS_Edge& ed = TopoDS::Edge(edges(i));
            if (e2f.Contains(ed)) {
                const TopTools_ListOfShape& fl = e2f.FindFromKey(ed);
                if (fl.Extent() == 2 && !c.IsNull()) {
                    gp_Pnt mid = c->Value((f + l) * 0.5);
                    gp_Dir n1, n2;
                    int got = 0;
                    for (TopTools_ListIteratorOfListOfShape it(fl); it.More(); it.Next()) {
                        const TopoDS_Face& fc = TopoDS::Face(it.Value());
                        Handle(Geom_Surface) su = BRep_Tool::Surface(fc);
                        if (su.IsNull()) break;
                        GeomAPI_ProjectPointOnSurf pj(mid, su);
                        if (!pj.IsDone() || pj.NbPoints() < 1) break;
                        Standard_Real u, v;
                        pj.LowerDistanceParameters(u, v);
                        GeomLProp_SLProps pr(su, u, v, 1, 1e-6);
                        if (!pr.IsNormalDefined()) break;
                        gp_Dir n = pr.Normal();
                        if (fc.Orientation() == TopAbs_REVERSED) n.Reverse();
                        if (got == 0) n1 = n; else n2 = n;
                        ++got;
                    }
                    if (got == 2 && n1.Dot(n2) > 0.99965) sm = 1; // cos(1.5°)
                }
            }
        } catch (...) {}
        e->smooth.push_back(sm);
        // THE EDGE'S REFERENCE DIRECTION is the connector's secondary axis.
        //
        // An edge has one axis of its own: along itself. Without a second axis the frame's roll is undefined,
        // and it used to be derived from the WORLD's Z axis — that is, from however the part happens to lie.
        // Two parts being mated ended up with different secondary axes, and the joint set the part at an
        // arbitrary rotation.
        // Here the secondary axis is taken from AN ADJACENT FACE.
        //
        // WHICH of the two neighbouring faces to take: the one with the SMALLER PERSISTENT id. The order in
        // the kernel's map shifts on a rebuild, and the persistent id exists precisely so that a reference does
        // not drift; a choice made "by geometry" (the lexicographically smaller normal, say) would be
        // discontinuous — turn the part a little within its own local frame and the roll jumps by 90 deg.
        double rf[3] = {0, 0, 0};
        try {
            const TopoDS_Edge& ed = TopoDS::Edge(edges(i));
            if (e2f.Contains(ed) && !c.IsNull()) {
                const TopTools_ListOfShape& fl = e2f.FindFromKey(ed);
                TopoDS_Face pick;
                uint32_t best = 0;
                for (TopTools_ListIteratorOfListOfShape it(fl); it.More(); it.Next()) {
                    const TopoDS_Face& fc = TopoDS::Face(it.Value());
                    uint32_t id = s->fids.IsBound(fc) ? static_cast<uint32_t>(s->fids.Find(fc)) : 0u;
                    if (pick.IsNull() || (id != 0 && (best == 0 || id < best))) {
                        pick = fc;
                        best = id;
                    }
                }
                if (!pick.IsNull()) {
                    gp_Pnt mid = c->Value((f + l) * 0.5);
                    Handle(Geom_Surface) su = BRep_Tool::Surface(pick);
                    GeomAPI_ProjectPointOnSurf pj(mid, su);
                    if (!su.IsNull() && pj.IsDone() && pj.NbPoints() >= 1) {
                        Standard_Real u, v;
                        pj.LowerDistanceParameters(u, v);
                        GeomLProp_SLProps pr(su, u, v, 1, 1e-6);
                        if (pr.IsNormalDefined()) {
                            gp_Dir n = pr.Normal();
                            if (pick.Orientation() == TopAbs_REVERSED) n.Reverse();
                            rf[0] = n.X(); rf[1] = n.Y(); rf[2] = n.Z();
                        }
                    }
                }
            }
        } catch (...) {}
        for (int k = 0; k < 3; ++k) e->refs.push_back(rf[k]);
        e->polys.push_back(std::move(pts));
        e->ids.push_back(s->eids.IsBound(edges(i)) ? static_cast<uint32_t>(s->eids.Find(edges(i))) : 0u);
    }
    return e;
}
extern "C" size_t qym_edges_count(const QymEdges* e) { return e ? e->polys.size() : 0; }
extern "C" uint32_t qym_edge_id(const QymEdges* e, size_t i) { return (e && i < e->ids.size()) ? e->ids[i] : 0u; }
extern "C" size_t qym_edge_point_count(const QymEdges* e, size_t i) { return (e && i < e->polys.size()) ? e->polys[i].size() / 3 : 0; }
extern "C" void qym_edge_copy_points(const QymEdges* e, size_t i, float* out) {
    if (e && i < e->polys.size() && !e->polys[i].empty()) std::memcpy(out, e->polys[i].data(), e->polys[i].size() * sizeof(float));
}
// Round edge `i`: writes cx,cy,cz, ax,ay,az, radius into `out[7]`; returns 1 if it is round (radius > 0), else 0.
extern "C" int qym_edge_circle(const QymEdges* e, size_t i, double* out) {
    if (!e || !out || (i + 1) * 7 > e->circles.size()) return 0;
    for (int k = 0; k < 7; ++k) out[k] = e->circles[i * 7 + k];
    return out[6] > 0.0 ? 1 : 0;
}
// Is edge i SMOOTH (a tangent junction, not to be offered for a fillet or a chamfer)
extern "C" int qym_edge_smooth(const QymEdges* e, size_t i) { return (e && i < e->smooth.size()) ? (int)e->smooth[i] : 0; }
// THE NEIGHBOURING FACE'S NORMAL for edge `i` at its middle -> `out[3]`; 1 if there is one, else 0 (out is
// zeroed).
extern "C" int qym_edge_ref_dir(const QymEdges* e, size_t i, double* out) {
    if (!e || !out) return 0;
    out[0] = out[1] = out[2] = 0.0;
    if ((i + 1) * 3 > e->refs.size()) return 0;
    for (int k = 0; k < 3; ++k) out[k] = e->refs[i * 3 + k];
    return (out[0] * out[0] + out[1] * out[1] + out[2] * out[2]) > 1e-12 ? 1 : 0;
}
extern "C" void qym_edges_free(QymEdges* e) { delete e; }

// Fillet or chamfer of SELECTED edges by their PERSISTENT ids (idx holds edge ids from qym_edge_id): each id
// is resolved to the current edge through s->eids, so the selection survives a rebuild and the edges do not
// drift by ordinal.
static int add_fillet_edges_by_id(BRepFilletAPI_MakeFillet& mk, const QymShape* s, double r, const uint32_t* idx, size_t n) {
    int added = 0;
    TopTools_MapOfShape seen; // an edge is met once for every face it bounds: it is added once
    const TopTools_IndexedDataMapOfShapeListOfShape e2f = faces_of_edges(s->shape);
    for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next()) {
        if (!s->eids.IsBound(ex.Current()) || !seen.Add(ex.Current()) || !bladeable(TopoDS::Edge(ex.Current()), e2f)) continue;
        uint32_t id = static_cast<uint32_t>(s->eids.Find(ex.Current()));
        for (size_t k = 0; k < n; ++k) {
            if (idx[k] == id) { mk.Add(r, TopoDS::Edge(ex.Current())); ++added; break; }
        }
    }
    return added;
}
static int add_chamfer_edges_by_id(BRepFilletAPI_MakeChamfer& mk, const QymShape* s, double d, const uint32_t* idx, size_t n) {
    int added = 0;
    TopTools_MapOfShape seen; // an edge is met once for every face it bounds: it is added once
    const TopTools_IndexedDataMapOfShapeListOfShape e2f = faces_of_edges(s->shape);
    for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next()) {
        if (!s->eids.IsBound(ex.Current()) || !seen.Add(ex.Current()) || !bladeable(TopoDS::Edge(ex.Current()), e2f)) continue;
        uint32_t id = static_cast<uint32_t>(s->eids.Find(ex.Current()));
        for (size_t k = 0; k < n; ++k) {
            if (idx[k] == id) { mk.Add(d, TopoDS::Edge(ex.Current())); ++added; break; }
        }
    }
    return added;
}
// THE SURFACES OF A FILLET AND OF A CHAMFER ARE NAMED BY ONE AND THE SAME MECHANISM.
//
// The surface a fillet or a chamfer builds has no source in the recipe other than the EDGE it grew from; a
// corner patch has none other than the VERTEX where the filleted edges met. In that sense both operations are
// the same, and yet a chamfer used to name NOTHING: measured on a scenario document, one chamfer added 6
// unnamed faces.
//
// `next` is the shared counter of free numbers: names are handed out BEFORE the positional fill-in, or the
// blocks run into "already taken" (that ordering mistake has already cost three operations).
static void name_blend_faces(BRepBuilderAPI_MakeShape& mk, const QymShape* s, QymShape* q,
                             const uint32_t* idx, size_t n, const unsigned* names, const unsigned* corners, int& next,
                             const unsigned* all_names, size_t n_all) {
        if (names) {
            for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next()) {
                if (!s->eids.IsBound(ex.Current())) continue;
                uint32_t id = static_cast<uint32_t>(s->eids.Find(ex.Current()));
                for (size_t k = 0; k < n; ++k) {
                    if (idx[k] != id || names[k] == 0) continue;
                    const TopTools_ListOfShape& gen = mk.Generated(ex.Current());
                    int piece = 0;
                    for (TopTools_ListIteratorOfListOfShape it(gen); it.More(); it.Next()) {
                        if (it.Value().IsNull() || it.Value().ShapeType() != TopAbs_FACE) continue;
                        // pieces of one surface (a fillet may have split) get the same name plus a piece
                        // number, which the model layer fills in from the split record
                        if (piece == 0) {
                            q->fids.Bind(it.Value(), (int)names[k]);
                        } else {
                            q->fids.Bind(it.Value(), next++);
                            q->fsplit_of.Bind(it.Value(), (int)names[k]);
                            q->fsplit_idx.Bind(it.Value(), piece);
                        }
                        ++piece;
                    }
                }
            }
        }
        // CORNER PATCHES: where several filleted edges meet, the kernel generates a face from the VERTEX
        // rather than from an edge, and it used to be left unnamed, dragging the whole solid back into the
        // positional scheme. A vertex is identified by its edges: the SMALLEST name among that vertex's
        // filleted edges is taken (just as an edge is identified by the pair of its faces).
        if (corners) {
            // ONE PATCH NAME MEANS ONE FACE. A corner patch's name is chosen as the SMALLEST among the
            // vertex's edges, and one edge has two vertices, so both of them chose the same name and two
            // different patches came out under a shared name (measured). The piece counter carries on between
            // vertices: the second one gets a fresh number and a "piece k" record.
            std::map<unsigned, int> corner_piece;
            TopTools_IndexedDataMapOfShapeListOfShape v2e;
            TopExp::MapShapesAndAncestors(s->shape, TopAbs_VERTEX, TopAbs_EDGE, v2e);
            for (int vi = 1; vi <= v2e.Extent(); ++vi) {
                const TopoDS_Shape& v = v2e.FindKey(vi);
                // the smallest name among this vertex's SELECTED edges
                unsigned best = 0;
                size_t best_k = 0;
                for (TopTools_ListIteratorOfListOfShape it(v2e.FindFromIndex(vi)); it.More(); it.Next()) {
                    if (!s->eids.IsBound(it.Value())) continue;
                    uint32_t eid = static_cast<uint32_t>(s->eids.Find(it.Value()));
                    for (size_t k = 0; k < n; ++k) {
                        if (idx[k] != eid || corners[k] == 0) continue;
                        if (best == 0 || corners[k] < best) {
                            best = corners[k];
                            best_k = k;
                        }
                    }
                }
                (void)best_k;
                if (best == 0) continue;
                const TopTools_ListOfShape& gen = mk.Generated(v);
                int& piece_state = corner_piece[best];
                int piece = piece_state;
                for (TopTools_ListIteratorOfListOfShape it(gen); it.More(); it.Next()) {
                    if (it.Value().IsNull() || it.Value().ShapeType() != TopAbs_FACE) continue;
                    if (q->fids.IsBound(it.Value())) continue;
                    if (piece == 0) {
                        q->fids.Bind(it.Value(), (int)best);
                    } else {
                        q->fids.Bind(it.Value(), next++);
                        q->fsplit_of.Bind(it.Value(), (int)best);
                        q->fsplit_idx.Bind(it.Value(), piece);
                    }
                    ++piece;
                }
                piece_state = piece;
            }
        }
    // THE EDGES THE OPERATION PICKED UP ON ITS OWN.
    //
    // A fillet is not confined to the selected edges: the kernel continues the surface across tangent
    // neighbours, and those faces are generated by an EDGE too — just not the one a person named. Names are
    // therefore prepared for ALL the solid's named edges: their recipe is the same ("the fillet surface from
    // edge E"), and without it 2 to 4 unnamed faces were left per operation (measured).
    if (all_names && n_all) {
        for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next()) {
            if (!s->eids.IsBound(ex.Current())) continue;
            const unsigned eid = (unsigned)s->eids.Find(ex.Current());
            unsigned nm = 0;
            for (size_t i2 = 0; i2 + 1 < n_all * 2; i2 += 2)
                if (all_names[i2] == eid) { nm = all_names[i2 + 1]; break; }
            if (nm == 0) continue;
            // ONE EDGE MEANS ONE FACE WITH THAT NAME. An edge can generate several faces (the surface split),
            // and giving them all one name would make the reference ambiguous: it would lead to both places at
            // once. A measurement caught this on a chamfer — 17 faces to 14 names. From the second piece on,
            // each gets a fresh number and a "piece k" record, as in the block for selected edges
            // Faces that block has already dealt with (they carry a piece record) are left alone.
            int piece = 0;
            for (TopTools_ListIteratorOfListOfShape it(mk.Generated(ex.Current())); it.More(); it.Next()) {
                if (it.Value().IsNull() || it.Value().ShapeType() != TopAbs_FACE) continue;
                if (q->fsplit_of.IsBound(it.Value())) { ++piece; continue; }
                if (q->fids.IsBound(it.Value()) && (q->fids.Find(it.Value()) & QYM_NAMED)) { ++piece; continue; }
                if (piece == 0) {
                    q->fids.Bind(it.Value(), (int)nm);
                } else {
                    q->fids.Bind(it.Value(), next++);
                    q->fsplit_of.Bind(it.Value(), (int)nm);
                    q->fsplit_idx.Bind(it.Value(), piece);
                }
                ++piece;
            }
        }
    }

}

// NAMES CARRIED BY PLACE from `old` to `now`: a face or an edge of `now` lying where a named one of `old` lay - the
// same centre and area for a face, the same centre and length for an edge, to 1e-6 - takes its name. For an operation
// that hands on no history and moves nothing but what it removes. Two named things of `old` landing on one of `now`:
// the first keeps it, the other is recorded in `absorbed` as yielded to it. `out` may be `was`.
static void names_by_place(TopAbs_ShapeEnum ty, const TopoDS_Shape& old, const TopTools_DataMapOfShapeInteger& was, const TopoDS_Shape& now,
                           TopTools_DataMapOfShapeInteger& out, std::vector<std::pair<unsigned, unsigned>>* absorbed) {
    auto key = [&](const TopoDS_Shape& sh) -> std::array<double, 4> {
        GProp_GProps g;
        if (ty == TopAbs_EDGE) BRepGProp::LinearProperties(sh, g); else BRepGProp::SurfaceProperties(sh, g);
        const gp_Pnt c = g.CentreOfMass();
        return {c.X(), c.Y(), c.Z(), g.Mass()};
    };
    std::vector<std::pair<std::array<double, 4>, int>> known;
    TopTools_IndexedMapOfShape olds;
    TopExp::MapShapes(old, ty, olds);
    for (int i = 1; i <= olds.Extent(); ++i)
        if (was.IsBound(olds(i))) known.push_back({key(olds(i)), was.Find(olds(i))});
    TopTools_IndexedMapOfShape news;
    TopExp::MapShapes(now, ty, news);
    std::vector<std::array<double, 4>> nk;
    for (int i = 1; i <= news.Extent(); ++i) nk.push_back(key(news(i)));
    TopTools_DataMapOfShapeInteger res;
    for (const auto& [k, id] : known) {
        int best = 0;
        double bd = 1e-6;
        for (int i = 1; i <= news.Extent(); ++i) {
            double d = 0.0;
            for (int j = 0; j < 4; ++j) d = std::max(d, std::abs(k[j] - nk[i - 1][j]));
            if (d < bd) { bd = d; best = i; }
        }
        if (best == 0) continue;
        if (!res.IsBound(news(best))) res.Bind(news(best), id);
        else if (absorbed && (id & QYM_NAMED) && (res.Find(news(best)) & QYM_NAMED) && id != res.Find(news(best)))
            absorbed->emplace_back((unsigned)id, (unsigned)res.Find(news(best)));
    }
    out = res;
}

// A FACE NARROWER THAN THE PRECISION IS A CRACK, NOT A FACE: taken out, its neighbours meeting across where it lay,
// and the number taken out returned. A rounding of a radius equal to the width of the wall it runs into takes on the
// step back of 2e-8 (the limiting case) and leaves 4e-8 of that wall standing - a strip of 1.6e-7 mm^2 beside the next
// corner - and a rounding of that corner afterwards fails: of four corners of two 2 mm pockets rounded R2 after R2 on
// their far corners, the two with the strip beside them refused and the other two took (+1.29 mm^3 each), and with the
// strips taken out all four take. It sits in the common `finish` funnel: any operation can leave such a strip.
// Only a body with a face under 1e-3 mm^2 is looked at, and the result is kept only if it is sound, has fewer faces
// and the same volume.
// IS THE FACE A CRACK? Under 1e-3 mm^2 of area AND narrow: its width - twice the area over the length of its border -
// under 1e-5 mm. The strip a limiting rounding leaves is 1.6e-7 mm^2 along millimetres, 4e-8 wide; a triangle of a
// mesh as small as 2e-4 mm^2 is 1e-2 wide, and the area alone took thousands of those for cracks on every operation
// over a body made of a mesh (800 of 800 faces of a 0.2 mm ball).
static bool crack_face(const TopoDS_Shape& f) {
    // a rough area first: the question is only whether it is small, and the full integration over the fitted walls of
    // a simplified handle took 1.4 s of an 11.6 s node for 162 faces none of which was
    GProp_GProps rough;
    BRepGProp::SurfaceProperties(f, rough, 0.1);
    if (rough.Mass() >= 1e-2) return false;
    GProp_GProps g;
    BRepGProp::SurfaceProperties(f, g);
    if (g.Mass() >= 1e-3) return false;
    GProp_GProps border;
    BRepGProp::LinearProperties(f, border);
    return border.Mass() > 0.0 && 2.0 * g.Mass() / border.Mass() < 1e-5;
}

// How many faces of the body are cracks.
extern "C" int qym_shape_sliver_count(const QymShape* s) {
    if (!s || s->shape.IsNull()) return 0;
    try {
        int n = 0;
        for (TopExp_Explorer fx(s->shape, TopAbs_FACE); fx.More(); fx.Next()) n += crack_face(fx.Current());
        return n;
    } catch (...) { return 0; }
}

extern "C" int qym_shape_drop_slivers(QymShape* s) {
    if (!s || s->shape.IsNull()) return 0;
    try {
        int cracks = 0, before = 0; // not `small`: a macro of the Windows headers
        for (TopExp_Explorer fx(s->shape, TopAbs_FACE); fx.More(); fx.Next()) {
            ++before;
            cracks += crack_face(fx.Current());
        }
        if (cracks == 0) return 0;
        // on a copy: the fixing edits the edges and faces it is handed in place, whether or not its result is kept -
        // run on the body itself it took names off a threaded shaft it then left as it was (88 faces, 86 names)
        const TopoDS_Shape copy = BRepBuilderAPI_Copy(s->shape).Shape();
        Handle(ShapeFix_FixSmallFace) fix = new ShapeFix_FixSmallFace();
        fix->Init(copy);
        fix->SetPrecision(1e-6);
        fix->SetMaxTolerance(1e-5);
        fix->Perform();
        const TopoDS_Shape out = fix->FixShape();
        if (out.IsNull() || !BRepCheck_Analyzer(out).IsValid()) return 0;
        int after = 0;
        for (TopExp_Explorer fx(out, TopAbs_FACE); fx.More(); fx.Next()) ++after;
        if (after >= before) return 0;
        GProp_GProps v0, v1;
        BRepGProp::VolumeProperties(s->shape, v0);
        BRepGProp::VolumeProperties(out, v1);
        if (std::abs(v1.Mass() - v0.Mass()) > 1e-6 * std::max(1.0, std::abs(v0.Mass()))) return 0;
        // the names by where things lie: the fixing hands on no history, and every edge and face but the strips
        // stays where it was to 1e-6 - the two long sides of a strip become one edge, one name keeping it and the
        // other recorded as yielded to it, so a reference to either still finds the edge
        int nf = next_local(s->fids), ne = next_local(s->eids);
        names_by_place(TopAbs_FACE, s->shape, s->fids, out, s->fids, &s->absorbed);
        names_by_place(TopAbs_EDGE, s->shape, s->eids, out, s->eids, &s->absorbed);
        // what moved by more than 1e-6 with the strips gone - the neighbours they were cut from - is named afresh
        fill_unnamed(out, TopAbs_FACE, s->fids, nf);
        fill_unnamed(out, TopAbs_EDGE, s->eids, ne);
        s->shape = out;
        return before - after;
    } catch (...) { return 0; } // it did not work out; the solid stays as it was, no worse
}

// A SEAM ON THE END OF AN EDGE TO BE BLENDED, moved away.
//
// A closed round face - a cylinder, a cone, a ball, a torus - carries its seam as an edge. Where that seam starts on a vertex of an
// edge to be rounded, it continues the edge in a straight line, and the rolling ball has to end against an edge
// that goes straight on: the kernel gives up at any radius. Measured: of eight edges of a symmetric part one would
// not take 8.6, nor 2, because the seam of the round corner beside it ran up from its end; at the other corners the
// seam lay elsewhere and the same edge took any radius. The seam is where the face happened to be made, not a
// property of the shape - on any part a person builds it can fall on the end of an edge.
//
// The face is cut in two along the line of its surface opposite the seam, both halves are laid on the same surface
// turned half a turn - whose own seam is that very line - and glued back: the same surface, the same body, the seam
// now opposite the edge. Names travel through every step. Returns whether any seam was moved.
static thread_local int again = 0; // the rounding tried once more after its seams were moved
// THE SAME SURFACE WITH ITS SEAM HALF A TURN AWAY from the seam edge `seam` of face `f`. A seam stands where the
// parameter starts: a round surface turned half a turn about its own axis is the very same surface with its parameter
// starting opposite. A seam along the other parameter - the circle of a torus - moves with the torus made a circle swept
// about the axis, the circle starting on its far side. Null when it cannot be moved.
struct Turn {
    Handle(Geom_Surface) surface; // the same surface, its seam half a turn away
    double du = 0.0, dv = 0.0;    // what a point's parameters gain on it: the old (u, v) is the new (u + du, v + dv)
};

static Turn turned_surface(const TopoDS_Face& f, const TopoDS_Edge& seam) {
    TopLoc_Location loc;
    const Handle(Geom_Surface) g = BRep_Tool::Surface(f, loc);
    if (g.IsNull() || !loc.IsIdentity()) return {};
    // which parameter the seam cuts across: it runs along the other one, at a constant value of its own
    Standard_Real a, b;
    const Handle(Geom2d_Curve) pc = BRep_Tool::CurveOnSurface(seam, f, a, b);
    if (pc.IsNull()) return {};
    const gp_Pnt2d p0 = pc->Value(a), p1 = pc->Value(b);
    const bool at_u = std::abs(p1.X() - p0.X()) < std::abs(p1.Y() - p0.Y()); // a seam at a constant U
    Handle(Geom_Surface) base = g;
    if (auto tr = Handle(Geom_RectangularTrimmedSurface)::DownCast(g)) base = tr->BasisSurface();
    // turned half a turn about its own axis, a round surface's angle of every point drops by pi
    auto about_axis = [&](gp_Ax3 ax) { ax.Rotate(ax.Axis(), M_PI); return ax; };
    if (at_u) {
        if (auto c = Handle(Geom_CylindricalSurface)::DownCast(base)) return {new Geom_CylindricalSurface(about_axis(c->Position()), c->Radius()), -M_PI, 0.0};
        if (auto c = Handle(Geom_ConicalSurface)::DownCast(base)) return {new Geom_ConicalSurface(about_axis(c->Position()), c->SemiAngle(), c->RefRadius()), -M_PI, 0.0};
        if (auto c = Handle(Geom_SphericalSurface)::DownCast(base)) return {new Geom_SphericalSurface(about_axis(c->Position()), c->Radius()), -M_PI, 0.0};
        if (auto c = Handle(Geom_ToroidalSurface)::DownCast(base)) return {new Geom_ToroidalSurface(about_axis(c->Position()), c->MajorRadius(), c->MinorRadius()), -M_PI, 0.0};
        if (auto c = Handle(Geom_SurfaceOfRevolution)::DownCast(base)) {
            Handle(Geom_Curve) turned = Handle(Geom_Curve)::DownCast(c->BasisCurve()->Copy());
            turned->Rotate(c->Axis(), M_PI);
            return {new Geom_SurfaceOfRevolution(turned, c->Axis()), -M_PI, 0.0};
        }
        return {};
    }
    // a torus across its tube: the same torus as a circle swept about the axis, the circle starting on its far side
    if (auto c = Handle(Geom_ToroidalSurface)::DownCast(base)) {
        const gp_Ax3& p = c->Position();
        if (!p.Direct()) return {};
        const gp_Pnt centre = p.Location().Translated(gp_Vec(p.XDirection()) * c->MajorRadius());
        // the circle C(v) = centre + r (cos v X + sin v Z): its normal is X ^ Z = -Y; started at -X it is C(v' + pi)
        const Handle(Geom_Circle) tube = new Geom_Circle(gp_Ax2(centre, p.YDirection().Reversed(), p.XDirection().Reversed()), c->MinorRadius());
        return {new Geom_SurfaceOfRevolution(tube, p.Axis()), 0.0, -M_PI};
    }
    return {};
}

// THE PIECES OF A ROUND FACE LAID ON THE TURNED SURFACE, their edges and vertices where they were: the lines on each piece
// are the old ones moved by the shift of the parameters, exactly, the poles of a ball included - worked out afresh by
// projection they come out a period off at a pole, and the piece is then wrong in the plane of its parameters.
class TurnPieces : public BRepTools_Modification {
public:
    struct On { Handle(Geom_Surface) surface; double du, dv; };
    std::vector<On> on;
    TopTools_DataMapOfShapeInteger face_to; // a piece -> its entry in `on`

    Standard_Boolean NewSurface(const TopoDS_Face& F, Handle(Geom_Surface)& S, TopLoc_Location& L, Standard_Real& Tol, Standard_Boolean& RevWires, Standard_Boolean& RevFace) override {
        if (!face_to.IsBound(F)) return Standard_False;
        S = on[face_to.Find(F)].surface;
        L = TopLoc_Location();
        Tol = BRep_Tool::Tolerance(F);
        RevWires = RevFace = Standard_False;
        return Standard_True;
    }
    Standard_Boolean NewCurve(const TopoDS_Edge&, Handle(Geom_Curve)&, TopLoc_Location&, Standard_Real&) override { return Standard_False; }
    Standard_Boolean NewPoint(const TopoDS_Vertex&, gp_Pnt&, Standard_Real&) override { return Standard_False; }
    Standard_Boolean NewCurve2d(const TopoDS_Edge& E, const TopoDS_Face& F, const TopoDS_Edge&, const TopoDS_Face&, Handle(Geom2d_Curve)& C, Standard_Real& Tol) override {
        if (!face_to.IsBound(F)) return Standard_False;
        const On& o = on[face_to.Find(F)];
        Standard_Real a, b;
        const Handle(Geom2d_Curve) old = BRep_Tool::CurveOnSurface(E, F, a, b);
        if (old.IsNull()) return Standard_False;
        C = Handle(Geom2d_Curve)::DownCast(old->Copy());
        C->Translate(gp_Vec2d(o.du, o.dv));
        Tol = BRep_Tool::Tolerance(E);
        return Standard_True;
    }
    Standard_Boolean NewParameter(const TopoDS_Vertex&, const TopoDS_Edge&, Standard_Real&, Standard_Real&) override { return Standard_False; }
    GeomAbs_Shape Continuity(const TopoDS_Edge& E, const TopoDS_Face& F1, const TopoDS_Face& F2, const TopoDS_Edge&, const TopoDS_Face&, const TopoDS_Face&) override {
        return BRep_Tool::Continuity(E, F1, F2);
    }
};

// A BLEND TRIED AGAIN OFF THE SEAMS: when a rounding or a bevel did not come out, the seams on the ends of its edges
// are moved away (move_seams_off) and `op` is run once more on that body; what comes out is glued back so the lines
// the moving left behind do not stay on the part. Null when there was no seam to move or it did not help.
static QymShape* off_seams(const QymShape* s, const uint32_t* idx, size_t n, const std::function<QymShape*(const QymShape*)>& op);

// THE LINES OF A FACE ALONG ITS PARAMETERS MADE EXACT: the cutting of a closed face lays its new lines as splines of the
// eighth degree, a hair off the circle of a ball's meridian (1e-5), and a line like that turned into the seam spoils
// the blend at the pole - the ball whose seam lies there exactly takes the same edge (the volume 454.364 either way).
// A line whose trace on the face runs straight along one parameter IS the iso line of the surface: given it, it is exact.
static void exact_isos(const TopoDS_Face& f) {
    TopLoc_Location sl;
    const Handle(Geom_Surface) surf = BRep_Tool::Surface(f, sl);
    if (surf.IsNull() || !sl.IsIdentity()) return;
    BRep_Builder bb;
    for (TopExp_Explorer ex(f, TopAbs_EDGE); ex.More(); ex.Next()) {
        const TopoDS_Edge e = TopoDS::Edge(ex.Current());
        if (BRep_Tool::Degenerated(e)) continue;
        Standard_Real c0, c1, p0, p1;
        TopLoc_Location cl;
        const Handle(Geom_Curve) c3 = BRep_Tool::Curve(e, cl, c0, c1);
        if (c3.IsNull() || !cl.IsIdentity() || !Handle(Geom_BSplineCurve)::DownCast(c3)) continue;
        Handle(Geom2d_Curve) pc = BRep_Tool::CurveOnSurface(e, f, p0, p1);
        if (auto tr = Handle(Geom2d_TrimmedCurve)::DownCast(pc)) pc = tr->BasisCurve();
        const Handle(Geom2d_Line) ln = Handle(Geom2d_Line)::DownCast(pc);
        if (ln.IsNull()) continue;
        const gp_Pnt2d o = ln->Location();
        const gp_Dir2d d = ln->Direction();
        const bool along_v = std::abs(d.X()) < 1e-12, along_u = std::abs(d.Y()) < 1e-12;
        if (!along_v && !along_u) continue;
        // the iso line and where the trace starts on it: the edge at its parameter t lies at iso(at + sense * t)
        Handle(Geom_Curve) iso = along_v ? surf->UIso(o.X()) : surf->VIso(o.Y());
        if (auto tr = Handle(Geom_TrimmedCurve)::DownCast(iso)) iso = tr->BasisCurve(); // a meridian comes trimmed to the poles
        const double at = along_v ? o.Y() : o.X(), sense = along_v ? d.Y() : d.X();
        Handle(Geom_Curve) exact;
        if (auto l = Handle(Geom_Line)::DownCast(iso)) {
            exact = new Geom_Line(gp_Ax1(l->Value(at), sense > 0 ? l->Position().Direction() : l->Position().Direction().Reversed()));
        } else if (auto ci = Handle(Geom_Circle)::DownCast(iso)) {
            const gp_Pnt centre = ci->Location();
            gp_Pnt start;
            gp_Vec tangent;
            ci->D1(at, start, tangent);
            const gp_Dir x(gp_Vec(centre, start)), y(tangent * sense);
            exact = new Geom_Circle(gp_Ax2(centre, x.Crossed(y), x), ci->Radius());
        }
        if (exact.IsNull()) continue;
        // no worse than the spline it replaces, over its own range
        double off = 0.0;
        for (int k = 0; k <= 8; ++k) {
            const double t = c0 + (c1 - c0) * k / 8.0;
            off = std::max(off, exact->Value(t).Distance(c3->Value(t)));
        }
        if (off > 10.0 * BRep_Tool::Tolerance(e) + 1e-7) continue;
        bb.UpdateEdge(e, exact, BRep_Tool::Tolerance(e));
    }
}

static bool move_seams_off(QymShape* q, const uint32_t* idx, size_t n) {
    // the vertices of the edges to be blended, by where they stand: the seam and the edge may each carry their own
    // vertex at the one point
    std::vector<gp_Pnt> ends;
    for (TopExp_Explorer ex(q->shape, TopAbs_EDGE); ex.More(); ex.Next()) {
        if (!q->eids.IsBound(ex.Current())) continue;
        const uint32_t id = (uint32_t)q->eids.Find(ex.Current());
        for (size_t k = 0; k < n; ++k)
            if (idx[k] == id) {
                for (TopExp_Explorer vx(ex.Current(), TopAbs_VERTEX); vx.More(); vx.Next()) ends.push_back(BRep_Tool::Pnt(TopoDS::Vertex(vx.Current())));
                break;
            }
    }
    auto on_end = [&](const gp_Pnt& p) { for (const auto& e : ends) if (e.Distance(p) < 1e-5) return true; return false; };
    // the round faces to turn: a face whose seam has a vertex on the end of an edge to be blended, with that seam
    std::vector<std::pair<TopoDS_Face, TopoDS_Edge>> to_turn;
    for (TopExp_Explorer fx(q->shape, TopAbs_FACE); fx.More(); fx.Next()) {
        const TopoDS_Face f = TopoDS::Face(fx.Current());
        for (TopExp_Explorer ex(f, TopAbs_EDGE); ex.More(); ex.Next()) {
            const TopoDS_Edge e = TopoDS::Edge(ex.Current());
            if (!BRep_Tool::IsClosed(e, f)) continue;
            bool touches = false;
            for (TopExp_Explorer vx(e, TopAbs_VERTEX); vx.More() && !touches; vx.Next()) touches = on_end(BRep_Tool::Pnt(TopoDS::Vertex(vx.Current())));
            if (touches) { to_turn.push_back({f, e}); break; }
        }
    }
    if (to_turn.empty()) return false;
    // every closed face cut in two, opposite its seam: no half is closed any more, so each can be laid on the turned
    // surface, and the cut is where that surface's own seam lies - glued back, the face takes its seam there. Cut in
    // thirds instead, the gluing put the seam on a third's line, 120 deg off, and the blend at a ball's pole failed
    ShapeUpgrade_ShapeDivideClosed div(q->shape);
    div.SetNbSplitPoints(1);
    if (!div.Perform()) return false;
    const Handle(ShapeBuild_ReShape) ctx = div.GetContext();
    QymShape cut{div.Result(), {}, {}, {}, {}};
    auto carry = [&](const TopTools_DataMapOfShapeInteger& from, TopTools_DataMapOfShapeInteger& to) {
        for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(from); it.More(); it.Next()) {
            TopoDS_Shape now = ctx->Value(it.Key());
            if (now.IsNull()) continue;
            // a face cut in pieces keeps its name on the first; the others are named afresh
            TopExp_Explorer first(now, it.Key().ShapeType());
            const TopoDS_Shape& one = first.More() ? first.Current() : now;
            if (!to.IsBound(one)) to.Bind(one, it.Value());
        }
    };
    carry(q->fids, cut.fids);
    carry(q->eids, cut.eids);
    // EVERY PIECE of such a face laid on the same round surface turned half a turn: glued back, the pieces make the one
    // face again, its seam now opposite, and nothing goes straight on from the end of the edge any more. All of them, not
    // only the two either side of the seam: a piece left on the old surface keeps a line between it and the turned ones,
    // and that line - a meridian of a ball - runs into the end of the edge at the pole just as the seam did
    Handle(TurnPieces) turn = new TurnPieces();
    for (const auto& [face, seam] : to_turn) {
        const Turn t = turned_surface(face, seam);
        if (t.surface.IsNull()) continue;
        for (TopExp_Explorer px(ctx->Value(face), TopAbs_FACE); px.More(); px.Next()) {
            const TopoDS_Face piece = TopoDS::Face(px.Current());
            // each piece on its own copy of the surface, its lines moved by the shift and by whole periods so that the
            // piece lies in one run of the parameters: two pieces sharing one surface would share one line between them,
            // a period apart on either side of the new seam
            Standard_Real u0, u1, v0, v1;
            BRepTools::UVBounds(piece, u0, u1, v0, v1);
            const Handle(Geom_Surface) own = Handle(Geom_Surface)::DownCast(t.surface->Copy());
            auto into_period = [](double mid, double d, bool periodic, double period) {
                if (!periodic || d == 0.0) return d;
                const double at = mid + d;
                return d - std::floor(at / period) * period; // the piece's middle in [0, period)
            };
            const double du = into_period(0.5 * (u0 + u1), t.du, own->IsUPeriodic(), own->IsUPeriodic() ? own->UPeriod() : 0.0);
            const double dv = into_period(0.5 * (v0 + v1), t.dv, own->IsVPeriodic(), own->IsVPeriodic() ? own->VPeriod() : 0.0);
            turn->face_to.Bind(piece, (int)turn->on.size());
            turn->on.push_back({own, du, dv});
        }
    }
    if (turn->on.empty()) return false;
    BRepTools_Modifier mod(cut.shape, turn);
    if (!mod.IsDone()) return false;
    TopoDS_Shape remade = mod.ModifiedShape(cut.shape);
    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(turn->face_to); it.More(); it.Next()) exact_isos(TopoDS::Face(mod.ModifiedShape(it.Key())));
    auto through = [&](TopTools_DataMapOfShapeInteger& ids) {
        TopTools_DataMapOfShapeInteger out;
        for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(ids); it.More(); it.Next()) {
            TopoDS_Shape now;
            try { now = mod.ModifiedShape(it.Key()); } catch (...) { continue; }
            if (!now.IsNull() && !out.IsBound(now)) out.Bind(now, it.Value());
        }
        ids = out;
    };
    through(cut.fids);
    through(cut.eids);
    // A NAME THE HISTORY DID NOT HAND ON is found by where the thing lies: the cutting and the healing rebuild a seam
    // edge and its neighbours without saying what became of them, while every edge and face but the one face turned
    // stays where it was - the same ends and middle for an edge, the same centre and area for a face
    auto by_place = [&](TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& was, TopTools_DataMapOfShapeInteger& now) {
        std::vector<std::pair<std::array<double, 7>, int>> known;
        auto key = [&](const TopoDS_Shape& sh) -> std::array<double, 7> {
            if (ty == TopAbs_EDGE) {
                // the centre and the length of the line: neither depends on which way it runs or where a closed one
                // starts - the circle of a tube starts elsewhere once its seam is moved
                GProp_GProps g;
                BRepGProp::LinearProperties(sh, g);
                const gp_Pnt c = g.CentreOfMass();
                return {c.X(), c.Y(), c.Z(), g.Mass(), 0.0, 0.0, 0.0};
            }
            GProp_GProps g;
            BRepGProp::SurfaceProperties(sh, g);
            const gp_Pnt c = g.CentreOfMass();
            return {c.X(), c.Y(), c.Z(), g.Mass(), 0.0, 0.0, 0.0};
        };
        TopTools_IndexedMapOfShape olds;
        TopExp::MapShapes(q->shape, ty, olds);
        for (int i = 1; i <= olds.Extent(); ++i)
            if (was.IsBound(olds(i))) known.push_back({key(olds(i)), was.Find(olds(i))});
        // the place decides where it is exact: a thing lying just where a named one lay takes that name, whatever the
        // history handed it, and the name is taken off anything else that carried it
        TopTools_IndexedMapOfShape news;
        TopExp::MapShapes(remade, ty, news);
        for (int i = 1; i <= news.Extent(); ++i) {
            const auto k = key(news(i));
            for (const auto& [kk, id] : known) {
                bool same = true;
                for (int j = 0; j < 7 && same; ++j) same = std::abs(k[j] - kk[j]) < 1e-6;
                if (!same) continue;
                std::vector<TopoDS_Shape> other;
                for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(now); it.More(); it.Next())
                    if (it.Value() == id && !it.Key().IsSame(news(i))) other.push_back(it.Key());
                for (const auto& o : other) now.UnBind(o);
                if (now.IsBound(news(i))) now.UnBind(news(i));
                now.Bind(news(i), id);
                break;
            }
        }
    };
    by_place(TopAbs_EDGE, q->eids, cut.eids);
    // AN EDGE TO BE BLENDED CUT IN PIECES by the cutting of a face closed both ways - the circle of a tube - hands its
    // name to every piece lying on it: the blend takes the chain, and the gluing after it joins the pieces again
    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(q->eids); it.More(); it.Next()) {
        bool asked = false;
        for (size_t k = 0; k < n && !asked; ++k) asked = idx[k] == (uint32_t)it.Value();
        if (!asked) continue;
        Standard_Real w0, w1;
        const Handle(Geom_Curve) whole = BRep_Tool::Curve(TopoDS::Edge(it.Key()), w0, w1); // placed in space
        if (whole.IsNull()) continue;
        int pieces = 0;
        bool found = false;
        for (TopTools_DataMapIteratorOfDataMapOfShapeInteger jt(cut.eids); jt.More() && !found; jt.Next()) found = jt.Value() == it.Value();
        if (found) continue;
        TopTools_IndexedMapOfShape news;
        TopExp::MapShapes(remade, TopAbs_EDGE, news);
        for (int i = 1; i <= news.Extent(); ++i) {
            BRepAdaptor_Curve c(TopoDS::Edge(news(i)));
            bool on = true;
            for (double t : {0.0, 0.5, 1.0}) {
                const gp_Pnt pnt = c.Value(c.FirstParameter() + t * (c.LastParameter() - c.FirstParameter()));
                ShapeAnalysis_Curve sac;
                gp_Pnt proj;
                Standard_Real prm;
                on = on && sac.Project(whole, pnt, 1e-6, proj, prm) < 1e-6;
            }
            if (on && !cut.eids.IsBound(news(i))) { cut.eids.Bind(news(i), it.Value()); ++pieces; }
        }
    }
    by_place(TopAbs_FACE, q->fids, cut.fids);
    int nf = next_local(q->fids), ne = next_local(q->eids);
    fill_unnamed(remade, TopAbs_FACE, cut.fids, nf);
    fill_unnamed(remade, TopAbs_EDGE, cut.eids, ne);
    // THE VERTICES THE BODY HAD are kept through the gluing: the lines meeting at one are then not joined into a line
    // made afresh - a circle joined so comes out running the other way (its axis -Z where it was +Z), and the bevel of
    // a cone and a plane is refused on it (the top of a cut cone: taken with its own circle, refused with the new one)
    TopTools_MapOfShape keep;
    for (TopExp_Explorer vx(q->shape, TopAbs_VERTEX); vx.More(); vx.Next()) {
        const TopoDS_Shape a = ctx->Value(vx.Current());
        if (a.IsNull() || a.ShapeType() != TopAbs_VERTEX) continue;
        try { const TopoDS_Shape b = mod.ModifiedShape(a); if (!b.IsNull()) keep.Add(b); } catch (...) {}
    }
    TopoDS_Shape glued = unify_monolithic(remade, cut.fids, cut.eids, &q->absorbed, &keep);
    // HEALED before the blend: the gluing leaves the faces of the ball all forward, a wire of one against its face, and
    // the blend at the pole then comes out with a face turned wrong (BRepCheck_BadOrientationOfSubshape) and less taken
    // off (455.673 of 458.129 left); healed first it takes what the ball with its seam there takes (454.364)
    {
        ShapeFix_Shape heal(glued);
        heal.Perform();
        const Handle(ShapeBuild_ReShape) hc = heal.Context();
        auto healed = [&](TopTools_DataMapOfShapeInteger& ids) {
            TopTools_DataMapOfShapeInteger out;
            for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(ids); it.More(); it.Next()) {
                TopoDS_Shape now = hc.IsNull() ? it.Key() : hc->Value(it.Key());
                if (now.IsNull()) continue;
                if (now.ShapeType() != it.Key().ShapeType()) {
                    TopExp_Explorer first(now, it.Key().ShapeType());
                    if (!first.More()) continue;
                    now = first.Current();
                }
                if (!out.IsBound(now)) out.Bind(now, it.Value());
            }
            ids = out;
        };
        healed(cut.fids);
        healed(cut.eids);
        glued = heal.Shape();
    }
    q->shape = glued;
    q->fids = cut.fids;
    q->eids = cut.eids;
    return true;
}

static QymShape* off_seams(const QymShape* s, const uint32_t* idx, size_t n, const std::function<QymShape*(const QymShape*)>& op) {
    if (again > 0) return nullptr;
    QymShape moved{s->shape, s->fids, s->eids, s->fsplit_of, s->fsplit_idx};
    if (!move_seams_off(&moved, idx, n)) return nullptr;
    ++again;
    QymShape* q = op(&moved);
    --again;
    // the lines the moving left are glued away: the part keeps the faces it had, with the blend on it
    if (q) q->shape = unify_monolithic(q->shape, q->fids, q->eids, &q->absorbed);
    return q;
}

// Every edge of a body as the list of their names, for the blends that take every edge.
static std::vector<uint32_t> every_edge(const QymShape* s) {
    std::vector<uint32_t> out;
    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(s->eids); it.More(); it.Next()) out.push_back((uint32_t)it.Value());
    return out;
}

extern "C" QymShape* qym_shape_fillet_edges(const QymShape* s, double r, const uint32_t* idx, size_t n, const unsigned* names, const unsigned* corners,
                                 const unsigned* all_names, size_t n_all) {
    if (!s) return why("fillet/asked", "there is no body to round"), nullptr;
    if (r <= 0.0) return why("fillet/asked", "the radius is zero"), nullptr;
    if (n == 0) return why("fillet/asked", "not one edge was named to round"), nullptr;
    try {
        if (along_a_seam(s, idx, n)) return off_seams_first(s, idx, n, [&](const QymShape* m) { return qym_shape_fillet_edges(m, r, idx, n, names, corners, all_names, n_all); });
        BRepFilletAPI_MakeFillet mk(s->shape);
        if (add_fillet_edges_by_id(mk, s, r, idx, n) == 0) return why_no_named_edges("fillet/edges", s, n), nullptr;
        mk.Build();
        // built but not whole is a failure too: the rounding against a seam that goes straight on from its end comes
        // back as a body the check refuses
        if (!mk.IsDone() || !BRepCheck_Analyzer(mk.Shape()).IsValid()) {
            // a seam on the end of an edge is moved away and the rounding tried once more
            if (QymShape* q = off_seams(s, idx, n, [&](const QymShape* m) { return qym_shape_fillet_edges(m, r, idx, n, names, corners, all_names, n_all); })) return q;
            return why("fillet/build", "the kernel could not round these edges at this radius"), nullptr;
        }
        QymShape* q = new QymShape{mk.Shape(), {}, {}, {}, {}};
        // ORDER OF WORK: ALL THE NAMES FIRST, THE NUMBERS AFTERWARDS.
        //
        // `propagate_ids` hands positional numbers to everything unnamed at the end — and below it stand two
        // blocks whose whole business is names (the fillet surface from its own edge, the corner patch from a
        // vertex). Both check whether a name is taken, and after the numbers had been handed out EVERYTHING
        // was taken: the blocks did nothing. Measured on a real document: 25 faces without a name, 15 of them
        // the very corner patches a name was being prepared for.
        int nf = next_local(s->fids), ne = next_local(s->eids);
        carry_ids(mk, s->shape, TopAbs_FACE, s->fids, q->fids, nf, false, &q->fsplit_of, &q->fsplit_idx);
        carry_ids(mk, s->shape, TopAbs_EDGE, s->eids, q->eids, ne);
        // A FILLET SURFACE is named after the EDGE that generated it: it has no other source in the recipe,
        // and an ordinal number would drift with any edit further up the timeline. The names arrive in parallel
        // with the list of selected edges.
        name_blend_faces(mk, s, q, idx, n, names, corners, nf, all_names, n_all);
        classify_unnamed(mk, s->shape, q, "fillet/chamfer", &s->fids);
        fill_unnamed(q->shape, TopAbs_FACE, q->fids, nf);
        fill_unnamed(q->shape, TopAbs_EDGE, q->eids, ne);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}
// A VARIABLE fillet: the radius runs linearly from r1 to r2 along the selected edges (by persistent id).
// A FILLET OF VARIABLE RADIUS, SET AT THE VERTICES.
//
// The former input took "the radius at the edge's start -> the radius at its end". That describes ONE edge,
// which has a direction; a SET of edges has none, and on a chain such a parameter is meaningless: neighbouring
// edges would meet at their shared vertex with different radii, forming a step.
//
// The radius is set AT A VERTEX. An edge gets the radii of its two ends, and the kernel carries the law
// between them itself (`Add(r1, r2, E)` is a linear law from the first end to the last). Neighbours share one
// vertex, hence one radius there — consistency follows on its own rather than from checks.
//
// The vertices arrive as POINTS rather than names: names are the model's business and geometry is enough for
// the kernel. A match is found by the nearest point within tolerance; an end with no entry takes the default
// radius.
extern "C" QymShape* qym_shape_fillet_at_vertices(const QymShape* s, double r_default, const uint32_t* idx, size_t n,
                                       const double* vpts, const double* vrads, size_t m_count, double tol) {
    if (!s) return why("fillet/asked", "there is no body to round"), nullptr;
    if (r_default <= 0.0) return why("fillet/asked", "the default radius is zero"), nullptr;
    if (n == 0) return why("fillet/asked", "not one edge was named to round"), nullptr;
    try {
        if (along_a_seam(s, idx, n)) return off_seams_first(s, idx, n, [&](const QymShape* m) { return qym_shape_fillet_at_vertices(m, r_default, idx, n, vpts, vrads, m_count, tol); });
        auto radius_at = [&](const gp_Pnt& p) -> double {
            double best = tol > 0.0 ? tol : 1e-6, out = r_default;
            for (size_t k = 0; k < m_count; ++k) {
                const double dx = p.X() - vpts[3 * k], dy = p.Y() - vpts[3 * k + 1], dz = p.Z() - vpts[3 * k + 2];
                const double d = std::sqrt(dx * dx + dy * dy + dz * dz);
                if (d <= best) { best = d; out = vrads[k]; }
            }
            return out > 0.0 ? out : r_default;
        };
        BRepFilletAPI_MakeFillet mk(s->shape);
        int added = 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next()) {
            if (!s->eids.IsBound(ex.Current())) continue;
            uint32_t id = static_cast<uint32_t>(s->eids.Find(ex.Current()));
            bool want = false;
            for (size_t k = 0; k < n && !want; ++k) want = (idx[k] == id);
            if (!want) continue;
            const TopoDS_Edge& e = TopoDS::Edge(ex.Current());
            TopoDS_Vertex v0 = TopExp::FirstVertex(e, Standard_True), v1 = TopExp::LastVertex(e, Standard_True);
            if (v0.IsNull() || v1.IsNull()) continue;
            mk.Add(radius_at(BRep_Tool::Pnt(v0)), radius_at(BRep_Tool::Pnt(v1)), e);
            ++added;
        }
        if (added == 0) return why_no_named_edges("fillet/edges", s, n), nullptr;
        mk.Build();
        if (!mk.IsDone() || !BRepCheck_Analyzer(mk.Shape()).IsValid()) {
            if (QymShape* q = off_seams(s, idx, n, [&](const QymShape* m) { return qym_shape_fillet_at_vertices(m, r_default, idx, n, vpts, vrads, m_count, tol); })) return q;
            return why("fillet/build", "the kernel could not round these edges with the radii given at their vertices"), nullptr;
        }
        QymShape* q = new QymShape{mk.Shape(), {}, {}, {}, {}};
        propagate_ids(mk, s->shape, TopAbs_FACE, s->fids, q->shape, q->fids);
        propagate_ids(mk, s->shape, TopAbs_EDGE, s->eids, q->shape, q->eids);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

extern "C" QymShape* qym_shape_fillet_var(const QymShape* s, double r1, double r2, const uint32_t* idx, size_t n) {
    if (!s) return why("fillet/asked", "there is no body to round"), nullptr;
    if (r1 <= 0.0 || r2 <= 0.0) return why("fillet/asked", "one end of the variable radius is zero"), nullptr;
    if (n == 0) return why("fillet/asked", "not one edge was named to round"), nullptr;
    try {
        if (along_a_seam(s, idx, n)) return off_seams_first(s, idx, n, [&](const QymShape* m) { return qym_shape_fillet_var(m, r1, r2, idx, n); });
        BRepFilletAPI_MakeFillet mk(s->shape);
        int added = 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next()) {
            if (!s->eids.IsBound(ex.Current())) continue;
            uint32_t id = static_cast<uint32_t>(s->eids.Find(ex.Current()));
            for (size_t k = 0; k < n; ++k) {
                if (idx[k] == id) { mk.Add(r1, r2, TopoDS::Edge(ex.Current())); ++added; break; }
            }
        }
        if (added == 0) return why_no_named_edges("fillet/edges", s, n), nullptr;
        mk.Build();
        if (!mk.IsDone() || !BRepCheck_Analyzer(mk.Shape()).IsValid()) {
            if (QymShape* q = off_seams(s, idx, n, [&](const QymShape* m) { return qym_shape_fillet_var(m, r1, r2, idx, n); })) return q;
            return why("fillet/build", "the kernel could not round these edges with a radius running between the two ends"), nullptr;
        }
        QymShape* q = new QymShape{mk.Shape(), {}, {}, {}, {}};
        propagate_ids(mk, s->shape, TopAbs_FACE, s->fids, q->shape, q->fids);
        propagate_ids(mk, s->shape, TopAbs_EDGE, s->eids, q->shape, q->eids);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}
extern "C" QymShape* qym_shape_chamfer_edges(const QymShape* s, double d, const uint32_t* idx, size_t n,
                                  const unsigned* names, const unsigned* corners, const unsigned* all_names, size_t n_all) {
    if (!s) return why("chamfer/asked", "there is no body to bevel"), nullptr;
    if (d <= 0.0) return why("chamfer/asked", "the setback is zero"), nullptr;
    if (n == 0) return why("chamfer/asked", "not one edge was named to bevel"), nullptr;
    try {
        if (along_a_seam(s, idx, n)) return off_seams_first(s, idx, n, [&](const QymShape* m) { return qym_shape_chamfer_edges(m, d, idx, n, names, corners, all_names, n_all); });
        BRepFilletAPI_MakeChamfer mk(s->shape);
        if (add_chamfer_edges_by_id(mk, s, d, idx, n) == 0) return why_no_named_edges("chamfer/edges", s, n), nullptr;
        mk.Build();
        if (!mk.IsDone() || !BRepCheck_Analyzer(mk.Shape()).IsValid()) {
            // the same seam as a rounding meets: moved off the ends of the edges, and the bevel tried once more
            if (QymShape* q = off_seams(s, idx, n, [&](const QymShape* m) { return qym_shape_chamfer_edges(m, d, idx, n, names, corners, all_names, n_all); })) return q;
            return why("chamfer/build", "the kernel could not bevel these edges at this setback"), nullptr;
        }
        QymShape* q = new QymShape{mk.Shape(), {}, {}, {}, {}};
        int nf = next_local(s->fids), ne = next_local(s->eids);
        carry_ids(mk, s->shape, TopAbs_FACE, s->fids, q->fids, nf, false, &q->fsplit_of, &q->fsplit_idx);
        carry_ids(mk, s->shape, TopAbs_EDGE, s->eids, q->eids, ne);
        name_blend_faces(mk, s, q, idx, n, names, corners, nf, all_names, n_all);
        classify_unnamed(mk, s->shape, q, "fillet/chamfer", &s->fids);
        fill_unnamed(q->shape, TopAbs_FACE, q->fids, nf);
        fill_unnamed(q->shape, TopAbs_EDGE, q->eids, ne);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// An ASYMMETRIC chamfer of the selected edges: mode 1 takes two distances (a is the leg ON the reference face,
// b the one on the adjacent face); mode 2 takes a leg and an angle (a is the leg on the reference face, b the
// angle in DEGREES from it). The reference face for each edge is chosen deterministically out of the two
// adjacent ones: flip 0 takes the first in the traversal, flip 1 the second. Edges with a single adjacent face
// (an open shell) get a symmetric chamfer `a`.
extern "C" QymShape* qym_shape_chamfer_edges_asym(const QymShape* s, double a, double b, int mode, int flip,
                                       uint32_t ref_face, const uint32_t* idx, size_t n) {
    if (!s) return why("chamfer/asked", "there is no body to bevel"), nullptr;
    if (a <= 0.0) return why("chamfer/asked", "the first setback is zero"), nullptr;
    if (n == 0) return why("chamfer/asked", "not one edge was named to bevel"), nullptr;
    try {
        if (along_a_seam(s, idx, n)) return off_seams_first(s, idx, n, [&](const QymShape* m) { return qym_shape_chamfer_edges_asym(m, a, b, mode, flip, ref_face, idx, n); });
        BRepFilletAPI_MakeChamfer mk(s->shape);
        TopTools_IndexedDataMapOfShapeListOfShape efMap;
        TopExp::MapShapesAndAncestors(s->shape, TopAbs_EDGE, TopAbs_FACE, efMap);
        int added = 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next()) {
            if (!s->eids.IsBound(ex.Current())) continue;
            uint32_t id = static_cast<uint32_t>(s->eids.Find(ex.Current()));
            bool want = false;
            for (size_t k = 0; k < n; ++k) { if (idx[k] == id) { want = true; break; } }
            if (!want) continue;
            const TopoDS_Edge& e = TopoDS::Edge(ex.Current());
            if (!efMap.Contains(e)) continue;
            const TopTools_ListOfShape& faces = efMap.FindFromKey(e);
            if (faces.IsEmpty()) continue;
            // pick the reference face (the first or second, by flip); with a single face, take that one
            TopTools_ListIteratorOfListOfShape it(faces);
            TopoDS_Face f = TopoDS::Face(it.Value());
            if (flip && faces.Extent() >= 2) { it.Next(); f = TopoDS::Face(it.Value()); }
            // if a reference face was given by hand (by PERSISTENT id) and it is adjacent to THIS edge, that
            // is the reference (leg `a` lies on it), overriding the choice by flip. If it is not adjacent, flip
            // decides.
            if (ref_face != 0) {
                for (TopTools_ListIteratorOfListOfShape jt(faces); jt.More(); jt.Next()) {
                    const TopoDS_Shape& cand = jt.Value();
                    if (s->fids.IsBound(cand) && static_cast<uint32_t>(s->fids.Find(cand)) == ref_face) {
                        f = TopoDS::Face(cand);
                        break;
                    }
                }
            }
            if (faces.Extent() < 2) {
                mk.Add(a, e); // an edge on the boundary: asymmetry cannot be set, so the chamfer is symmetric
            } else if (mode == 2) {
                mk.AddDA(a, b * M_PI / 180.0, e, f); // leg a on face f plus angle b in degrees
            } else {
                mk.Add(a, b, e, f); // two distances: a on face f, b on the adjacent one
            }
            ++added;
        }
        if (added == 0) return why_no_named_edges("fillet/edges", s, n), nullptr;
        mk.Build();
        if (!mk.IsDone() || !BRepCheck_Analyzer(mk.Shape()).IsValid()) {
            if (QymShape* q = off_seams(s, idx, n, [&](const QymShape* m) { return qym_shape_chamfer_edges_asym(m, a, b, mode, flip, ref_face, idx, n); })) return q;
            return why("chamfer/build", "the kernel could not bevel these edges with two setbacks"), nullptr;
        }
        QymShape* q = new QymShape{mk.Shape(), {}, {}, {}, {}};
        propagate_ids(mk, s->shape, TopAbs_FACE, s->fids, q->shape, q->fids);
        propagate_ids(mk, s->shape, TopAbs_EDGE, s->eids, q->shape, q->eids);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// DRAFT: tilt the selected faces `ids` by `angle_deg` relative to the NEUTRAL plane (origin plus normal — the
// line where the face meets it stays put) in the pull direction `pull` (usually the neutral plane's normal, the
// direction of extraction from the mould). The faces keep their persistent ids. The angle's sign: positive
// means the face "opens up" along the pull direction.
// `sides` holds pairs "name of the source face -> name of the draft face it GENERATED", as a flat list.
extern "C" QymShape* qym_shape_draft(const QymShape* s, const uint32_t* ids, size_t n, double angle_deg,
                          const double* pull, const double* np_origin, const double* np_normal,
                          const unsigned* sides, size_t nsides) {
    if (!s) return why("draft/asked", "there is no body to tilt the faces of"), nullptr;
    if (!ids || n == 0) return why("draft/asked", "not one face was named to tilt"), nullptr;
    if (!pull || !np_origin || !np_normal) return why("draft/asked", "the pull direction or the neutral plane is missing"), nullptr;
    try {
        gp_Dir pdir(pull[0], pull[1], pull[2]);
        gp_Pln neutral(gp_Pnt(np_origin[0], np_origin[1], np_origin[2]), gp_Dir(np_normal[0], np_normal[1], np_normal[2]));
        double ang = angle_deg * M_PI / 180.0;
        BRepOffsetAPI_DraftAngle draft(s->shape);
        int added = 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            uint32_t fid = s->fids.IsBound(ex.Current()) ? static_cast<uint32_t>(s->fids.Find(ex.Current())) : 0u;
            if (fid == 0) continue;
            bool want = false;
            for (size_t k = 0; k < n; ++k) { if (ids[k] == fid) { want = true; break; } }
            if (!want) continue;
            draft.Add(TopoDS::Face(ex.Current()), pdir, ang, neutral);
            // this face cannot be tilted with these parameters
        if (!draft.AddDone()) return why("draft/face", "one of the named faces cannot be tilted at this angle from this neutral plane"), nullptr;
            ++added;
        }
        if (added == 0) return why_no_named_edges("fillet/edges", s, n), nullptr;
        draft.Build();
        if (!draft.IsDone()) return why("draft/build", "the kernel could not tilt these faces"), nullptr;
        TopoDS_Shape res = draft.Shape();
        if (res.IsNull()) return why("draft/build", "the kernel reported success and returned nothing"), nullptr;
        QymShape* q = new QymShape{res, {}, {}, {}, {}};
        // A FACE GENERATED BY A DRAFT IS NOT THE SAME FACE.
        //
        // A tilted face arrives in the result TWICE: as `Modified` (the same element, keeping its name) and as
        // `Generated` (a new face at the side). The second took the source's name, found it taken and got a
        // positional number — measured: 7 such faces on a scenario document, one per draft, which patterns then
        // multiplied into dozens. Its name comes from the recipe, "the draft side from face F", exactly as a
        // shell's wall does.
        std::map<int, int> gen_names;
        for (size_t i = 0; sides && i + 1 < nsides * 2; i += 2) gen_names[(int)sides[i]] = (int)sides[i + 1];
        propagate_ids(draft, s->shape, TopAbs_FACE, s->fids, q->shape, q->fids, nullptr, nullptr, gen_names.empty() ? nullptr : &gen_names);
        propagate_ids(draft, s->shape, TopAbs_EDGE, s->eids, q->shape, q->eids);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// THICKENING A FACE: a face of a body becomes a PLATE of the given thickness.
//
// This is how a part is made out of a curved surface: take a face of the body, thicken it by 2 mm and the
// skin is ready. The thickness's sign sets the side it grows to; zero is rejected, since a plate of zero
// thickness is not a solid.
//
// Names are deliberately NOT carried over: a plate has TWO new faces (front and back) plus its sides, and the
// original face's former name describes none of them unambiguously. Fresh ones are seeded instead — more
// honest than handing one name to several faces and then chasing fillets that have drifted.
// The plate itself, made from a face, with no union. It is a function of its own because both entry points
// need it.
static TopoDS_Shape thicken_plate(const QymShape* s, uint32_t fid, double thickness, TopoDS_Face& face_out, BRepOffset_MakeOffset& mk) {
    for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
        const TopoDS_Face& f = TopoDS::Face(ex.Current());
        if (s->fids.IsBound(f) && (uint32_t)s->fids.Find(f) == fid) {
            face_out = f;
            break;
        }
    }
    if (face_out.IsNull()) return TopoDS_Shape();
    // Thickening = true turns a surface offset into a CLOSED solid (the original face, its offset copy and the
    // sides) rather than a lone second surface.
    mk.Initialize(face_out, thickness, 1.0e-4, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Arc, Standard_True);
    mk.MakeOffsetShape();
    if (!mk.IsDone()) return TopoDS_Shape();
    return mk.Shape();
}

// A THICKENED PLATE IS NAMED BY RECIPE, NOT BY ORDER.
//
// A thickening's history is complete and unambiguous (measured): the original face stays in the result as it
// is, that same face GENERATES the offset side, and every boundary edge generates its own side wall. The whole
// plate used to get purely positional numbers (`seeded`), and any reference to one of its faces rested on the
// numbering alone: 6 unnamed faces out of 6 on a probe, 18 out of 84 on a scenario part.
//
// `fmap` is "name of the original face -> name of its offset side", `emap` is "name of an edge -> name of its
// wall".
static void name_thicken(BRepOffset_MakeOffset& mk, const TopoDS_Shape& src, const QymShape* s, QymShape* q,
                         const unsigned* fmap, size_t nf, const unsigned* emap, size_t ne) {
    auto pick = [](const unsigned* m, size_t n, unsigned key) -> unsigned {
        for (size_t i = 0; i + 1 < n * 2; i += 2)
            if (m[i] == key) return m[i + 1];
        return 0u;
    };
    for (TopExp_Explorer ex(src, TopAbs_FACE); ex.More(); ex.Next()) {
        if (!s->fids.IsBound(ex.Current())) continue;
        const unsigned own = (unsigned)s->fids.Find(ex.Current());
        if (!q->fids.IsBound(ex.Current())) q->fids.Bind(ex.Current(), (int)own); // the face itself is still itself
        const unsigned off = fmap ? pick(fmap, nf, own) : 0u;
        if (off == 0) continue;
        for (TopTools_ListIteratorOfListOfShape it(mk.Generated(ex.Current())); it.More(); it.Next())
            if (!it.Value().IsNull() && it.Value().ShapeType() == TopAbs_FACE && !q->fids.IsBound(it.Value()))
                q->fids.Bind(it.Value(), (int)off);
    }
    // THE HISTORY DOES NOT REPORT A SHEET'S SIDE WALLS — measured, and it is a finding rather than an
    // oversight. On the thickening of a sheet `BRepOffset_MakeOffset` returns `Generated` only for the FACE
    // (the offset side); for the boundary edges both `Generated` and `Modified` are empty (4 edges out of 4
    // named, 0 generated faces). A wall therefore keeps a positional number until some recipe-based trait is
    // found for it. The loop below is kept: on a solid face the channel does work.
    for (TopExp_Explorer ex(src, TopAbs_EDGE); ex.More(); ex.Next()) {
        if (!s->eids.IsBound(ex.Current())) continue;
        const unsigned wall = emap ? pick(emap, ne, (unsigned)s->eids.Find(ex.Current())) : 0u;
        if (wall == 0) continue;
        for (TopTools_ListIteratorOfListOfShape it(mk.Generated(ex.Current())); it.More(); it.Next())
            if (!it.Value().IsNull() && it.Value().ShapeType() == TopAbs_FACE && !q->fids.IsBound(it.Value()))
                q->fids.Bind(it.Value(), (int)wall);
    }
    // A WALL THE HISTORY IS SILENT ABOUT IS FOUND BY ADJACENCY — AND THAT IS TOPOLOGY, NOT GEOMETRY.
    //
    // On the thickening of a SHEET `BRepOffset_MakeOffset` reports nothing about the side walls at all
    // (measured: for the boundary edges both Generated and Modified are empty). But by construction a wall
    // touches the original face EXACTLY ALONG ITS OWN EDGE: the original face stays in the result as the same
    // shape, so do its edges, and each such edge has exactly two faces in the result — the original one and its
    // wall. Not a single coordinate is needed for this, only the connections.
    if (emap && ne > 0) {
        TopTools_IndexedDataMapOfShapeListOfShape e2f;
        TopExp::MapShapesAndAncestors(q->shape, TopAbs_EDGE, TopAbs_FACE, e2f);
        for (TopExp_Explorer ex(src, TopAbs_EDGE); ex.More(); ex.Next()) {
            if (!s->eids.IsBound(ex.Current())) continue;
            const unsigned wall = pick(emap, ne, (unsigned)s->eids.Find(ex.Current()));
            if (wall == 0) continue;
            const int ei = e2f.FindIndex(ex.Current());
            if (ei < 1) continue;
            for (TopTools_ListIteratorOfListOfShape it(e2f.FindFromIndex(ei)); it.More(); it.Next())
                if (!q->fids.IsBound(it.Value())) q->fids.Bind(it.Value(), (int)wall);
        }
    }
    int nfree = next_local(q->fids), nefree = next_local(q->eids);
    fill_unnamed(q->shape, TopAbs_FACE, q->fids, nfree);
    fill_unnamed(q->shape, TopAbs_EDGE, q->eids, nefree);
}

extern "C" QymShape* qym_shape_thicken_face(const QymShape* s, uint32_t fid, double thickness,
                                 const unsigned* fmap, size_t nf, const unsigned* emap, size_t ne) {
    if (!s) return why("thicken/asked", "there is no body holding the face to thicken"), nullptr;
    if (fid == 0) return why("thicken/asked", "no face was named to thicken"), nullptr;
    if (std::abs(thickness) < 1e-9) return why("thicken/asked", "the thickness is zero"), nullptr;
    try {
        TopoDS_Face face;
        BRepOffset_MakeOffset mk;
        TopoDS_Shape plate = thicken_plate(s, fid, thickness, face, mk);
        if (plate.IsNull()) return why_no_named_faces("thicken/face", s, fid), nullptr;
        QymShape* q = new QymShape{plate, {}, {}, {}, {}};
        name_thicken(mk, face, s, q, fmap, nf, emap, ne);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// THE SAME, BUT UNIONED WITH ITS SOURCE — "a part is ONE solid".
//
// A function of its own rather than a flag on the previous one: "make a plate" is a self-contained kernel
// operation checked by its own tests for exact volume. The one-solid rule lives a storey above, in the feature.
// As a separate solid the plate was painted a different colour, and the screen showed a second part where
// there should have been one (reported behaviour).
extern "C" QymShape* qym_shape_thicken_face_join(const QymShape* s, uint32_t fid, double thickness,
                                      const unsigned* fmap, size_t nf_, const unsigned* emap, size_t ne_) {
    if (!s) return why("thicken/asked", "there is no body holding the face to thicken"), nullptr;
    if (fid == 0) return why("thicken/asked", "no face was named to thicken"), nullptr;
    if (std::abs(thickness) < 1e-9) return why("thicken/asked", "the thickness is zero"), nullptr;
    try {
        // A SHEET IS THICKENED WHOLE AND UNIONED WITH NOTHING. A surface has no solid for the plate to be
        // joined to — it IS the result itself. And it is taken from the whole sheet rather than from one face:
        // on a surface sewn out of pieces, thickening face by face would give a stack of plates instead of a
        // cover. This is what closes the design layer onto the timeline: patch -> thickness -> an ordinary
        // solid, which is then combined with the part by a boolean.
        if (qym_shape_kind(s) == 2) {
            BRepOffset_MakeOffset mk;
            mk.Initialize(s->shape, thickness, 1.0e-4, BRepOffset_Skin, Standard_False, Standard_False, GeomAbs_Arc, Standard_True);
            mk.MakeOffsetShape();
            if (!mk.IsDone()) return why("thicken/offset", "the kernel could not offset the sheet into a solid"), nullptr;
            TopoDS_Shape plate = mk.Shape();
            if (plate.IsNull()) return why("thicken/offset", "the kernel reported success and returned nothing"), nullptr;
            QymShape* q = new QymShape{plate, {}, {}, {}, {}};
            name_thicken(mk, s->shape, s, q, fmap, nf_, emap, ne_);
            return q;
        }
        TopoDS_Face face;
        BRepOffset_MakeOffset mk;
        TopoDS_Shape plate;
        QymShape plate_named{TopoDS_Shape(), {}, {}, {}, {}};
        // A FLAT FACE IS THICKENED BY A PRISM along its normal: the walls come out as the surfaces the prism sweeps
        // (a plane from a line, a cylinder from a circle), the same the part's own walls stand on, so they merge
        // with them. The offset of the face made its walls anew, and the top of a cylinder thickened 3 kept a seam
        // where the top had been - two cylindrical faces of one side.
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More() && face.IsNull(); ex.Next())
            if (s->fids.IsBound(ex.Current()) && (uint32_t)s->fids.Find(ex.Current()) == fid) face = TopoDS::Face(ex.Current());
        if (!face.IsNull() && BRepAdaptor_Surface(face).GetType() == GeomAbs_Plane) {
            const gp_Ax3 frame = BRepAdaptor_Surface(face).Plane().Position();
            gp_Dir n = frame.XDirection().Crossed(frame.YDirection());
            if (face.Orientation() == TopAbs_REVERSED) n.Reverse();
            BRepPrimAPI_MakePrism pr(face, gp_Vec(n) * thickness);
            pr.Build();
            if (pr.IsDone()) {
                plate = pr.Shape();
                plate_named.shape = plate;
                auto pick = [](const unsigned* m, size_t n, unsigned key) -> unsigned {
                    for (size_t i = 0; i + 1 < n * 2; i += 2)
                        if (m[i] == key) return m[i + 1];
                    return 0u;
                };
                const unsigned own = (unsigned)s->fids.Find(face);
                plate_named.fids.Bind(pr.FirstShape(), (int)own);
                const unsigned off = fmap ? pick(fmap, nf_, own) : 0u;
                if (off != 0 && !plate_named.fids.IsBound(pr.LastShape())) plate_named.fids.Bind(pr.LastShape(), (int)off);
                for (TopExp_Explorer ee(face, TopAbs_EDGE); ee.More(); ee.Next()) {
                    if (!s->eids.IsBound(ee.Current())) continue;
                    const unsigned wall = emap ? pick(emap, ne_, (unsigned)s->eids.Find(ee.Current())) : 0u;
                    if (wall == 0) continue;
                    for (TopTools_ListIteratorOfListOfShape it(pr.Generated(ee.Current())); it.More(); it.Next())
                        if (it.Value().ShapeType() == TopAbs_FACE && !plate_named.fids.IsBound(it.Value())) plate_named.fids.Bind(it.Value(), (int)wall);
                }
            }
        }
        if (plate.IsNull()) {
            face = TopoDS_Face();
            plate = thicken_plate(s, fid, thickness, face, mk);
            if (plate.IsNull()) return why_no_named_faces("thicken/face", s, fid), nullptr;
            // THE PLATE ENTERS THE BOOLEAN ALREADY NAMED, or its faces get numbers and everything put on them (an
            // edge fillet, a sketch on the offset side) drifts with the first edit.
            plate_named.shape = plate;
            name_thicken(mk, face, s, &plate_named, fmap, nf_, emap, ne_);
        }
        BRepAlgoAPI_Fuse fu;
        qym_boolean(fu, s->shape, plate);
        if (!fu.IsDone()) return why("thicken/join", "the plate did not join the part"), nullptr;
        TopoDS_Shape res = fu.Shape();
        if (res.IsNull()) return why("thicken/join", "the join came out empty"), nullptr;
        QymShape* q = new QymShape{res, {}, {}, {}, {}};
        int nf = next_local(s->fids), ne = next_local(s->eids);
        carry_ids(fu, s->shape, TopAbs_FACE, s->fids, q->fids, nf, false, &q->fsplit_of, &q->fsplit_idx);
        carry_ids(fu, plate, TopAbs_FACE, plate_named.fids, q->fids, nf, true);
        carry_ids(fu, s->shape, TopAbs_EDGE, s->eids, q->eids, ne);
        fill_unnamed(res, TopAbs_FACE, q->fids, nf);
        fill_unnamed(res, TopAbs_EDGE, q->eids, ne);
        q->shape = unify_monolithic(q->shape, q->fids, q->eids, &q->absorbed);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// A CYLINDRICAL FACE: the axis AND the radius. `qym_shape_face_axis` does not return the radius, and a
// measurement needs exactly that: a person points at a hole's wall and expects a diameter.
extern "C" int qym_shape_face_cylinder(const QymShape* s, uint32_t face_id, double* origin, double* dir, double* radius) {
    if (!s || face_id == 0 || !origin || !dir || !radius) return 0;
    try {
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            const TopoDS_Face& f = TopoDS::Face(ex.Current());
            if (!s->fids.IsBound(f) || (uint32_t)s->fids.Find(f) != face_id) continue;
            BRepAdaptor_Surface surf(f, Standard_True);
            if (surf.GetType() != GeomAbs_Cylinder) return 0;
            gp_Cylinder cyl = surf.Cylinder();
            gp_Pnt o = cyl.Axis().Location();
            gp_Dir d = cyl.Axis().Direction();
            origin[0] = o.X(); origin[1] = o.Y(); origin[2] = o.Z();
            dir[0] = d.X(); dir[1] = d.Y(); dir[2] = d.Z();
            *radius = cyl.Radius();
            return 1;
        }
        return 0;
    } catch (...) { return 0; }
}

// SPLIT FACES WITH A PLANE WITHOUT CUTTING THE SOLID.
//
// The difference from cutting a solid is fundamental: the solid stays ONE, but the faces the plane crossed
// fall into parts. That is how an area for painting, a pad for machining or a zone a feature is later attached
// to gets marked out, without breaking the part into pieces.
//
// The dividing line is computed as a section (`BRepAlgoAPI_Section`), and the section's edges are handed to
// `BRepFeat_SplitShape` together with their owner faces: without that binding the kernel does not know what to
// split and silently returns the original solid.
extern "C" QymShape* qym_shape_split_faces(const QymShape* s, const double* origin, const double* normal) {
    if (!s) return why("split face/asked", "there is no body to split"), nullptr;
    if (!origin || !normal) return why("split face/asked", "the cutting plane has no origin or no normal"), nullptr;
    try {
        gp_Pln pln(gp_Pnt(origin[0], origin[1], origin[2]), gp_Dir(normal[0], normal[1], normal[2]));
        BRepAlgoAPI_Section sec(s->shape, pln, Standard_False); // Standard_False: do not run it yet - the options come first
        qym_configure(sec);
        sec.ComputePCurveOn1(Standard_True);
        sec.Approximation(Standard_True);
        sec.Build();
        if (!sec.IsDone()) return why("split face/section", "the kernel could not intersect the body with the plane"), nullptr;

        BRepFeat_SplitShape sp(s->shape);
        int added = 0;
        for (TopExp_Explorer ex(sec.Shape(), TopAbs_EDGE); ex.More(); ex.Next()) {
            const TopoDS_Edge& e = TopoDS::Edge(ex.Current());
            TopoDS_Shape host;
            if (sec.HasAncestorFaceOn1(e, host) && !host.IsNull() && host.ShapeType() == TopAbs_FACE) {
                sp.Add(e, TopoDS::Face(host));
                ++added;
            }
        }
        // A PLANE THAT MISSES THE SOLID is an honest refusal: without it the feature would "succeed" while
        // splitting nothing.
        if (added == 0) return why("split face/plane", "the cutting plane does not cross a single face of this body"), nullptr;
        sp.Build();
        if (!sp.IsDone()) return why("split face/build", "the kernel could not split the faces along this section"), nullptr;
        TopoDS_Shape res = sp.Shape();
        if (res.IsNull()) return why("split face/build", "the kernel reported success and returned nothing"), nullptr;

        QymShape* q = new QymShape{res, {}, {}, {}, {}};
        int nf = next_local(s->fids);
        int ne = next_local(s->eids);
        // NAMES SURVIVE THE SPLIT: untouched faces keep their ids, and the pieces of a split face get its name
        // plus a piece number (fsplit_of / fsplit_idx) — otherwise the references of fillets and chamfers would
        // drift all over the part from a single marking-out.
        carry_ids(sp, s->shape, TopAbs_FACE, s->fids, q->fids, nf, false, &q->fsplit_of, &q->fsplit_idx);
        carry_ids(sp, s->shape, TopAbs_EDGE, s->eids, q->eids, ne);
        fill_unnamed(q->shape, TopAbs_FACE, q->fids, nf);
        fill_unnamed(q->shape, TopAbs_EDGE, q->eids, ne);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// SPLIT A SOLID WITH A PLANE: returns EVERY piece as a solid of its own.
//
// The plane is built as a face safely larger than the solid's bounding box and fed as the tool to
// BRepAlgoAPI_Splitter. There can be more than two halves — on a U-shaped part, for instance, the plane cuts
// off three pieces at once. Hence a list is returned rather than a pair: "two halves" is a special case, and
// counting on it means losing pieces silently.
//
// The names of faces and edges are carried into every piece: otherwise, after the cut, the references of
// fillets and chamfers would drift in all the parts at once.
extern "C" QymShapeList* qym_shape_split_by_plane(const QymShape* s, const double* origin, const double* normal, unsigned section) {
    if (!s || !origin || !normal) return nullptr;
    try {
        gp_Pnt o(origin[0], origin[1], origin[2]);
        gp_Dir n(normal[0], normal[1], normal[2]);
        Bnd_Box bb;
        BRepBndLib::Add(s->shape, bb);
        if (bb.IsVoid()) return nullptr;
        double xmin, ymin, zmin, xmax, ymax, zmax;
        bb.Get(xmin, ymin, zmin, xmax, ymax, zmax);
        double diag = gp_Pnt(xmin, ymin, zmin).Distance(gp_Pnt(xmax, ymax, zmax));
        if (diag < 1e-9) return nullptr;
        TopoDS_Face cutter = BRepBuilderAPI_MakeFace(gp_Pln(o, n), -diag, diag, -diag, diag).Face();
        if (cutter.IsNull()) return nullptr;

        TopTools_ListOfShape args, tools;
        args.Append(s->shape);
        tools.Append(cutter);
        BRepAlgoAPI_Splitter algo;
        qym_boolean_many(algo, args, tools);
        if (!algo.IsDone() || algo.HasErrors()) return nullptr;
        TopoDS_Shape res = algo.Shape();
        if (res.IsNull()) return nullptr;

        QymShapeList* lst = new QymShapeList();
        for (TopExp_Explorer ex(res, TopAbs_SOLID); ex.More(); ex.Next()) {
            QymShape* q = new QymShape{ex.Current(), {}, {}, {}, {}};
            int nf = next_local(s->fids);
            int ne = next_local(s->eids);
            carry_ids(algo, s->shape, TopAbs_FACE, s->fids, q->fids, nf, false, &q->fsplit_of, &q->fsplit_idx);
            carry_ids(algo, s->shape, TopAbs_EDGE, s->eids, q->eids, ne);
            // THE SECTION FACE COMES FROM THE CUTTING PLANE ITSELF. The tool here is a real face and the
            // history honestly reports what it turned into; the section has no other source. Without a name
            // that face was positional in every piece, and everything built at the place of the cut stands on
            // it.
            if (section != 0) {
                for (TopTools_ListIteratorOfListOfShape it(algo.Modified(cutter)); it.More(); it.Next()) {
                    if (it.Value().IsNull() || it.Value().ShapeType() != TopAbs_FACE) continue;
                    if (!q->fids.IsBound(it.Value())) q->fids.Bind(it.Value(), (int)section);
                }
            }
            classify_unnamed(algo, s->shape, q, "split/solid", &s->fids);
            classify_unnamed(algo, cutter, q, "split/plane");
            fill_unnamed(q->shape, TopAbs_FACE, q->fids, nf);
            fill_unnamed(q->shape, TopAbs_EDGE, q->eids, ne);
            lst->named.push_back(q);
        }
        // NO CUT HAPPENED — an honest refusal. A single piece means the plane missed the solid; without the
        // check the feature would "succeed" and change nothing.
        if (lst->named.size() < 2) { for (auto* q : lst->named) delete q; delete lst; return nullptr; }
        return lst;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// TRIM A SURFACE WITH ANOTHER SOLID. The sheet is cut along the line of intersection and the piece a person
// clicked on is what remains — the point `keep` is their answer to "what do we keep".
//
// A point rather than a piece number: the number is a property of today's traversal order, and after an edit
// to the base it points somewhere else. A point on the surface survives both a shift and a stretch, because
// "which piece is nearest this place" has the same answer as it had yesterday.
//
// A PIECE IS A CONNECTED GROUP OF FACES, not a single face: a trimmed contour of five faces stays one piece,
// and returning one face out of it would silently throw the other four away.
extern "C" QymShape* qym_shape_trim(const QymShape* s, const QymShape* tool, const double* keep) {
    if (!s) return why("trim/asked", "there is no body to trim"), nullptr;
    if (!tool) return why("trim/asked", "there is nothing to trim it with"), nullptr;
    if (!keep) return why("trim/asked", "the side to keep was not given"), nullptr;
    try {
        TopTools_ListOfShape args, tools;
        args.Append(s->shape);
        tools.Append(tool->shape);
        BRepAlgoAPI_Splitter algo;
        qym_boolean_many(algo, args, tools);
        if (!algo.IsDone() || algo.HasErrors()) return why("trim/cut", "the kernel could not cut the body with this tool"), nullptr;
        TopoDS_Shape res = algo.Shape();
        if (res.IsNull()) return why("trim/cut", "the cut came out empty"), nullptr;

        // the result's faces plus connectivity along shared edges (union-find)
        std::vector<TopoDS_Shape> faces;
        for (TopExp_Explorer ex(res, TopAbs_FACE); ex.More(); ex.Next()) faces.push_back(ex.Current());
        if (faces.size() < 2) return why("trim/cut", "the tool does not divide the body: there is nothing to trim away"), nullptr; // there was nothing to cut: an honest refusal, not a "success"
        std::vector<int> parent(faces.size());
        for (size_t i = 0; i < faces.size(); ++i) parent[i] = (int)i;
        std::function<int(int)> find = [&](int x) { while (parent[x] != x) { parent[x] = parent[parent[x]]; x = parent[x]; } return x; };
        // THE CUT LINE MUST NOT CONNECT. After the split, the pieces on either side still share the cut's
        // edge, and counting them as one piece through it means splitting nothing (the first attempt failed to
        // cut for exactly that reason: two faces, one group). The cut's edge is recognised by lying ON THE
        // TOOL, and connectivity does not travel along it.
        std::map<int, int> by_edge; // an edge's index in the shared map -> the first face met
        TopTools_IndexedMapOfShape emap;
        TopExp::MapShapes(res, TopAbs_EDGE, emap);
        std::map<int, bool> on_tool;
        for (size_t i = 0; i < faces.size(); ++i) {
            for (TopExp_Explorer ex(faces[i], TopAbs_EDGE); ex.More(); ex.Next()) {
                int ei = emap.FindIndex(ex.Current());
                auto ot = on_tool.find(ei);
                if (ot == on_tool.end()) {
                    BRepExtrema_DistShapeShape d(ex.Current(), tool->shape);
                    bool cut = d.IsDone() && d.NbSolution() > 0 && d.Value() < 1.0e-6;
                    ot = on_tool.emplace(ei, cut).first;
                }
                if (ot->second) continue; // the cut's edge: pieces do not grow together across it
                auto it = by_edge.find(ei);
                if (it == by_edge.end()) { by_edge[ei] = (int)i; continue; }
                int a = find(it->second), b = find((int)i);
                if (a != b) parent[a] = b;
            }
        }
        // the piece nearest the "keep" point
        gp_Pnt want(keep[0], keep[1], keep[2]);
        std::map<int, double> best; // a group's root -> the smallest distance to the point
        for (size_t i = 0; i < faces.size(); ++i) {
            BRepExtrema_DistShapeShape dss(faces[i], BRepBuilderAPI_MakeVertex(want).Vertex());
            if (!dss.IsDone() || dss.NbSolution() < 1) continue;
            double d = dss.Value();
            int r = find((int)i);
            auto it = best.find(r);
            if (it == best.end() || d < it->second) best[r] = d;
        }
        if (best.empty()) return why("trim/keep", "no piece of the cut body lies on the side that was asked to be kept"), nullptr;
        int keep_root = best.begin()->first;
        double keep_d = best.begin()->second;
        for (const auto& [r, d] : best) if (d < keep_d) { keep_root = r; keep_d = d; }

        BRep_Builder bb;
        TopoDS_Shell shell;
        bb.MakeShell(shell);
        int kept = 0, dropped = 0;
        for (size_t i = 0; i < faces.size(); ++i) {
            if (find((int)i) == keep_root) { bb.Add(shell, faces[i]); ++kept; }
            else ++dropped;
        }
        // NOTHING WAS TRIMMED is also a refusal: a feature that "succeeded" and changed nothing is worse than
        // a red node, because one never finds out about it.
        if (kept == 0 || dropped == 0) return why("trim/keep", kept == 0 ? "the side asked to be kept holds nothing" : "the tool cut nothing away"), nullptr;

        QymShape* q = new QymShape{shell, {}, {}, {}, {}};
        int nf = next_local(s->fids), ne = next_local(s->eids);
        carry_ids(algo, s->shape, TopAbs_FACE, s->fids, q->fids, nf, false, &q->fsplit_of, &q->fsplit_idx);
        carry_ids(algo, s->shape, TopAbs_EDGE, s->eids, q->eids, ne);
        fill_unnamed(shell, TopAbs_FACE, q->fids, nf);
        fill_unnamed(shell, TopAbs_EDGE, q->eids, ne);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// REMOVE A FACE AND HEAL OVER IT (defeaturing): the faces `ids` are taken away and their neighbours are
// extended so that the solid stays closed. That is how a fillet, a chamfer or a boss is removed without taking
// the timeline apart.
//
// The kernel does this itself (BRepAlgoAPI_Defeaturing); naively "cutting the face out" would leave a hole in
// the shell. If the neighbours cannot be extended (the face carries the whole shape — a cylinder's single side
// face, say), the algorithm honestly fails to converge, and a refusal is returned rather than a broken solid.
extern "C" QymShape* qym_shape_remove_faces(const QymShape* s, const uint32_t* ids, size_t n, int* out_reason) {
    if (out_reason) *out_reason = 0;
    if (!s) return why("remove face/asked", "there is no body to remove faces from"), nullptr;
    if (!ids || n == 0) return why("remove face/asked", "not one face was named to remove"), nullptr;
    try {
        TopTools_ListOfShape faces;
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            uint32_t fid = s->fids.IsBound(ex.Current()) ? static_cast<uint32_t>(s->fids.Find(ex.Current())) : 0u;
            if (fid == 0) continue;
            for (size_t k = 0; k < n; ++k) {
                if (ids[k] == fid) { faces.Append(ex.Current()); break; }
            }
        }
        // DIFFERENT FAILURES DESERVE DIFFERENT ANSWERS. "Face not found" and "the neighbours cannot be
        // extended" are fixed in different ways, and one message for both leaves a person guessing. -1 in
        // out_reason means not found, 0 means it did not work out.
        if (faces.IsEmpty()) { if (out_reason) *out_reason = -1; return why_no_named_faces("remove face/faces", s, ids[0]), nullptr; }
        BRepAlgoAPI_Defeaturing algo;
        qym_configure(algo);
        algo.SetShape(s->shape);
        algo.AddFacesToRemove(faces);
        algo.Build();
        if (!algo.IsDone() || algo.HasErrors()) return why("remove face/build", "the kernel could not close the body after removing these faces"), nullptr;
        TopoDS_Shape res = algo.Shape();
        if (res.IsNull()) return why("remove face/build", "the body came back empty"), nullptr;
        // AN HONEST REFUSAL INSTEAD OF A QUIET NOTHING. The kernel can report success without having removed a
        // single face (the neighbours could not be extended — a fillet's corner spheres remained, say). The
        // feature would then "succeed" and leave the solid unchanged: a person presses Enter, a step appears in
        // the timeline and the part is the same. The face count is checked, and if nothing was removed the
        // operation refuses.
        auto count_faces = [](const TopoDS_Shape& sh) {
            int c = 0;
            for (TopExp_Explorer ex(sh, TopAbs_FACE); ex.More(); ex.Next()) ++c;
            return c;
        };
        if (count_faces(res) >= count_faces(s->shape)) return why("remove face/result", "the body came back with as many faces as before: nothing was removed"), nullptr;
        QymShape* q = new QymShape{res, {}, {}, {}, {}};
        int nf = next_local(s->fids);
        int ne = next_local(s->eids);
        carry_ids(algo, s->shape, TopAbs_FACE, s->fids, q->fids, nf, false, &q->fsplit_of, &q->fsplit_idx);
        carry_ids(algo, s->shape, TopAbs_EDGE, s->eids, q->eids, ne);
        fill_unnamed(res, TopAbs_FACE, q->fids, nf);
        fill_unnamed(res, TopAbs_EDGE, q->eids, ne);
        return q;
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}

// The history of an offset spoken as a `BRepBuilderAPI_MakeShape`, so the names are carried by the one `carry_ids`.
class OffsetHistory : public BRepBuilderAPI_MakeShape {
public:
    explicit OffsetHistory(BRepOffset_MakeOffset& m) : mk(m) { myShape = m.Shape(); Done(); }
    void Build(const Message_ProgressRange& = Message_ProgressRange()) override {}
    const TopTools_ListOfShape& Generated(const TopoDS_Shape& s) override { return mk.Generated(s); }
    // the faces an offset leaves in place are rebuilt too, and `Modified` says nothing of them (10 names of 10 lost
    // on a pushed wall of a shell): their images are asked of the offset's own image of faces and edges, and only
    // what is in the result counts
    const TopTools_ListOfShape& Modified(const TopoDS_Shape& s) override {
        const TopTools_ListOfShape& own = mk.Modified(s);
        if (!own.IsEmpty()) return own;
        myGenerated.Clear();
        const BRepAlgo_Image& img = s.ShapeType() == TopAbs_FACE ? mk.OffsetFacesFromShapes() : mk.OffsetEdgesFromShapes();
        if (!img.HasImage(s)) return myGenerated;
        TopTools_ListOfShape last;
        img.LastImage(s, last);
        if (in_result.IsEmpty()) {
            TopExp::MapShapes(myShape, TopAbs_FACE, in_result);
            TopExp::MapShapes(myShape, TopAbs_EDGE, in_result);
        }
        for (TopTools_ListIteratorOfListOfShape it(last); it.More(); it.Next())
            if (!it.Value().IsSame(s) && in_result.Contains(it.Value())) myGenerated.Append(it.Value());
        return myGenerated;
    }
    Standard_Boolean IsDeleted(const TopoDS_Shape& s) override { return mk.IsDeleted(s); }
    void SetResult(const TopoDS_Shape& r) { myShape = r; in_result.Clear(); }
private:
    BRepOffset_MakeOffset& mk;
    TopTools_IndexedMapOfShape in_result;
};

// PUSH AND PULL A FACE — direct modelling.
//
// The face `fid` moves along its own normal by `dist`: positive adds material, negative cuts it away. Between
// flat neighbours it is an offset of that one face, the neighbours meeting it by intersection - a flat face or a
// curved one alike (the side of a cylinder of 10 pushed 2 is a cylinder of 12). A flat face with a curved neighbour
// falls back to a prism raised from the face and a boolean, EXACTLY from the original surface rather than its
// tessellation; a curved face with a curved neighbour is refused, since extending a rounding is a different operation
// and doing it "along the way" gives a silently wrong result on the first filleted part.
//
// The names of faces and edges are carried over the same way as in a boolean (`carry_ids`): otherwise the
// references of fillets and chamfers would drift after the very first push or pull.
extern "C" QymShape* qym_shape_push_face(const QymShape* s, uint32_t fid, double dist) {
    if (!s) return why("push face/asked", "there is no body holding the face to push"), nullptr;
    if (fid == 0) return why("push face/asked", "no face was named to push"), nullptr;
    if (std::abs(dist) < 1e-9) return why("push face/asked", "the distance is zero"), nullptr;
    try {
        TopoDS_Face target;
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            uint32_t id = s->fids.IsBound(ex.Current()) ? static_cast<uint32_t>(s->fids.Find(ex.Current())) : 0u;
            if (id == fid) { target = TopoDS::Face(ex.Current()); break; }
        }
        if (target.IsNull()) return why_no_named_faces("push face/face", s, fid), nullptr;
        BRepAdaptor_Surface ad(target);
        const bool flat = ad.GetType() == GeomAbs_Plane;
        // the normal of the surface is XDir ^ YDir, which is the axis only for a right-handed frame: the plane of a
        // chamfer comes left-handed, its axis points into the body, and a push of +1 or -1 left the body as it was
        gp_Vec v(0, 0, 1);
        if (flat) {
            const gp_Ax3 frame = ad.Plane().Position();
            gp_Dir n = frame.XDirection().Crossed(frame.YDirection());
            if (target.Orientation() == TopAbs_REVERSED) n.Reverse();
            v = gp_Vec(n);
            v *= std::abs(dist);
            if (dist < 0) v.Reverse();
        }
        auto finish = [&](BRepBuilderAPI_MakeShape& algo, const TopoDS_Shape& res, const TopoDS_Shape& moved) -> QymShape* {
            if (res.IsNull()) return why("push face/build", "the body came back empty after moving the face"), nullptr;
            QymShape* q = new QymShape{res, {}, {}, {}, {}};
            int nf = next_local(s->fids);
            int ne = next_local(s->eids);
            carry_ids(algo, s->shape, TopAbs_FACE, s->fids, q->fids, nf, false, &q->fsplit_of, &q->fsplit_idx);
            carry_ids(algo, s->shape, TopAbs_EDGE, s->eids, q->eids, ne);
            // THE PULLED FACE STAYS ITSELF. It was moved, so it is still the same face, just in a new place.
            // Without this it got a fresh positional number: it could not be picked a second time, editing the
            // distance broke the reference, and everything standing on it fell off.
            if (!moved.IsNull()) {
                bool bound = false;
                const TopTools_ListOfShape& mm = algo.Modified(moved);
                for (TopTools_ListIteratorOfListOfShape it(mm); it.More(); it.Next())
                    if (!q->fids.IsBound(it.Value())) { q->fids.Bind(it.Value(), (int)fid); bound = true; }
                if (!bound && !q->fids.IsBound(moved)) q->fids.Bind(moved, (int)fid);
            }
            fill_unnamed(res, TopAbs_FACE, q->fids, nf);
            fill_unnamed(res, TopAbs_EDGE, q->eids, ne);
            q->shape = unify_monolithic(q->shape, q->fids, q->eids, &q->absorbed);
            return q;
        };
        // THE FACE MOVES AND ITS NEIGHBOURS FOLLOW: an offset of this one face, the others at zero, joined by
        // intersection, so the faces beside it extend or trim to meet it. The face of a 2 x 2 chamfer on a 40 long
        // edge pushed 1 out leaves legs of 0.586 (+73.1 mm^3), 1 in legs of 3.414 (-153.1); a prism raised from the
        // face instead stands on it as a slab with walls square to it (+113.1).
        // only where every face beside it is flat: curved neighbours extended by intersection run off (a rim between
        // two roundings pushed 5 gained 19178 mm^3 for a strip of 2 x 36)
        bool flat_ring = true;
        {
            TopTools_IndexedDataMapOfShapeListOfShape e2f;
            TopExp::MapShapesAndAncestors(s->shape, TopAbs_EDGE, TopAbs_FACE, e2f);
            for (TopExp_Explorer ee(target, TopAbs_EDGE); ee.More() && flat_ring; ee.Next()) {
                const int k = e2f.FindIndex(ee.Current());
                if (k < 1) continue;
                for (TopTools_ListIteratorOfListOfShape it(e2f.FindFromIndex(k)); it.More(); it.Next())
                    if (!it.Value().IsSame(target) && BRepAdaptor_Surface(TopoDS::Face(it.Value())).GetType() != GeomAbs_Plane) flat_ring = false;
            }
        }
        if (flat_ring) {
            BRepOffset_MakeOffset mk;
            mk.Initialize(s->shape, 0.0, 1.0e-6, BRepOffset_Skin, Standard_True, Standard_False, GeomAbs_Intersection, Standard_False);
            mk.SetOffsetOnFace(target, dist);
            mk.MakeOffsetShape();
            // the offset of a solid comes back as its skin; the part is the solid inside it
            TopoDS_Shape got = mk.IsDone() ? mk.Shape() : TopoDS_Shape();
            if (!got.IsNull() && got.ShapeType() != TopAbs_SOLID) {
                TopoDS_Shape skin;
                int solids = 0, shells = 0;
                for (TopExp_Explorer x(got, TopAbs_SOLID); x.More(); x.Next()) { skin = x.Current(); ++solids; }
                if (solids == 0)
                    for (TopExp_Explorer x(got, TopAbs_SHELL); x.More(); x.Next()) { skin = x.Current(); ++shells; }
                if (solids == 1) got = skin;
                else if (solids == 0 && shells == 1) {
                    BRepBuilderAPI_MakeSolid ms(TopoDS::Shell(skin));
                    got = ms.IsDone() ? TopoDS_Shape(ms.Solid()) : TopoDS_Shape();
                    if (!got.IsNull()) { BRepLib::OrientClosedSolid(TopoDS::Solid(got)); }
                } else got = TopoDS_Shape();
            }
            GProp_GProps was, now;
            if (!got.IsNull()) { BRepGProp::VolumeProperties(s->shape, was); BRepGProp::VolumeProperties(got, now); }
            GProp_GProps fa;
            BRepGProp::SurfaceProperties(target, fa);
            // a face moved by d changes the body by about its area x d; three times that is no push but a run-off
            const bool fits = !got.IsNull() && std::abs(now.Mass() - was.Mass()) <= 3.0 * fa.Mass() * std::abs(dist) + 1e-6;
            if (fits && BRepCheck_Analyzer(got).IsValid()) {
                OffsetHistory h(mk);
                h.SetResult(got);
                TopoDS_Shape moved;
                const TopTools_ListOfShape& im = mk.Modified(target);
                if (!im.IsEmpty()) moved = im.First();
                else if (!mk.Generated(target).IsEmpty()) moved = mk.Generated(target).First();
                return finish(h, got, moved);
            }
        }
        // a curved face moves only as an offset of itself: a prism along one direction is no offset of a cylinder
        if (!flat) return why("push face/face", flat_ring ? "the offset of this curved face did not close into a body" : "a curved face moves only between flat neighbours"), nullptr;
        BRepPrimAPI_MakePrism prism(target, v);
        prism.Build();
        if (!prism.IsDone()) return why("push face/tool", "the face could not be extruded into the tool that moves it"), nullptr;
        TopoDS_Shape tool = prism.Shape();
        if (tool.IsNull()) return why("push face/tool", "the tool that moves the face came out empty"), nullptr;
        // THE IMAGE OF THE PULLED FACE ITSELF is the prism's cap: that is what ends up in the new position.
        TopoDS_Shape moved = prism.LastShape();
        if (dist > 0) {
            // THE ORDER OF THE OPERANDS CHANGES THE RESULT, AND THAT IS NOT A MATTER OF STYLE.
            //
            // On a real part (shell, then fillet, then chamfer, pulling the rim) `Fuse(solid, prism)` produced
            // a solid with inconsistent face orientations: the surface is right (its area agrees to within a
            // square millimetre) but the solid is formally invalid. On screen that is a hole — the renderer
            // shows the inside — and the volume is integrated with broken signs: 160,767 instead of 20,019,
            // when a bounding box of 50 x 50 x 55 physically cannot hold more than 137,500.
            //
            // `Fuse(prism, solid)` on the same inputs gives a valid solid and a volume of 20,019.4, exactly
            // what is expected. Neither BOP gluing, nor a larger tolerance, nor ShapeFix, nor BRepFeat, nor the
            // kernel's local offset cures this; swapping the operands does.
            //
            // Hence: the direct order first (the kernel is more used to it and it more often gives a tidy
            // history), and if the result does not pass the check, the reverse one. The name history works in
            // both: it is kept by subshape, not by argument number.
            BRepAlgoAPI_Fuse direct;
            { TopTools_ListOfShape aa, bbl; aa.Append(s->shape); bbl.Append(tool); direct.SetNonDestructive(Standard_True); qym_boolean_many(direct, aa, bbl); }
            if (direct.IsDone() && !direct.Shape().IsNull() && BRepCheck_Analyzer(direct.Shape()).IsValid()) {
                return finish(direct, direct.Shape(), moved);
            }
            BRepAlgoAPI_Fuse swapped;
            { TopTools_ListOfShape aa, bbl; aa.Append(tool); bbl.Append(s->shape); swapped.SetNonDestructive(Standard_True); qym_boolean_many(swapped, aa, bbl); }
            if (swapped.IsDone() && !swapped.Shape().IsNull() && BRepCheck_Analyzer(swapped.Shape()).IsValid()) {
                return finish(swapped, swapped.Shape(), moved);
            }
            return finish(direct, direct.Shape(), moved); // neither way worked; the barrier in the model will refuse
        }
        // A cut is asymmetric by meaning: the operands cannot be swapped, since "solid minus prism" is not the
        // same as "prism minus solid". If the result is invalid, the barrier refuses.
        BRepAlgoAPI_Cut algo;
        qym_boolean(algo, s->shape, tool);
        return finish(algo, algo.Shape(), moved);
    } QYM_WHY_CATCH("edge operation")
    return nullptr;
}


extern "C" QymShapeList* qym_step_solids(const char* path) {
    try {
        // the same walk as the import (`step_tree`), so that body k of a reopened document is the one it was
        QymShapeList* lst = new QymShapeList();
        QymTree tree;
        if (!step_tree(path, tree, lst->shapes)) {
            delete lst;
            return nullptr;
        }
        return lst;
    } catch (...) {
        return nullptr; // an exception crossing the C ABI aborts the process, so an honest refusal goes back instead
    }
}
extern "C" size_t qym_shapelist_count(const QymShapeList* l) { return l ? (l->named.empty() ? l->shapes.size() : l->named.size()) : 0; }
extern "C" QymShape* qym_shapelist_get(const QymShapeList* l, size_t i) {
    if (!l) return nullptr;
    try {
        if (!l->named.empty()) {
            if (i >= l->named.size()) return nullptr;
            const QymShape* src = l->named[i];
            return new QymShape{src->shape, src->fids, src->eids, src->fsplit_of, src->fsplit_idx};
        }
        if (i >= l->shapes.size()) return nullptr;
        return seeded(l->shapes[i]); // imported solids get face ids too
    } catch (...) {
        return nullptr;
    }
}
extern "C" void qym_shapelist_free(QymShapeList* l) {
    if (l) for (auto* q : l->named) delete q;
    delete l;
}

// Write `n` shapes (each with its own 3x4 transform into world space) into ONE STEP file (AsIs, an exact
// B-rep). `mats` is n x 12 row-major 3x4 (or nullptr for identity). Units are mm. 0 means success, non-zero an
// error.
extern "C" int qym_step_write(const QymShape** shapes, const double* mats, size_t n, const char* path) {
    try {
        std::lock_guard<std::mutex> one_at_a_time(xstep_lock());
        Interface_Static::SetCVal("write.step.unit", "MM");
        STEPControl_Writer writer;
        for (size_t i = 0; i < n; ++i) {
            if (!shapes[i] || shapes[i]->shape.IsNull()) continue;
            TopoDS_Shape s = shapes[i]->shape;
            if (mats) {
                const double* m = mats + i * 12;
                gp_Trsf t;
                t.SetValues(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]);
                s = BRepBuilderAPI_Transform(s, t, true).Shape();
            }
            if (writer.Transfer(s, STEPControl_AsIs) != IFSelect_RetDone) return 2;
        }
        return writer.Write(path) == IFSelect_RetDone ? 0 : 1;
    } catch (...) {
        return 3;
    }
}

