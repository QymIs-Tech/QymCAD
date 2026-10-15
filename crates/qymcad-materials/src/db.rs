//! Physical and manufacturing properties database for engineering materials.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// General engineering classification of a material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MaterialCategory {
    Metal,
    Polymer,
    Composite,
    Ceramic,
    Other,
}

impl MaterialCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Metal => "Metal",
            Self::Polymer => "Polymer",
            Self::Composite => "Composite",
            Self::Ceramic => "Ceramic",
            Self::Other => "Other",
        }
    }
}

/// Fundamental physical, mechanical, and thermal properties.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PhysicalProperties {
    /// Mass density in kg/m^3 (e.g. 2700 for Al 6061, 7850 for steel, 1240 for PLA).
    pub density: f64,
    /// Young's modulus of elasticity in GPa.
    pub youngs_modulus: f64,
    /// Poisson's ratio (dimensionless, typically 0.25 - 0.45).
    pub poissons_ratio: f64,
    /// Yield strength in MPa (0.2% offset).
    pub yield_strength: f64,
    /// Ultimate tensile strength in MPa.
    pub ultimate_strength: f64,
    /// Thermal conductivity in W/(m*K).
    pub thermal_conductivity: f64,
    /// Coefficient of thermal expansion in 10^-6 / K (ppm/K).
    pub cte: f64,
    /// Specific heat capacity in J/(kg*K).
    pub specific_heat: f64,
}

/// Manufacturing and economic characteristics.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManufacturingProperties {
    /// Relative machinability index (0.0 to 1.0, where 1.0 is free-cutting brass).
    pub machinability: f64,
    /// 3D printability suitability (0.0 to 1.0, where 1.0 is standard PLA).
    pub printability: f64,
    /// Estimated raw material cost per kg in USD.
    pub cost_per_kg_usd: f64,
}

/// An engineering material definition.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Material {
    /// Unique identifier (e.g. "al_6061_t6").
    pub id: String,
    /// Human-readable title (e.g. "Aluminum 6061-T6").
    pub name: String,
    /// Category classification.
    pub category: MaterialCategory,
    /// Physical and mechanical attributes.
    pub physical: PhysicalProperties,
    /// Manufacturing and cost attributes.
    pub manufacturing: ManufacturingProperties,
    /// Descriptive engineering notes.
    pub notes: String,
}

/// Material registry providing standard libraries and custom definitions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialRegistry {
    materials: BTreeMap<String, Material>,
}

impl Default for MaterialRegistry {
    fn default() -> Self {
        let mut reg = Self { materials: BTreeMap::new() };
        reg.populate_standard_library();
        reg
    }
}

impl MaterialRegistry {
    /// Creates an empty registry without pre-populated materials.
    pub fn empty() -> Self {
        Self { materials: BTreeMap::new() }
    }

    /// Retrieves a material by its unique ID.
    pub fn get(&self, id: &str) -> Option<&Material> {
        self.materials.get(id)
    }

    /// Looks up a material by matching name or ID (case-insensitive, all search terms must match).
    pub fn find_by_name(&self, query: &str) -> Option<&Material> {
        let q_words: Vec<String> = query.split_whitespace().map(|s| s.to_lowercase()).collect();
        if q_words.is_empty() {
            return None;
        }
        self.materials.values().find(|m| {
            let n = m.name.to_lowercase();
            let id = m.id.to_lowercase();
            q_words.iter().all(|w| n.contains(w) || id.contains(w))
        })
    }

    /// Returns an iterator over all registered materials.
    pub fn iter(&self) -> impl Iterator<Item = &Material> {
        self.materials.values()
    }

    /// Adds or updates a material in the registry.
    pub fn register(&mut self, material: Material) {
        self.materials.insert(material.id.clone(), material);
    }

    /// Number of materials currently registered.
    pub fn len(&self) -> usize {
        self.materials.len()
    }

    /// Checks if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.materials.is_empty()
    }

    /// Populates standard engineering metals and polymers.
    fn populate_standard_library(&mut self) {
        self.register(Material {
            id: "al_6061_t6".into(),
            name: "Aluminum 6061-T6".into(),
            category: MaterialCategory::Metal,
            physical: PhysicalProperties {
                density: 2700.0,
                youngs_modulus: 68.9,
                poissons_ratio: 0.33,
                yield_strength: 276.0,
                ultimate_strength: 310.0,
                thermal_conductivity: 167.0,
                cte: 23.2,
                specific_heat: 896.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.90, printability: 0.10, cost_per_kg_usd: 5.50 },
            notes: "General-purpose aerospace and structural aluminum alloy.".into(),
        });

        self.register(Material {
            id: "al_7075_t6".into(),
            name: "Aluminum 7075-T6".into(),
            category: MaterialCategory::Metal,
            physical: PhysicalProperties {
                density: 2810.0,
                youngs_modulus: 71.7,
                poissons_ratio: 0.33,
                yield_strength: 503.0,
                ultimate_strength: 572.0,
                thermal_conductivity: 130.0,
                cte: 23.4,
                specific_heat: 960.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.70, printability: 0.05, cost_per_kg_usd: 9.80 },
            notes: "High-strength zinc-alloyed structural aluminum.".into(),
        });

        self.register(Material {
            id: "steel_1018".into(),
            name: "Carbon Steel AISI 1018".into(),
            category: MaterialCategory::Metal,
            physical: PhysicalProperties {
                density: 7870.0,
                youngs_modulus: 205.0,
                poissons_ratio: 0.29,
                yield_strength: 370.0,
                ultimate_strength: 440.0,
                thermal_conductivity: 51.9,
                cte: 11.5,
                specific_heat: 486.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.78, printability: 0.15, cost_per_kg_usd: 2.20 },
            notes: "Mild low-carbon steel for general manufacturing and shafts.".into(),
        });

        self.register(Material {
            id: "steel_4140".into(),
            name: "Chromoly Steel AISI 4140".into(),
            category: MaterialCategory::Metal,
            physical: PhysicalProperties {
                density: 7850.0,
                youngs_modulus: 210.0,
                poissons_ratio: 0.29,
                yield_strength: 655.0,
                ultimate_strength: 850.0,
                thermal_conductivity: 42.6,
                cte: 12.3,
                specific_heat: 477.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.65, printability: 0.10, cost_per_kg_usd: 3.80 },
            notes: "High-strength chromium-molybdenum alloy steel.".into(),
        });

        self.register(Material {
            id: "ss_304".into(),
            name: "Stainless Steel 304".into(),
            category: MaterialCategory::Metal,
            physical: PhysicalProperties {
                density: 8000.0,
                youngs_modulus: 193.0,
                poissons_ratio: 0.29,
                yield_strength: 215.0,
                ultimate_strength: 505.0,
                thermal_conductivity: 16.2,
                cte: 17.3,
                specific_heat: 500.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.50, printability: 0.35, cost_per_kg_usd: 6.20 },
            notes: "Austenitic corrosion-resistant stainless steel.".into(),
        });

        self.register(Material {
            id: "ss_316".into(),
            name: "Stainless Steel 316L".into(),
            category: MaterialCategory::Metal,
            physical: PhysicalProperties {
                density: 8000.0,
                youngs_modulus: 193.0,
                poissons_ratio: 0.29,
                yield_strength: 290.0,
                ultimate_strength: 580.0,
                thermal_conductivity: 16.3,
                cte: 16.0,
                specific_heat: 500.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.45, printability: 0.40, cost_per_kg_usd: 8.50 },
            notes: "Marine-grade molybdenum-bearing stainless steel.".into(),
        });

        self.register(Material {
            id: "ti_6al_4v".into(),
            name: "Titanium Grade 5 (Ti-6Al-4V)".into(),
            category: MaterialCategory::Metal,
            physical: PhysicalProperties {
                density: 4430.0,
                youngs_modulus: 113.8,
                poissons_ratio: 0.34,
                yield_strength: 880.0,
                ultimate_strength: 950.0,
                thermal_conductivity: 6.7,
                cte: 8.6,
                specific_heat: 526.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.22, printability: 0.45, cost_per_kg_usd: 45.0 },
            notes: "Aerospace and biomedical high-strength titanium alloy.".into(),
        });

        self.register(Material {
            id: "brass_c360".into(),
            name: "Free-Cutting Brass C36000".into(),
            category: MaterialCategory::Metal,
            physical: PhysicalProperties {
                density: 8500.0,
                youngs_modulus: 97.0,
                poissons_ratio: 0.31,
                yield_strength: 310.0,
                ultimate_strength: 400.0,
                thermal_conductivity: 115.0,
                cte: 20.5,
                specific_heat: 380.0,
            },
            manufacturing: ManufacturingProperties { machinability: 1.00, printability: 0.05, cost_per_kg_usd: 9.20 },
            notes: "Standard benchmark for machinability ratings.".into(),
        });

        self.register(Material {
            id: "pla".into(),
            name: "PLA (Polylactic Acid)".into(),
            category: MaterialCategory::Polymer,
            physical: PhysicalProperties {
                density: 1240.0,
                youngs_modulus: 3.5,
                poissons_ratio: 0.36,
                yield_strength: 50.0,
                ultimate_strength: 65.0,
                thermal_conductivity: 0.13,
                cte: 68.0,
                specific_heat: 1800.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.30, printability: 1.00, cost_per_kg_usd: 18.0 },
            notes: "Standard biodegradable FDM 3D printing filament.".into(),
        });

        self.register(Material {
            id: "petg".into(),
            name: "PETG (Glycol Modified Polyethylene)".into(),
            category: MaterialCategory::Polymer,
            physical: PhysicalProperties {
                density: 1270.0,
                youngs_modulus: 2.1,
                poissons_ratio: 0.38,
                yield_strength: 45.0,
                ultimate_strength: 53.0,
                thermal_conductivity: 0.20,
                cte: 60.0,
                specific_heat: 1200.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.35, printability: 0.92, cost_per_kg_usd: 20.0 },
            notes: "Durable, chemical-resistant polymer with high impact strength.".into(),
        });

        self.register(Material {
            id: "abs".into(),
            name: "ABS (Acrylonitrile Butadiene Styrene)".into(),
            category: MaterialCategory::Polymer,
            physical: PhysicalProperties {
                density: 1040.0,
                youngs_modulus: 2.3,
                poissons_ratio: 0.35,
                yield_strength: 40.0,
                ultimate_strength: 48.0,
                thermal_conductivity: 0.17,
                cte: 90.0,
                specific_heat: 1400.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.40, printability: 0.80, cost_per_kg_usd: 22.0 },
            notes: "Impact-resistant thermoplastic suitable for enclosures.".into(),
        });

        self.register(Material {
            id: "pa12_nylon".into(),
            name: "PA12 (Nylon 12)".into(),
            category: MaterialCategory::Polymer,
            physical: PhysicalProperties {
                density: 1010.0,
                youngs_modulus: 1.6,
                poissons_ratio: 0.40,
                yield_strength: 45.0,
                ultimate_strength: 55.0,
                thermal_conductivity: 0.25,
                cte: 110.0,
                specific_heat: 1600.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.45, printability: 0.75, cost_per_kg_usd: 48.0 },
            notes: "High fatigue endurance, flexible and wear-resistant engineering plastic.".into(),
        });

        self.register(Material {
            id: "polycarbonate".into(),
            name: "Polycarbonate (PC)".into(),
            category: MaterialCategory::Polymer,
            physical: PhysicalProperties {
                density: 1200.0,
                youngs_modulus: 2.4,
                poissons_ratio: 0.37,
                yield_strength: 62.0,
                ultimate_strength: 72.0,
                thermal_conductivity: 0.20,
                cte: 65.0,
                specific_heat: 1250.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.42, printability: 0.70, cost_per_kg_usd: 35.0 },
            notes: "Extremely high impact resistance and optical clarity.".into(),
        });

        self.register(Material {
            id: "peek".into(),
            name: "PEEK (Polyetheretherketone)".into(),
            category: MaterialCategory::Polymer,
            physical: PhysicalProperties {
                density: 1320.0,
                youngs_modulus: 4.0,
                poissons_ratio: 0.39,
                yield_strength: 95.0,
                ultimate_strength: 100.0,
                thermal_conductivity: 0.25,
                cte: 47.0,
                specific_heat: 1340.0,
            },
            manufacturing: ManufacturingProperties { machinability: 0.50, printability: 0.40, cost_per_kg_usd: 180.0 },
            notes: "Ultra-high performance high-temperature polymer for demanding environments.".into(),
        });
    }
}
