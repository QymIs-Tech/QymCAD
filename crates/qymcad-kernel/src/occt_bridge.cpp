// The bridge to OCCT: primitives, booleans, names, healing.
//
// Split out of one 5 077-line file with 69 entry points, where finding a function meant scrolling.
// What the parts share lives in `occt_common.hpp`.
#include "occt_common.hpp"

// -- WHY THE KERNEL REFUSED ------------------------------------------------------------------------
//
// A refusal used to leave this file as a null pointer and nothing else. There are 250 places that return one,
// and finding which of them it was meant bisecting the source: hours on every geometric defect, in a project
// whose own rule says a silent failure is the worst answer. So the place - and, where the kernel says
// anything at all, its own words - are kept here for the caller to read.
//
// PER THREAD on purpose. Probes run on copies in worker threads while the main thread builds, and one
// thread's complaint must never be handed to another thread's operation.
//
// The text is for whoever is fixing the program, not for whoever is drawing a part: it names internals and
// comes from the kernel untranslated. What the user sees stays a `CoreError` with words from the catalogue.
static thread_local std::string g_why;

extern "C" void qym_why_clear() { g_why.clear(); }
extern "C" const char* qym_why() { return g_why.empty() ? nullptr : g_why.c_str(); }

// The Rust side refuses some requests before they ever reach OCCT - a sweep with no path, a loft through one
// section. To the caller that is the same refusal, so it goes into the same channel rather than into a second
// one nobody would think to read.
extern "C" void qym_why_set(const char* text) { g_why = text ? text : ""; }

/// Record why an operation refused: `where` names the place, `what` is whatever the kernel itself said.
void why(const char* where, const char* what) {
    g_why = where ? where : "?";
    if (what && *what) {
        g_why += ": ";
        g_why += what;
    }
}



static void add_face(const TopoDS_Face& face, QymBody& b, uint32_t fid) {
    TopLoc_Location loc;
    Handle(Poly_Triangulation) t = BRep_Tool::Triangulation(face, loc);
    if (t.IsNull()) return;

    const gp_Trsf trsf = loc.Transformation();
    const uint32_t base = static_cast<uint32_t>(b.verts.size() / 3);
    const uint32_t tstart = static_cast<uint32_t>(b.tris.size() / 3);

    const int nb = t->NbNodes();
    for (int i = 1; i <= nb; ++i) {
        gp_Pnt p = t->Node(i);
        p.Transform(trsf);
        b.verts.push_back(static_cast<float>(p.X()));
        b.verts.push_back(static_cast<float>(p.Y()));
        b.verts.push_back(static_cast<float>(p.Z()));
    }
    const bool rev = (face.Orientation() == TopAbs_REVERSED);
    const int nt = t->NbTriangles();
    for (int i = 1; i <= nt; ++i) {
        int n1 = 0, n2 = 0, n3 = 0;
        t->Triangle(i).Get(n1, n2, n3);
        if (rev) std::swap(n2, n3);
        b.tris.push_back(base + static_cast<uint32_t>(n1 - 1));
        b.tris.push_back(base + static_cast<uint32_t>(n2 - 1));
        b.tris.push_back(base + static_cast<uint32_t>(n3 - 1));
    }
    const uint32_t tcount = static_cast<uint32_t>(b.tris.size() / 3) - tstart;
    if (tcount > 0) {
        b.fstart.push_back(tstart);
        b.fcount.push_back(tcount);
        b.fid.push_back(fid);
        // The EXACT anchor: a point on the surface plus a normal from THE ANALYTIC form (for a plane, the plane itself)
        double anc[7] = {0, 0, 0, 0, 0, 0, 0};
        try {
            BRepAdaptor_Surface as(face, Standard_False);
            double u = (as.FirstUParameter() + as.LastUParameter()) * 0.5;
            double v = (as.FirstVParameter() + as.LastVParameter()) * 0.5;
            gp_Pnt p;
            gp_Vec du, dv;
            as.D1(u, v, p, du, dv);
            gp_Vec n = du.Crossed(dv);
            if (n.Magnitude() > 1e-12) {
                n.Normalize();
                if (face.Orientation() == TopAbs_REVERSED) n.Reverse();
                anc[0] = p.X(); anc[1] = p.Y(); anc[2] = p.Z();
                anc[3] = n.X(); anc[4] = n.Y(); anc[5] = n.Z();
                if (as.GetType() == GeomAbs_Plane) {
                    gp_Pln pl = as.Plane();
                    gp_Pnt o = pl.Location();
                    anc[0] = o.X(); anc[1] = o.Y(); anc[2] = o.Z();
                    gp_Dir nd = pl.Axis().Direction();
                    if (nd.X() * anc[3] + nd.Y() * anc[4] + nd.Z() * anc[5] < 0.0) nd.Reverse();
                    anc[3] = nd.X(); anc[4] = nd.Y(); anc[5] = nd.Z();
                    anc[6] = 1.0;
                }
            }
        } catch (...) {}
        for (int k = 0; k < 7; ++k) b.fanchor.push_back(anc[k]);
    }
}

// An empty id map, for the paths that carry no persistent names (a box, a STEP import).
static const TopTools_DataMapOfShapeInteger& empty_fids() {
    static const TopTools_DataMapOfShapeInteger e;
    return e;
}

static QymBody body_from_shape(const TopoDS_Shape& s, const TopTools_DataMapOfShapeInteger& fids) {
    QymBody b;
    for (TopExp_Explorer ex(s, TopAbs_FACE); ex.More(); ex.Next()) {
        const TopoDS_Shape& f = ex.Current();
        uint32_t id = fids.IsBound(f) ? static_cast<uint32_t>(fids.Find(f)) : 0u;
        add_face(TopoDS::Face(f), b, id);
    }
    return b;
}

QymDoc* doc_from_shape(const TopoDS_Shape& shape, double defl, const TopTools_DataMapOfShapeInteger& fids) {
    // Drop the previous triangulation: IncrementalMesh does NOT coarsen a mesh that is already finer. So
    // that the STL quality can be chosen exactly (an export may re-tessellate the same body more coarsely),
    // the count starts from nothing.
    BRepTools::Clean(shape);
    // An ANGULAR deflection of 0.3 rad (about 17 deg, at least 21 segments per full turn) instead of the
    // default 0.5 rad (13 segments): otherwise small circles come out faceted EVEN with a tiny linear
    // deflection — on a small radius the linear criterion never fires and the angular one decides
    // everything. Parallel tessellation costs nothing.
    BRepMesh_IncrementalMesh mesher(shape, defl, Standard_False, 0.3, Standard_True);
    mesher.Perform();
    QymDoc* doc = new QymDoc();
    // every solid becomes a body of its own, so it can be selected and hidden in the tree
    for (TopExp_Explorer ex(shape, TopAbs_SOLID); ex.More(); ex.Next()) {
        QymBody b = body_from_shape(ex.Current(), fids);
        if (!b.tris.empty()) doc->bodies.push_back(std::move(b));
    }
    // with no solids (sheet bodies), the whole shape becomes one body
    if (doc->bodies.empty()) {
        QymBody b = body_from_shape(shape, fids);
        if (!b.tris.empty()) doc->bodies.push_back(std::move(b));
    }
    return doc;
}
// the overload without ids (a box, a STEP import)
QymDoc* doc_from_shape(const TopoDS_Shape& shape, double defl) {
    return doc_from_shape(shape, defl, empty_fids());
}

// --- PERSISTENT NAMES for sub-shapes: both FACES and EDGES -----------------------------------
// Seed ids onto the sub-shapes of type `ty` (FACE or EDGE) in TopExp order (1..n) — stable for a recipe.
void seed_ids(const TopoDS_Shape& s, TopAbs_ShapeEnum ty, TopTools_DataMapOfShapeInteger& ids) {
    int id = 1;
    for (TopExp_Explorer ex(s, ty); ex.More(); ex.Next()) {
        if (!ids.IsBound(ex.Current())) ids.Bind(ex.Current(), id++);
    }
}
// A STRUCTURAL NAME is marked by this bit (a descriptor from the document's name table, see names.rs).
// Anything without the mark is a POSITIONAL number, which the kernel hands to whatever origin is not
// derived yet.

// A free POSITIONAL number: structural names do not count towards it. Otherwise "max + 1" taken from a
// descriptor (0x4000xxxx) would produce a number WITH THE MARK — it would look like a structural name
// while pointing past the table, and the reference would resolve onto the wrong face.
int next_local(const TopTools_DataMapOfShapeInteger& m) {
    int mx = 0;
    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(m); it.More(); it.Next())
        if ((it.Value() & QYM_NAMED) == 0 && it.Value() > mx) mx = it.Value();
    return mx + 1;
}
// Carry the ids of type `ty` from ONE OPERAND `a` onto the result through the operation's history
// (Modified and Generated). It does NOT fill in the unnamed ones — the operation may have a second
// operand whose names are just as legitimate (see `qym_shape_boolean`).
// `named_only` carries ONLY structural names. A positional number means something only inside its own
// body: "face 1" of the base and "face 1" of the tool are different faces, and carrying both would glue
// them into one name. So from the second operand only what the recipe named is taken, and the rest gets a
// fresh number.
void carry_ids(BRepBuilderAPI_MakeShape& algo, const TopoDS_Shape& a, TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& aid, TopTools_DataMapOfShapeInteger& out, int& next, bool named_only, TopTools_DataMapOfShapeInteger* splits_of, TopTools_DataMapOfShapeInteger* splits_idx, const std::map<int,int>* gen_names) {
    // THE NAMES ALREADY TAKEN IN THE RESULT. A name is an address: two elements of one body sharing an id
    // are indistinguishable, and a reference to one of them means both. Only the Modified loop used to
    // watch for this (within a single source), and a duplicate passed freely through Generated.
    std::unordered_set<int> taken;
    // WHOSE NUMBER IT IS. One number, one face: two faces under one name make a reference ambiguous, and
    // it leads to both places at once. Knowing that a number is taken is not enough — one has to know BY
    // WHOM: an image may take the number of ITS OWN source (that one has left the result), but not of
    // somebody else's.
    std::map<int, TopoDS_Shape> owner;
    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(out); it.More(); it.Next()) {
        taken.insert(it.Value());
        if (!owner.count(it.Value())) owner[it.Value()] = it.Key();
    }
    for (TopExp_Explorer ex(a, ty); ex.More(); ex.Next()) {
        const TopoDS_Shape& f = ex.Current();
        if (!aid.IsBound(f)) continue;
        int id = aid.Find(f);
        if (named_only && (id & QYM_NAMED) == 0) continue;
        if (!out.IsBound(f)) {
            // A FACE THAT SURVIVED UNCHANGED must be distinguishable too. A thread tool arrives as pieces
            // of the helical groove that share ONE name between them: the cut leaves their surfaces in the
            // body as they are, and without this check all twelve pieces carried the same number
            // (measured).
            auto own0 = owner.find(id);
            const int use0 = (own0 == owner.end() || own0->second.IsSame(f)) ? id : next++;
            out.Bind(f, use0);
            taken.insert(use0);
            if (!owner.count(use0)) owner[use0] = f;
            if (use0 != id && splits_of && splits_idx) {
                splits_of->Bind(f, id);
                splits_idx->Bind(f, (int)splits_idx->Extent() + 1);
            }
        }
        // A SPLIT (one source edge or face becoming several pieces): THE FIRST piece keeps the id and the
        // rest get FRESH ones. Otherwise every piece shares an id and selecting one catches its neighbours
        // (in a monolith the upper and lower segments of a vertical edge are DIFFERENT edges). The Modified
        // order is deterministic (TopExp), so the ids stay stable across a change of parameters: the same
        // topology gives the same split.
        const TopTools_ListOfShape& mod = algo.Modified(f);
        int piece = 0;
        for (TopTools_ListIteratorOfListOfShape it(mod); it.More(); it.Next()) {
            if (out.IsBound(it.Value())) continue;
            // AN IMAGE REPLACES ITS OWN SOURCE, so the first piece takes its id without asking whether it
            // is taken: it is taken by exactly that source, which is no longer in the result. Taking
            // SOMEBODY ELSE'S number is not allowed — and that is what the thread did: sixteen pieces of
            // the helical groove came from DIFFERENT faces of the tool sharing one name, and all sixteen
            // took it in the result (measured: 51 extra faces on one body). Another's number is yielded: a
            // fresh number and a record of "piece k".
            auto own = owner.find(id);
            const bool mine = own == owner.end() || own->second.IsSame(f);
            int use = (piece == 0 && mine) ? id : next++;
            out.Bind(it.Value(), use);
            taken.insert(use);
            if (!owner.count(use)) owner[use] = it.Value();
            // pieces from the second onwards (and anything that yielded another's number) become "piece k of face id"
            if (use != id && splits_of && splits_idx) {
                splits_of->Bind(it.Value(), id);
                splits_idx->Bind(it.Value(), piece + 1);
            }
            ++piece;
        }
        // A GENERATED face is NOT the source. In a shell it is the inner wall: taking the source's id made
        // it indistinguishable from its outer face, and a reference to an inner edge landed on the outer
        // one. If the caller seeded a name for this face's image, that name is handed out; otherwise the
        // old behaviour applies (operations where the image really does continue the source).
        const TopTools_ListOfShape& gen = algo.Generated(f);
        int gid = id;
        if (gen_names) {
            auto it2 = gen_names->find(id);
            if (it2 != gen_names->end()) gid = it2->second;
        }
        // A GENERATED ELEMENT TAKES THE SOURCE'S NAME ONLY IF THE SOURCE GAVE IT UP. Shelling a through
        // pocket: the outer edge of the end face stays itself and keeps its name, while the inner one is
        // born from it — and used to take the very same name. Two different edges with one id: a click on
        // one highlighted both, a fillet cut both, and there was NO WAY to pick one. On a real part 8 pairs
        // out of 24 stuck together this way.
        for (TopTools_ListIteratorOfListOfShape it(gen); it.More(); it.Next()) {
            if (out.IsBound(it.Value())) continue;
            int use = taken.count(gid) ? next++ : gid;
            out.Bind(it.Value(), use);
            taken.insert(use);
            if (!owner.count(use)) owner[use] = it.Value();
        }
    }
}

// Everything left unnamed after the transfer (the operation's new geometry) gets a POSITIONAL number.
void fill_unnamed(const TopoDS_Shape& res, TopAbs_ShapeEnum ty, TopTools_DataMapOfShapeInteger& out, int& next) {
    for (TopExp_Explorer ex(res, ty); ex.More(); ex.Next())
        if (!out.IsBound(ex.Current())) out.Bind(ex.Current(), next++);
}

// A single-source operation: transfer plus fill-in.
// The face's outward normal at the middle of its parametric domain (needed where a face has to be pushed
// INTO the body: the faces a shell removes are opened by a prism running exactly against it).
gp_Vec face_normal_vec(const TopoDS_Face& f) {
    Handle(Geom_Surface) su = BRep_Tool::Surface(f);
    if (su.IsNull()) return gp_Vec(0, 0, 0);
    Standard_Real u1, u2, v1, v2;
    BRepTools::UVBounds(f, u1, u2, v1, v2);
    GeomLProp_SLProps pr(su, (u1 + u2) * 0.5, (v1 + v2) * 0.5, 1, 1e-6);
    if (!pr.IsNormalDefined()) return gp_Vec(0, 0, 0);
    gp_Dir n = pr.Normal();
    if (f.Orientation() == TopAbs_REVERSED) n.Reverse();
    return gp_Vec(n);
}

void propagate_ids(BRepBuilderAPI_MakeShape& algo, const TopoDS_Shape& a, TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& aid, const TopoDS_Shape& res, TopTools_DataMapOfShapeInteger& out, TopTools_DataMapOfShapeInteger* splits_of, TopTools_DataMapOfShapeInteger* splits_idx, const std::map<int,int>* gen_names) {
    int next = next_local(aid);
    carry_ids(algo, a, ty, aid, out, next, false, splits_of, splits_idx, gen_names);
    fill_unnamed(res, ty, out, next);
}

// --- A FACE IS ONE CONNECTED PIECE OF SURFACE --------------------------------------------------
//
// A fillet or a chamfer can eat a face not all the way but down to ZERO WIDTH: the inner contour runs
// into the outer one, and what is left lies in two different places of the part — while in the kernel it
// is still ONE face with one id. Everything falls apart after that: a click highlights both places, a
// push raises both, a fillet cuts both, and there is NO WAY to pick one. On such a contour the
// tessellator gives up altogether and the face vanishes from the screen entirely, while the part's volume
// is computed from the holed mesh.
//
// The cure is not a patch on the tool but a repair of the topology: the face's edges are intersected with
// one another (coincident pieces become ONE edge), and then the regions are assembled anew. However many
// regions come out, that many faces there are; the first keeps the former name and the rest are called
// "piece k" of the same face (`fsplit_*`), just as in any other split.

// THE FACE IS PINCHED — a cheap check of whether the heavy machinery is worth calling. A pinch is exactly
// this: two pieces of the boundary have met where they should not. They cannot be kin — adjacent edges
// share a vertex, which is legitimate, so such pairs do not count. On an ordinary face every other pair is
// separated by its bounding box, and it never comes to computing distances.
//
// Looking at the CONTOURS ("a hole has touched the edge") is not enough: after the first repair there is
// one contour, and the pinch is still there — now inside it.
static bool face_is_pinched(const TopoDS_Face& f, double tol) {
    std::vector<TopoDS_Shape> es;
    std::vector<Bnd_Box> bx;
    for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next()) {
        bool dup = false;
        for (const TopoDS_Shape& p : es)
            if (p.IsSame(e.Current())) { dup = true; break; } // the seam of a periodic surface is legitimate
        if (dup) continue;
        es.push_back(e.Current());
        Bnd_Box b;
        BRepBndLib::Add(e.Current(), b);
        b.Enlarge(tol);
        bx.push_back(b);
    }
    for (size_t i = 0; i < es.size(); ++i) {
        for (size_t j = i + 1; j < es.size(); ++j) {
            if (bx[i].IsOut(bx[j])) continue;
            bool kin = false; // a shared vertex means the edges are merely neighbours along the contour
            for (TopExp_Explorer a(es[i], TopAbs_VERTEX); a.More() && !kin; a.Next())
                for (TopExp_Explorer b(es[j], TopAbs_VERTEX); b.More(); b.Next())
                    if (a.Current().IsSame(b.Current())) { kin = true; break; }
            if (kin) continue;
            BRepExtrema_DistShapeShape d(es[i], es[j]);
            if (d.IsDone() && d.NbSolution() > 0 && d.Value() <= tol) return true;
        }
    }
    return false;
}

// Carry the names of type `ty` from shape `a` onto its COPY `b`: the copy is walked in the same order as
// the original — that is a property of a copy, not a coincidence.
static void twin_map(const TopoDS_Shape& a, const TopoDS_Shape& b, TopAbs_ShapeEnum ty,
                     const TopTools_DataMapOfShapeInteger& src, TopTools_DataMapOfShapeInteger& dst) {
    TopExp_Explorer x(a, ty), y(b, ty);
    for (; x.More() && y.More(); x.Next(), y.Next())
        if (src.IsBound(x.Current()) && !dst.IsBound(y.Current())) dst.Bind(y.Current(), src.Find(x.Current()));
}

// THE 2D CURVE OF A PIECE ON THE NEIGHBOURING FACE — EXACTLY, NOT BY PROJECTION.
//
// The edges are intersected with one another, without faces, so the pieces have no projection onto the
// neighbouring face's surface — and without one the face is invalid: its area comes out as zero. A
// projection will not do here: on a cylinder it takes the wrong branch, a fillet's area grows by 2*pi, and
// the invalidity only surfaces AFTER tessellation, once the timeline has already accepted the body as the
// part.
//
// A piece lies on the same line as the source edge, so its 2D curve is A PIECE of the source's 2D curve.
// That curve is taken, trimmed at the piece's ends, and its parametrisation stretched linearly onto the
// piece's own range. The stretch is needed because a merged edge (two coincident ones became one) carries
// the parametrisation of one of the two while its 2D curve carries the other's: without the recomputation
// the face inflates.
static Handle(Geom2d_Curve) pcurve_of_piece(const TopoDS_Edge& src, const TopoDS_Edge& part, const TopoDS_Face& host) {
    if (!BRep_Tool::SameRange(src)) return Handle(Geom2d_Curve)(); // the 2D and 3D parameters have drifted apart: not our case
    Standard_Real a2 = 0.0, b2 = 0.0;
    Handle(Geom2d_Curve) pc = BRep_Tool::CurveOnSurface(src, host, a2, b2);
    if (pc.IsNull()) return Handle(Geom2d_Curve)();
    Standard_Real s0 = 0.0, s1 = 0.0;
    Handle(Geom_Curve) sc = BRep_Tool::Curve(src, s0, s1);
    if (sc.IsNull() || s1 - s0 < Precision::PConfusion()) return Handle(Geom2d_Curve)();

    TopoDS_Vertex v0 = TopExp::FirstVertex(part, Standard_True), v1 = TopExp::LastVertex(part, Standard_True);
    if (v0.IsNull() || v1.IsNull()) return Handle(Geom2d_Curve)();
    GeomAPI_ProjectPointOnCurve j0(BRep_Tool::Pnt(v0), sc, s0, s1), j1(BRep_Tool::Pnt(v1), sc, s0, s1);
    if (j0.NbPoints() < 1 || j1.NbPoints() < 1) return Handle(Geom2d_Curve)();
    const Standard_Real ta = j0.LowerDistanceParameter(), tb = j1.LowerDistanceParameter();
    if (std::abs(tb - ta) < Precision::PConfusion()) return Handle(Geom2d_Curve)();

    Standard_Real p0 = 0.0, p1 = 0.0;
    BRep_Tool::Range(part, p0, p1);
    if (p1 - p0 < Precision::PConfusion()) return Handle(Geom2d_Curve)();
    try {
        Handle(Geom2d_TrimmedCurve) cut = new Geom2d_TrimmedCurve(pc, std::min(ta, tb), std::max(ta, tb));
        Handle(Geom2d_BSplineCurve) bs = Geom2dConvert::CurveToBSplineCurve(cut);
        if (bs.IsNull()) return Handle(Geom2d_Curve)();
        if (tb < ta) bs->Reverse(); // the piece runs against the source, so its 2D curve has to run that way too
        TColStd_Array1OfReal knots(1, bs->NbKnots());
        bs->Knots(knots);
        BSplCLib::Reparametrize(p0, p1, knots);
        bs->SetKnots(knots);
        return bs;
    } catch (...) { return Handle(Geom2d_Curve)(); }
}

// Returns how many faces were repaired; 0 means there was nothing to touch, and then `shape` is unchanged.
int heal_pinched_faces(TopoDS_Shape& shape,
                              TopTools_DataMapOfShapeInteger& fids,
                              TopTools_DataMapOfShapeInteger& eids,
                              TopTools_DataMapOfShapeInteger& fsplit_of,
                              TopTools_DataMapOfShapeInteger& fsplit_idx) {
    if (shape.IsNull()) return 0;
    const double TOL = 1e-6; // a touch is a touch, not "nearly adjacent": the monolith's 1e-4 would be coarse here
    bool any = false;
    for (TopExp_Explorer e(shape, TopAbs_FACE); e.More() && !any; e.Next())
        any = face_is_pinched(TopoDS::Face(e.Current()), TOL);
    if (!any) return 0;

    // THE WORK IS DONE ON A COPY, AND THAT IS NOT MERE CAUTION. A boolean is ALLOWED to nudge the geometry
    // of its arguments, and the argument here is the part's live body. The price shows up later: the pass
    // might change nothing (one region came out, so there is nothing to split), yet the body had already
    // moved by 2e-5 and the NEXT operation missed the contact. In a run this looked like "every other
    // time": the answer depended on what a neighbouring thread was doing. The copy is thrown away if there
    // turned out to be nothing to repair.
    BRepBuilderAPI_Copy cp(shape);
    TopoDS_Shape work = cp.Shape();
    if (work.IsNull()) return 0;
    TopTools_DataMapOfShapeInteger wfids, weids, wsof, wsidx;
    twin_map(shape, work, TopAbs_FACE, fids, wfids);
    twin_map(shape, work, TopAbs_EDGE, eids, weids);
    twin_map(shape, work, TopAbs_FACE, fsplit_of, wsof);
    twin_map(shape, work, TopAbs_FACE, fsplit_idx, wsidx);

    TopTools_IndexedMapOfShape faces;
    TopExp::MapShapes(work, TopAbs_FACE, faces);
    // Who else holds this edge: a cut edge has a second face, and that face needs the same pieces.
    TopTools_IndexedDataMapOfShapeListOfShape owners;
    TopExp::MapShapesAndAncestors(work, TopAbs_EDGE, TopAbs_FACE, owners);
    BRepTools_ReShape rs;
    BRep_Builder bb;
    TopTools_DataMapOfShapeInteger new_f, new_e, new_split_of, new_split_idx;
    int nf = next_local(wfids), ne = next_local(weids), healed = 0;

    for (int i = 1; i <= faces.Extent(); ++i) {
        const TopoDS_Face& f = TopoDS::Face(faces(i));
        if (!face_is_pinched(f, TOL)) continue;

        TopTools_ListOfShape raw;
        for (TopExp_Explorer e(f, TopAbs_EDGE); e.More(); e.Next()) raw.Append(e.Current());
        // SELF-INTERSECTING THE EDGES: coincident pieces become ONE edge, and a long one is cut at the
        // points of contact. Without this the region builder reads the contours as "a hole inside" and
        // honestly returns one region covering two places.
        // AN ALLOCATOR OF ITS OWN PER CALL. The low-level BOP algorithms take OCCT's SHARED incremental
        // allocator by default — one per process and with no protection at all. While another part is being
        // computed alongside, that is not "slower" but GARBAGE: in a run the neighbouring thread got a
        // chamfer in the wrong place or a fillet of the wrong depth.
        Handle(NCollection_IncAllocator) heap = new NCollection_IncAllocator();
        BOPAlgo_Builder gf(heap);
        for (TopTools_ListIteratorOfListOfShape it(raw); it.More(); it.Next()) gf.AddArgument(it.Value());
        gf.Perform();
        if (gf.HasErrors()) continue;
        TopTools_ListOfShape parts;
        TopTools_MapOfShape uniq;
        for (TopExp_Explorer e(gf.Shape(), TopAbs_EDGE); e.More(); e.Next())
            if (uniq.Add(e.Current())) parts.Append(e.Current());

        // The edges that stayed themselves: if the intersection touched nothing, there is nothing to split.
        bool intact = parts.Extent() == raw.Extent();
        if (intact)
            for (TopTools_ListIteratorOfListOfShape it(raw); it.More(); it.Next())
                if (!uniq.Contains(it.Value())) { intact = false; break; }

        BOPAlgo_BuilderFace bf(heap);
        bf.SetFace(TopoDS::Face(f.Oriented(TopAbs_FORWARD)));
        bf.SetShapes(parts);
        bf.Perform();
        // ONE REGION IS ALSO A REASON, IF THE CONTOUR WAS REBUILT. While the end face is eaten from one
        // side the piece stays connected, but the contour is invalid — and the tessellator silently returns
        // nothing: the face vanishes from the screen entirely and the volume is computed from a holed mesh.
        // A rebuilt contour does tessellate.
        if (bf.HasErrors() || bf.Areas().IsEmpty()) continue;
        // ONE REGION IS A REASON ONLY WHEN THE FACE DOES NOT TESSELLATE. While the end face is eaten from
        // one side the piece stays connected and there is nothing to split; but if the contour is so
        // invalid that the tessellator returns nothing, the face vanishes from the part altogether — and
        // that is what gets repaired. A healthy face whose contours merely touched is left alone: rebuilding
        // for nothing shifts the geometry, and the next operation (a chamfer fitted tight) stops passing.
        if (bf.Areas().Extent() == 1) {
            TopoDS_Shape probe = BRepBuilderAPI_Copy(f).Shape();
            BRepMesh_IncrementalMesh(probe, 0.1, Standard_False, 0.3, Standard_False).Perform();
            TopLoc_Location loc;
            if (!BRep_Tool::Triangulation(TopoDS::Face(probe), loc).IsNull()) continue;
        }

        // A GUARD: the total area has to match the previous one. Otherwise this is not a repair of the
        // topology but a substitution of the geometry, and then leaving things as they were is better.
        GProp_GProps g0;
        BRepGProp::SurfaceProperties(f, g0);
        double sum = 0.0;
        for (TopTools_ListIteratorOfListOfShape it(bf.Areas()); it.More(); it.Next()) {
            GProp_GProps g;
            BRepGProp::SurfaceProperties(it.Value(), g);
            sum += g.Mass();
        }
        if (std::abs(sum - g0.Mass()) > 1e-6 * std::max(1.0, g0.Mass())) continue;

        for (TopTools_ListIteratorOfListOfShape it(raw); it.More(); it.Next()) {
            const TopoDS_Edge& src = TopoDS::Edge(it.Value());
            const TopTools_ListOfShape& im = gf.Modified(src);
            if (im.IsEmpty()) continue;
            // AN IMAGE EQUAL TO ITS SOURCE is not a replacement. Substituting an edge with a wire that
            // contains that very edge loops the rebuild: `ReShape` goes into endless recursion.
            if (im.Extent() == 1 && im.First().IsSame(src)) continue;
            // THE PIECES GET THEIR 2D CURVES FROM THE SOURCE EDGE, NOT BY PROJECTION. The edges were
            // intersected without faces, so the pieces have no projection onto the NEIGHBOUR's surface, and
            // without one the face is invalid: its area comes out as zero. A piece lies on the same curve
            // with the same parametrisation, so the source's 2D curve fits it exactly — there is nothing to
            // approximate, and an approximation is precisely what would be new geometry.
            if (owners.Contains(src)) {
                const TopTools_ListOfShape& hosts = owners.FindFromKey(src);
                for (TopTools_ListIteratorOfListOfShape of(hosts); of.More(); of.Next()) {
                    const TopoDS_Face& host = TopoDS::Face(of.Value());
                    for (TopTools_ListIteratorOfListOfShape k(im); k.More(); k.Next()) {
                        const TopoDS_Edge& part = TopoDS::Edge(k.Value());
                        Standard_Real c2 = 0.0, d2 = 0.0;
                        if (!BRep_Tool::CurveOnSurface(part, host, c2, d2).IsNull()) continue;
                        Handle(Geom2d_Curve) pc = pcurve_of_piece(src, part, host);
                        if (pc.IsNull()) continue; // it did not come out exactly, so the face is better left as it was
                        bb.UpdateEdge(part, pc, host, BRep_Tool::Tolerance(src));
                        bb.SameRange(part, Standard_True);
                        bb.SameParameter(part, Standard_False); // `BRepLib::SameParameter` below will bring it together
                    }
                }
            }
            // THE PIECES ARE LAID IN ORDER ALONG THE SOURCE EDGE AND FACE THE SAME WAY IT DOES. Replacing
            // one edge with a set substitutes a stretch of the neighbouring face's contour; out of order it
            // gives a contour that cannot be walked, and the body comes out invalid.
            std::vector<TopoDS_Shape> ordered;
            for (TopTools_ListIteratorOfListOfShape k(im); k.More(); k.Next()) {
                TopoDS_Shape q = k.Value();
                q.Orientation(TopAbs_FORWARD);
                ordered.push_back(q);
            }
            std::sort(ordered.begin(), ordered.end(), [](const TopoDS_Shape& u, const TopoDS_Shape& v) {
                Standard_Real u0 = 0.0, u1 = 0.0, v0 = 0.0, v1 = 0.0;
                BRep_Tool::Range(TopoDS::Edge(u), u0, u1);
                BRep_Tool::Range(TopoDS::Edge(v), v0, v1);
                return u0 < v0;
            });
            TopoDS_Wire w;
            bb.MakeWire(w);
            for (const TopoDS_Shape& q : ordered) bb.Add(w, q);
            rs.Replace(src.Oriented(TopAbs_FORWARD), w);
            if (!weids.IsBound(src)) continue;
            int id = weids.Find(src), piece = 0;
            for (TopTools_ListIteratorOfListOfShape k(im); k.More(); k.Next()) {
                if (new_e.IsBound(k.Value())) continue; // already named (two coincident edges gave one image)
                new_e.Bind(k.Value(), piece == 0 ? id : ne++);
                ++piece;
            }
        }

        TopoDS_Compound pieces;
        bb.MakeCompound(pieces);
        const int fid = wfids.IsBound(f) ? wfids.Find(f) : 0;
        int piece = 0;
        for (TopTools_ListIteratorOfListOfShape it(bf.Areas()); it.More(); it.Next()) {
            TopoDS_Shape a = it.Value();
            if (f.Orientation() == TopAbs_REVERSED) a.Reverse(); // the piece faces the same way the source did
            bb.Add(pieces, a);
            if (fid && !new_f.IsBound(a)) {
                new_f.Bind(a, piece == 0 ? fid : nf++);
                if (piece > 0) {
                    new_split_of.Bind(a, fid);
                    new_split_idx.Bind(a, piece);
                }
            }
            ++piece;
        }
        rs.Replace(f, pieces);
        ++healed;
    }
    if (!healed) return 0; // the copy is discarded: the part is exactly what the operation built
    TopoDS_Shape out = rs.Apply(work);
    if (out.IsNull()) return 0;
    // BRING THE PARAMETRISATION TOGETHER BEFORE MEASURING. The carried 2D curves have to agree with the
    // 3D ones — otherwise a wrong-branch miss on a cylinder is invisible to both the check and the area: it
    // only surfaces after tessellation, once the timeline has accepted the body as the part.
    BRepLib::SameParameter(out, Precision::Confusion(), Standard_True);
    // A GUARD OVER THE WHOLE BODY: the pass changes the TOPOLOGY, while the geometry has to stay the same
    // down to the micron. Rebuilding the contours touches the neighbouring faces, and on a cylindrical
    // neighbour carrying the 2D curve can miss by a branch: such a face's area grows fivefold, and the body
    // becomes invalid only after tessellation, once the timeline has accepted it as the part. Area and
    // volume catch it together.
    {
        GProp_GProps sa, sb, va, vb;
        BRepGProp::SurfaceProperties(work, sa);
        BRepGProp::SurfaceProperties(out, sb);
        BRepGProp::VolumeProperties(work, va);
        BRepGProp::VolumeProperties(out, vb);
        if (std::abs(sb.Mass() - sa.Mass()) > 1e-6 * std::max(1.0, sa.Mass())) return 0;
        if (std::abs(vb.Mass() - va.Mass()) > 1e-6 * std::max(1.0, std::abs(va.Mass()))) return 0;
    }
    // THE REPAIR HAS NO RIGHT TO SPOIL ANYTHING. Rebuilding the contours touches the neighbouring faces,
    // and if the body is invalid afterwards the part stays as it was: a face lying in two places is better
    // than a corpse.
    if (!BRepCheck_Analyzer(out).IsValid()) return 0;
    // THE NAMES MOVE ONTO THE IMAGES. Rebuilding the contours creates new shapes even where a face was not
    // split: the neighbour had an edge replaced, so the neighbour itself is now a different shape.
    auto move_map = [&](TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& src, TopTools_DataMapOfShapeInteger& dst) {
        for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(src); it.More(); it.Next()) {
            TopoDS_Shape im = rs.Apply(it.Key());
            if (im.IsNull() || im.ShapeType() != ty || dst.IsBound(im)) continue;
            dst.Bind(im, it.Value());
        }
    };
    move_map(TopAbs_FACE, wfids, new_f);
    move_map(TopAbs_EDGE, weids, new_e);
    // AN EMPTY MAP STAYS EMPTY: a primitive and an import have no names at all, and inventing them here is not allowed.
    if (!wfids.IsEmpty()) fill_unnamed(out, TopAbs_FACE, new_f, nf);
    if (!weids.IsEmpty()) fill_unnamed(out, TopAbs_EDGE, new_e, ne);
    for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(wsof); it.More(); it.Next()) {
        TopoDS_Shape im = rs.Apply(it.Key());
        if (im.IsNull() || im.ShapeType() != TopAbs_FACE || new_split_of.IsBound(im)) continue;
        new_split_of.Bind(im, it.Value());
        if (wsidx.IsBound(it.Key())) new_split_idx.Bind(im, wsidx.Find(it.Key()));
    }
    shape = out;
    fids = new_f;
    eids = new_e;
    fsplit_of = new_split_of;
    fsplit_idx = new_split_idx;
    return healed;
}

// THE WINNER OF A MERGE IS CHOSEN BY NAME, NOT BY TRAVERSAL ORDER.
//
// Merging coplanar faces is legitimate (the monolith), but DIFFERENT names collapse into one face and one
// of them has to yield. Whatever came later in the traversal used to yield — that is, the sub-shape
// numbering made the choice. Measured on a real part: one and the same edit gave the win now to the
// smaller name, now to the larger; six walls lost their name not because they had gone, but because the
// draw fell differently.
//
// The rule: a name beats a positional number, and among names THE SMALLER DESCRIPTOR wins. A descriptor is
// an index into the document's name table; it lives in the file and depends on neither the traversal nor
// the session, so one and the same merge always has the same winner.
// `absorbed` holds pairs of "losing name -> winning name". The loser is NOT LOST: the model layer
// remembers the absorption, and a reference to the former name goes on finding the same (now shared) face.
// Otherwise a fillet disappears exactly where two walls honestly became one.
static void bind_winner(TopTools_DataMapOfShapeInteger& out, const TopoDS_Shape& sh, int id,
                        std::vector<std::pair<unsigned, unsigned>>* absorbed) {
    if (!out.IsBound(sh)) { out.Bind(sh, id); return; }
    const int have = out.Find(sh);
    if (have == id) return;
    const bool have_named = (have & QYM_NAMED) != 0, id_named = (id & QYM_NAMED) != 0;
    auto note = [&](int loser, int winner) {
        if (absorbed && (loser & QYM_NAMED) && (winner & QYM_NAMED)) absorbed->emplace_back((unsigned)loser, (unsigned)winner);
    };
    if (id_named && !have_named) { out.Bind(sh, id); return; }          // a name beats a number
    if (id_named && have_named) {
        if ((unsigned)id < (unsigned)have) { note(have, id); out.Bind(sh, id); } // among names, the smaller one
        else note(id, have);
    }
}

// A MONOLITH: after a boolean, merge COPLANAR faces and COLLINEAR edges
// (ShapeUpgrade_UnifySameDomain). Without it the seam left by Fuse or Cut keeps doubled edges, and the
// tools (chamfer, fillet, shell) see "two edges instead of one" — the body becomes unusable. Ids are
// carried through the unify history: merged shapes take the id of THE FIRST, untouched ones keep their
// own, and anything new gets a fresh id.
TopoDS_Shape unify_monolithic(const TopoDS_Shape& in,
                                     TopTools_DataMapOfShapeInteger& fids,
                                     TopTools_DataMapOfShapeInteger& eids,
                                     std::vector<std::pair<unsigned, unsigned>>* absorbed) {
    if (in.IsNull()) return in;
    try {
        ShapeUpgrade_UnifySameDomain uni(in, Standard_True, Standard_True, Standard_False);
        // THE MERGING TOLERANCE: real models carry about 1e-5 of noise (snapping to the tessellation,
        // dragged heights) — "nearly coplanar" walls would not merge, the body was fractured by seams and
        // fillets died. 1e-4 mm is an order of magnitude below any design intent, so only noise merges.
        uni.SetLinearTolerance(1e-4);
        uni.SetAngularTolerance(1e-4);
        uni.Build();
        TopoDS_Shape res = uni.Shape();
        if (res.IsNull()) return in;
        Handle(BRepTools_History) hist = uni.History();
        auto remap = [&](TopAbs_ShapeEnum ty, TopTools_DataMapOfShapeInteger& ids) {
            TopTools_DataMapOfShapeInteger out;
            int next = next_local(ids);
            // IN A MERGE A NAME BEATS A NUMBER — and that is not a matter of style.
            //
            // The monolith glues coplanar pieces into one face. The glued result used to take the id of
            // whichever piece came first in the traversal — and the traversal does not know which is a name
            // and which a positional number. On a real part 7 names out of 13 were lost this way in a single
            // face push: the walls became nameless, and a thickening standing on them lost its face at the
            // very next edit.
            //
            // Hence two passes: the STRUCTURAL pieces hand out names first, the positional ones second. If
            // even one piece of the glue was named, the name survives the merge.
            for (int pass = 0; pass < 2; ++pass) {
                for (TopExp_Explorer ex(in, ty); ex.More(); ex.Next()) {
                    const TopoDS_Shape& s = ex.Current();
                    if (!ids.IsBound(s)) continue;
                    int id = ids.Find(s);
                    const bool named = (id & QYM_NAMED) != 0;
                    if ((pass == 0) != named) continue; // the first pass takes only names, the second only numbers
                    bool has_mod = false;
                    if (!hist.IsNull()) {
                        const TopTools_ListOfShape& mod = hist->Modified(s);
                        for (TopTools_ListIteratorOfListOfShape it(mod); it.More(); it.Next()) {
                            has_mod = true;
                            bind_winner(out, it.Value(), id, ty == TopAbs_FACE ? absorbed : nullptr);
                        }
                    }
                    bool removed = !hist.IsNull() && hist->IsRemoved(s);
                    if (!has_mod && !removed) bind_winner(out, s, id, ty == TopAbs_FACE ? absorbed : nullptr); // untouched: the same shape is in `res`
                }
            }
            for (TopExp_Explorer ex(res, ty); ex.More(); ex.Next())
                if (!out.IsBound(ex.Current())) out.Bind(ex.Current(), next++); // new sub-shapes get a fresh id
            ids = out;
        };
        remap(TopAbs_FACE, fids);
        remap(TopAbs_EDGE, eids);
        return res;
    } catch (...) {
        return in; // the unify failed, so it comes back as it was — no worse than before
    }
}


extern "C" QymDoc* qym_occt_step_read(const char* path, double defl) {
    try {
        STEPControl_Reader reader;
        if (reader.ReadFile(path) != IFSelect_RetDone) return nullptr;
        reader.TransferRoots();
        TopoDS_Shape shape = reader.OneShape();
        if (shape.IsNull()) return nullptr;
        return doc_from_shape(shape, defl);
    } catch (...) {
        return nullptr; // a broken or truncated STEP, or running out of memory: an honest refusal rather than a crash
    }
}

extern "C" QymDoc* qym_occt_box_doc(double dx, double dy, double dz, double defl) {
    try {
        TopoDS_Shape shape = BRepPrimAPI_MakeBox(dx, dy, dz).Shape();
        return doc_from_shape(shape, defl);
    } catch (...) {
        return nullptr; // degenerate dimensions (zero or negative): OCCT throws rather than returns
    }
}

// Extrude a closed 2D profile (xy at Z=0) to the height `height` along +Z, giving a body.
extern "C" QymDoc* qym_occt_extrude(const double* xy, size_t n, double height, double defl) {
    try {
        if (n < 3) return nullptr;
        BRepBuilderAPI_MakePolygon poly;
        for (size_t i = 0; i < n; ++i) {
            poly.Add(gp_Pnt(xy[2 * i], xy[2 * i + 1], 0.0));
        }
        poly.Close();
        if (!poly.IsDone()) return nullptr;
        BRepBuilderAPI_MakeFace mkface(poly.Wire(), Standard_True);
        if (!mkface.IsDone()) return nullptr;
        TopoDS_Shape solid = BRepPrimAPI_MakePrism(mkface.Face(), gp_Vec(0.0, 0.0, height)).Shape();
        if (solid.IsNull()) return nullptr;
        return doc_from_shape(solid, defl);
    } catch (...) {
        return nullptr; // an exception crossing the C ABI aborts the process, so an honest refusal goes back instead
    }
}

// Revolve a closed profile (xy at Z=0) about an axis (0 = X, 1 = Y) through an angle, giving a body.
extern "C" QymDoc* qym_occt_revolve(const double* xy, size_t n, int axis, double angle_deg, double defl) {
    try {
        if (n < 3) return nullptr;
        BRepBuilderAPI_MakePolygon poly;
        for (size_t i = 0; i < n; ++i) {
            poly.Add(gp_Pnt(xy[2 * i], xy[2 * i + 1], 0.0));
        }
        poly.Close();
        if (!poly.IsDone()) return nullptr;
        BRepBuilderAPI_MakeFace mkface(poly.Wire(), Standard_True);
        if (!mkface.IsDone()) return nullptr;
        gp_Dir dir = (axis == 0) ? gp_Dir(1.0, 0.0, 0.0) : gp_Dir(0.0, 1.0, 0.0);
        gp_Ax1 ax(gp_Pnt(0.0, 0.0, 0.0), dir);
        const double ang = angle_deg * 3.14159265358979323846 / 180.0;
        TopoDS_Shape solid = BRepPrimAPI_MakeRevol(mkface.Face(), ax, ang).Shape();
        if (solid.IsNull()) return nullptr;
        return doc_from_shape(solid, defl);
    } catch (...) {
        return nullptr; // an exception crossing the C ABI aborts the process, so an honest refusal goes back instead
    }
}

// A helper that extrudes a closed profile into a shape.
static TopoDS_Shape extrude_shape(const double* xy, size_t n, double h) {
    BRepBuilderAPI_MakePolygon poly;
    for (size_t i = 0; i < n; ++i) poly.Add(gp_Pnt(xy[2 * i], xy[2 * i + 1], 0.0));
    poly.Close();
    if (!poly.IsDone()) return TopoDS_Shape();
    BRepBuilderAPI_MakeFace mkface(poly.Wire(), Standard_True);
    if (!mkface.IsDone()) return TopoDS_Shape();
    return BRepPrimAPI_MakePrism(mkface.Face(), gp_Vec(0.0, 0.0, h)).Shape();
}

// --- EXACT profiles (a B-rep built from real curves) ------------------------------------------
// The encoding of the flat array of doubles: [L, then for each contour: nedges, then 8 numbers per edge:
//   kind, ax, ay, bx, by, cx, cy, ccw]. kind 0 is a segment (a to b), 1 an arc (a to b about c, ccw), 2 a
//   full circle (centre c, radius = ax — a contour of one "edge"). The first contour is the outer one, the
//   rest are holes.
// It gives EXACT faces (a cylinder is 3 faces, not a faceted prism) and real edge and face topology.
// The origins of one contour's edges (the 9th number of each edge), without moving the pointer.
static std::vector<int> wire_src(const double* p) {
    size_t nedges = (size_t)(p[0]);
    std::vector<int> out;
    out.reserve(nedges);
    for (size_t i = 0; i < nedges; ++i) out.push_back((int)p[1 + i * 9 + 8]);
    return out;
}

static TopoDS_Wire build_exact_wire_src(const double*& p, TopTools_DataMapOfShapeInteger* esrc) {
    size_t nedges = (size_t)(*p++);
    BRepBuilderAPI_MakeWire mkw;
    std::vector<int> srcs; // the origins, in the order the edges are added
    srcs.reserve(nedges);
    for (size_t i = 0; i < nedges; ++i) {
        double kind = p[0], ax = p[1], ay = p[2], bx = p[3], by = p[4], cx = p[5], cy = p[6], ccw = p[7];
        int src = (int)p[8]; // the edge's ORIGIN — the Id of the sketch entity (0 = unknown)
        p += 9;
        TopoDS_Edge e;
        if (kind == 2.0) { // a full circle
            gp_Circ circ(gp_Ax2(gp_Pnt(cx, cy, 0.0), gp_Dir(0.0, 0.0, 1.0)), ax);
            e = BRepBuilderAPI_MakeEdge(circ).Edge();
        } else if (kind == 1.0) { // an arc from a to b about the centre c
            double r = std::hypot(ax - cx, ay - cy);
            gp_Dir n(0.0, 0.0, ccw > 0.5 ? 1.0 : -1.0);
            gp_Circ circ(gp_Ax2(gp_Pnt(cx, cy, 0.0), n), r);
            e = BRepBuilderAPI_MakeEdge(circ, gp_Pnt(ax, ay, 0.0), gp_Pnt(bx, by, 0.0)).Edge();
        } else { // a segment
            e = BRepBuilderAPI_MakeEdge(gp_Pnt(ax, ay, 0.0), gp_Pnt(bx, by, 0.0)).Edge();
        }
        if (!e.IsNull()) {
            mkw.Add(e);
            srcs.push_back(src);
        }
    }
    if (!mkw.IsDone()) return TopoDS_Wire();
    TopoDS_Wire w = mkw.Wire();
    // THE ORIGINS ARE BOUND AFTER THE WIRE IS ASSEMBLED: the builder recreates the edges (orientation,
    // position) and the binding to the original objects is lost — before this exactly one edge in four
    // made it through. `BRepTools_WireExplorer` walks IN CONTOUR ORDER, which is the order they were added
    // in.
    if (esrc) {
        size_t i = 0;
        for (BRepTools_WireExplorer ex(w); ex.More() && i < srcs.size(); ex.Next(), ++i) {
            if (srcs[i] != 0 && !esrc->IsBound(ex.Current())) esrc->Bind(ex.Current(), srcs[i]);
        }
    }
    return w;
}

static TopoDS_Wire build_exact_wire(const double*& p) {
    return build_exact_wire_src(p, nullptr);
}

// A face from exact contours: the outer one plus the holes (at Z=0).
// The signed area of a loop in the XY plane: the sign gives the direction of travel (positive is
// counter-clockwise). It is computed from POINTS ALONG the edges (arcs and circles are sampled, otherwise
// a round hole would come out as zero), in the order the wire is walked and honouring each edge's
// orientation — which is exactly what `BRepTools_WireExplorer` gives.
static double wire_area_xy(const TopoDS_Wire& w) {
    std::vector<gp_Pnt2d> pts;
    for (BRepTools_WireExplorer ex(w); ex.More(); ex.Next()) {
        const TopoDS_Edge& e = ex.Current();
        double f = 0.0, l = 0.0;
        Handle(Geom_Curve) c = BRep_Tool::Curve(e, f, l);
        if (c.IsNull()) continue;
        const bool rev = (e.Orientation() == TopAbs_REVERSED);
        const int N = 12;
        for (int i = 0; i < N; ++i) {
            const double s = (double)i / (double)N;
            const double t = rev ? (l - (l - f) * s) : (f + (l - f) * s);
            gp_Pnt q = c->Value(t);
            pts.push_back(gp_Pnt2d(q.X(), q.Y()));
        }
    }
    if (pts.size() < 3) return 0.0;
    double a = 0.0;
    for (size_t i = 0; i < pts.size(); ++i) {
        const gp_Pnt2d& u = pts[i];
        const gp_Pnt2d& v = pts[(i + 1) % pts.size()];
        a += u.X() * v.Y() - v.X() * u.Y();
    }
    return 0.5 * a;
}

// A face from exact loops. THE ORIENTATION IS COMPUTED FROM THE GEOMETRY rather than assumed from a
// convention that "the first loop is wound like the rest": OCCT does not check the direction itself and
// silently builds a broken face if a hole runs the same way as the outer loop — the areas then ADD instead
// of subtracting.
//
// Reported behaviour: selecting an outer contour, a nested one and a new pad gave 48835 mm^2 instead of
// 8948 and a face with valid=false, rather than a body with a cut-out. The cause: the outer loop's exact
// edges ran CLOCKWISE while the hole's ran COUNTER-CLOCKWISE (even though the points of both contours
// counted as counter-clockwise). A blind `Reversed()` made the hole run the same way as the outer loop.
static TopoDS_Face build_exact_face_src(const double* data, size_t n, TopTools_DataMapOfShapeInteger* esrc) {
    if (n < 2) return TopoDS_Face();
    const double* p = data;
    size_t L = (size_t)(*p++);
    if (L < 1) return TopoDS_Face();
    TopoDS_Wire outer = build_exact_wire_src(p, esrc);
    if (outer.IsNull()) return TopoDS_Face();
    if (wire_area_xy(outer) < 0.0) outer = TopoDS::Wire(outer.Reversed()); // the outer loop runs counter-clockwise
    BRepBuilderAPI_MakeFace mkface(outer, Standard_True);
    for (size_t i = 1; i < L; ++i) {
        TopoDS_Wire hole = build_exact_wire_src(p, esrc);
        if (hole.IsNull()) continue;
        const double a = wire_area_xy(hole);
        // a hole runs CLOCKWISE, against the outer loop. Zero (a degenerate loop) keeps the old behaviour.
        mkface.Add(a > 0.0 ? TopoDS::Wire(hole.Reversed()) : hole);
    }
    return mkface.IsDone() ? mkface.Face() : TopoDS_Face();
}

static TopoDS_Face build_exact_face(const double* data, size_t n) {
    return build_exact_face_src(data, n, nullptr);
}

// A boolean over two extruded profiles: op 0 = cut (base minus tool), 1 = union, 2 = common.
extern "C" QymDoc* qym_occt_extrude_bool(const double* base_xy, size_t nb, double base_h,
                              const double* tool_xy, size_t nt, double tool_h,
                              int op, double defl) {
    try {
        if (nb < 3 || nt < 3) return nullptr;
        TopoDS_Shape base = extrude_shape(base_xy, nb, base_h);
        TopoDS_Shape tool = extrude_shape(tool_xy, nt, tool_h);
        if (base.IsNull() || tool.IsNull()) return nullptr;
        TopoDS_Shape res;
        if (op == 0) res = BRepAlgoAPI_Cut(base, tool).Shape();
        else if (op == 1) res = BRepAlgoAPI_Fuse(base, tool).Shape();
        else res = BRepAlgoAPI_Common(base, tool).Shape();
        if (res.IsNull()) return nullptr;
        return doc_from_shape(res, defl);
    } catch (...) {
        return nullptr; // an exception crossing the C ABI aborts the process, so an honest refusal goes back instead
    }
}

extern "C" size_t qym_doc_body_count(const QymDoc* d) { return d ? d->bodies.size() : 0; }

extern "C" size_t qym_body_vert_count(const QymDoc* d, size_t i) { return (d && i < d->bodies.size()) ? d->bodies[i].verts.size() / 3 : 0; }
extern "C" size_t qym_body_tri_count(const QymDoc* d, size_t i) { return (d && i < d->bodies.size()) ? d->bodies[i].tris.size() / 3 : 0; }
extern "C" size_t qym_body_face_count(const QymDoc* d, size_t i) { return (d && i < d->bodies.size()) ? d->bodies[i].fstart.size() : 0; }

extern "C" void qym_body_copy_verts(const QymDoc* d, size_t i, float* out) {
    if (d && i < d->bodies.size() && !d->bodies[i].verts.empty()) std::memcpy(out, d->bodies[i].verts.data(), d->bodies[i].verts.size() * sizeof(float));
}
extern "C" void qym_body_copy_tris(const QymDoc* d, size_t i, uint32_t* out) {
    if (d && i < d->bodies.size() && !d->bodies[i].tris.empty()) std::memcpy(out, d->bodies[i].tris.data(), d->bodies[i].tris.size() * sizeof(uint32_t));
}
extern "C" void qym_body_copy_face_starts(const QymDoc* d, size_t i, uint32_t* out) {
    if (d && i < d->bodies.size() && !d->bodies[i].fstart.empty()) std::memcpy(out, d->bodies[i].fstart.data(), d->bodies[i].fstart.size() * sizeof(uint32_t));
}
extern "C" void qym_body_copy_face_counts(const QymDoc* d, size_t i, uint32_t* out) {
    if (d && i < d->bodies.size() && !d->bodies[i].fcount.empty()) std::memcpy(out, d->bodies[i].fcount.data(), d->bodies[i].fcount.size() * sizeof(uint32_t));
}
// The body's persistent face ids, parallel to `face_starts` (0 = unknown).
extern "C" void qym_body_copy_face_ids(const QymDoc* d, size_t i, uint32_t* out) {
    if (d && i < d->bodies.size() && !d->bodies[i].fid.empty()) std::memcpy(out, d->bodies[i].fid.data(), d->bodies[i].fid.size() * sizeof(uint32_t));
}
// The faces' EXACT anchors (7 doubles per face: px, py, pz, nx, ny, nz, is_plane), parallel to `face_starts`.
extern "C" void qym_body_copy_face_anchors(const QymDoc* d, size_t i, double* out) {
    if (d && i < d->bodies.size() && !d->bodies[i].fanchor.empty()) std::memcpy(out, d->bodies[i].fanchor.data(), d->bodies[i].fanchor.size() * sizeof(double));
}

extern "C" void qym_doc_free(QymDoc* d) { delete d; }

// ---- Live B-rep shape handles (for a general boolean over any bodies) ----

static TopoDS_Shape revolve_shape(const double* xy, size_t n, int axis, double angle_deg) {
    BRepBuilderAPI_MakePolygon poly;
    for (size_t i = 0; i < n; ++i) poly.Add(gp_Pnt(xy[2 * i], xy[2 * i + 1], 0.0));
    poly.Close();
    if (!poly.IsDone()) return TopoDS_Shape();
    BRepBuilderAPI_MakeFace mkface(poly.Wire(), Standard_True);
    if (!mkface.IsDone()) return TopoDS_Shape();
    gp_Dir dir = (axis == 0) ? gp_Dir(1, 0, 0) : gp_Dir(0, 1, 0);
    gp_Ax1 ax(gp_Pnt(0, 0, 0), dir);
    return BRepPrimAPI_MakeRevol(mkface.Face(), ax, angle_deg * 3.14159265358979323846 / 180.0).Shape();
}

// A TEMPORARY MEASUREMENT (behind the QYM_CLASSIFY environment variable): where the names went.
//
// Three questions are put to the history, and all three are required: `Modified`, `Generated` and "this is
// the very same shape". Without the third a non-existent category "from nobody" appears — one attempt has
// already been burnt on it.
// WAS THE SOURCE ITSELF NAMED — the second required question to every answer. Without it "Modified" only
// means "the face came from somewhere", and that answer cannot tell AN INHERITED DEBT (the source is
// nameless too, so the fix belongs higher up the timeline) from A LOSS OF OUR OWN (the source had a name
// and the operation failed to carry it). The remedies differ, while the answer used to be one.
void classify_unnamed(BRepBuilderAPI_MakeShape& algo, const TopoDS_Shape& src, const QymShape* q, const char* tag,
                             const TopTools_DataMapOfShapeInteger* src_ids) {
    if (!std::getenv("QYM_CLASSIFY")) return;
    int same = 0, mod = 0, gen = 0, edge = 0, vert = 0, none = 0, lost = 0;
    auto named_src = [&](const TopoDS_Shape& f) {
        return src_ids && src_ids->IsBound(f) && (src_ids->Find(f) & QYM_NAMED);
    };
    for (TopExp_Explorer ex(q->shape, TopAbs_FACE); ex.More(); ex.Next()) {
        if (q->fids.IsBound(ex.Current()) && (q->fids.Find(ex.Current()) & QYM_NAMED)) continue;
        bool is_same = false, is_mod = false, is_gen = false, src_was_named = false;
        for (TopExp_Explorer s2(src, TopAbs_FACE); s2.More(); s2.Next()) {
            if (s2.Current().IsSame(ex.Current())) { is_same = true; src_was_named = named_src(s2.Current()); break; }
            for (TopTools_ListIteratorOfListOfShape it(algo.Modified(s2.Current())); it.More(); it.Next())
                if (it.Value().IsSame(ex.Current())) { is_mod = true; src_was_named = named_src(s2.Current()); break; }
            for (TopTools_ListIteratorOfListOfShape it(algo.Generated(s2.Current())); it.More(); it.Next())
                if (it.Value().IsSame(ex.Current())) { is_gen = true; src_was_named = named_src(s2.Current()); break; }
            if (is_mod || is_gen) break;
        }
        // A FACE CAN BE BORN FROM MORE THAN A FACE. In a fillet the surface is generated by AN EDGE, and
        // the corner patch by A VERTEX. Asking about faces alone brings back the non-existent category
        // "from nobody" — the same mistake that `IsSame` already caused.
        bool from_edge = false, from_vert = false;
        if (!is_same && !is_gen && !is_mod) {
            for (TopExp_Explorer s2(src, TopAbs_EDGE); s2.More() && !from_edge; s2.Next())
                for (TopTools_ListIteratorOfListOfShape it(algo.Generated(s2.Current())); it.More(); it.Next())
                    if (it.Value().IsSame(ex.Current())) { from_edge = true; break; }
            for (TopExp_Explorer s2(src, TopAbs_VERTEX); s2.More() && !from_vert && !from_edge; s2.Next())
                for (TopTools_ListIteratorOfListOfShape it(algo.Generated(s2.Current())); it.More(); it.Next())
                    if (it.Value().IsSame(ex.Current())) { from_vert = true; break; }
        }
        if (is_same) same++; else if (is_gen) gen++; else if (is_mod) mod++;
        else if (from_edge) edge++; else if (from_vert) vert++; else none++;
        if (src_was_named) lost++; // the source WAS named while the result is nameless: the loss is ours
    }
    if (same + mod + gen + edge + vert + none)
        fprintf(stderr, "CLASSIFIER %s: nameless — same shape %d, from a face (Gen) %d, Modified %d, FROM AN EDGE %d, FROM A VERTEX %d, from nobody %d; OF THOSE THE SOURCE WAS NAMED %d\n",
                tag, same, gen, mod, edge, vert, none, lost);
}


// --- THE TOPOLOGICAL NAMES OF A PRISM (a name from THE ORIGIN, not from the traversal order) ------
//
// `seed_ids` hands out ids in TopExp order: the walls, then the caps. Let the profile gain one more edge
// (a rectangle became L-shaped) and there are more walls — so THE CAPS CHANGE NUMBER. And the top face is
// usually where the sketches, holes and chamfers sit: the reference silently moves onto a neighbouring
// face. Measured on a bare case: there were 6 faces (bottom 5, top 6), then 8 — bottom 7, top 8.
//
// A name has to follow from THE RECIPE:
//   * the bottom -> 1 (the profile face it started from)
//   * the top    -> 2 (that face translated)
//   * a wall     -> 10 + k, where k is the number of THE PROFILE EDGE that generated it (`Generated`)
// Then the number of walls affects nothing: the caps are always 1 and 2, and a wall lives as long as its
// edge does. Edges are named after the faces they belong to: the pair (smaller face id, larger) gives a
// stable key.
// A REGION'S KEY is the smallest wall name among its profile's edges. The profiles merge into one flat
// face before the extrusion, so "profile number k" does not exist after the merge; the region's set of
// edges does exist, though, and the smallest name in it is a stable mark of the region itself.
static unsigned region_key(const TopoDS_Shape& profile, const TopTools_DataMapOfShapeInteger* esrc) {
    unsigned best = 0;
    if (!esrc) return 0;
    for (TopExp_Explorer ex(profile, TopAbs_EDGE); ex.More(); ex.Next()) {
        if (!esrc->IsBound(ex.Current())) continue;
        unsigned v = (unsigned)esrc->Find(ex.Current());
        if (v != 0 && (best == 0 || v < best)) best = v;
    }
    return best;
}

// The cap names for the region with key `key`: `caps` holds triples of [key, bottom, top].
static void caps_for(unsigned key, const unsigned* caps, size_t ncaps, unsigned& c0, unsigned& c1) {
    c0 = c1 = 0;
    if (!caps || ncaps < 3) return;
    if (ncaps == 3 && caps[0] == 0) { // a single region needs no key (see `region_cap_names`)
        c0 = caps[1];
        c1 = caps[2];
        return;
    }
    for (size_t i = 0; i + 2 < ncaps + 1 && (i + 1) * 3 <= ncaps; ++i) {
        if (caps[i * 3] == key) {
            c0 = caps[i * 3 + 1];
            c1 = caps[i * 3 + 2];
            return;
        }
    }
}

// SEEDING FROM THE RECIPE for any "profile to body" operation (extrude, revolve — they share the OCCT
// base class BRepPrimAPI_MakeSweep): the caps take the names handed in, and a side face takes the name of
// its own profile edge. One mechanism for all such operations: the name is derived from the recipe rather
// than from the traversal order.
static QymShape* seeded_sweep(BRepPrimAPI_MakeSweep& mk, const TopoDS_Face& profile, const TopTools_DataMapOfShapeInteger* esrc, unsigned cap0, unsigned cap1) {
    TopoDS_Shape res = mk.Shape();
    if (res.IsNull()) return nullptr;
    QymShape* q = new QymShape{res, {}, {}, {}, {}};
    auto bind_face = [&](const TopoDS_Shape& sh, int id) {
        if (!sh.IsNull() && sh.ShapeType() == TopAbs_FACE && !q->fids.IsBound(sh)) q->fids.Bind(sh, id);
    };
    // THE CAP NAMES COME FROM ABOVE (descriptors from the document's name table). The kernel used to call
    // them 1 and 2 — the same on EVERY body — so in a boolean the tool's cap matched the base's cap by
    // name, and the reference had to be rescued by renumbering. 0 means no name was given (the profile did
    // not come from a sketch), and the old local 1 and 2 remain.
    bind_face(mk.FirstShape(), cap0 != 0 ? (int)cap0 : 1); // the bottom is the profile itself
    bind_face(mk.LastShape(), cap1 != 0 ? (int)cap1 : 2);  // the top is that profile translated
    // A WALL'S NAME IS A DESCRIPTOR from the profile's encoding ("the wall of feature F from sketch entity
    // E"), otherwise the edge's ordinal. An ordinal shifts as soon as an edge is inserted INTO THE MIDDLE
    // of the contour, and every wall after it changes name. A name taken from the sketch entity does not do
    // that: a line stays itself however many neighbours are added to it.
    int k = 0, no_src = 0;
    for (TopExp_Explorer ex(profile, TopAbs_EDGE); ex.More(); ex.Next(), ++k) {
        int src = (esrc && esrc->IsBound(ex.Current())) ? esrc->Find(ex.Current()) : 0;
        if (src == 0) no_src++;
        const int name = (src != 0) ? src : 10 + k;
        const TopTools_ListOfShape& gen = mk.Generated(ex.Current());
        for (TopTools_ListIteratorOfListOfShape it(gen); it.More(); it.Next()) bind_face(it.Value(), name);
    }
    // anything left unnamed for whatever reason (degenerate cases) gets fresh numbers AFTER the walls
    int next = 10 + k + 1;
    for (TopExp_Explorer ex(res, TopAbs_FACE); ex.More(); ex.Next())
        if (!q->fids.IsBound(ex.Current())) q->fids.Bind(ex.Current(), next++);
    classify_unnamed(mk, profile, q, "sweep or revolve");
    // EDGES keep the old scheme (traversal order, plus transfer through the operations' history and repair
    // by geometric snapshots). Names built from a pair of faces were tried, and on a real document that
    // broke 19 chamfers: some features have no edge snapshots (the file predates the mechanism), and the
    // reference is found neither by name nor by snapshot. Changing the EDGE naming scheme needs a migration
    // of its own and is done separately from the faces.
    seed_ids(res, TopAbs_EDGE, q->eids);
    return q;
}

// The former name for an extrusion — the same mechanism.
static QymShape* seeded_prism(BRepPrimAPI_MakePrism& mk, const TopoDS_Face& profile, const TopTools_DataMapOfShapeInteger* esrc = nullptr, unsigned cap0 = 0, unsigned cap1 = 0) {
    return seeded_sweep(mk, profile, esrc, cap0, cap1);
}

// A PRIMITIVE FROM THE RECIPE: on a cylinder, cone, sphere or torus the roles of the faces are canonical
// and derived from THE GEOMETRY ITSELF rather than from the traversal order — the planar faces are the
// ends (lower by z, upper) and the curved ones are the side surface. A primitive's topology does not change
// with its parameters, so positional numbers did not drift here; but they were not UNIQUE, and a primitive
// used as a boolean tool lost its names entirely.
static QymShape* seeded_primitive(const TopoDS_Shape& sh, unsigned cap0, unsigned cap1, unsigned side) {
    if (sh.IsNull()) return nullptr;
    QymShape* q = new QymShape{sh, {}, {}, {}, {}};
    std::vector<std::pair<double, TopoDS_Shape>> planar; // (the centre's z, the face)
    std::vector<TopoDS_Shape> curved;
    for (TopExp_Explorer ex(sh, TopAbs_FACE); ex.More(); ex.Next()) {
        const TopoDS_Face& f = TopoDS::Face(ex.Current());
        GProp_GProps gp;
        BRepGProp::SurfaceProperties(f, gp);
        BRepAdaptor_Surface ad(f);
        if (ad.GetType() == GeomAbs_Plane) planar.emplace_back(gp.CentreOfMass().Z(), f);
        else curved.push_back(f);
    }
    std::sort(planar.begin(), planar.end(), [](const auto& a, const auto& b) { return a.first < b.first; });
    auto bind = [&](const TopoDS_Shape& f, unsigned name) {
        if (name != 0 && !q->fids.IsBound(f)) q->fids.Bind(f, (int)name);
    };
    if (!planar.empty()) bind(planar.front().second, cap0);
    if (planar.size() > 1) bind(planar.back().second, cap1);
    for (const auto& f : curved) bind(f, side);
    int next = next_local(q->fids);
    fill_unnamed(sh, TopAbs_FACE, q->fids, next);
    seed_ids(sh, TopAbs_EDGE, q->eids);
    return q;
}

// A new BASE body: seed ids onto the faces and edges (1..n in TopExp order) — stable for one recipe.
QymShape* seeded(const TopoDS_Shape& s) {
    if (s.IsNull()) return nullptr;
    QymShape* q = new QymShape{s, {}, {}, {}, {}};
    seed_ids(s, TopAbs_FACE, q->fids);
    seed_ids(s, TopAbs_EDGE, q->eids);
    return q;
}
// Carry the ids one-to-one in sub-shape order for type `ty` (the operation preserves topology: a transform or a mirror).
void copy_ids_by_order(const TopoDS_Shape& src, TopAbs_ShapeEnum ty, const TopTools_DataMapOfShapeInteger& sid, const TopoDS_Shape& dst, TopTools_DataMapOfShapeInteger& out) {
    TopExp_Explorer es(src, ty), ed(dst, ty);
    for (; es.More() && ed.More(); es.Next(), ed.Next()) {
        if (sid.IsBound(es.Current())) out.Bind(ed.Current(), sid.Find(es.Current()));
    }
}

extern "C" QymShape* qym_shape_extrude(const double* xy, size_t n, double h) {
    if (n < 3) return nullptr;
    try {
        return seeded(extrude_shape(xy, n, h));
    } catch (...) {
        return nullptr;
    }
}
extern "C" QymShape* qym_shape_revolve(const double* xy, size_t n, int axis, double angle_deg) {
    if (n < 3) return nullptr;
    try {
        return seeded(revolve_shape(xy, n, axis, angle_deg));
    } catch (...) {
        return nullptr;
    }
}
// Extrude an EXACT profile (see `build_exact_face`) to the height h along +Z, giving a body with exact faces.
extern "C" QymShape* qym_shape_extrude_profile(const double* data, size_t n, double h, unsigned cap0, unsigned cap1) {
    try {
        TopTools_DataMapOfShapeInteger esrc;
        TopoDS_Face f = build_exact_face_src(data, n, &esrc);
        if (f.IsNull()) return nullptr;
        BRepPrimAPI_MakePrism mk(f, gp_Vec(0.0, 0.0, h));
        return seeded_prism(mk, f, &esrc, cap0, cap1); // the cap and wall names come from the feature's recipe
    } catch (...) {
        return nullptr;
    }
}
// N EXACT profiles: their FLAT faces are merged by a 2D boolean (Fuse plus UnifySameDomain at Z=0 are
// dependable, unlike the solid versions), and then the ALREADY SINGLE face is extruded. Touching profiles
// give one body WITHOUT seam edges (before: N prisms plus N booleans gave doubled seam edges along the line
// of contact, which broke chamfers and fillets).
// `data` is the concatenation of `build_exact_face` encodings; `offsets[nprof+1]` holds the profile
// boundaries within `data`.
extern "C" QymShape* qym_shape_extrude_profiles_fused(const double* data, const size_t* offsets, size_t nprof, double h, const unsigned* caps, size_t ncaps) {
    try {
        if (nprof == 0) return nullptr;
        TopoDS_Shape merged;
        TopTools_DataMapOfShapeInteger esrc; // the origin of a profile edge gives the side face's name
        for (size_t i = 0; i < nprof; ++i) {
            TopoDS_Face f = build_exact_face_src(data + offsets[i], offsets[i + 1] - offsets[i], &esrc);
            if (f.IsNull()) return nullptr;
            if (merged.IsNull()) merged = f;
            else {
                merged = BRepAlgoAPI_Fuse(merged, f).Shape();
                if (merged.IsNull()) return nullptr;
            }
        }
        // Merge the coplanar faces and collinear edges of the plane, so the contact disappears from the topology (the monolith).
        //
        // Unify RECREATES the edges and would break the "edge to sketch entity" binding by which the side
        // faces are named. Simply not calling it for a single profile was tried — the body kept its seams
        // and fillets began seeing two edges instead of one (on a real part four chamfers stopped being
        // taken). So it is called as before, and the binding is CARRIED THROUGH ITS HISTORY.
        try {
            ShapeUpgrade_UnifySameDomain uni(merged, Standard_True, Standard_True, Standard_False);
            uni.Build();
            if (!uni.Shape().IsNull()) {
                Handle(BRepTools_History) hist = uni.History();
                if (!hist.IsNull()) {
                    TopTools_DataMapOfShapeInteger moved;
                    for (TopExp_Explorer ex(merged, TopAbs_EDGE); ex.More(); ex.Next()) {
                        const TopoDS_Shape& e = ex.Current();
                        if (!esrc.IsBound(e)) continue;
                        const int src = esrc.Find(e);
                        const TopTools_ListOfShape& mod = hist->Modified(e);
                        if (mod.IsEmpty()) {
                            if (!hist->IsRemoved(e) && !moved.IsBound(e)) moved.Bind(e, src);
                        } else {
                            for (TopTools_ListIteratorOfListOfShape it(mod); it.More(); it.Next())
                                if (!moved.IsBound(it.Value())) moved.Bind(it.Value(), src);
                        }
                    }
                    esrc = moved;
                }
                merged = uni.Shape();
            }
        } catch (...) {}
        // extrude every resulting face; a single solid goes back as it is, disconnected ones as a compound
        std::vector<TopoDS_Shape> prisms;
        std::vector<TopoDS_Face> profiles;
        for (TopExp_Explorer ex(merged, TopAbs_FACE); ex.More(); ex.Next()) {
            TopoDS_Face pf = TopoDS::Face(ex.Current());
            BRepPrimAPI_MakePrism mk(pf, gp_Vec(0.0, 0.0, h));
            TopoDS_Shape pr = mk.Shape();
            if (!pr.IsNull()) {
                prisms.push_back(pr);
                profiles.push_back(pf);
            }
        }
        if (prisms.empty()) return nullptr;
        if (prisms.size() == 1) {
            // ONE body is the ordinary case: the names come from the recipe (caps 1 and 2, a wall from its
            // own profile edge) rather than from the traversal order. Otherwise one extra edge in the profile
            // shifts the cap numbers, and everything built on them (sketches, holes, chamfers) moves onto a
            // neighbouring face.
            unsigned c0 = 0, c1 = 0;
            caps_for(region_key(profiles[0], &esrc), caps, ncaps, c0, c1);
            BRepPrimAPI_MakePrism mk(profiles[0], gp_Vec(0.0, 0.0, h));
            return seeded_prism(mk, profiles[0], &esrc, c0, c1);
        }
        // SEVERAL REGIONS: each is named by ITS OWN recipe (the caps by the region's key, the walls by their
        // own edges). A positional seeding of the whole compound used to stand here, and any part extruded
        // from several contours stayed entirely on the old naming scheme.
        // THE PRISM IS BUILT ONCE, AND THE NAMES LAND STRAIGHT ONTO IT.
        //
        // The prism used to be built TWICE here: one instance went into the compound, the other into
        // `seeded_prism` for the names, and the names were then carried across BY PARALLEL TRAVERSAL, on
        // the assumption of "the same topology, the same order". The order does not have to match, and
        // whatever did not match stayed nameless. Measured on a real file: the cut body had 34 positional
        // faces out of 64, and six fillet references could not be found by name because of it.
        //
        // Building once leaves nothing to carry.
        BRep_Builder bb;
        TopoDS_Compound comp;
        bb.MakeCompound(comp);
        QymShape* q = new QymShape{TopoDS_Shape(), {}, {}, {}, {}};
        for (size_t i = 0; i < profiles.size(); ++i) {
            unsigned c0 = 0, c1 = 0;
            caps_for(region_key(profiles[i], &esrc), caps, ncaps, c0, c1);
            BRepPrimAPI_MakePrism mk(profiles[i], gp_Vec(0.0, 0.0, h));
            QymShape* one = seeded_prism(mk, profiles[i], &esrc, c0, c1);
            if (!one) {
                bb.Add(comp, prisms[i]); // the recipe did not work out: it goes in as it is, and the fill-in supplies the names
                continue;
            }
            bb.Add(comp, one->shape);
            for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(one->fids); it.More(); it.Next())
                if (!q->fids.IsBound(it.Key())) q->fids.Bind(it.Key(), it.Value());
            for (TopTools_DataMapIteratorOfDataMapOfShapeInteger it(one->eids); it.More(); it.Next())
                if (!q->eids.IsBound(it.Key())) q->eids.Bind(it.Key(), it.Value());
            delete one;
        }
        q->shape = comp;
        int nf = next_local(q->fids);
        fill_unnamed(comp, TopAbs_FACE, q->fids, nf);
        int ne = next_local(q->eids);
        fill_unnamed(comp, TopAbs_EDGE, q->eids, ne);
        return q;
    } catch (...) {
        return nullptr;
    }
}

// --- EXACT primitives (OCCT's own constructors give exact topology) ----------------------------
// A cylinder: axis Z, base at z=0, centred at the origin. Exact, with 3 faces.
extern "C" QymShape* qym_shape_cylinder(double r, double h, unsigned cap0, unsigned cap1, unsigned side) {
    try {
        return seeded_primitive(BRepPrimAPI_MakeCylinder(r, h).Shape(), cap0, cap1, side);
    } catch (...) {
        return nullptr;
    }
}
// A sphere centred at the origin. Exact, with 1 face.
extern "C" QymShape* qym_shape_sphere(double r, unsigned cap0, unsigned cap1, unsigned side) {
    try {
        return seeded_primitive(BRepPrimAPI_MakeSphere(r).Shape(), cap0, cap1, side);
    } catch (...) {
        return nullptr;
    }
}
// A cone: base r1 at z=0, top r2 at z=h, axis Z.
extern "C" QymShape* qym_shape_cone(double r1, double r2, double h, unsigned cap0, unsigned cap1, unsigned side) {
    try {
        return seeded_primitive(BRepPrimAPI_MakeCone(r1, r2, h).Shape(), cap0, cap1, side);
    } catch (...) {
        return nullptr;
    }
}
// A torus in the XY plane, axis Z, centred at the origin.
extern "C" QymShape* qym_shape_torus(double major, double minor, unsigned cap0, unsigned cap1, unsigned side) {
    try {
        return seeded_primitive(BRepPrimAPI_MakeTorus(major, minor).Shape(), cap0, cap1, side);
    } catch (...) {
        return nullptr;
    }
}

// Revolve an EXACT profile about an axis (0 = X, 1 = Y) through an angle, giving a body.
extern "C" QymShape* qym_shape_revolve_profile(const double* data, size_t n, int axis, double angle_deg) {
    try {
        TopoDS_Face f = build_exact_face(data, n);
        if (f.IsNull()) return nullptr;
        gp_Dir dir = (axis == 0) ? gp_Dir(1.0, 0.0, 0.0) : gp_Dir(0.0, 1.0, 0.0);
        gp_Ax1 ax(gp_Pnt(0.0, 0.0, 0.0), dir);
        double ang = (std::abs(angle_deg) < 1e-6 ? 360.0 : angle_deg) * 3.14159265358979323846 / 180.0;
        return seeded(BRepPrimAPI_MakeRevol(f, ax, ang).Shape());
    } catch (...) {
        return nullptr;
    }
}
// Revolving a profile about an ARBITRARY axis (origin and direction in the sketch's LOCAL frame — the axis
// has to lie in the profile's plane). The application resolves a datum axis from world to local and passes
// it in here.
extern "C" QymShape* qym_shape_revolve_profile_axis(const double* data, size_t n, const double* origin, const double* dir, double angle_deg, unsigned cap0, unsigned cap1) {
    try {
        TopTools_DataMapOfShapeInteger esrc;
        TopoDS_Face f = build_exact_face_src(data, n, &esrc);
        if (f.IsNull() || !origin || !dir) return nullptr;
        gp_Dir d(dir[0], dir[1], dir[2]);
        gp_Ax1 ax(gp_Pnt(origin[0], origin[1], origin[2]), d);
        double ang = (std::abs(angle_deg) < 1e-6 ? 360.0 : angle_deg) * 3.14159265358979323846 / 180.0;
        BRepPrimAPI_MakeRevol mk(f, ax, ang);
        return seeded_sweep(mk, f, &esrc, cap0, cap1);
    } catch (...) {
        return nullptr;
    }
}
// Apply a 3x4 row-major placement (the X, Y and N axes plus the origin) to a raw shape; nullptr is the identity.
static TopoDS_Shape apply_place(const TopoDS_Shape& s, const double* m) {
    if (!m) return s;
    gp_Trsf t;
    t.SetValues(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]);
    return BRepBuilderAPI_Transform(s, t, Standard_True).Shape();
}

// A SWEEP. The profile (exact, from `build_exact_face`, holes included) is swept along the path — ONE wire
// (exact, from `build_exact_wire` with L=1). `path_tf` is the 3x4 placement of the path's sketch into world
// space (nullptr is the identity). `prof_tf` is NOT used for the orientation: the profile is MOVED
// AUTOMATICALLY to the start of the path and turned perpendicular to the tangent — the usual professional
// behaviour, where the profile may be drawn anywhere and the kernel puts it on the path, with the profile
// sketch's origin travelling along it. A profile that is a FACE makes `MakePipe` give a SOLID body, and the
// holes survive as cavities.
// Fresh persistent ids, because this is a new base body.
extern "C" QymShape* qym_shape_sweep(const double* prof, size_t np, const double* prof_tf,
                          const double* path, size_t npath, const double* path_tf,
                          unsigned cap0, unsigned cap1) {
    (void)prof_tf; // the profile's orientation is computed from the path (it turns automatically)
    try {
        TopTools_DataMapOfShapeInteger esrc; // a profile edge gives the name of the face it will sweep out
        TopoDS_Face f = build_exact_face_src(prof, np, &esrc);
        if (f.IsNull()) return why("sweep/profile", "the profile did not close into a face"), nullptr;
        if (npath < 2) return why("sweep/path", "the path is shorter than two numbers"), nullptr;
        const double* pp = path;
        size_t L = (size_t)(*pp++);
        if (L < 1) return why("sweep/path", "the path holds no loop at all"), nullptr;
        TopoDS_Wire spine = build_exact_wire(pp);
        if (spine.IsNull()) return why("sweep/path", "the path did not join into a wire"), nullptr;
        TopoDS_Wire spine_s = TopoDS::Wire(apply_place(spine, path_tf));
        if (spine_s.IsNull()) return why("sweep/path", "the path was lost when placed into the world"), nullptr;
        // the start of the path: the point P0 and the tangent T0, taken from the placed wire
        BRepAdaptor_CompCurve cc(spine_s);
        gp_Pnt P0; gp_Vec T;
        cc.D1(cc.FirstParameter(), P0, T);
        if (T.Magnitude() < 1e-9) return why("sweep/path", "the path has no direction at its start: the first segment is of zero length"), nullptr;
        gp_Dir Z(T);
        // a stable basis in the profile's plane: X perpendicular to Z (the world X if it is not parallel, otherwise Y)
        gp_Dir refX = (Abs(Z.Dot(gp_Dir(1, 0, 0))) > 0.9) ? gp_Dir(0, 1, 0) : gp_Dir(1, 0, 0);
        gp_Dir Y = Z.Crossed(refX);
        gp_Dir X = Y.Crossed(Z);
        // moving the profile (built in the world XY plane, normal +Z, sketch origin at 0) into the frame (P0, X, Y, Z)
        gp_Trsf place;
        place.SetDisplacement(gp_Ax3(gp::Origin(), gp::DZ(), gp::DX()), gp_Ax3(P0, Z, X));
        // THE ORIGINS TRAVEL WITH THE MOVE: the profile is repositioned at the start of the path and its
        // edges are RECREATED in the process — without carrying the binding there would be nothing to look
        // a face's name up by.
        BRepBuilderAPI_Transform tr(f, place, Standard_True);
        TopoDS_Shape prof_s = tr.Shape();
        if (prof_s.IsNull() || prof_s.ShapeType() != TopAbs_FACE) return why("sweep/profile", "the profile stopped being a face when moved to the start of the path"), nullptr;
        TopTools_DataMapOfShapeInteger esrc_s;
        for (TopExp_Explorer ex(f, TopAbs_EDGE); ex.More(); ex.Next()) {
            const TopoDS_Shape& e = ex.Current();
            if (!esrc.IsBound(e)) continue;
            const TopTools_ListOfShape& mod = tr.Modified(e);
            if (mod.IsEmpty()) {
                if (!esrc_s.IsBound(e)) esrc_s.Bind(e, esrc.Find(e));
            } else {
                for (TopTools_ListIteratorOfListOfShape it(mod); it.More(); it.Next())
                    if (!esrc_s.IsBound(it.Value())) esrc_s.Bind(it.Value(), esrc.Find(e));
            }
        }
        BRepOffsetAPI_MakePipe mk(spine_s, prof_s);
        mk.Build();
        if (!mk.IsDone()) return why("sweep/pipe", "the kernel could not run the profile along the path"), nullptr;
        if (mk.Shape().IsNull()) return why("sweep/pipe", "the kernel reported success and returned nothing"), nullptr;
        return seeded_sweep(mk, TopoDS::Face(prof_s), &esrc_s, cap0, cap1);
    } QYM_WHY_CATCH("sweep")
    return nullptr;
}
// A LOFT through sections. `data` is the concatenation of the sections' loop blocks (each read by
// `build_exact_wire`), `offsets[nsec+1]` holds where each section starts within `data`, and
// `places[nsec*12]` holds the 3x4 placements of the sections' planes (sketch frame into world). `ruled`
// gives ruled faces between the sections (otherwise a smooth B-spline surface). `solid` closes it into
// a solid (the section loops then have to be closed). `BRepOffsetAPI_ThruSections` runs the surface through
// all the sections in order. Fresh persistent ids, because this is a new base body.
extern "C" QymShape* qym_shape_loft(const double* data, size_t ndata, const size_t* offsets, size_t nsec,
                         const double* places, int ruled, int solid, unsigned cap0, unsigned cap1) {
    (void)ndata;
    try {
        if (nsec < 2) return why("loft/asked", "a loft needs two sections or more"), nullptr;
        if (!data || !offsets || !places) return why("loft/asked", "the sections, their offsets or their placements are missing"), nullptr;
        BRepOffsetAPI_ThruSections gen(solid ? Standard_True : Standard_False, ruled ? Standard_True : Standard_False, 1.0e-6);
        // A loft face is named after the edge of THE FIRST section that generated it (`GeneratedFace`): a
        // face between sections has no other "source" in the recipe, and an ordinal would drift with any edit.
        TopTools_DataMapOfShapeInteger esrc;
        TopoDS_Wire first_placed;
        for (size_t i = 0; i < nsec; ++i) {
            const double* p = data + offsets[i];
            TopoDS_Wire w = (i == 0) ? build_exact_wire_src(p, &esrc) : build_exact_wire(p);
            if (w.IsNull()) {
                char msg[96];
                snprintf(msg, sizeof(msg), "section %zu of %zu did not join into a closed loop", i + 1, nsec);
                return why("loft/section", msg), nullptr;
            }
            TopoDS_Shape placed = apply_place(w, places + i * 12);
            if (placed.IsNull() || placed.ShapeType() != TopAbs_WIRE) {
                char msg[96];
                snprintf(msg, sizeof(msg), "section %zu of %zu was lost when placed into the world", i + 1, nsec);
                return why("loft/section", msg), nullptr;
            }
            if (i == 0) {
                // the placement RECREATES the edges, so the binding is carried across by matching traversal
                // order (for one and the same wire that order is deterministic).
                TopTools_DataMapOfShapeInteger moved;
                TopExp_Explorer eo(w, TopAbs_EDGE), ep(placed, TopAbs_EDGE);
                for (; eo.More() && ep.More(); eo.Next(), ep.Next())
                    if (esrc.IsBound(eo.Current()) && !moved.IsBound(ep.Current())) moved.Bind(ep.Current(), esrc.Find(eo.Current()));
                esrc = moved;
                first_placed = TopoDS::Wire(placed);
            }
            gen.AddWire(TopoDS::Wire(placed));
        }
        gen.Build();
        if (!gen.IsDone()) return why("loft/build", "the kernel could not run a surface through these sections"), nullptr;
        TopoDS_Shape res = gen.Shape();
        if (res.IsNull()) return why("loft/build", "the kernel reported success and returned nothing"), nullptr;
        QymShape* q = new QymShape{res, {}, {}, {}, {}};
        auto bind_face = [&](const TopoDS_Shape& sh, int id) {
            if (!sh.IsNull() && sh.ShapeType() == TopAbs_FACE && !q->fids.IsBound(sh)) q->fids.Bind(sh, id);
        };
        if (cap0 != 0) bind_face(gen.FirstShape(), (int)cap0);
        if (cap1 != 0) bind_face(gen.LastShape(), (int)cap1);
        for (TopExp_Explorer ex(first_placed, TopAbs_EDGE); ex.More(); ex.Next()) {
            if (!esrc.IsBound(ex.Current())) continue;
            bind_face(gen.GeneratedFace(ex.Current()), esrc.Find(ex.Current()));
        }
        int next = next_local(q->fids);
        fill_unnamed(res, TopAbs_FACE, q->fids, next);
        seed_ids(res, TopAbs_EDGE, q->eids);
        return q;
    } QYM_WHY_CATCH("loft")
    return nullptr;
}
// SPLIT FACES: [the piece's id, the source face's name, the piece number]. The name "piece k of face N" is
// assembled on the Rust side; only the operation's facts live here.
// A SHELL OF 4x4 BEZIER PATCHES — the kernel taking in a subdivision cage.
//
// `pts` holds 48 doubles per patch: a 4x4 grid of control points, three coordinates each, row by row.
// `free_edges` (an output) says how many edges were left unsewn: zero means a closed shell without a single
// hole. That is the measurement this probe was written for — whether the kernel accepts our geometry and
// whether it closes on itself.
//
// `make_solid` asks for an attempt to turn a closed shell into a solid. If it fails, the shell comes back:
// a probe has to return what it got, not a refusal.
extern "C" QymShape* qym_shape_bezier_shell(const double* pts, size_t patches, double tol, int make_solid, unsigned* free_edges) {
    try {
        if (free_edges) *free_edges = 0;
        if (!pts || patches == 0) return nullptr;
        BRepBuilderAPI_Sewing sew(tol > 0.0 ? tol : 1.0e-6);
        for (size_t p = 0; p < patches; ++p) {
            TColgp_Array2OfPnt poles(1, 4, 1, 4);
            const double* q = pts + p * 48;
            for (int i = 1; i <= 4; ++i) {
                for (int j = 1; j <= 4; ++j) {
                    const double* v = q + ((i - 1) * 4 + (j - 1)) * 3;
                    poles.SetValue(i, j, gp_Pnt(v[0], v[1], v[2]));
                }
            }
            Handle(Geom_BezierSurface) surf = new Geom_BezierSurface(poles);
            BRepBuilderAPI_MakeFace mk(surf, Precision::Confusion());
            if (!mk.IsDone()) return nullptr;
            sew.Add(mk.Face());
        }
        sew.Perform();
        TopoDS_Shape shape = sew.SewedShape();
        if (shape.IsNull()) return nullptr;
        if (free_edges) *free_edges = (unsigned)sew.NbFreeEdges();

        if (make_solid && sew.NbFreeEdges() == 0) {
            TopExp_Explorer ex(shape, TopAbs_SHELL);
            if (ex.More()) {
                TopoDS_Shell sh = TopoDS::Shell(ex.Current());
                BRepBuilderAPI_MakeSolid mks(sh);
                if (mks.IsDone()) {
                    ShapeFix_Solid fix;
                    TopoDS_Shape solid = fix.SolidFromShell(sh);
                    if (!solid.IsNull()) shape = solid;
                    else shape = mks.Solid();
                }
            }
        }
        QymShape* out = new QymShape();
        out->shape = shape;
        seed_ids(out->shape, TopAbs_FACE, out->fids);
        seed_ids(out->shape, TopAbs_EDGE, out->eids);
        return out;
    } catch (...) {
        return nullptr;
    }
}

// A PIECE RECORD IS ONE-SHOT: ONCE NAMED, IT IS CLEARED.
//
// The model layer reads the records AFTER the operation, names the pieces and renames the faces. If a
// record survives that, THE NEXT operation folds it into its own group and re-elects the keeper of the name
// — the name moves onto a neighbour, and a reference written earlier leads to a different face. A
// measurement caught this in a live scenario: the body came out with 26 faces instead of 56, and pushing a
// face stopped building.
extern "C" void qym_shape_clear_face_splits(QymShape* s) {
    if (!s) return;
    s->fsplit_of.Clear();
    s->fsplit_idx.Clear();
}

extern "C" size_t qym_shape_face_splits(const QymShape* s, unsigned* out, size_t cap) {
    if (!s) return 0;
    size_t n = 0;
    for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
        const TopoDS_Shape& f = ex.Current();
        if (!s->fsplit_of.IsBound(f) || !s->fids.IsBound(f)) continue;
        if (out && (n + 1) * 3 <= cap) {
            out[n * 3 + 0] = (unsigned)s->fids.Find(f);
            out[n * 3 + 1] = (unsigned)s->fsplit_of.Find(f);
            out[n * 3 + 2] = (unsigned)(s->fsplit_idx.IsBound(f) ? s->fsplit_idx.Find(f) : 1);
        }
        ++n;
    }
    return n;
}

// Rewrite face names, given pairs of "was -> became".
extern "C" void qym_shape_rename_faces(QymShape* s, const unsigned* from, const unsigned* to, size_t n) {
    if (!s || !from || !to || n == 0) return;
    std::unordered_map<unsigned, unsigned> ren;
    for (size_t i = 0; i < n; ++i) ren[from[i]] = to[i];
    TopTools_DataMapOfShapeInteger out;
    for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
        if (!s->fids.IsBound(ex.Current())) continue;
        unsigned id = (unsigned)s->fids.Find(ex.Current());
        auto f = ren.find(id);
        out.Bind(ex.Current(), (int)(f == ren.end() ? id : f->second));
    }
    s->fids = out;
}

// AN EDGE'S NAME IS DERIVED FROM THE PAIR OF ITS FACES. The kernel only REPORTS the pairs: [the edge's id,
// face A's name, face B's name] for edges with exactly two adjacent faces. The name itself is assembled and
// interned on the Rust side, which alone knows the naming scheme. Calling with out=nullptr returns the size
// needed.
// WHAT TELLS TWO EDGES OF THE SAME FACE PAIR APART IS NOT A NUMBER BUT THEIR ENDS.
//
// An edge's name is the pair of its faces; when there are two such edges (a through slot gives an upper and
// a lower edge between the same walls), the rank between them used to be assigned by sorting on the kernel's
// number. That number changes with any edit higher up the timeline, and the twins swapped places SILENTLY:
// on a real part 12 edges out of 36 moved onto each other's places, and a fillet landed in the wrong spot.
//
// The stable mark comes from the recipe: WHICH NAMED FACES meet at the edge's ends, apart from its own two.
// For a slot's upper edge that is the cap, for the lower one the floor: different faces, different names,
// and none of it depends on numbering. The two SMALLEST such names are returned (0 when there are none), in
// rows of [the edge's id, name 1, name 2].
extern "C" size_t qym_shape_edge_end_faces(const QymShape* s, unsigned* out, size_t cap) {
    if (!s) return 0;
    TopTools_IndexedDataMapOfShapeListOfShape e2f, v2f, e2v;
    TopExp::MapShapesAndAncestors(s->shape, TopAbs_EDGE, TopAbs_FACE, e2f);
    TopExp::MapShapesAndAncestors(s->shape, TopAbs_VERTEX, TopAbs_FACE, v2f);
    size_t n = 0;
    for (int i = 1; i <= e2f.Extent(); ++i) {
        const TopoDS_Shape& e = e2f.FindKey(i);
        if (!s->eids.IsBound(e)) continue;
        const TopTools_ListOfShape& own = e2f.FindFromIndex(i);
        std::set<int> mine;
        for (TopTools_ListIteratorOfListOfShape it(own); it.More(); it.Next())
            if (s->fids.IsBound(it.Value())) mine.insert(s->fids.Find(it.Value()));
        std::set<unsigned> around;
        for (TopExp_Explorer vx(e, TopAbs_VERTEX); vx.More(); vx.Next()) {
            const int vi = v2f.FindIndex(vx.Current());
            if (vi < 1) continue;
            for (TopTools_ListIteratorOfListOfShape it(v2f.FindFromIndex(vi)); it.More(); it.Next()) {
                if (!s->fids.IsBound(it.Value())) continue;
                const int id = s->fids.Find(it.Value());
                if ((id & QYM_NAMED) == 0 || mine.count(id)) continue;   // positional ones do not count: they drift themselves
                around.insert((unsigned)id);
            }
        }
        if ((n + 1) * 3 <= cap && out) {
            out[n * 3 + 0] = (unsigned)s->eids.Find(e);
            auto it = around.begin();
            out[n * 3 + 1] = it == around.end() ? 0u : *it++;
            out[n * 3 + 2] = it == around.end() ? 0u : *it;
        }
        ++n;
    }
    return n;
}

extern "C" size_t qym_shape_edge_face_pairs(const QymShape* s, unsigned* out, size_t cap) {
    if (!s) return 0;
    TopTools_IndexedDataMapOfShapeListOfShape m;
    TopExp::MapShapesAndAncestors(s->shape, TopAbs_EDGE, TopAbs_FACE, m);
    size_t n = 0;
    for (int i = 1; i <= m.Extent(); ++i) {
        const TopoDS_Shape& e = m.FindKey(i);
        const TopTools_ListOfShape& fs = m.FindFromIndex(i);
        if (fs.Extent() != 2 || !s->eids.IsBound(e)) continue;
        TopTools_ListIteratorOfListOfShape it(fs);
        const TopoDS_Shape& f1 = it.Value();
        it.Next();
        const TopoDS_Shape& f2 = it.Value();
        if (!s->fids.IsBound(f1) || !s->fids.IsBound(f2)) continue;
        if (out && (n + 1) * 3 <= cap) {
            out[n * 3 + 0] = (unsigned)s->eids.Find(e);
            out[n * 3 + 1] = (unsigned)s->fids.Find(f1);
            out[n * 3 + 2] = (unsigned)s->fids.Find(f2);
        }
        ++n;
    }
    return n;
}

// Rewrite edge names, given pairs of "was -> became". Called right after the body is assembled, before
// anything refers to its edges.
extern "C" void qym_shape_rename_edges(QymShape* s, const unsigned* from, const unsigned* to, size_t n) {
    if (!s || !from || !to || n == 0) return;
    std::unordered_map<unsigned, unsigned> ren;
    for (size_t i = 0; i < n; ++i) ren[from[i]] = to[i];
    TopTools_DataMapOfShapeInteger out;
    for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next()) {
        if (!s->eids.IsBound(ex.Current())) continue;
        unsigned id = (unsigned)s->eids.Find(ex.Current());
        auto f = ren.find(id);
        out.Bind(ex.Current(), (int)(f == ren.end() ? id : f->second));
    }
    s->eids = out;
}

// --- ONE UNION INSTEAD OF A CHAIN OF PAIRWISE ONES ---------------------------------------------
// A pattern used to merge its copies one at a time: acc = acc union copy. Measurement showed that on a
// hollow part with a conical wall the third merge returns an EMPTY shape, while on neighbouring
// combinations the same union gives a DIFFERENT volume depending on the order of the operands — that is,
// the chain accumulates error. This is exactly why OCCT distinguishes a pairwise boolean from a
// multi-argument one: in the latter all the arguments are intersected ONCE, through shared vertices and
// edges, rather than afresh at every step.
extern "C" QymShape* qym_shape_fuse_many(const QymShape* const* parts, int n) {
    try {
        if (!parts || n <= 0 || !parts[0]) return nullptr;
        TopTools_ListOfShape args, tools;
        args.Append(parts[0]->shape);
        for (int i = 1; i < n; ++i) {
            if (!parts[i]) return nullptr;
            tools.Append(parts[i]->shape);
        }
        BRepAlgoAPI_Fuse algo;
        algo.SetArguments(args);
        algo.SetTools(tools);
        algo.Build();
        if (!algo.IsDone()) return nullptr;
        const TopoDS_Shape res = algo.Shape();
        if (res.IsNull()) return nullptr;
        QymShape* q = new QymShape{res, {}, {}, {}, {}};
        // The counter is kept by THE FIRST argument, exactly as in a pairwise boolean.
        int nf = next_local(parts[0]->fids);
        int ne = next_local(parts[0]->eids);
        carry_ids(algo, parts[0]->shape, TopAbs_FACE, parts[0]->fids, q->fids, nf, false, &q->fsplit_of, &q->fsplit_idx);
        carry_ids(algo, parts[0]->shape, TopAbs_EDGE, parts[0]->eids, q->eids, ne);
        for (int i = 1; i < n; ++i) {
            carry_ids(algo, parts[i]->shape, TopAbs_FACE, parts[i]->fids, q->fids, nf, true, &q->fsplit_of, &q->fsplit_idx);
            carry_ids(algo, parts[i]->shape, TopAbs_EDGE, parts[i]->eids, q->eids, ne, true);
        }
        fill_unnamed(res, TopAbs_FACE, q->fids, nf);
        fill_unnamed(res, TopAbs_EDGE, q->eids, ne);
        q->shape = unify_monolithic(q->shape, q->fids, q->eids, &q->absorbed);
        return q;
    } catch (...) {
        return nullptr;
    }
}

extern "C" QymShape* qym_shape_boolean(const QymShape* a, const QymShape* b, int op) {
    try {
        if (!a || !b) return why("boolean/asked", a ? "there is no tool body" : "there is no base body"), nullptr;
        // TWO NAME SPACES MERGE rather than being renumbered. Only the BASE's names used to be carried
        // through the history, and everything arriving from the tool got fresh positional numbers — so the
        // wall of a pocket cut into the body was named by traversal order and drifted with any edit of the
        // tool. The names are structural (they carry the Id of the feature that produced them inside), so
        // they cannot collide: the base's and the tool's names simply coexist in the result.
        // The name of the operation travels into the refusal: "the cut came out empty" and "the union came
        // out empty" send whoever is fixing it to different places.
        auto finish_bool = [&](BRepBuilderAPI_MakeShape& algo, const TopoDS_Shape& res, const char* what) -> QymShape* {
            if (res.IsNull()) return why("boolean/result", what), nullptr;
            QymShape* q = new QymShape{res, {}, {}, {}, {}};
            // The counter of positional numbers is kept by THE BASE: the tool's structural names do not
            // enter it (they carry their own mark), and the tool's positional ones are not carried at all.
            // Taking the maximum over both operands would shift the numbering of new edges — and the
            // references of chamfers and fillets still live by it (their names move onto the recipe in a
            // separate pass).
            int nf = next_local(a->fids);
            int ne = next_local(a->eids);
            carry_ids(algo, a->shape, TopAbs_FACE, a->fids, q->fids, nf, false, &q->fsplit_of, &q->fsplit_idx);
            carry_ids(algo, b->shape, TopAbs_FACE, b->fids, q->fids, nf, true, &q->fsplit_of, &q->fsplit_idx);
            carry_ids(algo, a->shape, TopAbs_EDGE, a->eids, q->eids, ne);
            carry_ids(algo, b->shape, TopAbs_EDGE, b->eids, q->eids, ne, true);
            classify_unnamed(algo, a->shape, q, "boolean/base", &a->fids);
            classify_unnamed(algo, b->shape, q, "boolean/tool", &b->fids);
            fill_unnamed(res, TopAbs_FACE, q->fids, nf);
            fill_unnamed(res, TopAbs_EDGE, q->eids, ne);
            q->shape = unify_monolithic(q->shape, q->fids, q->eids, &q->absorbed); // THE MONOLITH: merge the boolean's seam
            return q;
        };
        if (op == 0) {
            BRepAlgoAPI_Cut algo(a->shape, b->shape);
            return finish_bool(algo, algo.Shape(), "the cut left nothing of the base body");
        } else if (op == 1) {
            BRepAlgoAPI_Fuse algo(a->shape, b->shape);
            return finish_bool(algo, algo.Shape(), "the union of the two bodies came out empty");
        } else {
            BRepAlgoAPI_Common algo(a->shape, b->shape);
            return finish_bool(algo, algo.Shape(), "the two bodies have nothing in common");
        }
    } QYM_WHY_CATCH("boolean")
    return nullptr; // an exception crossing the C ABI aborts the process, so an honest refusal goes back instead
}
// --- A REAL THREAD (a helix, a pipe shell and a boolean cut) -----------------------------------
// The helix is an EDGE on a cylindrical surface: a 2D straight line (u = angle, v = height) on a
// `Geom_CylindricalSurface` gives a true helicoid rather than a polyline. `axes` is the cylinder's frame
// (its origin on the end face, the axis giving the direction).
static TopoDS_Wire make_helix_wire(const gp_Ax3& axes, double radius, double lead, double length, bool left) {
    Handle(Geom_CylindricalSurface) cyl = new Geom_CylindricalSurface(axes, radius);
    double slope = lead / (2.0 * M_PI);             // dv/du: over one turn (2*pi) the height grows by `lead`
    if (left) slope = -slope;                        // a left-hand thread runs the other way
    Handle(Geom2d_Line) line2 = new Geom2d_Line(gp_Pnt2d(0.0, 0.0), gp_Dir2d(1.0, slope));
    double umag = sqrt(1.0 + slope * slope);         // |(1, slope)| before OCCT normalises it, hence the parameter rescaling
    double t1 = (length / lead) * 2.0 * M_PI * umag; // the parameter where v = length (u is the angle wanted)
    Handle(Geom2d_TrimmedCurve) seg = new Geom2d_TrimmedCurve(line2, 0.0, t1);
    TopoDS_Edge e = BRepBuilderAPI_MakeEdge(seg, cyl).Edge();
    BRepLib::BuildCurves3d(e);                        // build the edge's 3D curve
    return BRepBuilderAPI_MakeWire(e).Wire();
}

// A SEGMENT of the helicoid from height z0 to z1 (for splitting a long thread: a short helix is more dependable).
TopoDS_Wire make_helix_wire_seg(const gp_Ax3& axes, double radius, double lead, double z0, double z1, bool left) {
    Handle(Geom_CylindricalSurface) cyl = new Geom_CylindricalSurface(axes, radius);
    double slope = lead / (2.0 * M_PI);
    if (left) slope = -slope;
    Handle(Geom2d_Line) line2 = new Geom2d_Line(gp_Pnt2d(0.0, 0.0), gp_Dir2d(1.0, slope));
    double umag = sqrt(1.0 + slope * slope);
    // THE HELIX IS LAID AS SEVERAL EDGES into one wire. As a single edge the sweep breaks on long threads:
    // from about 60 turns `MakePipeShell` simply refuses to build (M6x1 over 70 mm — a refusal in 0.17 s).
    // Splitting the segment itself and gluing the pieces with a boolean is not an option (tried: an overlap
    // drowns the BOP, a butt joint leaves a seam, and fusing the helices beforehand gives rubbish), whereas
    // a sweep along a wire of several edges assembles into ONE body at once — there are no seams by
    // construction.
    double turns = fabs(z1 - z0) / lead;
    double per = 8.0; if (const char* e = getenv("QYM_TURNS_PER_EDGE")) per = atof(e);
    int nedge = (int)ceil(turns / per); if (nedge < 1) nedge = 1;
    BRepBuilderAPI_MakeWire mw;
    for (int i = 0; i < nedge; ++i) {
        double a = z0 + (z1 - z0) * i / nedge, b = z0 + (z1 - z0) * (i + 1) / nedge;
        double t0 = a * umag / slope, t1 = b * umag / slope; // v=z ⇒ t=z·umag/slope
        Handle(Geom2d_TrimmedCurve) seg = new Geom2d_TrimmedCurve(line2, std::min(t0, t1), std::max(t0, t1));
        TopoDS_Edge e = BRepBuilderAPI_MakeEdge(seg, cyl).Edge();
        BRepLib::BuildCurves3d(e);
        mw.Add(e);
    }
    return mw.IsDone() ? mw.Wire() : TopoDS_Wire();
}

// The thread's profile (the groove's cross-section) — a closed trapezium in the meridional plane at the
// START of the helix, spanned by the radial direction `ra` and the axis `ax`. The crest's width is set by
// AN ANGLE: the profile's half-angle is angle/2 from the radial, so hw_top = hw_bot + depth * tan(angle/2).
// `form`: 0 is a triangle (nearly a point at the root), 1 a trapezium (a flat root), 2 a rounded one (an arc
// at the base). `clearance_crest` is extra axial clearance at the surface, `clearance_root` extra radial
// depth (the fit for 3D printing).
// An external thread cuts the groove inwards (negative radius) with an overshoot outwards; an internal one
// cuts outwards, into the wall.
TopoDS_Wire thread_profile_wire(const gp_Ax3& axes, double radius, double pitch,
                                       double angle_deg, double depth, bool internal,
                                       int form, double clearance_crest, double clearance_root) {
    gp_Vec ax(axes.Direction());
    gp_Vec ra(axes.XDirection());
    gp_Pnt S = axes.Location().Translated(ra * radius); // the point on the surface at the start (u=0, v=0)
    double a = angle_deg; if (a < 1.0) a = 1.0; if (a > 170.0) a = 170.0;
    double half = 0.5 * a * M_PI / 180.0;
    double cc = clearance_crest > 0 ? clearance_crest : 0.0;
    double cr = clearance_root  > 0 ? clearance_root  : 0.0;
    double depth_eff = depth + cr;                    // the root clearance means cutting a little deeper
    double dir_in0 = internal ? -1.0 : 1.0;
    double mo0 = 0.2 * pitch;                         // an overshoot past the surface keeps the boolean clean
    // --- A FULLY ROUND profile (form == 2, a round thread for 3D printing) --------------------------
    // The groove's contour is a cosine "bowl" running from overshoot to overshoot:
    // rr(u) = -depth + (depth + mo) * 0.5 * (1 - cos(pi*u/hp)) over u in [-hp, hp]. At the centre (u=0)
    // rr = -depth: the root, ROUNDED, with a horizontal tangent; at the edges (u = +/-hp) rr = +mo, the
    // overshoot past the surface. The cosine crosses the surface (rr=0) AT AN ANGLE, so the boolean cuts
    // cleanly — a tangent root used to give a no-op. Neighbouring grooves leave a tooth of finite thickness
    // with a ROUNDED crest (the cosine's shoulders). That way both the thread's faces and what lies between
    // the turns are smooth arcs. The top of the contour is the overshoot line (`Close`).
    if (form == 2) {
        double hp = 0.45 * pitch + cc; if (hp > 0.49 * pitch) hp = 0.49 * pitch; // crest clearance makes the tooth thinner
        BRepBuilderAPI_MakePolygon poly;
        const int M = 25;
        for (int i = 0; i < M; ++i) {                      // the cosine bowl: tL (overshoot) -> root -> tR (overshoot)
            double u = -hp + (2.0 * hp) * i / (M - 1);
            double rr = -depth_eff + (depth_eff + mo0) * 0.5 * (1.0 - cos(M_PI * u / hp)); // [−depth_eff, +mo0]
            poly.Add(S.Translated(ra * (rr * dir_in0)).Translated(ax * u));
        }
        poly.Close();                                      // the top: the overshoot line from tR to tL
        return poly.Wire();
    }
    // THE FORM sets THE CREST (the land between turns) through the groove's width at the surface, hw_top:
    // the land is pitch - 2*hw_top. A triangle gives hw_top of about P/2 (almost no land, a sharp crest); a
    // trapezium gives a narrower groove (a wide flat crest); the rounded one lies between. The root
    // (hw_bot) follows from the angle: hw_bot = hw_top - depth * tan(angle/2), so THE FLANKS sit at exactly
    // the angle asked for and the form only clamps the MINIMUM width of the root (sharp, flat or an arc).
    // THE ANGLE is the included V angle between the thread's two flanks (60 deg for the standard metric
    // form); the half-angle is angle/2 from the radial direction.
    double hw_top, hw_bot_min;
    if (form == 1) { hw_top = 0.30 * pitch; hw_bot_min = 0.22 * pitch; }       // a trapezium: a wide FLAT crest and a wide flat root
    else           { hw_top = 0.48 * pitch; hw_bot_min = 0.02 * pitch; }       // a triangle: a sharp crest
    hw_top += cc;                                     // crest clearance widens the groove at the surface, thinning the thread
    if (hw_top > 0.49 * pitch) hw_top = 0.49 * pitch; // a minimum land is kept, so the turns do not merge
    double mo = mo0;                                  // an overshoot past the surface keeps the boolean clean
    double dir_in = dir_in0;                          // outwards for an external thread, inwards for an internal one
    // THE SHARP V: the depth at which flanks at that angle meet in a point. If the depth asked for is at
    // least that, a TRUE triangle is built (3 points, a point at the root), CLAMPING the depth to the
    // meeting: then THE ANGLE always has a visible effect (a larger angle gives a shallower, sharper V,
    // rather than "it clamps and nothing changes"), and the pipe shell stays fast — a thin flat root is
    // exactly what used to freeze at large angles and depths.
    double v_close = hw_top / tan(half);
    if (form == 0 && depth_eff >= v_close - 1e-9) {
        gp_Pnt q1 = S.Translated(ra * (mo * dir_in)).Translated(ax *  hw_top);
        gp_Pnt q2 = S.Translated(ra * (mo * dir_in)).Translated(ax * -hw_top);
        gp_Pnt apex = S.Translated(ra * (-v_close * dir_in)); // the point (axially at 0) at the meeting depth
        return BRepBuilderAPI_MakePolygon(q1, q2, apex, Standard_True).Wire();
    }
    double hw_bot = hw_top - depth_eff * tan(half);   // the angle sets the taper towards the root (the flanks at that angle)
    if (hw_bot < hw_bot_min) hw_bot = hw_bot_min;     // by the form: a sharp or a flat root
    if (hw_bot > hw_top - 0.01 * pitch) hw_bot = hw_top - 0.01 * pitch; // the root is always narrower than the crest
    double top_r = mo * dir_in;                      // the top of the groove sits at the surface, with the overshoot
    double bot_r = -depth_eff * dir_in;              // the root of the groove sits at the thread's depth
    gp_Pnt p1 = S.Translated(ra * top_r).Translated(ax *  hw_top);
    gp_Pnt p2 = S.Translated(ra * top_r).Translated(ax * -hw_top);
    gp_Pnt p3 = S.Translated(ra * bot_r).Translated(ax * -hw_bot);
    gp_Pnt p4 = S.Translated(ra * bot_r).Translated(ax *  hw_bot);
    return BRepBuilderAPI_MakePolygon(p1, p2, p3, p4, Standard_True).Wire();
}

// A SMOOTH RUN-OUT (a vanishing thread): a homothetic scaling law for the thread's profile along the
// segment [z0, z1] (the pipe shell's parameter, 0 to 1). In the lead-in zone (globally [0, lin]) and the
// lead-out zone ([L - lout, L]) the scale melts towards zero, so the groove
// fades away smoothly at the ends (the thread's depth goes to zero), the way a rolled thread does — WITHOUT
// separate booleans and WITHOUT an edge. `nullptr` when this segment has no run-out (then an ordinary `Add`
// at full depth). The homothety is about the spine point (the helix at radius R, the crest), so what melts
// is THE DEPTH: the crest stays at R.
Handle(Law_Function) runout_law(double z0, double z1, double L, double lin, double lout) {
    const double lo = 0.06;                       // not 0, or the section degenerates to a point and the pipe shell tears
    double seg = z1 - z0; if (seg < 1e-9) return nullptr;
    double zf_start = lin;                         // [0, lin] is the lead-in ramp
    double zf_end   = L - lout;                    // [L - lout, L] is the lead-out ramp
    bool has_in  = lin  > 1e-6 && z0 < zf_start - 1e-9;
    bool has_out = lout > 1e-6 && z1 > zf_end + 1e-9;
    if (!has_in && !has_out) return nullptr;       // a full thread over the whole segment: an ordinary `Add`
    // f(t): the profile's scale at the segment's parameter t in [0, 1]. A smoothstep ramp from `lo` to 1 at
    // the ends and FLAT at 1 in the middle. It is built by DENSE sampling — a `Law_Interpol` cubic over
    // sparse flat points overshoots above 1, and the groove then swells and tears the body; a dense grid
    // holds the interpolant at its values with no overshoot.
    double pin  = has_in  ? (zf_start - z0) / seg : 0.0;         // the end of the lead-in ramp in segment parameter
    double pout = has_out ? (z1 - zf_end)  / seg : 0.0;          // the length of the lead-out ramp in segment parameter
    double z0_is_tip = (z0 <= 1e-9);                             // the segment really does begin at the end face
    double z1_is_tip = (z1 >= L - 1e-9);
    auto smooth = [](double x){ if (x <= 0) return 0.0; if (x >= 1) return 1.0; return x * x * (3.0 - 2.0 * x); };
    auto f = [&](double t){
        double v = 1.0;
        if (has_in && z0_is_tip && t < pin)            v = std::min(v, lo + (1.0 - lo) * smooth(t / std::max(pin, 1e-9)));
        if (has_out && z1_is_tip && t > 1.0 - pout)    v = std::min(v, lo + (1.0 - lo) * smooth((1.0 - t) / std::max(pout, 1e-9)));
        return v;
    };
    // The sampling is UNEVEN: dense where the law changes (the ramps at the ends), sparse over the flat
    // stretch. A uniform grid fails in either direction: a sparse one catches a short ramp with two points
    // and smears it over half the thread, while a dense one (a 1.5 mm run-out over a length of 100 mm would
    // need 800 knots) turns the law into a spline of hundreds of knots — the swept surfaces then ripple and
    // the body comes out INVALID. Three dozen knots in the right places describe the same ramp exactly and
    // without ripples.
    std::vector<double> ts{0.0, 1.0};
    const int RAMP = 14;                                  // knots per ramp
    if (has_in && z0_is_tip && pin > 1e-9) {
        for (int i = 0; i <= RAMP; ++i) ts.push_back(pin * i / RAMP);
        ts.push_back(std::min(1.0, pin * 1.15));          // a couple of points just past the ramp keep the plateau flat
        ts.push_back(std::min(1.0, pin * 1.5));
    }
    if (has_out && z1_is_tip && pout > 1e-9) {
        for (int i = 0; i <= RAMP; ++i) ts.push_back(1.0 - pout * i / RAMP);
        ts.push_back(std::max(0.0, 1.0 - pout * 1.15));
        ts.push_back(std::max(0.0, 1.0 - pout * 1.5));
    }
    for (int i = 1; i < 6; ++i) ts.push_back((double)i / 6.0); // a sparse grid across the whole plateau
    std::sort(ts.begin(), ts.end());
    std::vector<double> tt;
    for (double t : ts) if (t >= 0.0 && t <= 1.0 && (tt.empty() || t - tt.back() > 1e-6)) tt.push_back(t);
    if (tt.size() < 3) return nullptr;
    const int N = (int)tt.size();
    TColgp_Array1OfPnt2d arr(1, N);
    for (int i = 0; i < N; ++i) arr.SetValue(i + 1, gp_Pnt2d(tt[i], f(tt[i])));
    Handle(Law_Interpol) law = new Law_Interpol();
    law->Set(arr, 0.0, 0.0);                        // zero derivatives at the ends
    return law;
}

// A THREAD on the body `base` along the axis (origin, dir), the surface's radius being `radius` (from the
// geometry itself). It builds the helical groove or grooves from the thread's profile (pitch, angle,
// depth), with `starts` starts (the lead being starts * pitch, shifted by 360/starts), the hand given by
// `left`, internal or external — and cuts it out of `base` by a boolean. A real B-rep.
