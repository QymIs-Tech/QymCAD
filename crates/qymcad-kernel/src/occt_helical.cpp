#include <BRepCheck.hxx>
#include <sstream>
// A HELICAL RIB OR GROOVE FROM AN EXACT PROFILE: threads, augers, and the run-outs at their ends.
//
// The heaviest single subject in the bridge and the one most often edited, so it lives on its own.
#include "occt_common.hpp"

// --- A HELICAL RIB OR GROOVE FROM AN EXACT PROFILE ---------------------------------------------
// The profile arrives from the model's core ALREADY computed to a standard (ISO 68-1, ISO 2901, DIN 405 and
// so on) and encoded as an ordinary exact profile (`encode_profile`: segments plus ARCS). All that happens
// here is sweeping it along the helix and cutting from or adding to the base.
//
// Why this is better than the older `qym_shape_thread`: that one built the profile INSIDE the kernel as a
// polygon from invented coefficients (0.48*P, a cosine of 25 points for the "round" form) — there was no
// standard behind it, and on a faceted surface neither a chamfer nor a fillet can be made afterwards. Now
// arcs stay ARCS.
//
// The profile's coordinates: x runs along the axis, y radially from the surface of radius `radius` (for a
// groove negative values go inwards, for an auger's flight positive ones go outwards).
// `mode`: 0 subtracts (a thread), 1 unites (an auger or a helical rib).
// WHAT A THREADED BODY IS MADE OF: the base with all its names, every tool with the names its recipe made.
static QymResultSpec threading(const QymShape* base, const std::vector<QymShape>& tools) {
    QymResultSpec spec{{QymOperand{base, QymCarry::Everything, "thread"}}};
    for (const QymShape& t : tools) spec.operands.push_back(QymOperand{&t, QymCarry::RecipeOnly, "thread/tool"});
    return spec;
}

extern "C" QymShape* qym_shape_helical_profile(const QymShape* base, const double* origin, const double* dir,
                                    double radius, const double* prof, size_t nprof,
                                    double length, double lead, int starts, int left, int mode,
                                    double lead_in, double lead_out, const unsigned* gnames, size_t gn,
                                    const unsigned* rnames, size_t rn, double crest_relief) {
    // One reason each: a single refusal covering five different mistakes sends whoever is fixing it to read
    // the condition instead of the answer.
    if (!base) return why("helical profile/asked", "there is no body to cut the helix into"), nullptr;
    if (radius <= 1e-6) return why("helical profile/asked", "the surface radius is zero"), nullptr;
    if (length <= 1e-6) return why("helical profile/asked", "the length along the axis is zero"), nullptr;
    if (lead <= 1e-6) return why("helical profile/asked", "the lead is zero: a helix of no pitch is a circle"), nullptr;
    if (nprof < 2) return why("helical profile/asked", "the profile has fewer than two points"), nullptr;
    try {
        QYM_SIGNALS;
        gp_Pnt o(origin[0], origin[1], origin[2]);
        gp_Dir d(dir[0], dir[1], dir[2]);
        gp_Dir refx = (Abs(d.Dot(gp_Dir(1, 0, 0))) > 0.9) ? gp_Dir(0, 1, 0) : gp_Dir(1, 0, 0);
        gp_Vec xv = gp_Vec(refx) - gp_Vec(d) * gp_Vec(refx).Dot(gp_Vec(d));
        gp_Ax3 axes(o, d, gp_Dir(xv));
        int ns = starts < 1 ? 1 : starts;
        double slope = lead / (2.0 * M_PI); if (left != 0) slope = -slope;
        // --- THE PROFILE BECOMES A POLYLINE (x along the axis, y radially from the surface) ----------
        // The arcs are split into segments by sagitta. For printing that is no loss: 0.01 mm is many times
        // finer than the nozzle, and in exchange the rest of the geometry is built EXACTLY and predictably.
        // The names arrive IN BLOCKS PER START: gnames[start * gper + the profile's edge]. Otherwise every
        // copy of the groove in a multi-start thread would carry one name, and a reference to a face of the
        // second start would silently lead to the first.
        const size_t gper = (gnames && ns > 0 && gn % (size_t)ns == 0) ? gn / (size_t)ns : 0;
        const double* pp = prof;
        size_t loops = (size_t)(*pp++);
        if (loops < 1) return why("helix/profile", "the profile holds no loop at all"), nullptr;
        std::vector<gp_Pnt2d> pts;
        // A POINT'S LINEAGE: which profile edge it came out of. An arc is split into a polyline and the
        // number of its links depends on the radius, so a name must not be given by the POINT's number —
        // that is traversal order again. The PROFILE EDGE's number does not depend on the splitting: that is
        // the recipe.
        std::vector<unsigned> pedge;
        {
            size_t nedges = (size_t)(*pp++);
            if (nedges < 2) return why("helix/profile", "the profile loop has fewer than two edges"), nullptr;
            for (size_t i = 0; i < nedges; ++i) {
                double kind = pp[0], ax = pp[1], ay = pp[2], bx = pp[3], by = pp[4], cx = pp[5], cy = pp[6], ccw = pp[7];
                pp += 9;
                if (kind == 1.0) {                                  // AN ARC becomes a polyline by sagitta
                    double r = std::hypot(ax - cx, ay - cy);
                    double t0 = atan2(ay - cy, ax - cx), t1 = atan2(by - cy, bx - cx);
                    double dt = t1 - t0;
                    if (ccw > 0.5) { while (dt <= 0.0) dt += 2.0 * M_PI; }
                    else           { while (dt >= 0.0) dt -= 2.0 * M_PI; }
                    const double sag = 0.01;                        // the sagitta, in millimetres
                    double step = (r > sag) ? 2.0 * acos(1.0 - sag / r) : M_PI / 6.0;
                    int n = (int)ceil(fabs(dt) / std::max(step, 1e-3)); if (n < 2) n = 2; if (n > 400) n = 400;
                    for (int k = 0; k < n; ++k) {
                        double t = t0 + dt * k / n;
                        pts.emplace_back(cx + r * cos(t), cy + r * sin(t));
                        pedge.push_back((unsigned)i);
                    }
                } else if (kind == 2.0) {                            // a full circle
                    int n = 64;
                    for (int k = 0; k < n; ++k) { pts.emplace_back(cx + ax * cos(2.0 * M_PI * k / n), cy + ax * sin(2.0 * M_PI * k / n)); pedge.push_back((unsigned)i); }
                } else {                                             // a segment: its start point is enough
                    pts.emplace_back(ax, ay); pedge.push_back((unsigned)i);
                }
            }
        }
        // drop coincident neighbouring points (the contour is closed)
        {
            std::vector<gp_Pnt2d> q;
            std::vector<unsigned> qe;
            for (size_t i = 0; i < pts.size(); ++i) {
                if (q.empty() || q.back().Distance(pts[i]) > 1e-9) { q.push_back(pts[i]); qe.push_back(pedge[i]); }
            }
            while (q.size() > 2 && q.front().Distance(q.back()) < 1e-9) { q.pop_back(); qe.pop_back(); }
            pts.swap(q); pedge.swap(qe);
        }
        if (pts.size() < 3) return why("helix/profile", "fewer than three points came out of the profile, which is not a section"), nullptr;
        double py0 = pts[0].Y(), py1 = pts[0].Y(), px0 = pts[0].X(), px1 = pts[0].X();
        for (const gp_Pnt2d& p : pts) {
            py0 = std::min(py0, p.Y());
            py1 = std::max(py1, p.Y());
            px0 = std::min(px0, p.X());
            px1 = std::max(px1, p.X());
        }
        // AN AUGER'S FLIGHT MUST NOT STICK OUT PAST THE END FACE. The flight's profile is symmetric about
        // the start point, so half its thickness protruded past the shaft's face (measured on a real part:
        // +2.56 mm at a thickness of 5). The profile is shifted so that the flight begins EXACTLY at the end
        // face, and the sweep is shortened by that same thickness — the length asked for stays the length of
        // the flight rather than the axis of its middle.
        double span = px1 - px0;
        if (mode == 1 && span > 1e-9 && span < length * 0.9) {
            for (gp_Pnt2d& p : pts) p.SetX(p.X() - px0);
            length -= span;
        }
        // the profile has punched through the axis: deeper than the body is thick
        for (const gp_Pnt2d& p : pts) if (radius + p.Y() <= 1e-6) return why("helix/profile", "the profile reaches past the axis: it is deeper than the radius it is cut into"), nullptr;

        double li = lead_in  > 1e-6 ? lead_in  : 0.0;
        double lo = lead_out > 1e-6 ? lead_out : 0.0;
        if (li + lo > 0.95 * length) { double sc = 0.95 * length / (li + lo); li *= sc; lo *= sc; }

        // --- THE HELICAL BODY IS BUILT FROM FACES, NOT BY A SWEEP -------------------------------
        // A sweep (`BRepOffsetAPI_MakePipeShell`) over profiles with arcs lies coarsely and unpredictably —
        // measured against Pappus's theorem: a rectangle made of straight lines passes to an accuracy of
        // 0.00%, while the same rectangle with rounded corners removes NOTHING. It cannot be trusted.
        //
        // Here everything is exact by construction, the way the kernel itself does it: EVERY point of
        // the profile traces an EXACT helix — a straight line in the parameters of the cylinder at its own
        // radius; every segment of the profile gives a RULED surface between two neighbouring helices; the
        // ends are closed by flat caps; and it is all sewn into a body. No approximation beyond the sagitta
        // chosen for the arcs.
        double utot = 2.0 * M_PI * (length / lead);                  // the total angle of twist
        // The helical motion of a profile point; `sc` is the fraction of the thread's depth (the run-out takes it to zero).
        auto point_at = [&](const gp_Pnt2d& p, double u, double sc) {
            gp_Vec radial = gp_Vec(axes.XDirection()) * (radius + p.Y() * sc);
            radial.Rotate(gp_Ax1(o, d), ((left != 0) ? -1.0 : 1.0) * u);
            return o.Translated(radial + gp_Vec(d) * (p.X() + u * fabs(slope)));
        };
        // The helix of a profile point at its own radius, over the angular stretch [u0, u1]. A LEFT-hand
        // thread turns the angle the other way, but the rise is still UPWARDS — otherwise the thread would
        // run off past the part's end face.
        double sgn = (left != 0) ? -1.0 : 1.0;
        double asc = fabs(slope);
        // The helix of a profile point over [u0, u1]. With a constant depth it is a helix on a CYLINDER;
        // where the depth melts away (the run-out) it is one on a CONE. Both are built as a straight line in
        // the surface's parameters, that is, exactly. The cone is oriented so that the radius GROWS with the
        // parameter: OCCT does not accept a negative half-angle.
        auto helix_edge = [&](const gp_Pnt2d& p, double u0, double u1, double s0, double s1) {
            double r0 = radius + p.Y() * s0, r1 = radius + p.Y() * s1;
            if (r0 <= 1e-7 || r1 <= 1e-7) return TopoDS_Edge();
            double umag = sqrt(1.0 + asc * asc);
            TopoDS_Edge e;
            if (fabs(r1 - r0) < 1e-12) {
                Handle(Geom_CylindricalSurface) cyl = new Geom_CylindricalSurface(axes, r0);
                Handle(Geom2d_Line) ln = new Geom2d_Line(gp_Pnt2d(sgn * u0, p.X() + u0 * asc), gp_Dir2d(sgn, asc));
                Handle(Geom2d_TrimmedCurve) seg = new Geom2d_TrimmedCurve(ln, 0.0, (u1 - u0) * umag);
                e = BRepBuilderAPI_MakeEdge(seg, cyl).Edge();
            } else {
                double dz = (u1 - u0) * asc;                       // the rise over this stretch
                double ang = atan2(fabs(r1 - r0), dz);
                if (ang < 1e-6 || ang > M_PI * 0.5 - 1e-6) return TopoDS_Edge();
                bool grow = r1 > r0;                               // which way the parameter takes the radius
                double zbase = grow ? u0 * asc : u1 * asc;         // the end where the radius is smaller
                double rbase = grow ? r0 : r1;
                gp_Dir cdir = grow ? d : gp_Dir(gp_Vec(d) * -1.0);
                gp_Ax3 cax(o.Translated(gp_Vec(d) * (p.X() + zbase)), cdir, axes.XDirection());
                Handle(Geom_ConicalSurface) cone = new Geom_ConicalSurface(cax, ang, rbase);
                // With the axis flipped, only the STARTING angle changes sign: the direction of travel along
                // the parameter stays the same, because the angle is measured from the axis that flipped with it.
                double asign = grow ? sgn : -sgn;
                double ustart = grow ? u0 : u1;
                double dv = dz / cos(ang);
                Handle(Geom2d_Line) ln = new Geom2d_Line(gp_Pnt2d(asign * ustart, 0.0), gp_Dir2d(sgn * (u1 - u0) / dv, 1.0));
                double len2d = dv * sqrt(1.0 + ((u1 - u0) / dv) * ((u1 - u0) / dv));
                Handle(Geom2d_TrimmedCurve) seg = new Geom2d_TrimmedCurve(ln, 0.0, len2d);
                e = BRepBuilderAPI_MakeEdge(seg, cone).Edge();
                if (!grow) e.Reverse();                            // the pieces run the same way along the axis
            }
            BRepLib::BuildCurves3d(e);
            return e;
        };
        // A LONG thread is cut into stretches BY ANGLE, two turns each. This is NOT a boolean: the stretches
        // give neighbouring faces of one and the same shell, and no seams arise by construction. The size of
        // a stretch was chosen by measurement: as a single edge over 70 turns the kernel does not build at
        // all; in stretches of 8 turns it builds but the body comes out invalid; at two turns it is valid and
        // fast (70 turns in 7.8 s, 200 turns in 19 s).
        // --- THE THREAD'S BEGINNING IS EXTENDED PAST THE END FACE ---------------------------------
        // The groove starts one turn EARLIER than the rim chosen. Its end cap is a flat wall in an axial
        // plane, and building exactly from the rim puts that wall right inside the part: the first turn comes
        // out wider than the rest and breaks off vertically, and a matching pair cannot be screwed together.
        // Starting earlier moves the cap INTO THE AIR past the end face, and the thread begins exactly at the
        // face. In case the thread is placed not from an end face but in the middle of a body, this overrun
        // also FADES AWAY in depth.
        //
        // --- THE RUN-OUT AT THE FAR END ----------------------------------------------------------
        // There the groove runs into the body, and breaking it off with a wall is not allowed — the thread's
        // depth melts away smoothly instead. It is not taken all the way to zero: the section would degenerate
        // to a point and the body would not build.
        const double s_min = 0.02;
        // The overrun only makes sense for A CUT: on an auger a flight begun before the shaft would hang in the air
        // as a stretch of its own. EACH END is decided separately, by asking the part itself whether the
        // thread comes out into the open. If it does, the turn is extended past the face so that the groove's
        // end cap stays in the air (otherwise the first or last turn breaks off at a vertical wall). If the
        // thread runs into the body, what is needed there is a run-out rather than an overrun. A thread
        // passing right through the part gets a clean exit at BOTH ends.
        bool inner_probe = py1 > -py0;
        double depth_prof = inner_probe ? py1 : -py0;             // the thread's depth taken from the profile's extent
        double r_probe = inner_probe ? radius + (py1 * 0.5) : radius + (py0 * 0.5);
        // THE WHOLE PROFILE MOVES IN when the clearance did not fit into the groove's width.
        //
        // The width is capped by the pitch - a groove wider than that eats the turn beside it - so a clearance
        // larger than the cap has to come from somewhere else. It comes from moving the groove itself: the
        // flanks go in on a shaft and out in a hole, which is where a nut actually rubs.
        //
        // Measured, in this order. A ring that took the crest down and left the flanks alone got an M10x1.5
        // pair from 10.2 mm^3 of interference to 5.3 - better, not fixed. Moving the whole groove took it to
        // nothing at all. The ring was then removed and the pair stayed at nothing: it was doing no work, and
        // it brought its own trouble - its inner cylinder coincided with the crest, and on coinciding surfaces
        // a boolean yields a body the tessellator tears apart (an empty section cap over the threaded zone),
        // while with no lead-in cones beside it the cut came back empty altogether.
        //
        // The land between the grooves keeps the stock's own diameter, which is right: a blank is not turned
        // down for a clearance fit, the thread is simply cut a little deeper and a little narrower.
        //
        // THE SIDE COMES FROM THE PROFILE, not from `mode`. `mode` tells a thread from an auger; which way the
        // groove points is `inner_probe`. Taking one for the other narrowed a nut's bore instead of opening it
        // and made the pair bind worse than before the ring: 7.7 mm^3 against 5.3.
        //
        // The lambdas above capture `radius` by reference and are called further down, so moving the surface
        // here moves the whole groove with it.
        if (crest_relief > 1.0e-6) {
            radius = inner_probe ? radius + crest_relief : radius - crest_relief;
            if (radius <= 1e-6) return why("helix/asked", "the surface radius is zero"), nullptr;
        }
        BRepClass3d_SolidClassifier cls(base->shape);
        auto material_at = [&](double z) {
            gp_Vec rad = gp_Vec(axes.XDirection()) * r_probe;
            cls.Perform(o.Translated(rad + gp_Vec(d) * z), 1.0e-6);
            return cls.State() == TopAbs_IN;
        };
        // Each end is either OPEN (the thread comes out at a face) or RUNS INTO the body, and the two are
        // treated differently:
        //   * an open end with no run-out asked for: the turn is extended PAST the face, so the groove's end
        //     cap goes into the air and the turn does not break off at a vertical wall;
        //   * an open end WITH a run-out: a countersink cone plus a fading turn, so a nut can be started from
        //     that side (a through nut needs this at BOTH ends, to be turned either way);
        //   * an end inside the body: only the depth fades, since a cone there would be a recess in the shaft.
        // How far past the thread's boundary one has to GO to leave the material. On a plain end face that is
        // zero, but if the face carries a CHAMFER the thread's rim lies at its base and the material extends
        // for the whole chamfer: without this allowance the turn broke off exactly at the junction with the
        // chamfer and the junction came out ragged. The search steps by an eighth of a turn, no further than
        // three turns.
        // The end face is looked for ONLY NEARBY: within the profile's depth and a quarter of a turn (plus an
        // allowance for a chamfer, whose axial size is rarely greater than the thread's depth). Searching far
        // afield makes "the part ends somewhere" pass for an open end: on the blind end of a test shaft the
        // material ran out after 10 mm, a three-turn search found that, and the turn was extended past it,
        // removing 27% too much.
        double look = 2.0 * depth_prof + 0.25 * lead;
        auto exit_distance = [&](double from, double sign) {
            const int steps = 16;
            for (int i = 1; i <= steps; ++i) {
                double dz = look * i / steps;
                if (!material_at(from + sign * dz)) return dz;
            }
            return -1.0;                                          // never came out: the thread runs into the body
        };
        double out_start = mode == 0 ? exit_distance(0.0, -1.0) : -1.0;
        double out_end = mode == 0 ? exit_distance(length, 1.0) : -1.0;
        bool start_open = out_start >= 0.0;
        bool end_open = out_end >= 0.0;
        // THE OVERRUN BELONGS TO EVERY OPEN END, whether or not a run-out was asked for.
        //
        // It used to be granted only when the run-out was ZERO, and asking for one therefore switched it off
        // and switched on a fading depth instead: the groove melted away at the face, the last turn broke off
        // against a vertical wall, and a countersink cone was laid over the top of that. Together they erased
        // the thread over the whole run-out length. Reported plainly: setting a run-out breaks the entry.
        //
        // A lathe does not work that way. The tool does not float out of the cut - it either runs into a
        // relief groove or leaves past the end face. So an open end always carries the turn out into the air,
        // and the run-out asked for there means A CHAMFER, which is cut separately below.
        double ext = start_open ? out_start + lead : 0.0;
        double ext_end = end_open ? out_end + lead : 0.0;
        // THREAD RELIEF GROOVE (GOST 10549). If the thread ends INSIDE the body, a fading turn will not do:
        // reported behaviour is that a printed pair, tightened all the way home, tears out the incomplete
        // turns of the mating part. What is really cut there is a CIRCULAR groove one profile depth deep: the
        // thread breaks off inside it, the turn's end face is full, and the parts butt face to face. The
        // groove's width is the run-out that was asked for.
        bool relief_start = !start_open && li > 1e-9;
        bool relief_end = !end_open && lo > 1e-9;
        // WHERE THE GROOVE ITSELF ENDS. Where the relief is cut the turn has no business entering it: it has
        // to end EXACTLY AT THE ENTRY to the relief, or a tail of the turn is left behind it — reported
        // behaviour is "about half a turn's worth of stock after the relief".
        // ORDER OF WORK: the turn is first built STRAIGHT THROUGH the relief zone, and the relief then cuts it
        // away, which makes the entry into it smooth. Breaking the turn off at the relief's edge leaves a
        // shoulder wall on the crest (reported behaviour). The relief also has to eat the groove's TAIL, which
        // sticks out past the end of the sweep by half the profile's width, or a piece is left behind it.
        double half_prof = 0.5 * (px1 - px0);
        // The turn runs to `length - half the profile's width`: its material then ends EXACTLY at the length
        // mark, and the relief [length - run-out, length] eats its tail entirely while staying within the
        // asked-for length.
        double sweep_a = relief_start ? half_prof : 0.0;
        double sweep_b = relief_end ? std::max(length - half_prof, 0.0) : length;
        auto depth_scale = [&](double z) {
            double sc = 1.0;
            if (ext > 1e-9 && z < sweep_a)     sc = std::min(sc, s_min + (1.0 - s_min) * (1.0 + (z - sweep_a) / ext));
            if (ext_end > 1e-9 && z > sweep_b) sc = std::min(sc, s_min + (1.0 - s_min) * (1.0 - (z - sweep_b) / ext_end));
            // NO FADING INSIDE THE MATERIAL. The depth used to melt away over the run-out length, which is
            // what a cutter cannot do: it leaves a turn that grows shallower and shallower until it ends in a
            // wall, and nothing mates with that. Inside the thread the groove keeps its full depth; the two
            // branches above only taper the tool OUTSIDE the body, where nothing is being cut.
            return std::min(1.0, std::max(sc, s_min));
        };
        // Stretch boundaries by angle: the run-out zones are split finely (for smoothness), the middle two
        // turns at a time.
        std::vector<double> ub{(sweep_a - ext) / asc};
        auto push_span = [&](double za, double zb, double turns_per) {
            int n = (int)ceil(((zb - za) / lead) / turns_per); if (n < 1) n = 1;
            for (int i = 1; i <= n; ++i) ub.push_back((za + (zb - za) * i / n) / asc);
        };
        double z_in = (!relief_start && li > 1e-9) ? std::min(li, sweep_b) : sweep_a;
        double z_out = (!relief_end && lo > 1e-9) ? std::max(length - lo, z_in) : sweep_b;
        // Splitting the run-out zones: finer than 0.35 of a turn buys nothing — the same smoothness, but more
        // faces on the tool and a dearer boolean (measured: 0.15 of a turn -> 3.49 s, 0.35 -> 3.23 s, same
        // volume).
        const double fine = 0.35;
        if (ext > 1e-9) push_span(sweep_a - ext, sweep_a, fine);
        if (z_in > sweep_a + 1e-9) push_span(sweep_a, z_in, fine);
        if (z_out > z_in + 1e-9) push_span(z_in, z_out, 2.0);
        if (sweep_b > z_out + 1e-9) push_span(z_out, sweep_b, fine);
        if (ext_end > 1e-9) push_span(sweep_b, sweep_b + ext_end, fine);
        int nchunk = (int)ub.size() - 1;
        if (nchunk < 1) return why("helix/spans", "the helix came out shorter than a single span to build"), nullptr;
        std::vector<std::vector<TopoDS_Edge>> rails(nchunk);
        for (int c = 0; c < nchunk; ++c) {
            double u0 = ub[c], u1 = ub[c + 1];
            double s0 = depth_scale(u0 * asc), s1 = depth_scale(u1 * asc);
            rails[c].reserve(pts.size());
            for (const gp_Pnt2d& p : pts) {
                TopoDS_Edge e = helix_edge(p, u0, u1, s0, s1);
                if (e.IsNull()) return why("helix/rail", "a rail of the helical surface could not be built from the profile"), nullptr;
                rails[c].push_back(e);
            }
        }
        // The face between two neighbouring helices is a RULED surface on THE SAME edges. It must not be
        // built by lofting: a loft makes ITS OWN boundary edges, each with its own approximation, so
        // neighbouring faces stop resting on one and the same curve — the sewn shell comes out geometrically
        // inconsistent, and a boolean on it behaves wildly (it removed MORE than the tool's own volume).
        // BRepFill::Face reuses the edges themselves, so the seam is exact.
        BRepBuilderAPI_Sewing sew(1.0e-6);
        // A TURN FACE IS NAMED AFTER ITS PROFILE SEGMENT, NOT AFTER THE ORDER OF TRAVERSAL.
        //
        // The helical body is built face by face by hand, so the origin of each is known EXACTLY: face `i`
        // grew out of profile segment `i`. That is the recipe, and it depends neither on the number of turns
        // nor on the order in which the kernel later enumerates the subshapes. Without names, 72 of the
        // thread's 78 faces stayed unnamed (measured), and everything that stood on them fell off.
        std::vector<std::pair<TopoDS_Face, unsigned>> groove_named;   // face -> PROFILE EDGE NUMBER
        for (int c = 0; c < nchunk; ++c) {
            for (size_t i = 0; i < pts.size(); ++i) {
                TopoDS_Face f = BRepFill::Face(rails[c][i], rails[c][(i + 1) % pts.size()]);
                if (f.IsNull()) return why("helix/face", "two neighbouring rails did not span a face between them"), nullptr;
                sew.Add(f);
                if (gper > 0 && i < pedge.size() && pedge[i] < gper) groove_named.emplace_back(f, pedge[i]);
            }
        }
        for (int end = 0; end < 2; ++end) {                          // flat caps at the ends
            double u = end == 0 ? ub.front() : ub.back();
            BRepBuilderAPI_MakePolygon mp;
            for (const gp_Pnt2d& p : pts) mp.Add(point_at(p, u, depth_scale(u * asc)));
            mp.Close();
            if (!mp.IsDone()) return why("helix/cap", "the end cap of the helical body did not close into a loop"), nullptr;
            BRepBuilderAPI_MakeFace mf(mp.Wire(), Standard_True);
            if (!mf.IsDone()) return why("helix/cap", "the closed loop of the end cap did not become a face"), nullptr;
            sew.Add(mf.Face());
        }
        sew.Perform();
        TopoDS_Shape sewn = sew.SewedShape();
        if (sewn.IsNull()) return why("helix/sew", "the faces of the helical body did not sew into a shell"), nullptr;
        // sewing RECREATES the faces, so the names are carried over through its own map, not by matching
        TopTools_DataMapOfShapeInteger groove_ids;
        for (const auto& [f, edge] : groove_named) {
            const TopoDS_Shape& img = sew.IsModified(f) ? sew.Modified(f) : (const TopoDS_Shape&)f;
            if (!img.IsNull() && !groove_ids.IsBound(img)) groove_ids.Bind(img, (int)gnames[edge]);
        }
        TopoDS_Shape groove;
        for (TopExp_Explorer ex(sewn, TopAbs_SHELL); ex.More(); ex.Next()) {
            TopoDS_Shell sh = TopoDS::Shell(ex.Current());
            if (!BRep_Tool::IsClosed(sh)) continue;               // an open shell will not become a solid
            // MAKING THE FACE ORIENTATIONS AGREE is a required step, not a decoration. Sewing does not
            // align them by itself: the shape comes out right (the volume from the mesh agrees with the
            // calculation to within 0.05%), but some of the faces look inwards. The kernel then computes the
            // solid's volume wrongly (796 instead of 895) and the boolean removes too much — the cut carried
            // away TWICE as much as the tool itself occupies, and on some profiles it ADDED material instead
            // of removing it.
            ShapeFix_Shell fix(sh);
            fix.FixFaceOrientation(sh);
            TopoDS_Shell fixed = fix.Shell();
            if (fixed.IsNull()) fixed = sh;
            BRepBuilderAPI_MakeSolid ms(fixed);
            if (!ms.IsDone()) continue;
            TopoDS_Solid sol = ms.Solid();
            BRepLib::OrientClosedSolid(sol);
            // TOLERANCE TOUCH-UP: on a long thread (70 turns, nine stretches by angle) the joins between
            // stretches part by fractions of a micron, and without evening the tolerances out the finished
            // solid comes out invalid.
            ShapeFix_Solid fs(sol);
            fs.Perform();
            groove = fs.Shape().IsNull() ? TopoDS_Shape(sol) : fs.Shape();
            // WHICH SIDE THE SHELL FACES IS DECIDED FROM THE MESH. The direction of the axis flips the
            // profile's coordinate system and the solid comes out inside out: in a boolean it behaves as its
            // own complement and removes NOTHING (reported behaviour, with the rim picked from above and the
            // axis pointing down: along +Z it removed 12.6 cm^3, along -Z nothing). `VolumeProperties` cannot
            // be trusted to judge this: on helical surfaces that integrator is off by whole multiples, and its
            // sign came out positive in both cases. The signed volume of the triangulation is computed
            // honestly and costs next to nothing on a coarse mesh.
            {
                BRepTools::Clean(groove);
                BRepMesh_IncrementalMesh im(groove, std::max(radius * 0.05, 0.05), Standard_False, 0.6, Standard_True);
                im.Perform();
                double vsig = 0.0;
                for (TopExp_Explorer ex2(groove, TopAbs_FACE); ex2.More(); ex2.Next()) {
                    TopoDS_Face f = TopoDS::Face(ex2.Current());
                    TopLoc_Location L;
                    Handle(Poly_Triangulation) tr = BRep_Tool::Triangulation(f, L);
                    if (tr.IsNull()) continue;
                    bool rev = f.Orientation() == TopAbs_REVERSED;
                    for (int t = 1; t <= tr->NbTriangles(); ++t) {
                        int i1, i2, i3;
                        tr->Triangle(t).Get(i1, i2, i3);
                        gp_Pnt A = tr->Node(i1).Transformed(L), B = tr->Node(i2).Transformed(L), C = tr->Node(i3).Transformed(L);
                        if (rev) std::swap(B, C);
                        vsig += (A.X() * (B.Y() * C.Z() - C.Y() * B.Z()) - A.Y() * (B.X() * C.Z() - C.X() * B.Z()) + A.Z() * (B.X() * C.Y() - C.X() * B.Y())) / 6.0;
                    }
                }
                if (vsig < 0.0) groove.Reverse();
                BRepTools::Clean(groove);
            }
            break;
        }
        if (groove.IsNull()) return why("helix/groove", "no attempt produced a helical body to cut with"), nullptr;
        TopTools_ListOfShape args, tools;
        args.Append(base->shape);
        tools.Append(groove);
        std::vector<std::pair<TopoDS_Shape, TopTools_DataMapOfShapeInteger>> tool_named;
        if (!groove_ids.IsEmpty()) tool_named.emplace_back(groove, groove_ids);
        for (int k = 1; k < ns; ++k) {                               // multiple starts: copies every 360 deg / ns
            gp_Trsf r2; r2.SetRotation(gp_Ax1(o, d), 2.0 * M_PI * k / ns);
            BRepBuilderAPI_Transform xf(groove, r2, Standard_True);
            tools.Append(xf.Shape());
            // a copy has DIFFERENT faces; each name is taken from its own block and carried over by the
            // transformation's map
            if (gper > 0) {
                TopTools_DataMapOfShapeInteger cp;
                for (const auto& [src_f, edge] : groove_named) {
                    const TopoDS_Shape& sewn_f = sew.IsModified(src_f) ? sew.Modified(src_f) : (const TopoDS_Shape&)src_f;
                    TopoDS_Shape img = xf.ModifiedShape(sewn_f);
                    if (img.IsNull() || cp.IsBound(img)) continue;
                    cp.Bind(img, (int)gnames[(size_t)k * gper + (size_t)edge]);   // same edge, block of start k
                }
                if (!cp.IsEmpty()) tool_named.emplace_back(xf.Shape(), cp);
            }
        }
        // -- LEAD-IN AND RUN-OUT: A CONICAL CHAMFER, CUT SEPARATELY ------------------------------------
        // The former run-out reduced the turn's DEPTH towards the end face, which left the FULL diameter
        // there — a nut cannot be started on an end face like that (reported behaviour: "there is simply no
        // lead-in"). A lead-in is made with a CHAMFER: a cone from the root to the outer diameter cuts the
        // crests off the first few turns.
        //
        // The rings go in as a SEPARATE operation rather than together with the groove: they intersect it,
        // and a BOP on intersecting tools falls apart. Measured: with the axis pointing DOWN (the rim picked
        // on the top face, which is the usual case) the cut stopped removing anything at all (0 instead of
        // 12.4 cm^3), even though the tool itself was valid and its intersection with the body came out
        // right.
        // THE FACES OF THE RELIEF AND THE LEAD-IN ARE NAMED BY RECIPE. The ring and the cone are built here,
        // out of a cylinder and a cone, so the origin of every face is known exactly: which tool (relief or
        // lead-in), which end (the thread's start or finish) and which surface. The order of traversal has
        // nothing to do with it. Each ring used to add 3 unnamed faces to the body — including the very
        // lead-in chamfer a person sees and puts a fillet on.
        // Name layout: rnames[((tool * 2 + end) * 4) + surface],
        // tool 0 = relief, 1 = lead-in; surface 0 = near plane, 1 = far plane,
        // 2 = cylinder, 3 = cone.
        std::vector<std::pair<TopoDS_Shape, TopTools_DataMapOfShapeInteger>> chamfers;
        auto name_relief = [&](const TopoDS_Shape& tool, int which, int end_i) {
            TopTools_DataMapOfShapeInteger ids;
            if (!rnames || rn < 16 || tool.IsNull()) return ids;
            const unsigned* blk = rnames + ((size_t)(which * 2 + end_i) * 4);
            std::vector<std::pair<double, TopoDS_Shape>> planes;
            for (TopExp_Explorer ex(tool, TopAbs_FACE); ex.More(); ex.Next()) {
                BRepAdaptor_Surface as(TopoDS::Face(ex.Current()), Standard_False);
                switch ((int)as.GetType()) {
                    case GeomAbs_Cylinder: ids.Bind(ex.Current(), (int)blk[2]); break;
                    case GeomAbs_Cone:     ids.Bind(ex.Current(), (int)blk[3]); break;
                    case GeomAbs_Plane: {
                        // the two annular planes are told apart by their POSITION ALONG THE AXIS, which is a
                        // property of the construction itself (near face and far face), not of the order of
                        // enumeration
                        gp_Pnt lp = as.Plane().Location();
                        planes.emplace_back(gp_Vec(o, lp).Dot(gp_Vec(d)), ex.Current());
                        break;
                    }
                    default: break;
                }
            }
            std::sort(planes.begin(), planes.end(), [](const auto& x, const auto& y) { return x.first < y.first; });
            for (size_t k = 0; k < planes.size(); ++k) ids.Bind(planes[k].second, (int)blk[k == 0 ? 0 : 1]);
            return ids;
        };
        bool inner = inner_probe;                                     // the profile goes INTO THE WALL => internal
        double r_root = inner ? radius + depth_prof : radius - depth_prof;
        // The countersink chamfer is put on EVERY OPEN end where a run-out was asked for. A through nut needs
        // it on both sides, or it cannot be started from the second one. Where the thread runs into the body
        // there is no cone: it would be a recess rather than a lead-in, and only the fading turn depth works
        // there (see depth_scale above).
        // RELIEF GROOVES on blind ends: a ring one profile depth deep and as wide as the run-out asked for.
        // The turn's end face hides inside it, so no wall is left and the mating part butts on its face
        // rather than on a turn.
        if (mode == 0 && r_root > 1e-6) {
            for (int end = 0; end < 2; ++end) {
                double L = end == 0 ? li : lo;
                bool relief_here = end == 0 ? relief_start : relief_end;
                if (L <= 1e-9 || !relief_here) continue;
                double zb = end == 0 ? 0.0 : length - L;
                double LL = L;
                gp_Ax2 rax(o.Translated(gp_Vec(d) * zb), d);
                // The relief goes A LITTLE DEEPER than the thread's root. Exactly at the root will not do: the
                // relief's cylinder would coincide with the root surface, and on coinciding surfaces a boolean
                // yields a solid the kernel calls valid but the tessellator tears apart — the volume from the
                // mesh came out LARGER than the stock, and on screen that is the holes one sees. In metal the
                // relief is likewise cut deeper than the thread.
                double eps_r = std::max(0.02 * depth_prof, 1.0e-3);
                double r_rel = inner ? r_root + eps_r : r_root - eps_r;
                TopoDS_Shape ring;
                if (inner) {
                    ring = BRepPrimAPI_MakeCylinder(rax, r_rel, LL).Shape();  // in a hole: outwards as far as the root
                } else {
                    TopoDS_Shape big = BRepPrimAPI_MakeCylinder(rax, radius * 2.0 + 1.0, LL).Shape();
                    TopoDS_Shape core = BRepPrimAPI_MakeCylinder(rax, r_rel, LL).Shape();
                    if (big.IsNull() || core.IsNull()) continue;
                    BRepAlgoAPI_Cut rr;
                    qym_boolean(rr, big, core);
                    if (!rr.IsDone() || rr.Shape().IsNull()) continue;
                    ring = rr.Shape();
                }
                if (!ring.IsNull()) chamfers.emplace_back(ring, name_relief(ring, 0, end));
            }
            for (int end = 0; end < 2; ++end) {
                double L = end == 0 ? li : lo;
                bool open_here = end == 0 ? start_open : end_open;
                if (L <= 1e-9 || !open_here) continue;   // inside the body a cone would be a recess, not a lead-in
                gp_Pnt p0 = o.Translated(gp_Vec(d) * (end == 0 ? 0.0 : length));
                gp_Ax2 ax2(p0, gp_Dir(gp_Vec(d) * (end == 0 ? 1.0 : -1.0)));
                // the cone: root diameter right at the end face, outer diameter one lead-in length along
                TopoDS_Shape cone = BRepPrimAPI_MakeCone(ax2, r_root, radius, L).Shape();
                if (cone.IsNull()) continue;
                if (inner) {
                    chamfers.emplace_back(cone, name_relief(cone, 1, end)); // in a hole the cone itself is removed
                } else {
                    double rbig = std::max(radius, r_root) * 2.0 + 1.0;
                    TopoDS_Shape cyl = BRepPrimAPI_MakeCylinder(ax2, rbig, L).Shape();
                    if (cyl.IsNull()) continue;
                    BRepAlgoAPI_Cut ring;
                    qym_boolean(ring, cyl, cone);
                    if (ring.IsDone() && !ring.Shape().IsNull()) chamfers.emplace_back(ring.Shape(), name_relief(ring.Shape(), 1, end));
                }
            }
        }
        // the groove and the reliefs give the names their recipe made; the base gives all of its own
        // NAMES FROM EVERY OPERAND COME BEFORE POSITIONAL NUMBERS ARE HANDED OUT, and the pieces of split faces are
        // recorded: the groove cuts the base cylinder into dozens of strips (the crests between turns), and without
        // "piece k of face N" they stayed positional - 10 unnamed cylinders out of 33 faces on one part.
        std::vector<QymShape> tools_named;
        for (const auto& [ts, tids] : tool_named) tools_named.push_back(QymShape{ts, tids, {}, {}, {}, {}, {}});
        QymShape* q = nullptr;
        if (mode == 1) {
            BRepAlgoAPI_Fuse algo;
            qym_boolean_many(algo, args, tools);
            if (!algo.IsDone() || algo.Shape().IsNull()) return why("helix/subtract", "the helical grooves did not cut out of the part"), nullptr;
            QymBuilderHistory history(algo);
            q = qym_result(history, algo.Shape(), threading(base, tools_named));
        } else {
            BRepAlgoAPI_Cut algo;
            qym_boolean_many(algo, args, tools);
            if (!algo.IsDone() || algo.Shape().IsNull()) return why("helix/unite", "the helical ribs did not unite with the part"), nullptr;
            QymBuilderHistory history(algo);
            q = qym_result(history, algo.Shape(), threading(base, tools_named));
        }
        for (const auto& [ch, ch_ids] : chamfers) {
            BRepAlgoAPI_Cut cut;
            qym_boolean(cut, q->shape, ch);
            if (!cut.IsDone() || cut.Shape().IsNull()) continue;     // the chamfer failed; the thread stays
            const QymShape relief{ch, ch_ids, {}, {}, {}, {}, {}};
            QymBuilderHistory history(cut);
            QymResultSpec spec{{QymOperand{q, QymCarry::Everything, "thread"}, QymOperand{&relief, QymCarry::RecipeOnly, "thread/relief"}}};
            // THE PIECE RECORDS SURVIVE THE RELIEF CUT. The model layer reads them AFTER the whole operation,
            // and the relief is a separate cut that used to start from a clean slate — the entire ancestry of
            // the pieces, earned during the threading, was lost at the last step.
            spec.recipe = [&](QymShape& n, QymCounters&) {
                for (TopExp_Explorer ex(q->shape, TopAbs_FACE); ex.More(); ex.Next()) {
                    const TopoDS_Shape& f = ex.Current();
                    if (!q->fsplit_of.IsBound(f)) continue;
                    const int of = q->fsplit_of.Find(f), ix = q->fsplit_idx.IsBound(f) ? q->fsplit_idx.Find(f) : 1;
                    const TopTools_ListOfShape& mod = cut.Modified(f);
                    if (mod.IsEmpty()) {
                        if (!n.fsplit_of.IsBound(f) && n.fids.IsBound(f)) { n.fsplit_of.Bind(f, of); n.fsplit_idx.Bind(f, ix); }
                        continue;
                    }
                    for (TopTools_ListIteratorOfListOfShape it(mod); it.More(); it.Next())
                        if (!n.fsplit_of.IsBound(it.Value())) { n.fsplit_of.Bind(it.Value(), of); n.fsplit_idx.Bind(it.Value(), ix); }
                }
            };
            QymShape* n = qym_result(history, cut.Shape(), spec);
            delete q;
            q = n;
        }
        return q;
    } QYM_WHY_CATCH("helical profile")
    return nullptr;
}

extern "C" QymShape* qym_shape_thread(const QymShape* base, const double* origin, const double* dir,
                           double radius, double length, double pitch, double angle_deg,
                           double depth, int starts, int left, int internal,
                           int form, double clearance_crest, double clearance_root,
                           double lead_in, double lead_out) {
    // One reason each, as for the helical profile below: five conditions behind one refusal send whoever is
    // fixing it to read the condition instead of the answer.
    if (!base) return why("thread/asked", "there is no body to cut the thread into"), nullptr;
    if (radius <= 1e-6) return why("thread/asked", "the surface radius is zero"), nullptr;
    if (pitch <= 1e-6) return why("thread/asked", "the pitch is zero: a thread of no pitch is a groove round the body"), nullptr;
    if (length <= 1e-6) return why("thread/asked", "the threaded length along the axis is zero"), nullptr;
    if (depth <= 1e-6) return why("thread/asked", "the profile depth is zero: the thread would cut nothing"), nullptr;
    try {
        QYM_SIGNALS;
        gp_Pnt o(origin[0], origin[1], origin[2]);
        gp_Dir d(dir[0], dir[1], dir[2]);
        gp_Dir refx = (Abs(d.Dot(gp_Dir(1, 0, 0))) > 0.9) ? gp_Dir(0, 1, 0) : gp_Dir(1, 0, 0);
        gp_Vec xv = gp_Vec(refx) - gp_Vec(d) * gp_Vec(refx).Dot(gp_Vec(d)); // refx ⟂ d
        gp_Ax3 axes(o, d, gp_Dir(xv));
        int ns = starts < 1 ? 1 : starts;
        double lead = pitch * ns;                    // axial travel per revolution of a multi-start thread
        // Lead-in and run-out: instead of a single section, SEVERAL sections are laid along the spine (a
        // loft) — at the ends the turn's DEPTH goes to about 0 (the root rises to the surface) while width and
        // pitch stay the same, so the turn fades away like a real thread run-out. The groove is built at FULL
        // depth (which is reliable) and then TRIMMED by a surface of revolution whose depth falls linearly to
        // 0 at the ends (a boolean common). Only reliable operations are used (revolve plus common), with no
        // brittle multi-section pipe or spline laws, which used to break on short run-outs.
        // SEGMENTATION: a pipe-shell over a long helix (more than about 30 turns) breaks or is dreadfully
        // slow. The thread is split into segments of about 18 turns at most (a short helix builds quickly and
        // RELIABLY), the profile is placed at the start of each segment (the helix's angle at z0 plus a shift
        // along the axis), and the grooves are cut out of the base ONE AT A TIME (a cut is more reliable than
        // fusing thin helical grooves). The segments overlap by about a turn, so the seam has no gap.
        double slope = lead / (2.0 * M_PI); if (left != 0) slope = -slope;
        double turns_total = length / lead;
        int nseg = (int)ceil(turns_total / 18.0); if (nseg < 1) nseg = 1;
        double seg_len = length / nseg;
        double ov = std::min(lead, seg_len * 0.5);
        // SMOOTH RUN-OUT: at the ends the turn's depth fades to 0 over lead_in / lead_out mm (see
        // runout_law). It is clamped so the ramps do not overlap (at most 0.95 of the length). It is built
        // into the pipe-shell as a homothety law, with no booleans.
        double li = lead_in  > 1e-6 ? lead_in  : 0.0;
        double lo = lead_out > 1e-6 ? lead_out : 0.0;
        if (li + lo > 0.95 * length) { double sc = 0.95 * length / (li + lo); li *= sc; lo *= sc; }
        std::vector<TopoDS_Shape> grooves;
        for (int s = 0; s < nseg; ++s) {
            double z0 = s * seg_len - (s > 0 ? ov : 0.0);            if (z0 < 0.0) z0 = 0.0;
            double z1 = (s + 1) * seg_len + (s < nseg - 1 ? ov : 0.0); if (z1 > length) z1 = length;
            TopoDS_Wire helix = make_helix_wire_seg(axes, radius, lead, z0, z1, left != 0);
            TopoDS_Wire prof0 = thread_profile_wire(axes, radius, pitch, angle_deg, depth,
                                                    internal != 0, form, clearance_crest, clearance_root);
            gp_Trsf rot; rot.SetRotation(gp_Ax1(o, d), z0 / slope);  // the helix's angle at height z0
            gp_Trsf mv;  mv.SetTranslation(gp_Vec(d) * z0);
            TopoDS_Shape profs = BRepBuilderAPI_Transform(prof0, mv.Multiplied(rot), Standard_True).Shape();
            if (profs.ShapeType() != TopAbs_WIRE) return why("helix/start", "the profile stopped being a loop when moved to the start of the helix"), nullptr;
            BRepOffsetAPI_MakePipeShell mk(helix);
            mk.SetMode(axes.Direction());
            Handle(Law_Function) law = runout_law(z0, z1, length, li, lo); // depth fading at the ends
            if (!law.IsNull()) mk.SetLaw(TopoDS::Wire(profs), law, Standard_False, Standard_False);
            else               mk.Add(TopoDS::Wire(profs));
            mk.Build();
            if (!mk.IsDone() || !mk.MakeSolid()) return why("helix/pipe", "the kernel could not run the profile along the helix"), nullptr;
            TopoDS_Shape g0 = mk.Shape();
            if (g0.IsNull()) return why("helix/pipe", "the kernel reported success and returned nothing"), nullptr;
            for (int k = 0; k < ns; ++k) {                          // multiple starts: copies rotated by 360k/ns
                if (k == 0) { grooves.push_back(g0); continue; }
                gp_Trsf r2; r2.SetRotation(gp_Ax1(o, d), 2.0 * M_PI * k / ns);
                grooves.push_back(BRepBuilderAPI_Transform(g0, r2, Standard_True).Shape());
            }
        }
        // the grooves are cut out of the base ONE AT A TIME, the names running through every cut by the door of the result
        std::unique_ptr<QymShape> q;
        for (auto& g : grooves) {
            const QymShape* cur = q ? q.get() : base;
            BRepAlgoAPI_Cut cut;
            qym_boolean(cut, cur->shape, g);
            if (!cut.IsDone() || cut.Shape().IsNull()) return why("helix/cut", "the helical body could not be cut out of the part"), nullptr;
            QymBuilderHistory history(cut);
            q.reset(qym_result(history, cut.Shape(), QymResultSpec{{QymOperand{cur, QymCarry::Everything, "thread"}}}));
            if (!q) return why("helix/cut", "the cut left no body to name"), nullptr;
        }
        // The run-out is built into the pipe-shell (runout_law), so the ends need no separate handling.
        return q.release();
    } QYM_WHY_CATCH("thread")
    return nullptr;
}

// The volume of the solids of the shape (mm^3), for tests and checks (GProp). 0 on failure.
// ONLY SOLIDS HOLD VOLUME: VolumeProperties of an open shell takes it as if it were closed - two faces of a block
// 40 x 30 x 10 stitched along their shared edge weighed 2000 mm^3. A sheet, or the sheet part of a compound, weighs 0.
extern "C" double qym_shape_volume(const QymShape* s) {
    if (!s) return 0.0;
    try {
        QYM_SIGNALS;
        double v = 0.0;
        for (TopExp_Explorer ex(s->shape, TopAbs_SOLID); ex.More(); ex.Next()) {
            GProp_GProps g;
            BRepGProp::VolumeProperties(ex.Current(), g);
            v += g.Mass();
        }
        return v;
    } catch (...) {
        return 0.0;
    }
}

// The solid's BOUNDING BOX (Bnd_Box) in its OWN coordinate system -> out[6] = xmin,ymin,zmin,xmax,ymax,zmax.
// It is needed to derive the tessellation's deflection FROM THE SOLID'S SIZE (a fixed 0.5 mm made small parts
// faceted and huge ones unmanageable). 0 means an empty or broken shape (the caller falls back to a default).
// ABSORBED NAMES: pairs of "former name -> name of the shared face", two numbers per pair. Returns the number
// of PAIRS, not of elements. `out == nullptr` asks for the size only.
extern "C" size_t qym_shape_absorbed(const QymShape* s, unsigned* out, size_t max_pairs) {
    if (!s) return 0;
    if (!out) return s->absorbed.size();
    size_t n = s->absorbed.size() < max_pairs ? s->absorbed.size() : max_pairs;
    for (size_t i = 0; i < n; ++i) {
        out[i * 2] = s->absorbed[i].first;
        out[i * 2 + 1] = s->absorbed[i].second;
    }
    return n;
}

extern "C" int qym_shape_bbox(const QymShape* s, double* out) {
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull() || !out) return 0;
        Bnd_Box b;
        BRepBndLib::Add(s->shape, b);
        if (b.IsVoid()) return 0;
        b.Get(out[0], out[1], out[2], out[3], out[4], out[5]);
        return 1;
    } catch (...) {
        return 0;
    }
}

// The box of the geometry itself (`AddOptimal`), not of the poles of its curves and surfaces.
extern "C" int qym_shape_bbox_exact(const QymShape* s, double* out) {
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull() || !out) return 0;
        Bnd_Box b;
        BRepBndLib::AddOptimal(s->shape, b, Standard_False, Standard_False);
        if (b.IsVoid()) return 0;
        b.Get(out[0], out[1], out[2], out[3], out[4], out[5]);
        return 1;
    } catch (...) {
        return 0;
    }
}

// B-rep validity (BRepCheck_Analyzer): 1 for a valid solid, 0 for a broken or self-intersecting one. It lets
// operations fail gracefully when the kernel "builds" them (IsDone = true) but the result is geometrically
// broken — a fillet with a radius larger than the thickness of a thin wall, say, or a shell on unsuitable
// geometry (otherwise the viewport shows half-transparent walls and the part is ruined). An empty or
// uncheckable shape counts as invalid.
// A FACE THAT LIES IN TWO PLACES IS NOT A FACE. Take pinched faces apart into real ones and return how many
// were taken apart. It sits in the common `finish` funnel, because ANY operation can leave a face like that,
// not only the one it was caught on.
extern "C" int qym_shape_heal_pinched_faces(QymShape* s) {
    if (!s) return 0;
    try {
        QYM_SIGNALS;
        return heal_pinched_faces(s->shape, s->fids, s->eids, s->fsplit_of, s->fsplit_idx);
    } catch (...) { return 0; } // it did not work out; the solid stays as it was, no worse
}

// REPLACE ONE FACE WITH A SHEET THAT DOES NOT MEET ITS EDGES: the neighbours reach the sheet, as the professional systems
// replace a face. The body is grown out of the face along its normal far past the sheet, cut by the sheet carried on
// past its edges, and what lies on the body's side of it is kept: a top replaced by a sheet lifted 5 makes the block 5
// taller, one sunk 3 into it makes it 3 lower. The base's faces keep their names through the growing and the cut, the
// face on the sheet takes the sheet's. Null when it is not one face and one sheet face, or the cut leaves no solid.
static QymShape* replace_by_reaching(const QymShape* base, const uint32_t* idx, size_t n, const QymShape* surf) {
    if (n != 1) return nullptr;
    TopoDS_Face face, sheet;
    for (TopExp_Explorer ex(base->shape, TopAbs_FACE); ex.More(); ex.Next())
        if (base->fids.IsBound(ex.Current()) && static_cast<uint32_t>(base->fids.Find(ex.Current())) == idx[0]) { face = TopoDS::Face(ex.Current()); break; }
    int sheets = 0;
    for (TopExp_Explorer ex(surf->shape, TopAbs_FACE); ex.More(); ex.Next()) { sheet = TopoDS::Face(ex.Current()); ++sheets; }
    if (face.IsNull() || sheets != 1) return nullptr;
    // the way out of the face, at its middle
    double u0, u1, v0, v1;
    BRepTools::UVBounds(face, u0, u1, v0, v1);
    BRepAdaptor_Surface af(face);
    gp_Pnt mid;
    gp_Vec du, dv;
    af.D1(0.5 * (u0 + u1), 0.5 * (v0 + v1), mid, du, dv);
    gp_Vec out = du.Crossed(dv);
    if (out.Magnitude() < 1e-12) return nullptr;
    out.Normalize();
    if (face.Orientation() == TopAbs_REVERSED) out.Reverse();
    Bnd_Box box;
    BRepBndLib::Add(base->shape, box);
    BRepBndLib::Add(surf->shape, box);
    const double far_off = 2.0 * std::sqrt(box.SquareExtent()) + 1.0; // not `far`: a macro of the Windows headers
    // grown out of the face, then cut by the sheet carried on
    BRepPrimAPI_MakePrism prism(face, out.Multiplied(far_off));
    if (!prism.IsDone()) return nullptr;
    BRepAlgoAPI_Fuse grow;
    if (!qym_boolean(grow, base->shape, prism.Shape())) return nullptr;
    TopoDS_Face reach;
    BRepLib::ExtendFace(sheet, far_off, true, true, true, true, reach);
    if (reach.IsNull()) return nullptr;
    BRepAlgoAPI_Splitter cut;
    TopTools_ListOfShape args, tools;
    args.Append(grow.Shape());
    tools.Append(reach);
    if (!qym_boolean_many(cut, args, tools)) return nullptr;
    // the pieces on the body's side of the sheet: their middle lies against the way out
    GeomAPI_ProjectPointOnSurf on(mid, BRep_Tool::Surface(reach));
    BRep_Builder bb;
    TopoDS_Compound kept;
    bb.MakeCompound(kept);
    int n_kept = 0;
    for (TopExp_Explorer ex(cut.Shape(), TopAbs_SOLID); ex.More(); ex.Next()) {
        GProp_GProps g;
        BRepGProp::VolumeProperties(ex.Current(), g);
        const gp_Pnt c = g.CentreOfMass();
        on.Perform(c);
        if (on.NbPoints() == 0) continue;
        if (gp_Vec(on.NearestPoint(), c).Dot(out) < 0.0) { bb.Add(kept, ex.Current()); ++n_kept; }
    }
    if (n_kept == 0) return nullptr;
    TopoDS_Shape result = kept;
    BRepAlgoAPI_Fuse join;
    bool joined = false;
    if (n_kept == 1) { TopExp_Explorer one(kept, TopAbs_SOLID); result = one.Current(); }
    else {
        // pieces the sheet parted on the kept side belong together
        TopTools_ListOfShape all;
        for (TopExp_Explorer ex(kept, TopAbs_SOLID); ex.More(); ex.Next()) all.Append(ex.Current());
        TopoDS_Shape first = all.First();
        all.RemoveFirst();
        TopTools_ListOfShape a1;
        a1.Append(first);
        if (!qym_boolean_many(join, a1, all)) return nullptr;
        result = join.Shape();
        joined = true;
    }
    if (!BRepCheck_Analyzer(result).IsValid()) return nullptr;
    // every element of the result that stands on one of the base, through the growing, the cut and the join of the
    // pieces, takes its name - but the face replaced, which the sheet stands for now; the sheet's face takes its own
    QymShape kept_names{base->shape, base->fids, base->eids, {}, {}, {}, {}};
    kept_names.fids.UnBind(face);
    QymBuilderHistory grown(grow), parted(cut), pieced(join);
    QymChainHistory grown_parted(grown, parted);
    QymChainHistory all_of_it(grown_parted, pieced);
    QymHistory& history = joined ? static_cast<QymHistory&>(all_of_it) : static_cast<QymHistory&>(grown_parted);
    const int sheet_id = surf->fids.IsBound(sheet) ? surf->fids.Find(sheet) : 0;
    QymResultSpec spec{{QymOperand{&kept_names, QymCarry::Everything, "replace face"}}};
    spec.recipe = [&](QymShape& q, QymCounters&) {
        if (sheet_id == 0) return;
        TopTools_ListOfShape m;
        for (TopTools_ListIteratorOfListOfShape it(cut.Modified(reach)); it.More(); it.Next()) m.Append(it.Value());
        TopTools_ListOfShape now;
        for (TopTools_ListIteratorOfListOfShape it(m); it.More(); it.Next()) {
            const TopTools_ListOfShape& j = joined ? join.Modified(it.Value()) : TopTools_ListOfShape();
            if (j.IsEmpty()) now.Append(it.Value());
            else for (TopTools_ListIteratorOfListOfShape k(j); k.More(); k.Next()) now.Append(k.Value());
        }
        for (TopTools_ListIteratorOfListOfShape it(now); it.More(); it.Next())
            for (TopExp_Explorer r(result, TopAbs_FACE); r.More(); r.Next())
                if (r.Current().IsSame(it.Value()) && !q.fids.IsBound(r.Current())) q.fids.Bind(r.Current(), sheet_id);
    };
    return qym_result(history, result, spec);
}

// REPLACE A SOLID'S FACES WITH A SURFACE. This is where the design layer joins the timeline: the whole point
// of that layer is "take a face off, edit it apart, put it back on the solid", not "the design sits next to
// the part".
//
// The recipe is the classic one and is already used elsewhere in the bridge (see `qym_shape_bezier_shell`):
// assemble a shell from the base's faces WITHOUT the ones being replaced, add the surface's faces, sew, make a
// solid. Free edges after sewing are an honest sign that the surface did NOT close the hole: such a result is
// not returned at all, because an "almost solid" behaves like rubbish further down the timeline.
//
// NAMES MOVE ALONG THE SEWING'S HISTORY: the base's surviving faces stay themselves, and the surface's faces
// carry their own names. Without this, everything standing on the solid below this node would fall off at
// once.
extern "C" QymShape* qym_shape_replace_faces(const QymShape* base, const uint32_t* idx, size_t n, const QymShape* surf, double tol, unsigned* out_free) {
    if (out_free) *out_free = 0;
    if (!base || !surf || n == 0) return nullptr;
    try {
        QYM_SIGNALS;
        const double t = tol > 0.0 ? tol : 1.0e-6;
        BRepBuilderAPI_Sewing sew(t);
        // the names of the faces kept; a face replaced leaves with its name, which the surface does not inherit
        QymShape kept_names{base->shape, base->fids, base->eids, {}, {}, {}, {}};
        int kept = 0, dropped = 0;
        for (TopExp_Explorer ex(base->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            uint32_t id = base->fids.IsBound(ex.Current()) ? static_cast<uint32_t>(base->fids.Find(ex.Current())) : 0u;
            bool replace = false;
            for (size_t k = 0; k < n && !replace; ++k) replace = (idx[k] == id);
            if (replace) {
                ++dropped;
                kept_names.fids.UnBind(ex.Current());
                continue;
            }
            sew.Add(ex.Current());
            ++kept;
        }
        if (dropped == 0 || kept == 0) return nullptr; // nothing to replace, or nothing to keep
        for (TopExp_Explorer ex(surf->shape, TopAbs_FACE); ex.More(); ex.Next()) sew.Add(ex.Current());
        sew.Perform();
        TopoDS_Shape shape = sew.SewedShape();
        if (shape.IsNull()) return nullptr;
        if (sew.NbFreeEdges() != 0) {
            // ONE FACE AND A SHEET OFF ITS EDGES: the neighbours reach the sheet
            if (QymShape* reached = replace_by_reaching(base, idx, n, surf)) return reached;
            // HOW MANY EDGES WERE LEFT WITHOUT A PARTNER is the answer a person needs. A silent "it did not
            // work" left them guessing: replacing an end face's ring with a cap over the whole opening leaves
            // boundaries that do not match, and the message gave no way to tell.
            if (out_free) *out_free = (unsigned)sew.NbFreeEdges();
            return nullptr;
        }

        TopExp_Explorer sh(shape, TopAbs_SHELL);
        if (!sh.More()) return nullptr;
        ShapeFix_Solid fix;
        TopoDS_Shape solid = fix.SolidFromShell(TopoDS::Shell(sh.Current()));
        if (solid.IsNull()) {
            BRepBuilderAPI_MakeSolid mks(TopoDS::Shell(sh.Current()));
            if (!mks.IsDone()) return nullptr;
            solid = mks.Solid();
        }

        // the faces kept and the surface's go through what the sewing made of them; the faces replaced were never sewn
        QymSewingHistory history(sew);
        return qym_result(history, solid, QymResultSpec{{QymOperand{&kept_names, QymCarry::Everything, "replace face"}, QymOperand{surf, QymCarry::Everything, "replace face/surface"}}});
    } catch (...) { return nullptr; }
}

// SEW SEVERAL SHEETS INTO ONE. A surface is rarely born whole: a patch here, a copy of a face there, and a
// third piece between them. As long as those are separate shapes they can neither be worked on as one surface
// nor be given a thickness: thickening would take each piece on its own and produce a stack of plates.
//
// WHETHER ANYTHING WAS SEWN is asked directly (`NbContigousEdges`) rather than guessed from the look of the
// result. Sheets that do not touch each other are silently returned by the sewing as a compound of two
// islands: formally it "worked", but in substance those are the same two sheets under one name. That has to
// be a refusal, so the number of sewn edges is passed out.
//
// IF IT CLOSED, IT IS A SOLID. No free edges are left, so the shell is closed, and keeping it as a sheet
// would demand an extra step from a person for something that has already happened.
//
// NAMES: the first sheet stays itself, and from the rest only names that are not yet taken are used. Two
// pieces are numbered each in its own space, and "face 1" means different faces in each; merging them under
// one name would turn a reference to one of them into a reference to both.
extern "C" QymShape* qym_shape_stitch(const QymShape* const* parts, size_t n, double tol, unsigned* out_free, unsigned* out_joined) {
    if (out_free) *out_free = 0;
    if (out_joined) *out_joined = 0;
    if (!parts || n < 2) return nullptr;
    try {
        QYM_SIGNALS;
        const double t = tol > 0.0 ? tol : 1.0e-6;
        BRepBuilderAPI_Sewing sew(t);
        int faces = 0;
        for (size_t i = 0; i < n; ++i) {
            const QymShape* p = parts[i];
            if (!p || p->shape.IsNull()) return nullptr;
            for (TopExp_Explorer ex(p->shape, TopAbs_FACE); ex.More(); ex.Next(), ++faces) sew.Add(ex.Current());
        }
        if (faces == 0) return nullptr;
        sew.Perform();
        TopoDS_Shape shape = sew.SewedShape();
        if (shape.IsNull()) return nullptr;
        if (out_free) *out_free = (unsigned)sew.NbFreeEdges();
        if (out_joined) *out_joined = (unsigned)sew.NbContigousEdges();
        if (sew.NbContigousEdges() == 0) return nullptr; // nothing grew together, so this is not a sewing

        // IT CLOSED, so a solid is returned rather than a shell.
        if (sew.NbFreeEdges() == 0) {
            TopExp_Explorer sh(shape, TopAbs_SHELL);
            if (sh.More()) {
                ShapeFix_Solid fix;
                TopoDS_Shape solid = fix.SolidFromShell(TopoDS::Shell(sh.Current()));
                if (solid.IsNull()) {
                    BRepBuilderAPI_MakeSolid mks(TopoDS::Shell(sh.Current()));
                    if (mks.IsDone()) solid = mks.Solid();
                }
                if (!solid.IsNull()) shape = solid;
            }
        }

        // every sheet's names go through what the sewing made of them; a number another sheet already holds is yielded
        // for a fresh one, so "face 1" of two sheets does not become one name over two faces
        QymSewingHistory history(sew);
        QymResultSpec spec;
        for (size_t i = 0; i < n; ++i) spec.operands.push_back(QymOperand{parts[i], QymCarry::Everything, "stitch"});
        return qym_result(history, shape, spec);
    } catch (...) { return nullptr; }
}

// WHAT THE SHAPE IS: 0 empty, 1 a SOLID (solids present), 2 a SHEET (faces present, no solids).
//
// The `finish` funnel needs the distinction: a sheet has no volume BY NATURE, and reading its zero as "the
// part is gone" would keep every surface out of the document. A degenerate SOLID with zero volume is still a
// corpse, though, and must not be let through (one has already occurred on a real part).
// PATCH: STRETCH A SURFACE OVER A CHAIN OF EDGES.
//
// This is the layer's first tool that creates a shape the solid DID NOT HAVE: a copy of a face merely
// repeated what existed, whereas a patch closes an opening. Hence the order — the patch first, then
// everything that edits it.
//
// The edges set the BOUNDARY, and the kernel looks for the surface of minimal curvature resting on it
// (`BRepOffsetAPI_MakeFilling`). An open boundary is not a refusal by construction: the kernel will stretch a
// surface over an open chain too, and that is meaningful (a piece of a dome over three edges). The result is
// judged by the common `finish` barrier, not by guesswork here.
// THE FACE BESIDE EACH EDGE OF A PATCH. The filling makes its edges anew and keeps no history of them, so an edge of the
// patch is matched to the boundary edge of the body it lies along, by place: the middle of the patch edge on that edge,
// to 1e-4 mm. The face beside it is the largest named face of the body at the boundary edge - the face the shape is
// continued from, the one a tangent patch takes for its support - a choice that does not move from rebuild to rebuild.
static void note_beside_boundary(const QymShape* s, const std::vector<TopoDS_Edge>& boundary, const TopoDS_Shape& patch, TopTools_DataMapOfShapeInteger& ebeside) {
    TopTools_IndexedDataMapOfShapeListOfShape e2f;
    TopExp::MapShapesAndAncestors(s->shape, TopAbs_EDGE, TopAbs_FACE, e2f);
    for (TopExp_Explorer ex(patch, TopAbs_EDGE); ex.More(); ex.Next()) {
        BRepAdaptor_Curve c(TopoDS::Edge(ex.Current()));
        const gp_Pnt mid = c.Value(0.5 * (c.FirstParameter() + c.LastParameter()));
        const TopoDS_Vertex at = BRepBuilderAPI_MakeVertex(mid);
        for (const TopoDS_Edge& b : boundary) {
            BRepExtrema_DistShapeShape d(at, b);
            if (!d.IsDone() || d.Value() > 1e-4) continue;
            const int i = e2f.FindIndex(b);
            if (i < 1) break;
            double best = -1.0;
            int name = 0;
            for (TopTools_ListIteratorOfListOfShape it(e2f.FindFromIndex(i)); it.More(); it.Next()) {
                if (!s->fids.IsBound(it.Value())) continue;
                GProp_GProps g;
                BRepGProp::SurfaceProperties(it.Value(), g);
                if (g.Mass() > best) {
                    best = g.Mass();
                    name = s->fids.Find(it.Value());
                }
            }
            if (name != 0) ebeside.Bind(ex.Current(), name);
            break;
        }
    }
}

extern "C" QymShape* qym_shape_patch(const QymShape* s, const uint32_t* idx, size_t n, int tangent, unsigned name) {
    if (!s) return why("patch/asked", "there is no body to patch"), nullptr;
    if (n == 0) return why("patch/asked", "not one edge was named as the boundary of the patch"), nullptr;
    try {
        QYM_SIGNALS;
        BRepOffsetAPI_MakeFilling mf;
        int added = 0;
        TopoDS_Edge last;
        // TANGENCY IS SET BY A NEIGHBOURING FACE, not by a wish: for the surface to meet the edge smoothly
        // the kernel has to know WHAT it should be tangent to. Hence the map "edge -> its faces".
        TopTools_IndexedDataMapOfShapeListOfShape efmap;
        if (tangent) TopExp::MapShapesAndAncestors(s->shape, TopAbs_EDGE, TopAbs_FACE, efmap);
        // THE TRAVERSAL YIELDS AN EDGE ONCE PER FACE IT BELONGS TO. Without filtering, a boundary of four
        // edges arrives as eight, and the kernel honestly refuses to stretch a surface over that.
        TopTools_MapOfShape seen;
        std::vector<TopoDS_Edge> boundary; // the edges of the body the patch is spanned across
        for (TopExp_Explorer ex(s->shape, TopAbs_EDGE); ex.More(); ex.Next()) {
            if (!seen.Add(ex.Current())) continue;
            if (!s->eids.IsBound(ex.Current())) continue;
            uint32_t id = static_cast<uint32_t>(s->eids.Find(ex.Current()));
            bool want = false;
            for (size_t k = 0; k < n && !want; ++k) want = (idx[k] == id);
            if (!want) continue;
            // THE SUPPORT IS THE LARGEST OF THE EDGE'S NEIGHBOURING FACES. An edge has two of them and
            // tangency can be set to only one: what should be continued is the surface that gives the shape,
            // not a narrow strip of an end face. The choice also has to be STABLE — "whichever comes first"
            // changes from rebuild to rebuild, and one and the same patch would come out flat one time and
            // bulged the next.
            TopoDS_Face support;
            if (tangent && efmap.Contains(ex.Current())) {
                double best = -1.0;
                for (TopTools_ListIteratorOfListOfShape it(efmap.FindFromKey(ex.Current())); it.More(); it.Next()) {
                    GProp_GProps g;
                    BRepGProp::SurfaceProperties(it.Value(), g);
                    if (g.Mass() > best) {
                        best = g.Mass();
                        support = TopoDS::Face(it.Value());
                    }
                }
            }
            // With a support, G1 is asked for (smooth along the tangent); without one, plain C0 by position.
            if (!support.IsNull()) {
                mf.Add(TopoDS::Edge(ex.Current()), support, GeomAbs_G1);
            } else {
                mf.Add(TopoDS::Edge(ex.Current()), GeomAbs_C0);
            }
            ++added;
            last = TopoDS::Edge(ex.Current());
            boundary.push_back(last);
        }
        // one edge does not define a boundary - unless it is closed: the rim of a hole is a whole circle
        const bool ring = added == 1 && BRep_Tool::IsClosed(last);
        if (added < 2 && !ring) {
            char msg[160];
            snprintf(msg, sizeof(msg), "only %d of the %zu named edges are in this body, and one edge does not bound a patch", added, n);
            return why("patch/boundary", msg), nullptr;
        }
        TopoDS_Shape face;
        mf.Build();
        if (mf.IsDone()) face = mf.Shape();
        // a closed flat edge the filling will not take is spanned by the plane it lies in: the disc over a circle
        if (face.IsNull() && ring) {
            BRepBuilderAPI_MakeFace mk(BRepBuilderAPI_MakeWire(last).Wire(), Standard_True);
            if (mk.IsDone()) face = mk.Face();
        }
        if (face.IsNull() && !mf.IsDone()) return why("patch/build", "the kernel could not span a face across these edges"), nullptr;
        if (face.IsNull()) return why("patch/build", "the kernel reported success and returned nothing"), nullptr;
        // A PATCH IS NAMED AFTER THE FEATURE ITSELF. There is one patch per feature and its surface has no
        // other source. The sheet's single face used to be positional, and everything built on it (a
        // thickening, a face replacement) inherited that number instead of a name.
        return qym_born(face, [&](QymShape& q, QymCounters&) {
            if (name != 0) q.fids.Bind(q.shape, (int)name);
            note_beside_boundary(s, boundary, q.shape, q.ebeside);
        });
    } QYM_WHY_CATCH("patch")
    return nullptr;
}

// THE FACE ACROSS EACH EDGE: `source` is a face of `s`, `made` the history that took it - a copy, an offset of the copy -
// and every image of each of its edges is noted with the name of the face across that edge in `s`.
void note_beside(const QymShape* s, const TopoDS_Shape& source, QymHistory& made, TopTools_DataMapOfShapeInteger& ebeside) {
    TopTools_IndexedDataMapOfShapeListOfShape e2f;
    TopExp::MapShapesAndAncestors(s->shape, TopAbs_EDGE, TopAbs_FACE, e2f);
    TopTools_MapOfShape seen;
    for (TopExp_Explorer ex(source, TopAbs_EDGE); ex.More(); ex.Next()) {
        if (!seen.Add(ex.Current())) continue;
        const int i = e2f.FindIndex(ex.Current());
        if (i < 1) continue;
        int across = 0;
        for (TopTools_ListIteratorOfListOfShape it(e2f.FindFromIndex(i)); it.More() && across == 0; it.Next())
            if (!it.Value().IsSame(source) && s->fids.IsBound(it.Value())) across = s->fids.Find(it.Value());
        if (across == 0) continue;
        for (TopTools_ListIteratorOfListOfShape it(made.Modified(ex.Current())); it.More(); it.Next())
            if (it.Value().ShapeType() == TopAbs_EDGE) ebeside.Bind(it.Value(), across);
    }
}

// A COPY OF FACES AS A SURFACE OF ITS OWN. The bridge from the parametric side into the design layer: a
// solid's face becomes a SHEET in its own right, which can then be edited and put back on the solid by
// replacing the face.
//
// The faces are COPIED rather than reused: put the same shape into the new shell and an edit of the sheet
// would travel into the source solid, bypassing the timeline. The copy's name is set by the caller (`names`):
// its origin is known to the model, not to the kernel.
extern "C" QymShape* qym_shape_copy_faces(const QymShape* s, const uint32_t* idx, const uint32_t* names, size_t n) {
    if (!s || n == 0) return nullptr;
    try {
        QYM_SIGNALS;
        BRep_Builder bb;
        TopoDS_Shell shell;
        bb.MakeShell(shell);
        QymShape named{};
        QymShape* q = &named;
        int added = 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            if (!s->fids.IsBound(ex.Current())) continue;
            uint32_t id = static_cast<uint32_t>(s->fids.Find(ex.Current()));
            for (size_t k = 0; k < n; ++k) {
                if (idx[k] != id) continue;
                BRepBuilderAPI_Copy copier(ex.Current());
                const TopoDS_Shape cp = copier.Shape();
                if (cp.IsNull()) break;
                bb.Add(shell, cp);
                q->fids.Bind(cp, static_cast<int>(names[k]));
                QymBuilderHistory copied(copier);
                note_beside(s, ex.Current(), copied, q->ebeside);
                ++added;
                break;
            }
        }
        if (added == 0) return nullptr;
        // the edges are numbered by the door and named after the faces beside them (`ebeside`)
        return qym_born(shell, [&](QymShape& made, QymCounters&) {
            made.fids = named.fids;
            made.ebeside = named.ebeside;
        });
    } catch (...) { return nullptr; }
}

// DID THE FACE GO THE DISTANCE, AND DOES IT FACE THE SAME WAY: the middle of the face `f0`, moved `dist` along its normal,
// lies on `f1`, and `f1` points where `f0` did there. A cylinder of radius 10 moved in by 12 comes back a cylinder of
// radius 2 turned inside out - its normal reversed - and is refused, not handed on.
static bool went_the_distance(const TopoDS_Face& f0, const TopoDS_Face& f1, double dist) {
    double u0, u1, v0, v1;
    BRepTools::UVBounds(f0, u0, u1, v0, v1);
    BRepAdaptor_Surface a0(f0);
    gp_Pnt p0;
    gp_Vec du, dv;
    a0.D1(0.5 * (u0 + u1), 0.5 * (v0 + v1), p0, du, dv);
    gp_Vec nrm = du.Crossed(dv);
    if (nrm.Magnitude() < 1e-12) return true; // a degenerate middle tells nothing
    nrm.Normalize();
    if (f0.Orientation() == TopAbs_REVERSED) nrm.Reverse();
    const gp_Pnt want = p0.Translated(nrm.Multiplied(dist));
    GeomAPI_ProjectPointOnSurf proj(want, BRep_Tool::Surface(f1));
    if (proj.NbPoints() == 0) return false;
    double pu, pv;
    proj.LowerDistanceParameters(pu, pv);
    BRepAdaptor_Surface a1(f1);
    gp_Pnt p1;
    gp_Vec du1, dv1;
    a1.D1(pu, pv, p1, du1, dv1);
    gp_Vec nrm1 = du1.Crossed(dv1);
    if (f1.Orientation() == TopAbs_REVERSED) nrm1.Reverse();
    return nrm1.Magnitude() > 1e-12 && nrm1.Dot(nrm) > 0.0 && p1.Distance(want) <= 1e-3 * std::max(1.0, std::abs(dist));
}

// THE FACE OF THE OFFSET EACH TAKEN FACE BECAME, its twin in the offset of the shell they make (`QymTwinHistory`: the
// simple offset keeps the topology one for one and answers no history), each checked to have gone the distance: named
// after the face it came from into `fids`. False when a face has no twin or did not go the distance.
static bool name_moved(const TopoDS_Shape& shell, const TopoDS_Shape& out, const std::vector<std::pair<TopoDS_Shape, uint32_t>>& taken, double dist, TopTools_DataMapOfShapeInteger& fids) {
    QymTwinHistory history(shell, out);
    std::vector<TopoDS_Face> moved;
    for (const auto& t : taken) {
        const TopTools_ListOfShape& images = history.Modified(t.first);
        if (images.Extent() != 1) return false;
        if (!went_the_distance(TopoDS::Face(t.first), TopoDS::Face(images.First()), dist)) return false;
        moved.push_back(TopoDS::Face(images.First()));
    }
    for (size_t k = 0; k < moved.size(); ++k) fids.Bind(moved[k], static_cast<int>(taken[k].second));
    return true;
}

// AN OFFSET SHEET: the faces `idx` of the shape taken out and moved `dist` along their normals - outward for the faces of
// a solid, a negative distance going in, zero a copy in place. Every face of the sheet is named `names[k]` after the one
// it came from. Faces meeting smoothly move as one sheet; faces meeting at a sharp edge - the top and the front of a
// block - move apart, each a sheet of its own in one surface body, as the professional systems offset them. Null when no
// face is found, or a face turns inside out (a cylinder of radius 10 moved in by 12).
extern "C" QymShape* qym_shape_offset_faces(const QymShape* s, const uint32_t* idx, const uint32_t* names, size_t n, double dist) {
    if (!s || n == 0) return nullptr;
    try {
        QYM_SIGNALS;
        BRep_Builder bb;
        TopoDS_Shell shell;
        bb.MakeShell(shell);
        std::vector<std::pair<TopoDS_Shape, uint32_t>> taken;
        std::vector<TopoDS_Shape> sources; // the faces of `s` the sheet is taken from, in the order of `taken`
        std::vector<std::unique_ptr<BRepBuilderAPI_Copy>> copies; // what made each taken face, kept for its history
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            if (!s->fids.IsBound(ex.Current())) continue;
            uint32_t id = static_cast<uint32_t>(s->fids.Find(ex.Current()));
            for (size_t k = 0; k < n; ++k) {
                if (idx[k] != id) continue;
                copies.push_back(std::make_unique<BRepBuilderAPI_Copy>(ex.Current()));
                const TopoDS_Shape cp = copies.back()->Shape();
                if (cp.IsNull()) break;
                bb.Add(shell, cp);
                taken.emplace_back(cp, names[k]);
                sources.push_back(ex.Current());
                break;
            }
        }
        if (taken.empty()) return nullptr;
        QymShape named{};
        QymShape* q = &named;
        TopoDS_Shape out;
        // what made the sheet of each taken face: its offset, one for all of them or one of its own
        // what each taken face was offset as - the shell of them all, or a shell of its own - and what came of it
        std::vector<std::pair<TopoDS_Shape, TopoDS_Shape>> offset_of(taken.size());
        BRepOffset_MakeSimpleOffset together;
        if (std::abs(dist) < 1e-9) {
            out = shell;
            for (const auto& t : taken) q->fids.Bind(t.first, static_cast<int>(t.second));
        } else {
            together.Initialize(shell, dist);
            together.Perform();
            // the faces together hold only when the sheet they make is sound: at a sharp edge they part, the shell tears
            if (together.IsDone() && !together.GetResultShape().IsNull() && BRepCheck_Analyzer(together.GetResultShape()).IsValid() && name_moved(shell, together.GetResultShape(), taken, dist, q->fids)) {
                out = together.GetResultShape();
                for (auto& o : offset_of) o = {shell, out};
            } else {
                q->fids.Clear();
                TopoDS_Compound c;
                bb.MakeCompound(c);
                for (size_t k = 0; k < taken.size(); ++k) {
                    const auto& t = taken[k];
                    TopoDS_Shell alone; // the simple offset takes a shell, not a bare face
                    bb.MakeShell(alone);
                    bb.Add(alone, t.first);
                    BRepOffset_MakeSimpleOffset one(alone, dist);
                    one.Perform();
                    if (!one.IsDone() || one.GetResultShape().IsNull()) return nullptr;
                    std::vector<std::pair<TopoDS_Shape, uint32_t>> single{t};
                    if (!name_moved(alone, one.GetResultShape(), single, dist, q->fids)) return nullptr;
                    offset_of[k] = {alone, one.GetResultShape()};
                    for (TopExp_Explorer ex(one.GetResultShape(), TopAbs_FACE); ex.More(); ex.Next()) bb.Add(c, ex.Current());
                }
                out = c;
            }
        }
        if (out.IsNull()) return nullptr;
        BRepCheck_Analyzer check(out);
        if (!check.IsValid()) return nullptr;
        // the face across each edge of a taken face, carried by what took it: the copy, then its offset
        for (size_t k = 0; k < sources.size(); ++k) {
            QymBuilderHistory copied(*copies[k]);
            if (offset_of[k].first.IsNull()) {
                note_beside(s, sources[k], copied, q->ebeside);
                continue;
            }
            QymTwinHistory offset(offset_of[k].first, offset_of[k].second);
            QymChainHistory both(copied, offset);
            note_beside(s, sources[k], both, q->ebeside);
        }
        // the edges are numbered by the door and named after the faces beside them (`ebeside`)
        return qym_born(out, [&](QymShape& made, QymCounters&) {
            made.fids = named.fids;
            made.ebeside = named.ebeside;
        });
    } catch (...) { return nullptr; }
}

extern "C" int qym_shape_kind(const QymShape* s) {
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull()) return 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_SOLID); ex.More(); ex.Next()) return 1;
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) return 2;
        return 0;
    } catch (...) { return 0; }
}


// THE SMALLEST FILLET OR CHAMFER ON THE SOLID (the radius of its cylindrical and toroidal faces). It is
// needed to tell a person the truth: a wall thicker than that radius eats it whole when offset, and the shell
// does not build. 0 means there are no round faces and hence no limit.
extern "C" double qym_shape_min_round_radius(const QymShape* s) {
    double best = 0.0;
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull()) return 0.0;
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            Handle(Geom_Surface) su = BRep_Tool::Surface(TopoDS::Face(ex.Current()));
            if (su.IsNull()) continue;
            double r = 0.0;
            if (Handle(Geom_CylindricalSurface) c = Handle(Geom_CylindricalSurface)::DownCast(su)) r = c->Radius();
            else if (Handle(Geom_ToroidalSurface) t = Handle(Geom_ToroidalSurface)::DownCast(su)) r = t->MinorRadius();
            else if (Handle(Geom_ConicalSurface) k = Handle(Geom_ConicalSurface)::DownCast(su)) {
                // A CHAMFER IS A CONE, and its width limits the shell just as a fillet's radius does: an
                // offset thicker than it eats the face whole. What is measured is the width of the cone's
                // strip along parameter V (the generatrix's length), not the radius: here the radius is the
                // size of the hole.
                Standard_Real u1, u2, v1, v2;
                BRepTools::UVBounds(TopoDS::Face(ex.Current()), u1, u2, v1, v2);
                r = std::abs(v2 - v1);
            }
            if (r > 1e-9 && (best < 1e-9 || r < best)) best = r;
        }
    } catch (...) { return 0.0; }
    return best;
}


// HOW MANY SOLIDS THE SHAPE HOLDS. An operation that has broken the part into pieces violates the rule "a
// part is one solid", and that is what a person has to be told, not "broken solid": it is a different trouble
// calling for different action.
extern "C" int qym_shape_shell_count(const QymShape* s) {
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull()) return 0;
        int n = 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_SHELL); ex.More(); ex.Next()) ++n;
        return n;
    } QYM_WHY_CATCH("shape_shell_count")
    return 0;
}

extern "C" int qym_shape_solid_count(const QymShape* s) {
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull()) return 0;
        int n = 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_SOLID); ex.More(); ex.Next()) ++n;
        return n;
    } QYM_WHY_CATCH("shape_solid_count")
    return 0;
}

// THE FACES OF THE SHAPE BY THE KIND OF SURFACE THEY LIE ON, into `out[7]`: plane, cylinder, cone, sphere, torus,
// free form (B-spline or Bezier), any other (extrusion, revolution, offset). A rounded straight edge between two
// planes is one more cylinder, a cut one is one more plane: this is what tells a rounding that was made from one
// that was dropped while the node stayed green.
extern "C" int qym_shape_face_kinds(const QymShape* s, int* out) {
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull() || !out) return 0;
        for (int i = 0; i < 7; ++i) out[i] = 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) {
            BRepAdaptor_Surface surf(TopoDS::Face(ex.Current()), Standard_False);
            switch (surf.GetType()) {
                case GeomAbs_Plane: ++out[0]; break;
                case GeomAbs_Cylinder: ++out[1]; break;
                case GeomAbs_Cone: ++out[2]; break;
                case GeomAbs_Sphere: ++out[3]; break;
                case GeomAbs_Torus: ++out[4]; break;
                case GeomAbs_BSplineSurface:
                case GeomAbs_BezierSurface: ++out[5]; break;
                default: ++out[6]; break;
            }
        }
        return 1;
    } catch (...) { return 0; }
}


// REPAIR A SHAPE THE CHECK CALLS INVALID. Kernel operations sometimes hand back a solid with a small defect:
// the check rejects it and a person sees "broken solid" where the part is essentially right. ShapeFix
// straightens such defects out; the names of faces and edges are carried across through the repair's context
// (old shape -> its image), or the part would lose all its references at once.
extern "C" QymShape* qym_shape_heal(const QymShape* s) {
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull()) return nullptr;
        ShapeFix_Shape fx(s->shape);
        fx.Perform();
        TopoDS_Shape res = fx.Shape();
        if (res.IsNull()) return nullptr;
        BRepCheck_Analyzer an(res, Standard_True);
        if (!an.IsValid()) return nullptr;
        // a face the repair cut in pieces keeps its name on the first and records the rest as its pieces: all of them
        // under the one name made two faces a reference could not tell apart
        QymRepairHistory history(fx.Context(), QymRepairAsk::Apply);
        return qym_result(history, res, QymResultSpec{{QymOperand{s, QymCarry::Everything, "heal"}}});
    } catch (...) { return nullptr; }
}

// The name the kernel gives a status of its check (`InvalidCurveOnSurface`), for the words of a refusal.
static std::string check_word(BRepCheck_Status st) {
    std::ostringstream o;
    BRepCheck::Print(st, o);
    std::string w = o.str();
    while (!w.empty() && (w.back() == '\n' || w.back() == ' ')) w.pop_back();
    return w;
}

extern "C" int qym_shape_is_valid(const QymShape* s) {
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull()) return 0;
        BRepCheck_Analyzer an(s->shape, Standard_True);
        if (an.IsValid()) return 1;
        if (getenv("QYM_WHY_INVALID")) {
            {   // TWO QUESTIONS AT ONCE: is the shape valid without the GEOMETRIC checks, and is it fixable.
                BRepCheck_Analyzer bare(s->shape, Standard_False);
                ShapeFix_Shape fx(s->shape);
                fx.Perform();
                TopoDS_Shape fixed = fx.Shape();
                bool ok_fixed = false;
                if (!fixed.IsNull()) {
                    BRepCheck_Analyzer af(fixed, Standard_True);
                    ok_fixed = af.IsValid();
                }
                fprintf(stderr, "QYMWHY without geometric checks: %s; after repair: %s\n",
                        bare.IsValid() ? "VALID" : "invalid", ok_fixed ? "VALID" : "invalid");
            }
            int nso = 0, nsh = 0, nf = 0;
            for (TopExp_Explorer ex(s->shape, TopAbs_SOLID); ex.More(); ex.Next()) ++nso;
            for (TopExp_Explorer ex(s->shape, TopAbs_SHELL); ex.More(); ex.Next()) ++nsh;
            for (TopExp_Explorer ex(s->shape, TopAbs_FACE); ex.More(); ex.Next()) ++nf;
            fprintf(stderr, "QYMWHY contents: type %d, solids %d, shells %d, faces %d\n", (int)s->shape.ShapeType(), nso, nsh, nf); // the "what exactly is invalid" breakdown, on demand rather than in a normal run
            {   // the shape's root: SOLID-level statuses (not closed, inside out) hang on it
                Handle(BRepCheck_Result) r0 = an.Result(s->shape);
                if (!r0.IsNull()) {
                    for (BRepCheck_ListIteratorOfListOfStatus it(r0->Status()); it.More(); it.Next()) {
                        if (it.Value() != BRepCheck_NoError) fprintf(stderr, "QYMWHY root (%d) status %d\n", (int)s->shape.ShapeType(), (int)it.Value());
                    }
                }
            }
            const TopAbs_ShapeEnum tys[6] = {TopAbs_SOLID, TopAbs_SHELL, TopAbs_FACE, TopAbs_EDGE, TopAbs_VERTEX, TopAbs_WIRE};
            const char* nm[6] = {"solid", "shell", "face", "edge", "vertex", "wire"};
            for (int t = 0; t < 6; ++t) {
                int k = 0;
                for (TopExp_Explorer ex(s->shape, tys[t]); ex.More(); ex.Next(), ++k) {
                    // NO FILTERING BY IsValid: a shell- or solid-level status (not closed, inside out) can
                    // sit on the subshape itself while IsValid on it stays silent.
                    // one sub-shape the analyzer holds no record of throws on `Result`; it is passed over, not the rest
                    try {
                        QYM_SIGNALS;
                        Handle(BRepCheck_Result) r = an.Result(ex.Current());
                        if (r.IsNull()) continue;
                        for (BRepCheck_ListIteratorOfListOfStatus it(r->Status()); it.More(); it.Next()) {
                            if (it.Value() != BRepCheck_NoError) {
                                const int kind = tys[t] == TopAbs_FACE ? (int)BRepAdaptor_Surface(TopoDS::Face(ex.Current()), Standard_False).GetType() : -1;
                                fprintf(stderr, "QYMWHY %s #%d status %s (surface type %d)\n", nm[t], k, check_word(it.Value()).c_str(), kind);
                            }
                        }
                        // AND IN EVERY CONTEXT: an edge's faults sit on it as seen from each of its faces
                        for (r->InitContextIterator(); r->MoreShapeInContext(); r->NextShapeInContext()) {
                            for (BRepCheck_ListIteratorOfListOfStatus it(r->StatusOnShape()); it.More(); it.Next()) {
                                if (it.Value() != BRepCheck_NoError) fprintf(stderr, "QYMWHY %s #%d in %d status %s\n", nm[t], k, (int)r->ContextualShape().ShapeType(), check_word(it.Value()).c_str());
                            }
                        }
                    } catch (...) {
                    }
                }
            }
        }
        return 0;
    } catch (Standard_Failure const& e) {
        fprintf(stderr, "QYMWHY the check threw an exception: %s\n", e.GetMessageString() ? e.GetMessageString() : "no text");
        return 0;
    } catch (...) {
        fprintf(stderr, "QYMWHY the check threw an unknown exception\n");
        return 0;
    }
}




// THE VOLUME OF THE INTERSECTION of two solids (interference in an assembly). More than eps means the parts
// penetrate each other. A quick rejection by bounding boxes (Bnd_Box) comes first: if they do not overlap
// there is definitely no interference, and no costly boolean is needed. Otherwise BRepAlgoAPI_Common gives the
// volume of the shared solid (GProp). Face-to-face contact yields about 0, which is not interference. The
// solids are expected to be in a common coordinate system already (world or context); the caller applies the
// transform.
//
// A MEASURED ZERO AND A FAILURE ARE DIFFERENT ANSWERS, and they leave through different doors: the volume
// goes to `*out`, and the returned 1 or 0 says whether it means anything. Both used to be the same zero, and
// an assembly reading that zero calls the parts clear of each other - the one mistake this function exists to
// prevent.
extern "C" int qym_shape_interference_volume(const QymShape* a, const QymShape* b, double* out) {
    if (!out) return 0;
    *out = 0.0;
    if (!a || !b) return 0;
    try {
        QYM_SIGNALS;
        Bnd_Box ba, bb;
        BRepBndLib::Add(a->shape, ba);
        BRepBndLib::Add(b->shape, bb);
        if (ba.IsVoid() || bb.IsVoid()) return 0; // no box means nothing can be said about the pair
        if (ba.IsOut(bb)) return 1;               // the boxes are apart, so there is no intersection: a measured nothing
        BRepAlgoAPI_Common common;
        qym_boolean(common, a->shape, b->shape);
        if (!common.IsDone()) return 0;
        TopoDS_Shape res = common.Shape();
        if (res.IsNull()) return 0;
        GProp_GProps props;
        BRepGProp::VolumeProperties(res, props);
        double v = props.Mass();
        *out = v > 0.0 ? v : 0.0;
        return 1;
    } catch (...) {
        return 0;
    }
}
extern "C" QymDoc* qym_shape_tessellate(const QymShape* s, double defl) {
    if (!s) return nullptr;
    try {
        QYM_SIGNALS;
        return doc_from_shape(s->shape, defl, s->fids);
    } catch (...) {
        return nullptr; // the tessellator throws on broken topology; report "no mesh" instead of crashing
    }
}

// THE SOLIDS OF THE SHAPE, in the explorer's order: the centre of mass and the volume of each, into `centres[3 * i]` and
// `volumes[i]`, at most `max` of them. Returns how many there are (possibly more than `max`), 0 on failure. The order
// is the one `qym_shape_solid_at` counts in.
extern "C" int qym_shape_solids_info(const QymShape* s, double* centres, double* volumes, int max) {
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull()) return 0;
        int n = 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_SOLID); ex.More(); ex.Next(), ++n) {
            if (n >= max) continue;
            GProp_GProps g;
            BRepGProp::VolumeProperties(ex.Current(), g);
            const gp_Pnt c = g.CentreOfMass();
            centres[3 * n] = c.X();
            centres[3 * n + 1] = c.Y();
            centres[3 * n + 2] = c.Z();
            volumes[n] = g.Mass();
        }
        return n;
    } catch (...) {
        return 0;
    }
}

// THE SOLID NUMBER `index` OF THE SHAPE, in the order of `qym_shape_solids_info`. The solid is the body's own, not
// rebuilt, so every face and edge keeps its name: a fillet on a piece that became a body of its own still finds its
// edges.
extern "C" QymShape* qym_shape_solid_at(const QymShape* s, int index) {
    try {
        QYM_SIGNALS;
        if (!s || s->shape.IsNull() || index < 0) return nullptr;
        int n = 0;
        for (TopExp_Explorer ex(s->shape, TopAbs_SOLID); ex.More(); ex.Next(), ++n) {
            if (n != index) continue;
            const TopoDS_Shape out = ex.Current();
            return qym_born(out, [&](QymShape& q, QymCounters&) {
                for (TopExp_Explorer f(out, TopAbs_FACE); f.More(); f.Next())
                    if (s->fids.IsBound(f.Current()) && !q.fids.IsBound(f.Current())) q.fids.Bind(f.Current(), s->fids.Find(f.Current()));
                for (TopExp_Explorer e(out, TopAbs_EDGE); e.More(); e.Next())
                    if (s->eids.IsBound(e.Current()) && !q.eids.IsBound(e.Current())) q.eids.Bind(e.Current(), s->eids.Find(e.Current()));
            });
        }
        return nullptr;
    } catch (...) {
        return nullptr;
    }
}
