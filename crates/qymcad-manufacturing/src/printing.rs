//! 3D printing preparation, build volume validation, and slicer integration adapters.

use serde::{Deserialize, Serialize};

/// 3D printer hardware build envelope and specifications.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrinterProfile {
    /// Friendly machine name (e.g. "Bambu Lab X1-Carbon", "Prusa MK4").
    pub name: String,
    /// Build volume dimensions in millimeters [X, Y, Z].
    pub build_volume_mm: [f64; 3],
    /// Default nozzle diameter in mm (e.g. 0.4).
    pub nozzle_diameter_mm: f64,
    /// Maximum heated bed temperature in deg C.
    pub max_bed_temp_c: f64,
    /// Maximum nozzle temperature in deg C.
    pub max_hotend_temp_c: f64,
    /// Enclosed build chamber capability.
    pub is_enclosed: bool,
}

impl PrinterProfile {
    /// Bambu Lab X1 / P1 series profile (256 x 256 x 256 mm).
    pub fn bambu_x1() -> Self {
        Self { name: "Bambu Lab X1-Carbon".into(), build_volume_mm: [256.0, 256.0, 256.0], nozzle_diameter_mm: 0.4, max_bed_temp_c: 110.0, max_hotend_temp_c: 300.0, is_enclosed: true }
    }

    /// Prusa MK4 profile (250 x 210 x 220 mm).
    pub fn prusa_mk4() -> Self {
        Self { name: "Prusa MK4".into(), build_volume_mm: [250.0, 210.0, 220.0], nozzle_diameter_mm: 0.4, max_bed_temp_c: 120.0, max_hotend_temp_c: 290.0, is_enclosed: false }
    }

    /// Generic open FDM profile (220 x 220 x 250 mm).
    pub fn generic_fdm() -> Self {
        Self { name: "Generic FDM (220x220x250)".into(), build_volume_mm: [220.0, 220.0, 250.0], nozzle_diameter_mm: 0.4, max_bed_temp_c: 100.0, max_hotend_temp_c: 260.0, is_enclosed: false }
    }
}

/// An axis-aligned bounding box defined by minimum and maximum corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingBox3d {
    /// Minimum corner [X, Y, Z] in millimeters.
    pub min_pt: [f64; 3],
    /// Maximum corner [X, Y, Z] in millimeters.
    pub max_pt: [f64; 3],
}

impl PrinterProfile {
    /// Checks whether an axis-aligned bounding box fits within the build volume.
    pub fn fits_bounding_box(&self, bbox: &BoundingBox3d) -> bool {
        let dx = (bbox.max_pt[0] - bbox.min_pt[0]).abs();
        let dy = (bbox.max_pt[1] - bbox.min_pt[1]).abs();
        let dz = (bbox.max_pt[2] - bbox.min_pt[2]).abs();

        dx <= self.build_volume_mm[0] && dy <= self.build_volume_mm[1] && dz <= self.build_volume_mm[2]
    }
}

/// Slicer backend target for dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlicerKind {
    /// OrcaSlicer CLI interface.
    OrcaSlicer,
    /// PrusaSlicer CLI interface.
    PrusaSlicer,
    /// BambuStudio CLI interface.
    BambuStudio,
    /// Generic RepRap G-code generator.
    GenericGcode,
}

/// Request to invoke external slicer with settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlicingJob {
    /// Path to input 3MF or STL mesh model.
    pub input_model_path: String,
    /// Destination path for generated G-code or machine file.
    pub output_gcode_path: String,
    /// Target slicer engine.
    pub slicer: SlicerKind,
    /// Selected layer height in mm (e.g. 0.20).
    pub layer_height_mm: f64,
    /// Infill percentage (0.0 to 100.0).
    pub infill_percent: f64,
    /// Enable support generation.
    pub generate_supports: bool,
}

impl SlicingJob {
    /// Formats command-line arguments to invoke the external slicer binary.
    pub fn build_cli_arguments(&self) -> Vec<String> {
        let mut args = Vec::new();
        match self.slicer {
            SlicerKind::OrcaSlicer | SlicerKind::BambuStudio => {
                args.push("--slice".into());
                args.push("0".into());
                args.push("--export-gcode".into());
                args.push(self.output_gcode_path.clone());
                args.push(self.input_model_path.clone());
            }
            SlicerKind::PrusaSlicer => {
                args.push("--slice".into());
                args.push("--export-gcode".into());
                args.push("--output".into());
                args.push(self.output_gcode_path.clone());
                args.push(self.input_model_path.clone());
            }
            SlicerKind::GenericGcode => {
                args.push("-s".into());
                args.push(self.input_model_path.clone());
                args.push("-o".into());
                args.push(self.output_gcode_path.clone());
            }
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_volume_fit_check() {
        let printer = PrinterProfile::bambu_x1();
        let fits = BoundingBox3d { min_pt: [0.0, 0.0, 0.0], max_pt: [100.0, 100.0, 100.0] };
        let overflows = BoundingBox3d { min_pt: [0.0, 0.0, 0.0], max_pt: [300.0, 100.0, 100.0] };
        assert!(printer.fits_bounding_box(&fits));
        assert!(!printer.fits_bounding_box(&overflows));
    }

    #[test]
    fn slicer_cli_arguments_formatting() {
        let job = SlicingJob {
            input_model_path: "part.stl".into(),
            output_gcode_path: "part.gcode".into(),
            slicer: SlicerKind::PrusaSlicer,
            layer_height_mm: 0.2,
            infill_percent: 20.0,
            generate_supports: true,
        };
        let args = job.build_cli_arguments();
        assert!(args.contains(&"--export-gcode".to_string()));
        assert!(args.contains(&"part.gcode".to_string()));
    }
}
