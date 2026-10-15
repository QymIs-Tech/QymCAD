//! Design For Manufacturing (DFM) analysis for additive and subtractive processes.

use serde::{Deserialize, Serialize};

/// Target manufacturing process for DFM evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManufacturingProcess {
    /// Fused Deposition Modeling / FFF 3D printing.
    FdmPrinting,
    /// Stereolithography / resin 3D printing.
    SlaPrinting,
    /// 3-axis CNC vertical milling.
    CncMilling3Axis,
    /// Sheet metal stamping and laser cutting.
    SheetMetal,
}

/// Severity classification of a DFM diagnostic finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    /// Meets or exceeds manufacturing rules.
    Info,
    /// Marginal feature that may cause print defects or chatter.
    Warning,
    /// Geometric rule violation that will cause print or machining failure.
    Critical,
}

/// A single actionable finding from a DFM check.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DfmFinding {
    /// Identifier of the rule (e.g. "overhang_angle", "min_wall_thickness").
    pub rule: String,
    /// Diagnostic finding severity.
    pub severity: Severity,
    /// Concise description of what was measured and why it matters.
    pub message: String,
    /// Approximate 3D coordinates where the issue is concentrated.
    pub location: Option<[f64; 3]>,
    /// Measurable metric found (e.g. angle in deg, thickness in mm).
    pub measured_value: f64,
    /// Recommended threshold or parameter.
    pub threshold_value: f64,
}

/// Overall report produced by running DFM analysis on a part.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DfmReport {
    /// Analyzed process.
    pub process: Option<ManufacturingProcess>,
    /// List of individual diagnostic findings.
    pub findings: Vec<DfmFinding>,
    /// Estimated percentage of surface area requiring support structures.
    pub support_area_ratio: f64,
    /// Overall readiness flag.
    pub is_manufacturable: bool,
}

/// Configuration thresholds for DFM checks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DfmConfig {
    /// Critical overhang angle in degrees from vertical (default 45.0).
    pub critical_overhang_deg: f64,
    /// Minimum allowed wall thickness in mm (default 1.2 mm for FDM, 0.8 mm for CNC).
    pub min_wall_thickness_mm: f64,
    /// Minimum internal radius for CNC tool accessibility in mm (default 1.5 mm).
    pub min_internal_radius_mm: f64,
    /// Maximum unsupported bridge span in mm (default 10.0 mm).
    pub max_bridge_span_mm: f64,
}

impl Default for DfmConfig {
    fn default() -> Self {
        Self { critical_overhang_deg: 45.0, min_wall_thickness_mm: 1.2, min_internal_radius_mm: 1.5, max_bridge_span_mm: 10.0 }
    }
}

/// Performs DFM geometric analysis on a set of surface triangles.
pub fn analyze_triangles_dfm(vertices: &[[f64; 3]], indices: &[[u32; 3]], process: ManufacturingProcess, config: &DfmConfig) -> DfmReport {
    let mut findings = Vec::new();
    let mut total_area = 0.0;
    let mut support_area = 0.0;

    let up = [0.0, 0.0, 1.0];

    for tri in indices {
        let p0 = vertices[tri[0] as usize];
        let p1 = vertices[tri[1] as usize];
        let p2 = vertices[tri[2] as usize];

        let v1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let v2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

        let nx = v1[1] * v2[2] - v1[2] * v2[1];
        let ny = v1[2] * v2[0] - v1[0] * v2[2];
        let nz = v1[0] * v2[1] - v1[1] * v2[0];
        let n_len = (nx * nx + ny * ny + nz * nz).sqrt();

        if n_len < 1e-9 {
            continue;
        }

        let area = 0.5 * n_len;
        total_area += area;

        let normal = [nx / n_len, ny / n_len, nz / n_len];
        let dot_up = normal[0] * up[0] + normal[1] * up[1] + normal[2] * up[2];

        // Downward facing surface
        if dot_up < -1e-4 {
            // Angle of the surface plane relative to horizontal bed (0 deg = flat ceiling, 90 deg = vertical wall).
            let angle_from_horizontal = (-dot_up).min(1.0).acos().to_degrees();
            if angle_from_horizontal < config.critical_overhang_deg {
                support_area += area;
            }
        }
    }

    let support_ratio = if total_area > 1e-6 { support_area / total_area } else { 0.0 };

    if process == ManufacturingProcess::FdmPrinting && support_ratio > 0.15 {
        findings.push(DfmFinding {
            rule: "overhang_support_needed".into(),
            severity: Severity::Warning,
            message: format!("Surface requires {:.1}% support area. Consider reorienting part.", support_ratio * 100.0),
            location: None,
            measured_value: support_ratio * 100.0,
            threshold_value: 15.0,
        });
    }

    let is_manufacturable = !findings.iter().any(|f| f.severity == Severity::Critical);

    DfmReport { process: Some(process), findings, support_area_ratio: support_ratio, is_manufacturable }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_upward_triangle_needs_no_support() {
        let vertices = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]];
        let indices = vec![[0, 1, 2]];
        let report = analyze_triangles_dfm(&vertices, &indices, ManufacturingProcess::FdmPrinting, &DfmConfig::default());
        assert_eq!(report.support_area_ratio, 0.0);
        assert!(report.is_manufacturable);
    }

    #[test]
    fn downward_facing_triangle_detects_support() {
        // Normal pointing downwards (-Z)
        let vertices = vec![[0.0, 0.0, 10.0], [0.0, 10.0, 10.0], [10.0, 0.0, 10.0]];
        let indices = vec![[0, 1, 2]];
        let report = analyze_triangles_dfm(&vertices, &indices, ManufacturingProcess::FdmPrinting, &DfmConfig::default());
        assert!(report.support_area_ratio > 0.99);
    }
}
