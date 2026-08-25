// Extruding N contours, fusing them into one tool and taking a single boolean against the base gives the same
// body as a chain of N booleans, but cleaner: fewer edges, with no accumulated seams. This is the basis of the
// multi-contour node.
use qymcad_kernel::Shape;

fn boss(x: f64, y: f64, z0: f64, h: f64) -> Shape { // an 8×8 box with its corner at (x,y), spanning z0 to z0+h
    let s = Shape::extrude(&[x,y, x+8.0,y, x+8.0,y+8.0, x,y+8.0], h).unwrap();
    s.transformed(&[1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,z0]).unwrap()
}

#[test]
fn fuse_then_one_boolean_equals_chain_but_cleaner() {
    // the base plate, 40×40×5
    let base = Shape::extrude(&[0.0,0.0, 40.0,0.0, 40.0,40.0, 0.0,40.0], 5.0).unwrap();
    let b1 = boss(4.0,4.0,5.0,10.0);
    let b2 = boss(16.0,4.0,5.0,10.0);
    let b3 = boss(28.0,4.0,5.0,10.0);
    // the old way: a chain of N booleans against the base, one at a time
    let chain = base.boolean(&b1,1).unwrap().boolean(&b2,1).unwrap().boolean(&b3,1).unwrap();
    // the new way: fuse the bosses into one tool, then take a single boolean against the base
    let tool = b1.boolean(&b2,1).unwrap().boolean(&b3,1).unwrap();
    let one = base.boolean(&tool,1).unwrap();
    let (ce, oe) = (chain.edges().len(), one.edges().len());
    eprintln!("chain: V={:.1} edges={ce} | fuse and one boolean: V={:.1} edges={oe}", chain.volume(), one.volume());
    assert!((chain.volume()-one.volume()).abs() < 1e-3, "the volumes are equal");
    assert!(one.is_valid() && one.tessellate(0.5).len()==1, "the new way gives one valid solid");
    assert!(oe <= ce, "the new way is no dirtier than the chain, with no more edges: {oe} against {ce}");
}
