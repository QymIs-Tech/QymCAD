// A BODY FROM FACES RECOGNISED ON A MESH.
//
// The recognition module finds the surfaces, the corners and edges where they meet and the loops that bound each face;
// here that becomes OCCT geometry. A Geom surface per face; per edge an exact line or circle - or, where the module
// found neither, a B-spline through the edge's points - trimmed at its corners; faces bounded by wires of those edges,
// their curves on the surface added by ShapeFix_Face - on a fitted wall laid by the rule it was fitted with; the faces
// and the regions left as triangles put together on their shared edges, sewn only where sides do not pair up, and a
// closed shell made a solid.
#include "occt_common.hpp"
#include <BRepCheck_Wire.hxx>
#include <BRepLib.hxx>
#include <Geom2dAPI_Interpolate.hxx>
#include <Geom2d_BSplineCurve.hxx>
#include <TColStd_HArray1OfReal.hxx>
#include <TColgp_Array1OfPnt2d.hxx>
#include <TColgp_HArray1OfPnt2d.hxx>
#include <GeomAdaptor_Surface.hxx>
#include <ElSLib.hxx>
#include <Geom2d_Line.hxx>
#include <gp_Vec2d.hxx>
#include <gp_Dir2d.hxx>
#include <TopExp.hxx>
#include <TopTools_IndexedDataMapOfShapeListOfShape.hxx>
#include <algorithm>
#include <array>
#include <cstring>
#include <functional>
#include <map>
#include <unordered_map>
#include <BRep_Builder.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRepTools.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <gp_Pnt2d.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <ElCLib.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_Circle.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepOffsetAPI_MakeFilling.hxx>
#include <Geom_Ellipse.hxx>
#include <gp_Elips.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Line.hxx>
#include <Geom_Plane.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <GeomAPI_PointsToBSpline.hxx>
#include <GeomAPI_PointsToBSplineSurface.hxx>
#include <Geom_BSplineSurface.hxx>
#include <TColgp_Array2OfPnt.hxx>
#include <GeomAPI_ProjectPointOnCurve.hxx>
#include <ShapeFix_Face.hxx>
#include <ShapeFix_Shell.hxx>
#include <ShapeFix_Solid.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>

namespace {

// how many numbers describe one surface in `sparam`
constexpr size_t NUMBERS = 20;

gp_Pnt pnt(const double* a) { return gp_Pnt(a[0], a[1], a[2]); }
gp_Dir dir(const double* a) { return gp_Dir(a[0], a[1], a[2]); }

// A right-handed frame whose Z is `z` and whose X points away from `inside`: OCCT's cylinder, cone, sphere and torus on
// it have their normal pointing out, and their seam - where the angle round the axis starts - on the far side from the
// face. A seam or a pole at a face's own corner split a sliver off it: measured on a bar rounded by 2 mm, with every
// sphere's axis along Z its pole stood at a corner of its patch, and each of the 8 corners came back with a ninth face of
// 0.0004 mm^2.
gp_Ax3 frame(const gp_Pnt& o, const gp_Dir& z, const gp_Pnt& inside) {
    gp_Vec away = gp_Vec(inside, o);
    away -= gp_Vec(z) * away.Dot(gp_Vec(z));
    if (away.Magnitude() < 1.0e-9) {
        const gp_Dir ref = std::abs(z.X()) < 0.9 ? gp_Dir(1, 0, 0) : gp_Dir(0, 1, 0);
        return gp_Ax3(o, z, gp_Dir(gp_Vec(z).Crossed(gp_Vec(ref))));
    }
    return gp_Ax3(o, z, gp_Dir(away));
}

// Twenty numbers per surface: 0 a plane (point, normal), 1 a cylinder (point, axis, radius), 2 a cone (apex, axis from
// the apex into the cone, half angle), 3 a sphere (centre, -, radius), 4 a torus (centre, axis, major, minor), 6 a helix
// (a point on the axis, the axis, the reference the angle starts from, rise per radian, the profile's normal radial and
// axial, offset, the face's angles from and to, its distances from and to), 7 a coil (a point on the axis, the axis, the
// reference, rise per radian, the radius of the wire's middle, its lift, the wire's radius, the face's angles from and to),
// 8 a round helix (a point on the axis, the axis, the reference, rise per radian, the circle's middle from the axis and
// its height, its radius, the face's angles along the turn from and to, its angles round the circle from and to);
// `inside` a point amid the face. A sphere's
// axis is laid square to the way the face lies from the centre, so both poles stand aside of it.
Handle(Geom_Surface) surface(int kind, const double* p, const gp_Pnt& inside) {
    switch (kind) {
    case 0: return new Geom_Plane(pnt(p), dir(p + 3));
    case 1: return new Geom_CylindricalSurface(frame(pnt(p), dir(p + 3), inside), p[6]);
    case 2: return new Geom_ConicalSurface(frame(pnt(p), dir(p + 3), inside), p[6], 0.0);
    case 3: {
        gp_Vec toward(pnt(p), inside);
        if (toward.Magnitude() < 1.0e-9) toward = gp_Vec(1, 0, 0);
        const gp_Vec ref = std::abs(toward.Normalized().Z()) < 0.9 ? gp_Vec(0, 0, 1) : gp_Vec(1, 0, 0);
        return new Geom_SphericalSurface(frame(pnt(p), gp_Dir(toward.Crossed(ref)), inside), p[6]);
    }
    case 4: return new Geom_ToroidalSurface(frame(pnt(p), dir(p + 3), inside), p[6], p[7]);
    case 6: {
        // A HELIX has no surface of its own in OCCT: it is sampled every 5 deg of its turn and at four distances across
        // its reach, and interpolated. A cubic through points 5 deg apart stands off a circle of radius R by R x 1.5e-7,
        // and along the distance the profile is straight, which a cubic holds exactly. The distance runs outside in, so
        // that the surface's own normal points the way (radial, axial) does. Past the face's own angles and distances it
        // reaches on by 0.2 rad and half its reach: the loops are trimmed on it, not on its edge.
        const gp_Vec a(dir(p + 3)), x(dir(p + 6));
        const gp_Vec y = a.Crossed(x);
        const double rise = p[9], radial = p[10], axial = p[11], offset = p[12];
        const double t0 = p[13] - 0.2, t1 = p[14] + 0.2;
        const double wide = std::max(p[16] - p[15], 1.0e-3);
        const double r0 = std::max(p[15] - 0.5 * wide, 0.5 * p[15]), r1 = p[16] + 0.5 * wide;
        if (std::abs(axial) < 1.0e-9) return Handle(Geom_Surface)();
        const int nu = std::max(8, (int)std::ceil((t1 - t0) / (M_PI / 36.0))) + 1;
        TColgp_Array2OfPnt grid(1, nu, 1, 4);
        for (int i = 0; i < nu; ++i) {
            const double t = t0 + (t1 - t0) * i / (nu - 1);
            for (int j = 0; j < 4; ++j) {
                const double r = r1 + (r0 - r1) * j / 3.0;
                grid.SetValue(i + 1, j + 1, pnt(p).Translated(x * (r * std::cos(t)) + y * (r * std::sin(t)) + a * (rise * t + (offset - radial * r) / axial)));
            }
        }
        GeomAPI_PointsToBSplineSurface fit;
        fit.Interpolate(grid);
        return fit.IsDone() ? Handle(Geom_Surface)(fit.Surface()) : Handle(Geom_Surface)();
    }
    case 8: {
        // A ROUND HELIX, sampled every 5 deg along its turn and round its circle, closed round the circle where the face
        // goes all round it. Round the circle it runs backwards, so that its own normal points away from the middle;
        // past the face's own angles either way it reaches on by 0.2 rad.
        const gp_Vec a(dir(p + 3)), x(dir(p + 6));
        const gp_Vec y = a.Crossed(x);
        const double rise = p[9], middle = p[10], height = p[11], round = p[12];
        const double t0 = p[13] - 0.2, t1 = p[14] + 0.2;
        const bool closed = p[16] - p[15] > 2.0 * M_PI - 0.4;
        const double f0 = closed ? 0.0 : p[15] - 0.2, f1 = closed ? 2.0 * M_PI : p[16] + 0.2;
        const int nu = closed ? 72 : std::max(4, (int)std::ceil((f1 - f0) / (M_PI / 36.0))) + 1;
        const int nv = std::max(8, (int)std::ceil((t1 - t0) / (M_PI / 36.0))) + 1;
        TColgp_Array2OfPnt grid(1, nu, 1, nv);
        for (int j = 0; j < nv; ++j) {
            const double t = t0 + (t1 - t0) * j / (nv - 1);
            const gp_Vec out = x * std::cos(t) + y * std::sin(t);
            for (int i = 0; i < nu; ++i) {
                const double phi = closed ? -2.0 * M_PI * i / nu : f1 - (f1 - f0) * i / (nu - 1);
                grid.SetValue(i + 1, j + 1, pnt(p).Translated(out * (middle + round * std::cos(phi)) + a * (height + round * std::sin(phi) + rise * t)));
            }
        }
        GeomAPI_PointsToBSplineSurface fit;
        fit.Interpolate(grid, closed ? Standard_True : Standard_False);
        return fit.IsDone() ? Handle(Geom_Surface)(fit.Surface()) : Handle(Geom_Surface)();
    }
    case 7: {
        // A COIL, sampled every 10 deg round the wire and every 5 deg along its turn, interpolated closed round the wire.
        // Round the wire it runs the way that keeps its own normal pointing out of the wire; past the face's own angles it
        // reaches on by 0.2 rad.
        const gp_Vec a(dir(p + 3)), x(dir(p + 6));
        const gp_Vec y = a.Crossed(x);
        const double rise = p[9], radius = p[10], lift = p[11], wire = p[12];
        const double t0 = p[13] - 0.2, t1 = p[14] + 0.2;
        const int nu = 36, nv = std::max(8, (int)std::ceil((t1 - t0) / (M_PI / 36.0))) + 1;
        TColgp_Array2OfPnt grid(1, nu, 1, nv);
        for (int j = 0; j < nv; ++j) {
            const double t = t0 + (t1 - t0) * j / (nv - 1);
            const gp_Vec out = x * std::cos(t) + y * std::sin(t);
            const gp_Vec ahead = (y * std::cos(t) - x * std::sin(t)) * radius + a * rise;
            const gp_Vec tangent = ahead.Normalized(), normal = -out;
            const gp_Vec binormal = tangent.Crossed(normal);
            const gp_Pnt middle = pnt(p).Translated(out * radius + a * (lift + rise * t));
            for (int i = 0; i < nu; ++i) {
                const double phi = 2.0 * M_PI * i / nu;
                grid.SetValue(i + 1, j + 1, middle.Translated((normal * std::cos(phi) + binormal * std::sin(phi)) * wire));
            }
        }
        GeomAPI_PointsToBSplineSurface fit;
        fit.Interpolate(grid, Standard_True);
        return fit.IsDone() ? Handle(Geom_Surface)(fit.Surface()) : Handle(Geom_Surface)();
    }
    default: return Handle(Geom_Surface)();
    }
}

// How far the angle `b` lies past `a` turning forwards, in [0, 2 pi).
double ahead(double a, double b) {
    double d = std::fmod(b - a, 2.0 * M_PI);
    return d < 0.0 ? d + 2.0 * M_PI : d;
}

} // namespace

// `sparam` twenty numbers per surface (see `surface`); `vxyz` three per corner; per edge `ekind` 0 a line (point, unit
// direction along the edge), 1 a circle (centre, axis, radius), 2 a curve through its points (within how many tolerances of
// them), 3 a polyline through them, 4 an ellipse
// (centre, normal, major direction, major and minor half-axes), eleven numbers of `eparam`, its two
// corners in `eends` (-1 for a closed edge) and its points from `epts[3 * epstart[e]]` to `epts[3 * epstart[e + 1]]`;
// per face its surface, whether that surface's own normal points out of the body, a point amid it (three numbers of
// `finside`, surface `i` taking the point of face `i`), and its loops from `floops[f]` to `floops[f + 1]`, loop `l` holding
// the items from `lstart[l]` to `lstart[l + 1]`, each an edge's index plus one, negative where the loop walks the edge
// against its points. `out_faces` gets how many faces were built, `out_free` how many sides no second face met in the sewing, `out_free_at` the middle of each of up to `cap_free` of them, `out_dropped` how many triangles of the regions left as mesh made no face.
// THE FACES OF ONE RECOGNITION, KEPT BETWEEN ITS ROUNDS: a round takes a few faces back and builds every face again,
// and on a mesh of 30 636 triangles that was 6.5 s a round for faces of which all but a handful were the same. A face is
// kept under everything it is built from - its surface, the point it holds, its loops' edges with their points - and
// taken as it is where all of that is the same again. Kept per thread, from `qym_faces_cache(1)` to `qym_faces_cache(0)`.
static thread_local std::unordered_map<std::string, std::pair<TopoDS_Face, double>>* face_cache = nullptr;

extern "C" void qym_faces_cache(int on) {
    try {
        QYM_SIGNALS;
        delete face_cache;
        face_cache = on ? new std::unordered_map<std::string, std::pair<TopoDS_Face, double>>() : nullptr;
    } QYM_WHY_CATCH("faces_cache")
}

// Does a loop of `face` cross itself on the face?
static bool crosses_itself(const TopoDS_Face& face) {
    try {
        QYM_SIGNALS;
        for (TopExp_Explorer wx(face, TopAbs_WIRE); wx.More(); wx.Next()) {
            BRepCheck_Wire loop(TopoDS::Wire(wx.Current()));
            loop.InContext(face);
            TopoDS_Edge e1, e2;
            if (loop.SelfIntersect(face, e1, e2, Standard_True) == BRepCheck_SelfIntersectingWire) return true;
        }
    } catch (...) {
        return true;
    }
    return false;
}

extern "C" QymShape* qym_shape_from_faces(size_t ns, const int* skind, const double* sparam, size_t nv, const double* vxyz,
                                          size_t ne, const int* ekind, const double* eparam, const int64_t* eends,
                                          const size_t* epstart, const double* epts, size_t nf, const int* fsurface,
                                          const int* foutward, const double* finside, const size_t* floops,
                                          const size_t* lstart, const int64_t* litems, size_t nloose, const double* loose, double tol, uint32_t* out_faces,
                                          double* out_area, uint32_t* out_free, double* out_free_at, size_t cap_free, uint32_t* out_dropped,
                                          int faces_only, const size_t* fpstart, const double* fpts) {
    if (out_faces) *out_faces = 0;
    if (out_free) *out_free = 0;
    if (out_dropped) *out_dropped = 0;
    for (size_t f = 0; out_area && f < nf; ++f) out_area[f] = -1.0;
    if ((ns == 0 || nf == 0) && nloose == 0) return why("faces/asked", "there is no face to build"), nullptr;
    if (ns > nf) return why("faces/asked", "more surfaces than faces to take a point amid each from"), nullptr;
    try {
        QYM_SIGNALS;
        // how much of its surface a face took, into `out_area`: a face that took a wrong piece shows it here first
        auto measured = [&](size_t f, const TopoDS_Face& face) {
            if (!out_area) return;
            GProp_GProps props;
            // to a thousandth: the default integration of a B-spline face was 5 to 20 % off on a smooth handle, and a
            // millionth took 8 s of every 10 s round of building
            BRepGProp::SurfaceProperties(face, props, 1.0e-3);
            out_area[f] = props.Mass();
        };
        const double step = std::max(tol, 1.0e-7);
        std::vector<Handle(Geom_Surface)> S(ns);
        for (size_t i = 0; i < ns; ++i) S[i] = surface(skind[i], sparam + NUMBERS * i, pnt(finside + 3 * i));
        // A FITTED WALL: a bicubic B-spline surface on clamped uniform knots, its poles the face's points (one surface
        // per face, in the faces' order)
        for (size_t i = 0; i < ns && fpstart && fpts; ++i) {
            if (skind[i] != 9) continue;
            const int nu = (int)sparam[NUMBERS * i], nvp = (int)sparam[NUMBERS * i + 1];
            if (nu < 4 || nvp < 4 || fpstart[i + 1] - fpstart[i] != (size_t)(nu * nvp)) continue;
            TColgp_Array2OfPnt poles(1, nu, 1, nvp);
            for (int b = 0; b < nvp; ++b)
                for (int a = 0; a < nu; ++a) poles(a + 1, b + 1) = pnt(fpts + 3 * (fpstart[i] + (size_t)(b * nu + a)));
            auto knots = [](int n, TColStd_Array1OfReal& k, TColStd_Array1OfInteger& m) {
                const int spans = n - 3;
                for (int j = 0; j <= spans; ++j) {
                    k(j + 1) = (double)j / spans;
                    m(j + 1) = (j == 0 || j == spans) ? 4 : 1;
                }
            };
            TColStd_Array1OfReal ku(1, nu - 2), kv(1, nvp - 2);
            TColStd_Array1OfInteger mu(1, nu - 2), mv(1, nvp - 2);
            knots(nu, ku, mu);
            knots(nvp, kv, mv);
            S[i] = new Geom_BSplineSurface(poles, ku, kv, mu, mv, 3, 3);
        }
        BRep_Builder B;
        std::vector<gp_Pnt> at(nv);
        std::vector<double> reach(nv, step);
        for (size_t i = 0; i < nv; ++i) at[i] = pnt(vxyz + 3 * i);

        // THE CURVES, and how far each corner lies off the curves that end in it: a corner is placed on its faces'
        // surfaces as nearly as they allow, not on the curve two of them meet in, and its tolerance has to span the gap
        struct Piece { Handle(Geom_Curve) c; double u0 = 0.0, u1 = 0.0; bool closed = false; gp_Pnt start; };
        std::vector<Piece> piece(ne);
        for (size_t e = 0; e < ne; ++e) {
            const size_t a = epstart[e], z = epstart[e + 1];
            if (z < a + 2) continue;
            std::vector<gp_Pnt> P;
            for (size_t k = a; k < z; ++k) P.push_back(pnt(epts + 3 * k));
            Piece& pc = piece[e];
            pc.closed = eends[2 * e] < 0;
            const gp_Pnt p0 = pc.closed ? P.front() : at[eends[2 * e]];
            const gp_Pnt p1 = pc.closed ? P.front() : at[eends[2 * e + 1]];
            const double* q = eparam + 11 * e;
            if (ekind[e] == 0) {
                gp_Lin line(pnt(q), dir(q + 3));
                pc.c = new Geom_Line(line);
                pc.u0 = ElCLib::Parameter(line, p0);
                pc.u1 = ElCLib::Parameter(line, p1);
                if (pc.closed || pc.u1 <= pc.u0) { pc.c.Nullify(); continue; }
            } else if (ekind[e] == 1) {
                gp_Circ circ(gp_Ax2(pnt(q), dir(q + 3)), q[6]);
                // the circle turns the way the edge's points run: through its middle point on the way to its end
                const double t0 = ElCLib::Parameter(circ, P.front());
                const double tm = ElCLib::Parameter(circ, P[pc.closed ? 1 : P.size() / 2]);
                const double tz = ElCLib::Parameter(circ, P.back());
                // AN ARC KNOWN BY ITS ENDS ALONE GOES THE SHORT WAY: with no point between them the middle one is the end
                // itself, every way round passed through it, and an arc of 0.017 mm on a gear of `cube_gears` became most
                // of a circle of 32.9 - its face took 3408 mm^2 for a triangle of 4e-6
                const bool forwards = pc.closed ? ahead(t0, tm) < M_PI
                                      : P.size() < 3 ? ahead(t0, tz) <= M_PI
                                      : ahead(t0, tm) <= ahead(t0, tz);
                if (!forwards) circ = gp_Circ(gp_Ax2(pnt(q), dir(q + 3).Reversed()), q[6]);
                pc.c = new Geom_Circle(circ);
                pc.u0 = ElCLib::Parameter(circ, p0);
                pc.u1 = pc.closed ? pc.u0 + 2.0 * M_PI : pc.u0 + ahead(pc.u0, ElCLib::Parameter(circ, p1));
                if (!pc.closed && pc.u1 <= pc.u0 + 1.0e-12) pc.u1 += 2.0 * M_PI;
            } else if (ekind[e] == 4) {
                gp_Elips elips(gp_Ax2(pnt(q), dir(q + 3), dir(q + 6)), q[9], q[10]);
                // turned the way the edge's points run, as a circle is
                const double t0 = ElCLib::Parameter(elips, P.front());
                const double tm = ElCLib::Parameter(elips, P[pc.closed ? 1 : P.size() / 2]);
                const double tz = ElCLib::Parameter(elips, P.back());
                const bool forwards = pc.closed ? ahead(t0, tm) < M_PI
                                      : P.size() < 3 ? ahead(t0, tz) <= M_PI
                                      : ahead(t0, tm) <= ahead(t0, tz);
                if (!forwards) elips = gp_Elips(gp_Ax2(pnt(q), dir(q + 3).Reversed(), dir(q + 6)), q[9], q[10]);
                pc.c = new Geom_Ellipse(elips);
                pc.u0 = ElCLib::Parameter(elips, p0);
                pc.u1 = pc.closed ? pc.u0 + 2.0 * M_PI : pc.u0 + ahead(pc.u0, ElCLib::Parameter(elips, p1));
                if (!pc.closed && pc.u1 <= pc.u0 + 1.0e-12) pc.u1 += 2.0 * M_PI;
            } else if (ekind[e] == 3) {
                // A POLYLINE through the points as they are: an edge beside a region left as mesh has to meet the sides of
                // its triangles exactly, and a smooth curve through the same points bows off them between
                if (pc.closed) P.push_back(P.front());
                std::vector<gp_Pnt> Q;
                for (const gp_Pnt& q : P) {
                    if (Q.empty() || q.Distance(Q.back()) > 1.0e-12) Q.push_back(q);
                }
                if (Q.size() < 2) continue;
                const int n = (int)Q.size();
                TColgp_Array1OfPnt poles(1, n);
                TColStd_Array1OfReal knots(1, n);
                TColStd_Array1OfInteger mults(1, n);
                double run = 0.0;
                for (int k = 0; k < n; ++k) {
                    if (k > 0) run += Q[k].Distance(Q[k - 1]);
                    poles.SetValue(k + 1, Q[k]);
                    knots.SetValue(k + 1, run);
                    mults.SetValue(k + 1, (k == 0 || k == n - 1) ? 2 : 1);
                }
                pc.c = new Geom_BSplineCurve(poles, knots, mults, 1);
                pc.u0 = pc.c->FirstParameter();
                pc.u1 = pc.c->LastParameter();
            } else {
                if (pc.closed) P.push_back(P.front());
                TColgp_Array1OfPnt arr(1, (int)P.size());
                for (size_t k = 0; k < P.size(); ++k) arr.SetValue((int)k + 1, P[k]);
                GeomAPI_PointsToBSpline fit(arr, 1, 3, GeomAbs_C1, (q[0] > 0.0 ? q[0] : 4.0) * step);
                if (!fit.IsDone()) continue;
                pc.c = fit.Curve();
                pc.u0 = pc.c->FirstParameter();
                pc.u1 = pc.c->LastParameter();
            }
            pc.start = pc.c->Value(pc.u0);
            if (!pc.closed) {
                for (int end = 0; end < 2; ++end) {
                    const size_t v = (size_t)eends[2 * e + end];
                    const gp_Pnt on = pc.c->Value(end == 0 ? pc.u0 : pc.u1);
                    reach[v] = std::max(reach[v], 1.5 * on.Distance(at[v]));
                }
            }
        }
        std::vector<TopoDS_Vertex> V(nv);
        for (size_t i = 0; i < nv; ++i) B.MakeVertex(V[i], at[i], reach[i]);
        std::vector<TopoDS_Edge> E(ne);
        size_t lost_edges = 0;
        for (size_t e = 0; e < ne; ++e) {
            const Piece& pc = piece[e];
            if (pc.c.IsNull()) { ++lost_edges; continue; }
            TopoDS_Vertex v0, v1;
            if (pc.closed) {
                B.MakeVertex(v0, pc.start, 1.5 * step);
                v1 = v0;
            } else {
                v0 = V[eends[2 * e]];
                v1 = V[eends[2 * e + 1]];
            }
            BRepBuilderAPI_MakeEdge mk(pc.c, v0, v1, pc.u0, pc.u1);
            if (mk.IsDone()) E[e] = mk.Edge(); else ++lost_edges;
        }

        // HOW FAR AN EDGE AND ITS CURVE ON A SURFACE PART, measured at `at_t` and made its tolerance. The two run at the
        // same parameters by construction, so the distance at each is all there is to know. Left to the kernel's
        // SameParameter it was not measured at all - an edge given a curve on a surface keeps its flag that the two
        // agree, and 250 edges of a vaulted box stood off their walls past their tolerance - and measured by it, it took
        // 300 of 370 s on a smooth handle, approximating every curve anew. An edge that parts from the surface by a
        // hundred tolerances has swung off it, and the face is not built.
        std::vector<std::pair<TopoDS_Edge, double>> needed; // every tolerance measured, to stand after all the faces
        auto held = [&](const TopoDS_Edge& edge, const Handle(Geom_Curve)& c3, const Handle(Geom2d_Curve)& c2, const Handle(Geom_Surface)& on, const std::vector<double>& at_t) {
            double apart = 0.0;
            for (double t : at_t) {
                const gp_Pnt2d uv = c2->Value(t);
                apart = std::max(apart, c3->Value(t).Distance(on->Value(uv.X(), uv.Y())));
            }
            if (apart > 100.0 * step) return false;
            // half again over the most measured: the kernel's own check samples the edge elsewhere, and at a tenth over it
            // found edges of a smooth handle off by their tolerance to the last digit
            if (1.5 * apart > BRep_Tool::Tolerance(edge)) B.UpdateEdge(edge, 1.5 * apart);
            needed.emplace_back(edge, 1.5 * apart);
            return true;
        };
        // the face `f` on the analytic surface `surf`, its loops' curves on it found by the surface's own parameters and
        // unwrapped along each loop; a band between two loops that go round the surface opposite ways gets a seam of its
        // own. A null face where the surface is not one of those, a loop goes round it otherwise or passes by a pole, an
        // edge parts from it, or the face does not hold the point amid its region.
        auto lay_primitive_on = [&](size_t f, const Handle(Geom_Surface)& surf, bool out, std::vector<TopoDS_Edge>& touched, TopoDS_Face& face) -> bool {
            GeomAdaptor_Surface ad(surf);
            const GeomAbs_SurfaceType ty = ad.GetType();
            if (ty != GeomAbs_Cylinder && ty != GeomAbs_Cone && ty != GeomAbs_Sphere && ty != GeomAbs_Torus) return false;
            if (floops[f] == floops[f + 1]) return false;
            const double up = surf->IsUPeriodic() ? surf->UPeriod() : 0.0, vp = surf->IsVPeriodic() ? surf->VPeriod() : 0.0;
            auto uv_of = [&](const gp_Pnt& p) {
                double u = 0.0, v = 0.0;
                switch (ty) {
                    case GeomAbs_Cylinder: ElSLib::Parameters(ad.Cylinder(), p, u, v); break;
                    case GeomAbs_Cone: ElSLib::Parameters(ad.Cone(), p, u, v); break;
                    case GeomAbs_Sphere: ElSLib::Parameters(ad.Sphere(), p, u, v); break;
                    default: ElSLib::Parameters(ad.Torus(), p, u, v); break;
                }
                return gp_Pnt2d(u, v);
            };
            // brought within half a turn of `ref`
            auto turned_to = [&](gp_Pnt2d q, const gp_Pnt2d& ref) { // not `near`: a macro of the Windows headers
                if (up > 0.0) q.SetX(q.X() + up * std::round((ref.X() - q.X()) / up));
                if (vp > 0.0) q.SetY(q.Y() + vp * std::round((ref.Y() - q.Y()) / vp));
                return q;
            };
            const double pole = ty == GeomAbs_Sphere ? 0.5 * M_PI - 1.0e-3 : 1.0e300;
            // every loop's edges, sampled at their own parameters and unwrapped in the order the loop walks them
            struct Laid { size_t idx; bool fwd; std::vector<double> t; std::vector<gp_Pnt2d> uv; std::vector<double> check; };
            struct Loop { std::vector<Laid> edges; gp_Pnt2d start, end; TopoDS_Vertex first; };
            std::vector<Loop> loops;
            try {
                QYM_SIGNALS;
                for (size_t l = floops[f]; l < floops[f + 1]; ++l) {
                    Loop loop;
                    bool first = true;
                    gp_Pnt2d prev;
                    for (size_t k = lstart[l]; k < lstart[l + 1]; ++k) {
                        const int64_t it = litems[k];
                        const size_t idx = (size_t)(std::llabs(it) - 1);
                        if (idx >= ne || E[idx].IsNull()) return false;
                        double a = 0.0, z = 0.0;
                        Handle(Geom_Curve) c = BRep_Tool::Curve(E[idx], a, z);
                        if (c.IsNull() || z <= a) return false;
                        Laid one{idx, it > 0, {}, {}, {}};
                        Handle(Geom_BSplineCurve) broken = Handle(Geom_BSplineCurve)::DownCast(c);
                        if (ekind[idx] == 3 && !broken.IsNull() && broken->Degree() == 1) {
                            // a polyline: its corners and the middles between them, measured at quarters of every piece
                            for (int j = 1; j <= broken->NbKnots(); ++j) {
                                one.t.push_back(broken->Knot(j));
                                if (j < broken->NbKnots()) one.t.push_back(0.5 * (broken->Knot(j) + broken->Knot(j + 1)));
                            }
                            for (size_t j = 0; j + 1 < one.t.size(); ++j)
                                for (int q = 0; q < 4; ++q) one.check.push_back(one.t[j] + (one.t[j + 1] - one.t[j]) * q / 4.0);
                            one.check.push_back(one.t.back());
                        } else {
                            const int n = (int)std::clamp<size_t>(2 * (epstart[idx + 1] - epstart[idx]), 8, 200);
                            for (int j = 0; j < n; ++j) one.t.push_back(a + (z - a) * j / (n - 1));
                            for (int j = 0; j <= 4 * n; ++j) one.check.push_back(a + (z - a) * j / (4 * n));
                        }
                        one.uv.resize(one.t.size());
                        for (size_t j = 0; j < one.t.size(); ++j) {
                            const size_t at = one.fwd ? j : one.t.size() - 1 - j;
                            gp_Pnt2d q = uv_of(c->Value(one.t[at]));
                            if (std::abs(q.Y()) > pole) return false;
                            if (first) {
                                loop.start = q;
                                loop.first = TopoDS::Vertex(TopExp::FirstVertex(TopoDS::Edge(one.fwd ? E[idx] : E[idx].Reversed()), Standard_True));
                                first = false;
                            } else {
                                q = turned_to(q, prev);
                            }
                            one.uv[at] = q;
                            prev = q;
                        }
                        loop.edges.push_back(std::move(one));
                    }
                    if (first) return false;
                    loop.end = prev;
                    loops.push_back(std::move(loop));
                }
                // A HOLE LIES ON THE TURN OF THE LOOP AROUND IT: every loop after the first is moved by whole turns to
                // stand nearest the first loop's middle - taken near its first corner instead, a hole could fall a turn
                // aside of a loop reaching over half the surface.
                auto middle = [](const Loop& loop) {
                    double su = 0.0, sv = 0.0;
                    size_t n = 0;
                    for (const Laid& one : loop.edges)
                        for (const gp_Pnt2d& q : one.uv) {
                            su += q.X();
                            sv += q.Y();
                            ++n;
                        }
                    return gp_Pnt2d(su / (double)std::max<size_t>(n, 1), sv / (double)std::max<size_t>(n, 1));
                };
                auto move = [](Loop& loop, const gp_Vec2d& by) {
                    for (Laid& one : loop.edges)
                        for (gp_Pnt2d& q : one.uv) q.Translate(by);
                    loop.start.Translate(by);
                    loop.end.Translate(by);
                };
                // how many turns each loop makes round the surface, each way
                auto turns = [&](const Loop& loop, double period, bool along_u) {
                    if (period <= 0.0) return 0L;
                    const double d = along_u ? loop.end.X() - loop.start.X() : loop.end.Y() - loop.start.Y();
                    return std::lround(d / period);
                };
                bool round = false;
                for (const Loop& loop : loops) round |= turns(loop, up, true) != 0 || turns(loop, vp, false) != 0;
                // A BAND: two loops, each once round one way of the surface and back opposite ways. The seam runs from the
                // first loop's first corner to the second's, laid on the surface straight in its parameters, and holds
                // two curves on it a turn apart.
                gp_Vec2d shift(0.0, 0.0), turn(0.0, 0.0);
                if (!round) {
                    const gp_Pnt2d m0 = middle(loops.front());
                    for (size_t i = 1; i < loops.size(); ++i) {
                        const gp_Pnt2d mi = middle(loops[i]);
                        move(loops[i], gp_Vec2d(mi, turned_to(mi, m0)));
                    }
                } else {
                    if (loops.size() != 2) return false;
                    const long u0 = turns(loops[0], up, true), u1 = turns(loops[1], up, true);
                    const long v0 = turns(loops[0], vp, false), v1 = turns(loops[1], vp, false);
                    if (std::abs(u0) + std::abs(v0) != 1 || u0 != -u1 || v0 != -v1) return false;
                    turn = gp_Vec2d(loops[0].start, loops[0].end);
                    // the second loop begins where the seam from the end of the first reaches it
                    const gp_Pnt2d at = turned_to(loops[1].start, loops[0].end);
                    shift = gp_Vec2d(loops[1].start, at);
                }
                B.MakeFace(face, surf, step);
                auto lay_edges = [&](const Loop& loop, const gp_Vec2d& by, TopoDS_Wire& w) {
                    for (const Laid& one : loop.edges) {
                        const TopoDS_Edge& edge = E[one.idx];
                        double a = 0.0, z = 0.0;
                        Handle(Geom_Curve) c = BRep_Tool::Curve(edge, a, z);
                        Handle(TColgp_HArray1OfPnt2d) pts = new TColgp_HArray1OfPnt2d(1, (int)one.t.size());
                        Handle(TColStd_HArray1OfReal) prm = new TColStd_HArray1OfReal(1, (int)one.t.size());
                        for (size_t j = 0; j < one.t.size(); ++j) {
                            pts->SetValue((int)j + 1, one.uv[j].Translated(by));
                            prm->SetValue((int)j + 1, one.t[j]);
                        }
                        Geom2dAPI_Interpolate lay(pts, prm, Standard_False, 1.0e-12);
                        lay.Perform();
                        if (!lay.IsDone()) return false;
                        const Handle(Geom2d_BSplineCurve) on = lay.Curve(); // its own type: OCCT 7.8 does not copy it into a base handle
                        B.UpdateEdge(edge, on, face, step);
                        touched.push_back(edge);
                        if (!held(edge, c, on, surf, one.check)) return false;
                        B.Add(w, one.fwd ? edge : TopoDS::Edge(edge.Reversed()));
                    }
                    return true;
                };
                if (!round) {
                    for (const Loop& loop : loops) {
                        TopoDS_Wire w;
                        B.MakeWire(w);
                        if (!lay_edges(loop, gp_Vec2d(0.0, 0.0), w)) return false;
                        w.Closed(Standard_True);
                        B.Add(face, out ? w : TopoDS::Wire(w.Reversed()));
                    }
                } else {
                    const gp_Pnt2d p = loops[0].end, q = loops[1].start.Translated(shift);
                    if (p.Distance(q) <= 1.0e-12 || loops[0].first.IsSame(loops[1].first)) return false;
                    Handle(Geom2d_Line) there = new Geom2d_Line(p, gp_Dir2d(gp_Vec2d(p, q)));
                    const double length = p.Distance(q);
                    BRepBuilderAPI_MakeEdge mk(there, surf, loops[0].first, loops[1].first, 0.0, length);
                    if (!mk.IsDone()) return false;
                    TopoDS_Edge seam = mk.Edge();
                    if (!BRepLib::BuildCurve3d(seam, step)) return false;
                    Handle(Geom2d_Curve) back = Handle(Geom2d_Curve)::DownCast(there->Translated(-turn));
                    // the seam walked forwards lies on `there`: in a wire turned round for a face looking in, that walk is
                    // the reversed one
                    if (out) B.UpdateEdge(seam, there, back, face, step); else B.UpdateEdge(seam, back, there, face, step);
                    B.Range(seam, 0.0, length);
                    B.SameRange(seam, Standard_False);
                    B.SameParameter(seam, Standard_False);
                    BRepLib::SameParameter(seam, step);
                    if (!BRep_Tool::SameParameter(seam)) return false;
                    double a = 0.0, z = 0.0;
                    Handle(Geom_Curve) c3 = BRep_Tool::Curve(seam, a, z);
                    if (c3.IsNull()) return false;
                    std::vector<double> at_t;
                    for (int j = 0; j <= 32; ++j) at_t.push_back(a + (z - a) * j / 32.0);
                    if (!held(seam, c3, there, surf, at_t)) return false;
                    TopoDS_Wire w;
                    B.MakeWire(w);
                    if (!lay_edges(loops[0], gp_Vec2d(0.0, 0.0), w)) return false;
                    B.Add(w, seam);
                    if (!lay_edges(loops[1], shift, w)) return false;
                    B.Add(w, TopoDS::Edge(seam.Reversed()));
                    w.Closed(Standard_True);
                    B.Add(face, out ? w : TopoDS::Wire(w.Reversed()));
                }
                // the face holds the point amid its region, brought to the face's own turn
                gp_Pnt2d amid = uv_of(pnt(finside + 3 * (size_t)fsurface[f]));
                double u0 = 0.0, u1 = 0.0, v0 = 0.0, v1 = 0.0;
                BRepTools::UVBounds(face, u0, u1, v0, v1);
                amid = turned_to(amid, gp_Pnt2d(0.5 * (u0 + u1), 0.5 * (v0 + v1)));
                BRepClass_FaceClassifier inside(face, amid, 10.0 * step);
                return inside.State() != TopAbs_OUT;
            } catch (...) {
                return false;
            }
        };
        // THE CURVES A FAILED TRY LAID ON THE EDGES ARE TAKEN OFF AGAIN: they are kept by the surface, and the mending
        // that builds the face next on the same surface took them for its own.
        auto lay_primitive = [&](size_t f, const Handle(Geom_Surface)& surf, bool out) -> TopoDS_Face {
            std::vector<TopoDS_Edge> touched;
            TopoDS_Face face;
            if (lay_primitive_on(f, surf, out, touched, face)) return face;
            if (!face.IsNull())
                for (const TopoDS_Edge& edge : touched) B.UpdateEdge(edge, Handle(Geom2d_Curve)(), face, BRep_Tool::Tolerance(edge));
            return TopoDS_Face();
        };
        // THE FACES: a wire per loop, in the loop's order; a face whose surface faces inwards is built on its wires
        // turned round and then turned over, so that every face ends looking out of the body
        std::vector<TopoDS_Face> faces;
        for (size_t f = 0; f < nf; ++f) {
            if (fsurface[f] >= 0 && (size_t)fsurface[f] < ns && skind[(size_t)fsurface[f]] == 5) {
                // A FREE FORM FILLS ITS BORDER, running through the points amid it; one border only - a hole in such a face
                // is left to the mesh
                if (floops[f + 1] - floops[f] != 1 || !fpstart || !fpts) continue;
                BRepOffsetAPI_MakeFilling fill(3, 15, 2, Standard_False, 1.0e-5, 10.0 * step, 0.01, 0.1, 8, 9);
                bool whole = true;
                for (size_t k = lstart[floops[f]]; k < lstart[floops[f] + 1]; ++k) {
                    const size_t idx = (size_t)(std::llabs(litems[k]) - 1);
                    if (idx >= ne || E[idx].IsNull()) { whole = false; break; }
                    fill.Add(E[idx], GeomAbs_C0);
                }
                if (!whole) continue;
                for (size_t k = fpstart[f]; k < fpstart[f + 1]; ++k) fill.Add(pnt(fpts + 3 * k));
                try {
                    QYM_SIGNALS;
                    fill.Build();
                } catch (...) {
                    continue;
                }
                if (!fill.IsDone()) continue;
                TopoDS_Face face;
                for (TopExp_Explorer ex(fill.Shape(), TopAbs_FACE); ex.More(); ex.Next()) { face = TopoDS::Face(ex.Current()); break; }
                if (face.IsNull()) continue;
                // turned to face out: its normal at the point amid it against the mean of its triangles
                BRepAdaptor_Surface on(face);
                GeomAPI_ProjectPointOnSurf onto_face(pnt(finside + 3 * (size_t)fsurface[f]), BRep_Tool::Surface(face));
                if (onto_face.IsDone() && onto_face.NbPoints() > 0) {
                    double u = 0.0, v = 0.0;
                    onto_face.LowerDistanceParameters(u, v);
                    gp_Pnt at;
                    gp_Vec du, dv;
                    on.D1(u, v, at, du, dv);
                    gp_Vec normal = du.Crossed(dv);
                    if (face.Orientation() == TopAbs_REVERSED) normal.Reverse();
                    const double* q = sparam + NUMBERS * (size_t)fsurface[f];
                    if (normal.Dot(gp_Vec(q[3], q[4], q[5])) < 0.0) face.Reverse();
                }
                measured(f, face);
                faces.push_back(face);
                continue;
            }
            if (fsurface[f] < 0 || (size_t)fsurface[f] >= ns || S[(size_t)fsurface[f]].IsNull()) continue;
            const bool out = foutward[f] != 0;
            if (skind[(size_t)fsurface[f]] == 9) {
                // A FITTED WALL IS LAID ON ITS EDGES BY THE RULE IT WAS FITTED WITH, not by a search: every point of its region
                // sits on the surface at the coordinates the region was laid flat to, so each edge's curve on the surface is
                // those coordinates of the edge's own points, at the edge's own parameters. The search the mending of a face
                // makes found them on the far side of the surface where it runs on past the region: measured on a smooth
                // handle, faces took 5 to 20 % more or less than their regions, and 70 of 120 walls went back to the mesh.
                const size_t si = (size_t)fsurface[f];
                const double* q = sparam + NUMBERS * si;
                const gp_XYZ e1(q[2], q[3], q[4]), e2(q[5], q[6], q[7]);
                auto flat = [&](const gp_Pnt& p3) { return gp_Pnt2d((p3.XYZ().Dot(e1) - q[8]) / q[9], (p3.XYZ().Dot(e2) - q[10]) / q[11]); };
                TopoDS_Face face;
                bool whole = true;
                try {
                QYM_SIGNALS;
                B.MakeFace(face, S[si], step);
                for (size_t l = floops[f]; l < floops[f + 1] && whole; ++l) {
                    TopoDS_Wire w;
                    B.MakeWire(w);
                    for (size_t k = lstart[l]; k < lstart[l + 1]; ++k) {
                        const int64_t it = litems[k];
                        const size_t idx = (size_t)(std::llabs(it) - 1);
                        if (idx >= ne || E[idx].IsNull()) { whole = false; break; }
                        const TopoDS_Edge& edge = E[idx];
                        double a = 0.0, z = 0.0;
                        Handle(Geom_Curve) c = BRep_Tool::Curve(edge, a, z);
                        if (c.IsNull() || z <= a) { whole = false; break; }
                        // A POLYLINE LIES ON THE WALL AS A POLYLINE: the wall was laid flat by a linear map, so the image of each
                        // straight piece is straight, at the same knots. A cubic through samples of it swung off between
                        // the corners, past the edge's tolerance: measured on a vaulted box, 8 edges of its two walls.
                        Handle(Geom_BSplineCurve) broken = Handle(Geom_BSplineCurve)::DownCast(c);
                        if (ekind[idx] == 3 && !broken.IsNull() && broken->Degree() == 1) {
                            TColgp_Array1OfPnt2d poles2(1, broken->NbPoles());
                            for (int j = 1; j <= broken->NbPoles(); ++j) poles2.SetValue(j, flat(broken->Pole(j)));
                            TColStd_Array1OfReal knots2(1, broken->NbKnots());
                            TColStd_Array1OfInteger mults2(1, broken->NbKnots());
                            broken->Knots(knots2);
                            broken->Multiplicities(mults2);
                            Handle(Geom2d_BSplineCurve) on = new Geom2d_BSplineCurve(poles2, knots2, mults2, 1);
                            B.UpdateEdge(edge, on, face, step);
                            // every piece at a point each half millimetre, 16 to 256 of them: the wall bends between the points
                            // it was fitted to, and a side of 33 mm of a simplified handle measured at 17 points showed
                            // 0.004 where it stood 0.027 off - the points fell where the wall met its data
                            std::vector<double> at_t;
                            for (int j = 1; j <= knots2.Length(); ++j) {
                                const double len = j < knots2.Length() ? knots2(j + 1) - knots2(j) : 0.0;
                                const int per = (int)std::clamp(std::ceil(len / 0.5), 16.0, 256.0);
                                if (j < knots2.Length())
                                    for (int q = 0; q < per; ++q) at_t.push_back(knots2(j) + len * q / per);
                                else
                                    at_t.push_back(knots2(j));
                            }
                            if (!held(edge, c, on, S[si], at_t)) { whole = false; break; }
                            B.Add(w, it > 0 ? edge : TopoDS::Edge(edge.Reversed()));
                            continue;
                        }
                        const int n = (int)std::clamp<size_t>(2 * (epstart[idx + 1] - epstart[idx]), 8, 200);
                        Handle(TColgp_HArray1OfPnt2d) pts = new TColgp_HArray1OfPnt2d(1, n);
                        Handle(TColStd_HArray1OfReal) prm = new TColStd_HArray1OfReal(1, n);
                        for (int j = 0; j < n; ++j) {
                            const double t = a + (z - a) * j / (n - 1);
                            pts->SetValue(j + 1, flat(c->Value(t)));
                            prm->SetValue(j + 1, t);
                        }
                        Geom2dAPI_Interpolate lay(pts, prm, Standard_False, 1.0e-12);
                        lay.Perform();
                        if (!lay.IsDone()) { whole = false; break; }
                        const Handle(Geom2d_BSplineCurve) on = lay.Curve(); // its own type: OCCT 7.8 does not copy it into a base handle
                        B.UpdateEdge(edge, on, face, step);
                        std::vector<double> at_t;
                        for (int j = 0; j <= 4 * n; ++j) at_t.push_back(a + (z - a) * j / (4 * n));
                        if (!held(edge, c, on, S[si], at_t)) { whole = false; break; }
                        B.Add(w, it > 0 ? edge : TopoDS::Edge(edge.Reversed()));
                    }
                    if (!whole) break;
                    w.Closed(Standard_True);
                    B.Add(face, w);
                }
                } catch (...) {
                    whole = false;
                }
                if (!whole || crosses_itself(face)) continue;
                if (!out) face.Reverse();
                measured(f, face);
                faces.push_back(face);
                continue;
            }
            if (floops[f] == floops[f + 1]) {
                // A FACE WITH NO LOOP is its whole surface: a sphere or a torus that is the body by itself
                BRepBuilderAPI_MakeFace whole_face(S[(size_t)fsurface[f]], Precision::Confusion());
                if (!whole_face.IsDone()) continue;
                TopoDS_Face face = whole_face.Face();
                if (!out) face.Reverse();
                measured(f, face);
                faces.push_back(face);
                continue;
            }
            // A FACE OF A CYLINDER, A CONE, A SPHERE OR A TORUS IS LAID ON ITS EDGES BY ITS SURFACE'S OWN PARAMETERS,
            // unwrapped along each loop, where no loop goes round the surface: its edges stay the edges its neighbours
            // hold. Mended by ShapeFix instead, such faces came out with copies of their edges - measured on a smooth
            // handle, 33 sides no second face held, 18 of them still free after sewing, and a second whole build that put
            // 4 300 triangles of their neighbours back to the mesh. A loop that goes round (a band, a cap on a pole) is left
            // to the mending, which adds the seam and the pole.
            if (TopoDS_Face laid = lay_primitive(f, S[(size_t)fsurface[f]], out); !laid.IsNull()) {
                if (crosses_itself(laid)) continue;
                if (!out) laid.Reverse();
                measured(f, laid);
                faces.push_back(laid);
                continue;
            }
            std::string key;
            // only the rounds that ask the areas take a face kept from before: the body is built on faces made on this
            // call's edges, each edge one and the same in both faces it parts, or the faces would not pair up unsewn
            if (face_cache && faces_only) {
                const auto put = [&](const double* d, size_t n) { key.append((const char*)d, n * sizeof(double)); };
                const auto puti = [&](int64_t v) { key.append((const char*)&v, sizeof v); };
                const size_t si = (size_t)fsurface[f];
                puti(skind[si]);
                put(sparam + NUMBERS * si, NUMBERS);
                put(finside + 3 * si, 3);
                puti(foutward[f]);
                put(&tol, 1);
                for (size_t l = floops[f]; l < floops[f + 1]; ++l) {
                    puti(-1);
                    for (size_t k = lstart[l]; k < lstart[l + 1]; ++k) {
                        const int64_t it = litems[k];
                        const size_t idx = (size_t)(std::llabs(it) - 1);
                        puti(it > 0 ? 1 : -1);
                        if (idx >= ne) continue;
                        puti(ekind[idx]);
                        put(eparam + 11 * idx, 11);
                        for (int end = 0; end < 2; ++end) {
                            const int64_t v = eends[2 * idx + end];
                            // the corner's tolerance too: it comes from every edge that meets there, not from this face's
                            if (v >= 0 && (size_t)v < nv) {
                                put(vxyz + 3 * v, 3);
                                put(&reach[(size_t)v], 1);
                            } else {
                                puti(-2);
                            }
                        }
                        put(epts + 3 * epstart[idx], 3 * (epstart[idx + 1] - epstart[idx]));
                    }
                }
                const auto hit = face_cache->find(key);
                if (hit != face_cache->end()) {
                    if (out_area) out_area[f] = hit->second.second;
                    faces.push_back(hit->second.first);
                    continue;
                }
            }
            std::vector<TopoDS_Wire> W;
            bool whole = true;
            for (size_t l = floops[f]; l < floops[f + 1] && whole; ++l) {
                BRepBuilderAPI_MakeWire mw;
                for (size_t k = lstart[l]; k < lstart[l + 1]; ++k) {
                    const int64_t it = litems[k];
                    const size_t idx = (size_t)(std::llabs(it) - 1);
                    if (idx >= ne || E[idx].IsNull()) { whole = false; break; }
                    mw.Add(it > 0 ? E[idx] : TopoDS::Edge(E[idx].Reversed()));
                    if (!mw.IsDone()) { whole = false; break; }
                }
                if (whole) W.push_back(out ? mw.Wire() : TopoDS::Wire(mw.Wire().Reversed()));
            }
            if (!whole || W.empty()) continue;
            const Handle(Geom_Surface)& surf = S[(size_t)fsurface[f]];
            // A LOOP ON A CLOSED SURFACE BOUNDS TWO PIECES OF IT, and ShapeFix_Face keeps the one the loop's turn in the
            // surface's parameters points to - where a pole or the seam folds those parameters, it keeps the other.
            // Measured on the bar rounded by 2 mm: every corner's sphere came back as the rest of the sphere, 43.66 mm^2
            // of its 50.27 instead of 6.24, and the bar 268 mm^3 short. So the face has to hold the point amid its
            // triangles; it is built again without ShapeFix deciding the turn, then on its loops turned round, until
            // it does.
            GeomAPI_ProjectPointOnSurf onto(pnt(finside + 3 * (size_t)fsurface[f]), surf);
            auto build = [&](bool decide, bool turned) -> TopoDS_Face {
                BRepBuilderAPI_MakeFace mf(surf, turned ? TopoDS::Wire(W[0].Reversed()) : W[0], Standard_True);
                for (size_t i = 1; i < W.size() && mf.IsDone(); ++i) mf.Add(turned ? TopoDS::Wire(W[i].Reversed()) : W[i]);
                if (!mf.IsDone()) return TopoDS_Face();
                ShapeFix_Face fix(mf.Face());
                fix.SetPrecision(step);
                fix.FixOrientationMode() = decide ? 1 : 0;
                // not the mending of degenerate edges on a periodic surface: OCCT 7.9 falls over inside it - measured,
                // SIGSEGV in ShapeFix_Face::FixPeriodicDegenerated on Duck.glb (2 091 regions, 43 of them spheres)
                fix.FixPeriodicDegeneratedMode() = 0;
                fix.Perform();
                return fix.Face();
            };
            auto holds = [&](const TopoDS_Face& face) {
                if (face.IsNull()) return false;
                if (!onto.IsDone() || onto.NbPoints() == 0) return true;
                double u = 0.0, v = 0.0;
                onto.LowerDistanceParameters(u, v);
                // the projection's angle may stand a whole turn off the face's own range: brought into it, or the point
                // of the right piece reads as out and the wrong piece is kept - as it was on the lower four corners
                double u0 = 0.0, u1 = 0.0, v0 = 0.0, v1 = 0.0;
                BRepTools::UVBounds(face, u0, u1, v0, v1);
                auto into = [](double x, double lo, double period) {
                    double y = lo + std::fmod(x - lo, period);
                    return y < lo ? y + period : y;
                };
                if (surf->IsUPeriodic()) u = into(u, u0, surf->UPeriod());
                if (surf->IsVPeriodic()) v = into(v, v0, surf->VPeriod());
                BRepClass_FaceClassifier inside(face, gp_Pnt2d(u, v), 10.0 * step);
                return inside.State() != TopAbs_OUT;
            };
            const bool tries[3][2] = {{true, false}, {false, false}, {false, true}};
            TopoDS_Face face;
            for (const auto& t : tries) {
                TopoDS_Face tried = build(t[0], t[1]);
                if (face.IsNull()) face = tried;
                if (holds(tried)) {
                    face = tried;
                    break;
                }
            }
            if (face.IsNull()) continue;
            // A LOOP THAT CROSSES ITSELF ON ITS FACE MAKES NO FACE: the face's region goes back to the mesh in the rounds of
            // the recognition, instead of the body it would go into being refused at the end - measured on a smooth handle,
            // 20 faces (17 of them strips of cylinders) with such loops, and the node red. Only this one question is asked
            // of the face alone: the whole check of a face out of its body refuses sound rings and helices.
            if (crosses_itself(face)) continue;
            if (!out) face.Reverse();
            measured(f, face);
            faces.push_back(face);
            if (face_cache && faces_only) {
                double took = out_area ? out_area[f] : -1.0;
                face_cache->emplace(std::move(key), std::make_pair(face, took));
            }
        }
        // THE TOLERANCES MEASURED STAND: the mending of a plane face beside a wall measures the shared edge on the plane
        // alone and brought its tolerance back down - a 33 mm side of a simplified handle stood 0.027 off its wall under
        // a tolerance of 0.010, and the body was refused
        for (const auto& [edge, tol_needed] : needed)
            if (BRep_Tool::Tolerance(edge) < tol_needed) B.UpdateEdge(edge, tol_needed);
        // ONLY THE FACES, where only the area each took is asked: no loose triangles, no sewing, no solid
        if (faces_only) {
            if (out_faces) *out_faces = (uint32_t)faces.size();
            TopoDS_Compound took;
            B.MakeCompound(took);
            for (const TopoDS_Face& face : faces) B.Add(took, face);
            return qym_born(took);
        }
        // THE BODY WITHOUT SEWING. Every edge between two faces is one edge already, made once above; the regions left
        // as mesh go in as their triangles on the same corners and the same sides: an edge beside such a region is cut
        // into the mesh's own sides, and its points are the mesh's corners to the last bit. So the shell is put together
        // as it stands. Sewing it instead cut and merged those sides and laid them on the faces again by searching the
        // surface: measured on a smooth handle, 12 walls came out of it with loops crossing themselves and the body was
        // refused, and it took 12 to 16 s of a build. Sewing is left for a body whose sides do not all pair up.
        size_t direct_faces = 0; // the triangles in it
        TopoDS_Shell direct; // put together, with the sides that did not pair up left for the sewing
        if (!faces.empty() || nloose > 0) {
            struct Bits {
                size_t operator()(const std::array<uint64_t, 3>& k) const { return std::hash<uint64_t>()(k[0] * 0x9E3779B97F4A7C15ull ^ k[1] * 0xC2B2AE3D27D4EB4Full ^ k[2]); }
            };
            auto key = [](const double* q) {
                std::array<uint64_t, 3> k;
                for (int i = 0; i < 3; ++i) {
                    const double x = q[i] + 0.0; // -0 and +0 are one corner
                    std::memcpy(&k[i], &x, sizeof x);
                }
                return k;
            };
            std::unordered_map<std::array<uint64_t, 3>, TopoDS_Vertex, Bits> corner_at;
            std::map<std::pair<const void*, const void*>, TopoDS_Edge> side_of;
            // the sides of the edges beside the mesh, by their two corners
            for (size_t e = 0; e < ne; ++e) {
                if (ekind[e] != 3 || E[e].IsNull() || eends[2 * e] < 0 || epstart[e + 1] != epstart[e] + 2) continue;
                const TopoDS_Vertex& a = V[(size_t)eends[2 * e]];
                const TopoDS_Vertex& z = V[(size_t)eends[2 * e + 1]];
                corner_at.emplace(key(epts + 3 * epstart[e]), a);
                corner_at.emplace(key(epts + 3 * (epstart[e] + 1)), z);
                side_of.emplace(std::make_pair(a.TShape().get(), z.TShape().get()), E[e]);
            }
            TopoDS_Shell shell;
            B.MakeShell(shell);
            for (const TopoDS_Face& face : faces) B.Add(shell, face);
            size_t lost = 0;
            auto pair_of = [](const TopoDS_Vertex& a, const TopoDS_Vertex& b) { return std::make_pair(a.TShape().get(), b.TShape().get()); };
            std::vector<std::array<TopoDS_Vertex, 3>> tri(nloose);
            // A TRIANGLE WITH ITS THREE CORNERS ON ONE LINE makes no face, and its long side is then a side of one face
            // only: the neighbour across that side runs through the middle corner instead, as the triangle's two short
            // sides do. Two corners at one place make no side at all, and the triangle just drops out.
            std::map<std::pair<const void*, const void*>, TopoDS_Vertex> through;
            std::unordered_map<const void*, gp_Pnt> at_mesh; // where each corner stands on the mesh
            std::vector<char> flat(nloose, 0);
            for (size_t t = 0; t < nloose; ++t) {
                for (int k = 0; k < 3; ++k) {
                    const double* q = loose + 9 * t + 3 * k;
                    auto hit = corner_at.find(key(q));
                    if (hit == corner_at.end()) {
                        TopoDS_Vertex v;
                        B.MakeVertex(v, pnt(q), step);
                        hit = corner_at.emplace(key(q), v).first;
                    }
                    tri[t][k] = hit->second;
                    at_mesh.emplace(hit->second.TShape().get(), pnt(q));
                }
                const auto& c = tri[t];
                if (c[0].IsSame(c[1]) || c[1].IsSame(c[2]) || c[2].IsSame(c[0])) { flat[t] = 2; continue; }
                const gp_Pnt p0 = BRep_Tool::Pnt(c[0]), p1 = BRep_Tool::Pnt(c[1]), p2 = BRep_Tool::Pnt(c[2]);
                if (gp_Vec(p0, p1).Crossed(gp_Vec(p0, p2)).Magnitude() > 1.0e-18) continue;
                flat[t] = 1;
                // the middle corner: the one away from the longest side's ends
                const double l[3] = {p0.Distance(p1), p1.Distance(p2), p2.Distance(p0)};
                const int longest = (int)(std::max_element(l, l + 3) - l);
                const TopoDS_Vertex& a = c[longest];
                const TopoDS_Vertex& z = c[(longest + 1) % 3];
                const TopoDS_Vertex& mid = c[(longest + 2) % 3];
                through.emplace(pair_of(a, z), mid);
                through.emplace(pair_of(z, a), mid);
            }
            // the sides from `from` to `to`, through the middle corners of the flat triangles on it
            std::function<bool(const TopoDS_Vertex&, const TopoDS_Vertex&, TopoDS_Wire&, int)> run = [&](const TopoDS_Vertex& from, const TopoDS_Vertex& to, TopoDS_Wire& w, int depth) {
                if (depth < 64) {
                    auto mid = through.find(pair_of(from, to));
                    if (mid != through.end()) return run(from, mid->second, w, depth + 1) && run(mid->second, to, w, depth + 1);
                }
                auto there = side_of.find(pair_of(from, to));
                if (there != side_of.end()) {
                    B.Add(w, there->second);
                    return true;
                }
                auto back = side_of.find(pair_of(to, from));
                if (back != side_of.end()) {
                    B.Add(w, TopoDS::Edge(back->second.Reversed()));
                    return true;
                }
                // a new side runs between the mesh's own corners, where a corner shared with a face may stand off them
                // by its tolerance
                const gp_Pnt pa = at_mesh.count(from.TShape().get()) ? at_mesh[from.TShape().get()] : BRep_Tool::Pnt(from);
                const gp_Pnt pz = at_mesh.count(to.TShape().get()) ? at_mesh[to.TShape().get()] : BRep_Tool::Pnt(to);
                if (pa.Distance(pz) <= 1.0e-12) return false;
                BRepBuilderAPI_MakeEdge mk(new Geom_Line(pa, gp_Dir(gp_Vec(pa, pz))), from, to, 0.0, pa.Distance(pz));
                if (!mk.IsDone()) return false;
                side_of.emplace(pair_of(from, to), mk.Edge());
                B.Add(w, mk.Edge());
                return true;
            };
            size_t made = 0;
            for (size_t t = 0; t < nloose; ++t) {
                if (flat[t]) continue;
                TopoDS_Wire w;
                B.MakeWire(w);
                bool whole = true;
                for (int k = 0; k < 3 && whole; ++k) whole = run(tri[t][k], tri[t][(k + 1) % 3], w, 0);
                if (!whole) { ++lost; continue; }
                w.Closed(Standard_True);
                // the triangle's own plane: the plane through its wire is searched for on the curves of its sides, and a
                // side shared with a face ends in a corner off the mesh by its tolerance
                const gp_Pnt m0 = pnt(loose + 9 * t), m1 = pnt(loose + 9 * t + 3), m2 = pnt(loose + 9 * t + 6);
                BRepBuilderAPI_MakeFace mf(gp_Pln(m0, gp_Dir(gp_Vec(m0, m1).Crossed(gp_Vec(m0, m2)))), w, Standard_True);
                if (!mf.IsDone()) { ++lost; continue; }
                B.Add(shell, mf.Face());
                ++made;
            }
            // does every side pair up: two faces on it, or one face it closes on itself (a seam)
            TopTools_IndexedDataMapOfShapeListOfShape by;
            TopExp::MapShapesAndUniqueAncestors(shell, TopAbs_EDGE, TopAbs_FACE, by);
            int open = 0;
            for (int i = 1; i <= by.Extent() && lost == 0; ++i) {
                const TopoDS_Edge& side = TopoDS::Edge(by.FindKey(i));
                if (BRep_Tool::Degenerated(side)) continue;
                const TopTools_ListOfShape& on = by(i);
                if (on.Extent() == 2) continue;
                if (on.Extent() == 1 && BRep_Tool::IsClosed(side, TopoDS::Face(on.First()))) continue;
                ++open;
            }
            if (lost == 0 && open == 0) {
                if (out_faces) *out_faces = (uint32_t)(faces.size() + made);
                shell.Closed(Standard_True);
                BRepLib::UpdateTolerances(shell, Standard_False);
                ShapeFix_Solid fs;
                TopoDS_Solid solid = fs.SolidFromShell(shell);
                if (!solid.IsNull()) return qym_born(solid);
            }
            if (lost == 0) {
                direct = shell;
                direct_faces = made;
            }
        }

        // THE REGIONS LEFT AS MESH: one shell of their triangles, the edges between them shared from the start and
        // neighbours on one plane one face, so that the sewing has only the borders of the patches to meet. Every triangle
        // a face of its own made the sewing match every side of every one: on a mesh of 225 154 triangles 300 s of a 360 s
        // build. Where that way does not come through, every triangle goes in as a flat face, as before.
        std::vector<TopoDS_Shape> patches;
        if (nloose > 0 && direct.IsNull()) {
            std::vector<uint32_t> soup(3 * nloose);
            for (size_t k = 0; k < 3 * nloose; ++k) soup[k] = (uint32_t)k;
            size_t lost = 0;
            TopoDS_Shape shells = mesh_shells(loose, 3 * nloose, soup.data(), nloose, false, false, &lost);
            if (!shells.IsNull()) {
                if (out_dropped) *out_dropped += (uint32_t)lost;
                for (TopoDS_Iterator it(shells); it.More(); it.Next()) patches.push_back(it.Value());
            } else {
                for (size_t t = 0; t < nloose; ++t) {
                    const gp_Pnt a = pnt(loose + 9 * t), b = pnt(loose + 9 * t + 3), c = pnt(loose + 9 * t + 6);
                    const auto dropped = [&] { if (out_dropped) ++*out_dropped; };
                    if (gp_Vec(a, b).Crossed(gp_Vec(a, c)).Magnitude() <= 1.0e-18) { dropped(); continue; }
                    BRepBuilderAPI_MakePolygon w(a, b, c, Standard_True);
                    if (!w.IsDone()) { dropped(); continue; }
                    BRepBuilderAPI_MakeFace mf(w.Wire(), Standard_True);
                    if (mf.IsDone()) faces.push_back(mf.Face()); else dropped();
                }
            }
        }
        if (out_faces) *out_faces = (uint32_t)(faces.size() + direct_faces);
        if (faces.empty() && patches.empty() && direct.IsNull()) {
            return why("faces", lost_edges ? "no face could be built: edges of its loops could not be made" : "no face could be built on its loops"), nullptr;
        }

        // SEWN, AND A CLOSED SHELL MADE A SOLID
        BRepBuilderAPI_Sewing sew(10.0 * step);
        if (!direct.IsNull()) {
            // only the sides that did not pair up are free to the sewing: the rest are shared already
            sew.Add(direct);
        } else {
            for (const TopoDS_Face& face : faces) sew.Add(face);
            for (const TopoDS_Shape& patch : patches) sew.Add(patch);
        }
        sew.Perform();
        // the sides no second face met: how far the shell is from closing, and where to look for the hole
        if (out_free) *out_free = (uint32_t)sew.NbFreeEdges();
        // and where they are: the middle of each, to look for the hole by
        for (int k = 1; out_free_at && k <= sew.NbFreeEdges() && (size_t)k <= cap_free; ++k) {
            const TopoDS_Edge& side = TopoDS::Edge(sew.FreeEdge(k));
            double a = 0.0, z = 0.0;
            Handle(Geom_Curve) along = BRep_Tool::Curve(side, a, z);
            const gp_Pnt at = along.IsNull() ? gp_Pnt() : along->Value(0.5 * (a + z));
            out_free_at[3 * (k - 1)] = at.X();
            out_free_at[3 * (k - 1) + 1] = at.Y();
            out_free_at[3 * (k - 1) + 2] = at.Z();
        }
        TopoDS_Shape sewn = sew.SewedShape();
        if (sewn.IsNull()) return why("faces", "sewing the faces gave nothing"), nullptr;
        TopoDS_Compound bodies;
        B.MakeCompound(bodies);
        TopoDS_Shape only;
        int n = 0;
        for (TopExp_Explorer ex(sewn, TopAbs_SHELL); ex.More(); ex.Next()) {
            ShapeFix_Shell fsh(TopoDS::Shell(ex.Current()));
            fsh.SetPrecision(step);
            fsh.Perform();
            TopoDS_Shape piece_shape = fsh.Shell();
            // A SHELL THE SEWING CLOSED STAYS CLOSED: where the mending cannot orient it, it hands back a shell that no longer
            // closes - measured on gears 5, 9 and 13 of `cube_gears`, every edge shared by two faces before it, the shell
            // open after, and the body a shell of the right volume - and then the sewn one is taken
            if (!BRep_Tool::IsClosed(piece_shape) && BRep_Tool::IsClosed(ex.Current())) piece_shape = ex.Current();
            if (BRep_Tool::IsClosed(piece_shape)) {
                ShapeFix_Solid fs;
                TopoDS_Solid solid = fs.SolidFromShell(TopoDS::Shell(piece_shape));
                if (!solid.IsNull()) piece_shape = solid;
            }
            B.Add(bodies, piece_shape);
            only = piece_shape;
            ++n;
        }
        if (n == 0) {
            // NO SHELL CAME OUT OF SEWING: a single face closed on itself - a torus, a sphere - is left a face; it goes
            // into a shell of its own and, closed, becomes a solid
            TopoDS_Shell shell;
            B.MakeShell(shell);
            for (TopExp_Explorer ex(sewn, TopAbs_FACE); ex.More(); ex.Next()) B.Add(shell, ex.Current());
            shell.Closed(BRep_Tool::IsClosed(shell));
            if (shell.Closed()) {
                ShapeFix_Solid fs;
                TopoDS_Solid solid = fs.SolidFromShell(shell);
                if (!solid.IsNull()) return qym_born(solid);
            }
            return qym_born(sewn);
        }
        return qym_born(n == 1 ? only : TopoDS_Shape(bodies));
    } QYM_WHY_CATCH("faces")
    return nullptr;
}
