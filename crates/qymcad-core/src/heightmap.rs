//! A height map built from a mesh: the basis of drop-cutter 3D machining.
//!
//! Seen from above, each cell of the XY grid stores the maximum height of the surface. The tool is dropped onto
//! the map according to its own shape, flat or ball-nosed.

use crate::geom::{Contour, Mesh, Point2};

pub struct Heightmap {
    pub min: Point2,
    pub cell: f64,
    pub nx: usize,
    pub ny: usize,
    pub floor: f64,
    z: Vec<f64>,
}

impl Heightmap {
    /// Build from a mesh with a grid step of `cell`, in mm.
    pub fn from_mesh(mesh: &Mesh, cell: f64) -> Option<Self> {
        let b = mesh.bounds()?;
        let cell = cell.max(0.05);
        let nx = (((b.max.x - b.min.x) / cell).ceil() as usize).max(1) + 1;
        let ny = (((b.max.y - b.min.y) / cell).ceil() as usize).max(1) + 1;
        let floor = b.min.z;
        let mut hm = Heightmap {
            min: Point2::new(b.min.x, b.min.y),
            cell,
            nx,
            ny,
            floor,
            z: vec![floor; nx * ny],
        };

        for i in 0..mesh.tris.len() {
            hm.raster_triangle(mesh.triangle(i));
        }
        Some(hm)
    }

    fn idx(&self, ix: usize, iy: usize) -> usize {
        iy * self.nx + ix
    }

    /// Flat stock: the whole grid at the height `top`.
    pub fn flat(min: Point2, width: f64, height: f64, cell: f64, top: f64) -> Self {
        let cell = cell.max(0.1);
        let nx = ((width / cell).ceil() as usize).max(1) + 1;
        let ny = ((height / cell).ceil() as usize).max(1) + 1;
        Heightmap { min, cell, nx, ny, floor: top, z: vec![top; nx * ny] }
    }

    /// Remove material with the tool at a point: the cells within the radius drop to the level of the tip
    /// according to its shape, but never below what has already been removed.
    pub fn carve(&mut self, x: f64, y: f64, z: f64, radius: f64, ball: bool) {
        let cx = ((x - self.min.x) / self.cell).round() as isize;
        let cy = ((y - self.min.y) / self.cell).round() as isize;
        let rc = (radius / self.cell).ceil() as isize;
        for dy in -rc..=rc {
            for dx in -rc..=rc {
                let (ox, oy) = (dx as f64 * self.cell, dy as f64 * self.cell);
                let d2 = ox * ox + oy * oy;
                if d2 > radius * radius {
                    continue;
                }
                let (ix, iy) = (cx + dx, cy + dy);
                if ix < 0 || iy < 0 || ix as usize >= self.nx || iy as usize >= self.ny {
                    continue;
                }
                let shape = if ball { radius - (radius * radius - d2).max(0.0).sqrt() } else { 0.0 };
                let surf = z + shape;
                let k = self.idx(ix as usize, iy as usize);
                if surf < self.z[k] {
                    self.z[k] = surf;
                }
            }
        }
    }

    /// Remove material along a segment, sampled at the step of the map.
    pub fn carve_segment(&mut self, a: crate::geom::Point3, b: crate::geom::Point3, radius: f64, ball: bool) {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let dz = b.z - a.z;
        let len = (dx * dx + dy * dy).sqrt();
        let n = (len / (self.cell * 0.5)).ceil().max(1.0) as usize;
        for i in 0..=n {
            let t = i as f64 / n as f64;
            self.carve(a.x + dx * t, a.y + dy * t, a.z + dz * t, radius, ball);
        }
    }

    /// Copy the heights into a mesh, for drawing the result of a simulation.
    pub fn to_mesh(&self) -> Mesh {
        let mut verts = Vec::with_capacity(self.nx * self.ny);
        for iy in 0..self.ny {
            for ix in 0..self.nx {
                verts.push(crate::geom::Point3::new(
                    self.min.x + ix as f64 * self.cell,
                    self.min.y + iy as f64 * self.cell,
                    self.z[self.idx(ix, iy)],
                ));
            }
        }
        let mut tris = Vec::new();
        for iy in 0..self.ny - 1 {
            for ix in 0..self.nx - 1 {
                let a = (iy * self.nx + ix) as u32;
                let b = a + 1;
                let c = a + self.nx as u32;
                let d = c + 1;
                tris.push([a, c, b]);
                tris.push([b, c, d]);
            }
        }
        Mesh { verts, tris }
    }

    fn raster_triangle(&mut self, t: [crate::geom::Point3; 3]) {
        let (a, b, c) = (t[0], t[1], t[2]);
        let minx = a.x.min(b.x).min(c.x);
        let maxx = a.x.max(b.x).max(c.x);
        let miny = a.y.min(b.y).min(c.y);
        let maxy = a.y.max(b.y).max(c.y);

        let ix0 = (((minx - self.min.x) / self.cell).floor() as isize).max(0) as usize;
        let iy0 = (((miny - self.min.y) / self.cell).floor() as isize).max(0) as usize;
        let ix1 = ((((maxx - self.min.x) / self.cell).ceil() as isize).min(self.nx as isize - 1)).max(0) as usize;
        let iy1 = ((((maxy - self.min.y) / self.cell).ceil() as isize).min(self.ny as isize - 1)).max(0) as usize;

        for iy in iy0..=iy1 {
            for ix in ix0..=ix1 {
                let px = self.min.x + ix as f64 * self.cell;
                let py = self.min.y + iy as f64 * self.cell;
                if let Some((u, v, w)) = bary(px, py, a.x, a.y, b.x, b.y, c.x, c.y) {
                    let z = u * a.z + v * b.z + w * c.z;
                    let k = self.idx(ix, iy);
                    if z > self.z[k] {
                        self.z[k] = z;
                    }
                }
            }
        }
    }

    /// The height at a cell; outside the grid it is the floor.
    pub fn at(&self, ix: isize, iy: isize) -> f64 {
        if ix < 0 || iy < 0 || ix as usize >= self.nx || iy as usize >= self.ny {
            self.floor
        } else {
            self.z[self.idx(ix as usize, iy as usize)]
        }
    }

    fn pt(&self, ix: usize, iy: usize) -> Point2 {
        Point2::new(self.min.x + ix as f64 * self.cell, self.min.y + iy as f64 * self.cell)
    }

    /// The iso-contours of the surface at the height `level`, by marching squares: the boundary of the area
    /// where the part is no lower than `level`. Used by waterline machining.
    pub fn iso_contours(&self, level: f64) -> Vec<Contour> {
        let mut segs: Vec<(Point2, Point2)> = Vec::new();
        let cross = |pa: Point2, va: f64, pb: Point2, vb: f64| -> Point2 {
            let t = if (vb - va).abs() < 1e-12 { 0.5 } else { ((level - va) / (vb - va)).clamp(0.0, 1.0) };
            Point2::new(pa.x + (pb.x - pa.x) * t, pa.y + (pb.y - pa.y) * t)
        };
        for iy in 0..self.ny.saturating_sub(1) {
            for ix in 0..self.nx.saturating_sub(1) {
                let (i0, i1) = (ix as isize, (ix + 1) as isize);
                let (j0, j1) = (iy as isize, (iy + 1) as isize);
                let (v00, v10, v11, v01) = (self.at(i0, j0), self.at(i1, j0), self.at(i1, j1), self.at(i0, j1));
                let (p00, p10, p11, p01) = (self.pt(ix, iy), self.pt(ix + 1, iy), self.pt(ix + 1, iy + 1), self.pt(ix, iy + 1));
                let mut code = 0u8;
                if v00 >= level { code |= 1 }
                if v10 >= level { code |= 2 }
                if v11 >= level { code |= 4 }
                if v01 >= level { code |= 8 }
                let bottom = || cross(p00, v00, p10, v10);
                let right = || cross(p10, v10, p11, v11);
                let top = || cross(p01, v01, p11, v11);
                let left = || cross(p00, v00, p01, v01);
                match code {
                    1 | 14 => segs.push((left(), bottom())),
                    2 | 13 => segs.push((bottom(), right())),
                    3 | 12 => segs.push((left(), right())),
                    4 | 11 => segs.push((right(), top())),
                    6 | 9 => segs.push((bottom(), top())),
                    7 | 8 => segs.push((left(), top())),
                    5 => {
                        segs.push((left(), bottom()));
                        segs.push((right(), top()));
                    }
                    10 => {
                        segs.push((left(), top()));
                        segs.push((bottom(), right()));
                    }
                    _ => {}
                }
            }
        }
        crate::geom::stitch_segments(segs, self.cell * 0.25)
    }

    /// Drop the tool at the point (x, y): the height of the tip according to its shape. `ball` selects a
    /// ball-nosed cutter; otherwise it is flat.
    pub fn drop_tool(&self, x: f64, y: f64, radius: f64, ball: bool) -> f64 {
        let cx = ((x - self.min.x) / self.cell).round() as isize;
        let cy = ((y - self.min.y) / self.cell).round() as isize;
        let rcells = (radius / self.cell).ceil() as isize;
        let mut best = self.floor;
        for dy in -rcells..=rcells {
            for dx in -rcells..=rcells {
                let ox = dx as f64 * self.cell;
                let oy = dy as f64 * self.cell;
                let d2 = ox * ox + oy * oy;
                if d2 > radius * radius {
                    continue;
                }
                let h = self.at(cx + dx, cy + dy);
                let shape = if ball {
                    let d = d2.sqrt();
                    radius - (radius * radius - d * d).max(0.0).sqrt()
                } else {
                    0.0
                };
                let cand = h - shape;
                if cand > best {
                    best = cand;
                }
            }
        }
        best
    }
}

/// The barycentric coordinates of a point in a triangle, in 2D. `None` means outside.
#[allow(clippy::too_many_arguments)]
fn bary(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64) -> Option<(f64, f64, f64)> {
    let v0x = bx - ax;
    let v0y = by - ay;
    let v1x = cx - ax;
    let v1y = cy - ay;
    let v2x = px - ax;
    let v2y = py - ay;
    let d00 = v0x * v0x + v0y * v0y;
    let d01 = v0x * v1x + v0y * v1y;
    let d11 = v1x * v1x + v1y * v1y;
    let d20 = v2x * v0x + v2y * v0y;
    let d21 = v2x * v1x + v2y * v1y;
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-12 {
        return None;
    }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    let u = 1.0 - v - w;
    let e = -1e-9;
    if u >= e && v >= e && w >= e {
        Some((u, v, w))
    } else {
        None
    }
}
