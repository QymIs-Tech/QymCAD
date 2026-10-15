//! Engineering materials database, mechanical and thermal properties, and physical mass calculation.

pub mod db;
pub mod mass_props;

pub use db::{ManufacturingProperties, Material, MaterialCategory, MaterialRegistry, PhysicalProperties};
pub use mass_props::{calculate_mass_properties, GeometryMeasurements, MassProperties};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_metals_and_polymers() {
        let reg = MaterialRegistry::default();
        assert!(reg.len() >= 10);
        let steel = reg.find_by_name("steel 1018").expect("1018 found");
        assert_eq!(steel.category, MaterialCategory::Metal);

        let pla = reg.find_by_name("PLA").expect("pla found");
        assert_eq!(pla.category, MaterialCategory::Polymer);
        assert!(pla.manufacturing.printability >= 0.9);
    }
}
