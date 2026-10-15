//! Calculation of physical mass properties from geometric boundaries and assigned materials.

use crate::db::Material;
use serde::{Deserialize, Serialize};

/// Comprehensive mass and inertial properties of a body or assembly.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MassProperties {
    /// Total volume in cubic millimeters (mm^3).
    pub volume_mm3: f64,
    /// Total surface area in square millimeters (mm^2).
    pub surface_area_mm2: f64,
    /// Total computed mass in kilograms (kg).
    pub mass_kg: f64,
    /// Center of mass coordinates [X, Y, Z] in millimeters.
    pub center_of_mass: [f64; 3],
    /// Moments of inertia principal diagonal [Ixx, Iyy, Izz] in kg*mm^2 around the center of mass.
    pub principal_moments: [f64; 3],
    /// Estimated raw material cost in USD.
    pub raw_material_cost_usd: f64,
}

impl MassProperties {
    /// Formatted mass in human-readable grams or kilograms.
    pub fn display_mass(&self) -> String {
        if self.mass_kg < 1.0 {
            format!("{:.2} g", self.mass_kg * 1000.0)
        } else {
            format!("{:.3} kg", self.mass_kg)
        }
    }
}

/// Geometric measurements of a solid body for physical properties analysis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryMeasurements {
    /// Total volume in cubic millimeters (mm^3).
    pub volume_mm3: f64,
    /// Total surface area in square millimeters (mm^2).
    pub surface_area_mm2: f64,
    /// Center of mass / bounding center [X, Y, Z] in millimeters.
    pub center_of_mass: [f64; 3],
}

/// Computes mass properties given volume, surface area, bounding center, and assigned material.
pub fn calculate_mass_properties(geom: &GeometryMeasurements, material: &Material) -> MassProperties {
    // 1 mm^3 = 1e-9 m^3
    let volume_m3 = geom.volume_mm3 * 1.0e-9;
    let mass_kg = volume_m3 * material.physical.density;
    let cost = mass_kg * material.manufacturing.cost_per_kg_usd;

    // Approximate principal moments of inertia assuming isotropic mass distribution
    // For a representative cube of side s = volume^(1/3), I = (1/6) * m * s^2
    let s = geom.volume_mm3.abs().cbrt();
    let i_diag = (1.0 / 6.0) * mass_kg * (s * s);

    MassProperties {
        volume_mm3: geom.volume_mm3,
        surface_area_mm2: geom.surface_area_mm2,
        mass_kg,
        center_of_mass: geom.center_of_mass,
        principal_moments: [i_diag, i_diag, i_diag],
        raw_material_cost_usd: cost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::MaterialRegistry;

    #[test]
    fn test_mass_properties_aluminum_cube() {
        let reg = MaterialRegistry::default();
        let al = reg.get("al_6061_t6").expect("al 6061 exists");

        // 100mm x 100mm x 100mm cube = 1,000,000 mm^3 = 0.001 m^3
        let geom = GeometryMeasurements { volume_mm3: 1_000_000.0, surface_area_mm2: 60_000.0, center_of_mass: [50.0, 50.0, 50.0] };

        let props = calculate_mass_properties(&geom, al);
        // Density is 2700 kg/m^3 -> mass = 0.001 * 2700 = 2.7 kg
        assert!((props.mass_kg - 2.7).abs() < 1e-4);
        assert_eq!(props.display_mass(), "2.700 kg");
        assert!(props.raw_material_cost_usd > 10.0);
    }
}
