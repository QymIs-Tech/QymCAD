// Writes the stored body the edge checks read (`tests/data/unsampled_edge.qymb`): two edges in a compound, in the blob
// format `qym_shape_to_brep` writes. The first lies on an offset curve whose basis starts degenerate - four poles at
// one point - so OCCT throws when a point of it is asked for; the second is a plain straight line. The blob in the
// tree was written with OCCT 7.9.3 (Arch's opencascade 1:7.9.3); an OCCT that cannot read it rebuilds it from here.
// Build against the same OCCT and run with the path:
//
//     g++ -std=c++17 -I/usr/include/opencascade gen_unsampled_edge.cpp -o gen -lTKBRep -lTKTopAlgo -lTKG3d -lTKG2d \
//         -lTKMath -lTKernel -lTKGeomBase && ./gen unsampled_edge.qymb
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRep_Builder.hxx>
#include <BinTools.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_OffsetCurve.hxx>
#include <TColStd_Array1OfInteger.hxx>
#include <TColStd_Array1OfReal.hxx>
#include <TColgp_Array1OfPnt.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Vertex.hxx>
#include <cstdint>
#include <cstdio>
#include <fstream>
#include <sstream>

int main(int argc, char** argv) {
    if (argc < 2) return 1;
    TColgp_Array1OfPnt poles(1, 6);
    for (int i = 1; i <= 4; ++i) poles(i) = gp_Pnt(0, 0, 0);
    poles(5) = gp_Pnt(2, 1, 0);
    poles(6) = gp_Pnt(3, 0, 0);
    TColStd_Array1OfReal knots(1, 4);
    knots(1) = 0; knots(2) = 1; knots(3) = 2; knots(4) = 3;
    TColStd_Array1OfInteger mults(1, 4);
    mults(1) = 4; mults(2) = 1; mults(3) = 1; mults(4) = 4;
    Handle(Geom_BSplineCurve) basis = new Geom_BSplineCurve(poles, knots, mults, 3);
    Handle(Geom_OffsetCurve) offset = new Geom_OffsetCurve(basis, 0.5, gp_Dir(0, 0, 1));
    BRep_Builder b;
    TopoDS_Edge bad;
    b.MakeEdge(bad, offset, 1e-7); // the curve is not evaluated here
    b.Range(bad, 0.0, 3.0);
    TopoDS_Vertex v0, v1;
    b.MakeVertex(v0, gp_Pnt(0, 0, 0), 1e-3);
    b.MakeVertex(v1, gp_Pnt(3, 0.5, 0), 1e-3);
    b.Add(bad, v0.Oriented(TopAbs_FORWARD));
    b.Add(bad, v1.Oriented(TopAbs_REVERSED));
    TopoDS_Edge good = BRepBuilderAPI_MakeEdge(gp_Pnt(0, 5, 0), gp_Pnt(10, 5, 0)).Edge();
    TopoDS_Compound c;
    b.MakeCompound(c);
    b.Add(c, bad);
    b.Add(c, good);
    std::ostringstream os(std::ios::binary);
    BinTools::Write(c, os, Standard_False, Standard_False, BinTools_FormatVersion_CURRENT);
    const std::string brep = os.str();
    const uint32_t head[5] = {2u, 0u, 2u, 1u, 2u}; // version 2, no faces, two edges named 1 and 2
    std::ofstream out(argv[1], std::ios::binary);
    out.write("QYMB", 4);
    out.write(reinterpret_cast<const char*>(head), sizeof head);
    out.write(brep.data(), static_cast<std::streamsize>(brep.size()));
    std::printf("wrote %s: %zu bytes\n", argv[1], 4 + sizeof head + brep.size());
    return 0;
}
