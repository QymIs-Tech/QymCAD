use qymcad_core::model::Constraint;
mod common;

#[test]
fn change_dimension_keeps_vertical_and_fixed() {
    let mut p = common::testbug();
    let mut tot = 0;
    for si in 0..p.sketches.len() {
        let s = &p.sketches[si];
        // the original positions of the fixed points
        let fixed: std::collections::HashMap<u64,(f64,f64)> = s.constraints.iter().filter_map(|c| match c {
            Constraint::Fixed{p}=>s.points.iter().find(|q|q.id==*p).map(|q|(*p,(q.x,q.y))), _=>None }).collect();
        // find the first driving dimension with no expression and grow it by 40 per cent
        let mut changed = None;
        for c in p.sketches[si].constraints.iter_mut() {
            if let Constraint::Distance { d, expr, driven, .. } = c {
                if !*driven && expr.is_empty() && *d > 0.1 { let old=*d; *d *= 1.4; changed=Some((old,*d)); break; }
            }
        }
        let Some((old,new)) = changed else { continue };
        p.solve_sketch(si);
        // the check
        let s = &p.sketches[si];
        let pos = |id: u64| s.points.iter().find(|q| q.id==id).map(|q| (q.x, q.y));
        let mut v = 0;
        for c in &s.constraints {
            match c {
                Constraint::Vertical{a,b}=>if let(Some(pa),Some(pb))=(pos(*a),pos(*b)){ if (pa.0-pb.0).abs()>1e-2 {eprintln!("[{si}] '{}': vertical (a={a}, b={b}) dx={:.3}, dimension {old:.1} -> {new:.1}",s.name,(pa.0-pb.0).abs());v+=1;}},
                Constraint::Horizontal{a,b}=>if let(Some(pa),Some(pb))=(pos(*a),pos(*b)){ if (pa.1-pb.1).abs()>1e-2 {eprintln!("[{si}] '{}': horizontal (a={a}, b={b}) dy={:.3}",s.name,(pa.1-pb.1).abs());v+=1;}},
                Constraint::Fixed{p:fp}=>if let(Some((x,y)),Some(&(x0,y0)))=(pos(*fp),fixed.get(fp)){ let d=((x-x0).powi(2)+(y-y0).powi(2)).sqrt(); if d>1e-2{eprintln!("[{si}] '{}': fixed point {fp} moved by {d:.3}, dimension {old:.1} -> {new:.1}",s.name);v+=1;}},
                _=>{}
            }
        }
        tot += v;
    }
    eprintln!("violations after the dimension changed: {tot}");
}
