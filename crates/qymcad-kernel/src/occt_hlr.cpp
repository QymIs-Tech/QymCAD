// HIDDEN-LINE PROJECTION OF ONE BODY: the outlines a drawing sheet shows.
//
// A view direction goes in, visible and hidden polylines come out - in 3D, before any projection onto
// the sheet. The sheet workbench projects the points itself, so this stays a geometry query with no
// screen in it. Exact HLR (HLRBRep), not the tessellated kind: a drawing dimensioned off these lines
// must meet the body, not its triangulation.
#include "occt_common.hpp"
#include <cmath>
#include <HLRBRep_Algo.hxx>
#include <HLRAlgo_Projector.hxx>
#include <HLRBRep_HLRToShape.hxx>
#include <TopExp.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Vertex.hxx>
#include <Bnd_Box.hxx>
#include <BRepBndLib.hxx>
#include <Geom_Curve.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <gp_Ax2.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Vec2d.hxx>
#include <vector>
#include <array>

// One projected body: the polylines and, per polyline, whether it is visible.
struct QymHlr {
    std::vector<std::vector<std::array<double, 3>>> lines;
    std::vector<char> vis;
};

// Points per outline. Uniform in the parameter, corners on the curve: a straight edge is exact with
// any count, and 33 points hold a full circle within 0.15 per cent of its radius.
static const int HLR_SAMPLES = 33;
// Dense samples for tying a projected outline back to its body edge.
static const int MATCH_SAMPLES = 257;

// An edge of the body with its 3D curve: what a projected outline is drawn from.
struct OrigEdge {
    Handle(Geom_Curve) curve;
    double first;
    double last;
};

// The 2D distance from a point to a polyline.
static double dist_to_poly(const std::vector<gp_Pnt2d>& poly, const gp_Pnt2d& p) {
    double best = 1e300;
    for (size_t k = 0; k + 1 < poly.size(); ++k) {
        gp_Vec2d ab(poly[k], poly[k + 1]);
        gp_Vec2d ap(poly[k], p);
        double denom = ab.SquareMagnitude();
        double t = (denom > 0.0) ? ap.Dot(ab) / denom : 0.0;
        t = std::max(0.0, std::min(1.0, t));
        gp_Pnt2d q(poly[k].X() + ab.X() * t, poly[k].Y() + ab.Y() * t);
        best = std::min(best, gp_Vec2d(q, p).Magnitude());
    }
    return best;
}

// The edges of one visibility class out of the HLR tool, sampled into `out`. The tool hands over
// outlines in the projection plane (2D points with no usable 3D curve), so every outline is tied back
// to the body edge it came from in that same 2D plane: the body curve is sampled densely, projected
// with the view frame, and the nearest one through both ends wins. Sampling then runs on the 3D
// curve between the matched parameters, so the points are world points of the body.
static void collect_hlr(const TopoDS_Shape& body, const TopoDS_Shape& compound, const gp_Pnt& eye, const gp_Dir& xd, const gp_Dir& yd, bool visible, QymHlr& out) {
    struct Cand {
        OrigEdge edge;
        std::vector<gp_Pnt2d> flat;
    };
    std::vector<Cand> orig;
    for (TopExp_Explorer ex(body, TopAbs_EDGE); ex.More(); ex.Next()) {
        TopoDS_Edge e = TopoDS::Edge(ex.Current());
        if (BRep_Tool::Degenerated(e)) continue;
        Standard_Real f = 0.0, l = 0.0;
        Handle(Geom_Curve) c = BRep_Tool::Curve(e, f, l);
        if (c.IsNull() || !(l > f)) continue;
        Cand cand{{c, (double)f, (double)l}, {}};
        cand.flat.reserve(MATCH_SAMPLES);
        for (int k = 0; k < MATCH_SAMPLES; ++k) {
            double u = cand.edge.first + (cand.edge.last - cand.edge.first) * (double)k / (double)(MATCH_SAMPLES - 1);
            gp_Pnt p = c->Value(u);
            gp_Vec v(eye, p);
            cand.flat.push_back(gp_Pnt2d(v.Dot(gp_Vec(xd)), v.Dot(gp_Vec(yd))));
        }
        orig.push_back(std::move(cand));
    }
    for (TopExp_Explorer ex(compound, TopAbs_EDGE); ex.More(); ex.Next()) {
        TopoDS_Edge edge = TopoDS::Edge(ex.Current());
        // First the edge's own 3D curve, where the tool hands one over (outline edges may carry
        // theirs): sampled straight, with no matching involved.
        {
            Standard_Real f = 0.0, l = 0.0;
            Handle(Geom_Curve) c = BRep_Tool::Curve(edge, f, l);
            if (!c.IsNull() && l > f && !BRep_Tool::Degenerated(edge)) {
                std::vector<std::array<double, 3>> pts;
                pts.reserve(HLR_SAMPLES);
                for (int k = 0; k < HLR_SAMPLES; ++k) {
                    double u = f + (l - f) * (double)k / (double)(HLR_SAMPLES - 1);
                    gp_Pnt p = c->Value(u);
                    pts.push_back({p.X(), p.Y(), p.Z()});
                }
                out.lines.push_back(std::move(pts));
                out.vis.push_back(visible ? 1 : 0);
                continue;
            }
        }
        TopoDS_Vertex v0, v1;
        TopExp::Vertices(edge, v0, v1);
        if (v0.IsNull() || v1.IsNull()) continue;
        gp_Pnt a3 = BRep_Tool::Pnt(v0);
        gp_Pnt b3 = BRep_Tool::Pnt(v1);
        gp_Pnt2d a(a3.X(), a3.Y());
        gp_Pnt2d b(b3.X(), b3.Y());
        bool closed = v0.IsSame(v1);
        if (!closed && gp_Vec2d(a, b).Magnitude() < 1e-12) continue; // a point, not a line to draw
        // The body edge through both ends; the ends are split points of that very curve, so the
        // winner stands orders of magnitude nearer than any other edge. A closed outline (one vertex)
        // matches by that point alone and takes the whole original curve.
        const Cand* src = nullptr;
        double su0 = 0.0, su1 = 0.0, best = 1e-4;
        for (size_t oi = 0; oi < orig.size(); ++oi) {
            const Cand& o = orig[oi];
            double d = closed ? dist_to_poly(o.flat, a) : dist_to_poly(o.flat, a) + dist_to_poly(o.flat, b);
            if (d < best) {
                best = d;
                src = &o;
                // Parameters from the nearest dense samples.
                double bi0 = 0.0, bi1 = 0.0, bd0 = 1e300, bd1 = 1e300;
                for (int k = 0; k < MATCH_SAMPLES; ++k) {
                    double da = gp_Vec2d(o.flat[k], a).Magnitude();
                    double db = gp_Vec2d(o.flat[k], b).Magnitude();
                    if (da < bd0) { bd0 = da; bi0 = k; }
                    if (db < bd1) { bd1 = db; bi1 = k; }
                }
                su0 = o.edge.first + (o.edge.last - o.edge.first) * bi0 / (double)(MATCH_SAMPLES - 1);
                su1 = o.edge.first + (o.edge.last - o.edge.first) * bi1 / (double)(MATCH_SAMPLES - 1);
            }
        }
        if (!src) continue;
        if (closed) {
            su0 = src->edge.first;
            su1 = src->edge.last;
        }
        std::vector<std::array<double, 3>> pts;
        pts.reserve(HLR_SAMPLES);
        for (int k = 0; k < HLR_SAMPLES; ++k) {
            double u = su0 + (su1 - su0) * (double)k / (double)(HLR_SAMPLES - 1);
            gp_Pnt p = src->edge.curve->Value(u);
            pts.push_back({p.X(), p.Y(), p.Z()});
        }
        out.lines.push_back(std::move(pts));
        out.vis.push_back(visible ? 1 : 0);
    }
}

extern "C" {

// Hidden-line projection of the body behind `s` seen along `dir` (3 doubles, need not be unit).
// Null when the direction is missing or zero, the body is empty, or the computation did not run:
// better an honest refusal than outlines of nothing.
QymHlr* qym_shape_hlr(const QymShape* s, const double* dir) {
    if (!s || !dir) return nullptr;
    double dl = sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
    if (!(dl > 0.0) || !std::isfinite(dl)) return nullptr;
    if (s->shape.IsNull()) return nullptr;
    QymHlr* out = new (std::nothrow) QymHlr();
    if (!out) return nullptr;
    try {
        gp_Dir d(dir[0] / dl, dir[1] / dl, dir[2] / dl);
        // The eye stands well outside the body: a projector at the origin can sit inside the
        // shape itself, and then there is nothing sensible to project.
        Bnd_Box box;
        BRepBndLib::Add(s->shape, box);
        double x0, y0, z0, x1, y1, z1;
        box.Get(x0, y0, z0, x1, y1, z1);
        gp_Pnt center(0.5 * (x0 + x1), 0.5 * (y0 + y1), 0.5 * (z0 + z1));
        double diag = sqrt((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0) + (z1 - z0) * (z1 - z0));
        gp_Pnt eye(center.X() - d.X() * 10.0 * diag, center.Y() - d.Y() * 10.0 * diag, center.Z() - d.Z() * 10.0 * diag);
        gp_Ax2 frame(eye, d);
        HLRAlgo_Projector projector(frame);
        // The algorithm is reference-counted: the shape tool holds a Handle to it, so a stack
        // instance would be freed twice (once by the Handle, once by the scope) and corrupt the
        // heap. It lives on the heap and dies with the last Handle.
        Handle(HLRBRep_Algo) algo = new HLRBRep_Algo();
        algo->Add(s->shape);
        algo->Projector(projector);
        algo->Update();
        algo->Hide();
        HLRBRep_HLRToShape tool(algo);
        // Sharp and smooth topology edges alike: a smooth edge on the silhouette (the seam of a
        // face) is a legitimate outline there, drawn exact. What never draws from topology are
        // true silhouette curves with no edge behind them - those come tessellated from the mesh.
        collect_hlr(s->shape, tool.VCompound(), eye, frame.XDirection(), frame.YDirection(), true, *out);
        collect_hlr(s->shape, tool.HCompound(), eye, frame.XDirection(), frame.YDirection(), false, *out);
    } catch (...) {
        delete out;
        return nullptr;
    }
    if (out->lines.empty()) {
        delete out;
        return nullptr;
    }
    return out;
}

size_t qym_hlr_count(const QymHlr* h) {
    if (!h) return 0;
    return h->lines.size();
}

int qym_hlr_visible(const QymHlr* h, size_t i) {
    if (!h || i >= h->vis.size()) return 0;
    return h->vis[i] ? 1 : 0;
}

size_t qym_hlr_points(const QymHlr* h, size_t i) {
    if (!h || i >= h->lines.size()) return 0;
    return h->lines[i].size();
}

void qym_hlr_copy(const QymHlr* h, size_t i, double* pts_out) {
    if (!h || !pts_out || i >= h->lines.size()) return;
    for (size_t k = 0; k < h->lines[i].size(); ++k) {
        pts_out[3 * k + 0] = h->lines[i][k][0];
        pts_out[3 * k + 1] = h->lines[i][k][1];
        pts_out[3 * k + 2] = h->lines[i][k][2];
    }
}

void qym_hlr_free(QymHlr* h) {
    delete h;
}

} // extern "C"
