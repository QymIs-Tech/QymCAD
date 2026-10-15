//! Manufacturing cost estimation engine.

use serde::{Deserialize, Serialize};

/// Detailed breakdown of estimated manufacturing costs.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CostBreakdown {
    /// Raw material cost based on mass and material price.
    pub material_usd: f64,
    /// Machine operational time cost (depreciation + maintenance).
    pub machine_usd: f64,
    /// Electricity and utility cost during run.
    pub energy_usd: f64,
    /// Setup, programming, and operator labor cost.
    pub labor_usd: f64,
    /// Post-processing (support removal, deburring, surface finish).
    pub post_processing_usd: f64,
    /// Total cost per single manufactured unit.
    pub unit_total_usd: f64,
    /// Total cost for the requested production quantity.
    pub batch_total_usd: f64,
}

/// Rates and parameters for manufacturing cost estimation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CostModelRates {
    /// Machine hourly operational cost in USD/hour.
    pub machine_hourly_rate: f64,
    /// Electricity cost per kilowatt-hour in USD/kWh.
    pub electricity_kwh_rate: f64,
    /// Machine power draw in kilowatts (kW).
    pub machine_power_kw: f64,
    /// Operator hourly rate in USD/hour.
    pub labor_hourly_rate: f64,
    /// One-time job setup time in hours.
    pub setup_time_hours: f64,
    /// Post-processing time per unit in hours.
    pub post_processing_hours_per_unit: f64,
}

impl Default for CostModelRates {
    fn default() -> Self {
        Self { machine_hourly_rate: 4.50, electricity_kwh_rate: 0.16, machine_power_kw: 0.35, labor_hourly_rate: 28.0, setup_time_hours: 0.25, post_processing_hours_per_unit: 0.10 }
    }
}

/// Parameters describing a production run for cost estimation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ProductionRun {
    /// Raw material cost in USD for one unit.
    pub raw_material_cost_usd: f64,
    /// Estimated machine cycle time in hours for one unit.
    pub cycle_time_hours: f64,
    /// Total number of units in the production batch.
    pub quantity: u32,
}

/// Computes a full cost breakdown for a manufacturing run.
pub fn estimate_manufacturing_cost(run: ProductionRun, rates: &CostModelRates) -> CostBreakdown {
    let qty = (run.quantity.max(1)) as f64;

    let machine_per_unit = run.cycle_time_hours * rates.machine_hourly_rate;
    let energy_per_unit = run.cycle_time_hours * rates.machine_power_kw * rates.electricity_kwh_rate;
    let post_proc_per_unit = rates.post_processing_hours_per_unit * rates.labor_hourly_rate;

    let total_setup_cost = rates.setup_time_hours * rates.labor_hourly_rate;
    let setup_per_unit = total_setup_cost / qty;

    let unit_total = run.raw_material_cost_usd + machine_per_unit + energy_per_unit + setup_per_unit + post_proc_per_unit;
    let batch_total = unit_total * qty;

    CostBreakdown {
        material_usd: run.raw_material_cost_usd,
        machine_usd: machine_per_unit,
        energy_usd: energy_per_unit,
        labor_usd: setup_per_unit,
        post_processing_usd: post_proc_per_unit,
        unit_total_usd: unit_total,
        batch_total_usd: batch_total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_unit_cost_calculation() {
        let rates = CostModelRates::default();
        let run = ProductionRun { raw_material_cost_usd: 5.0, cycle_time_hours: 2.0, quantity: 1 };
        let cost = estimate_manufacturing_cost(run, &rates);
        assert!(cost.unit_total_usd > 5.0);
        assert_eq!(cost.unit_total_usd, cost.batch_total_usd);
    }

    #[test]
    fn batch_quantity_amortizes_setup() {
        let rates = CostModelRates::default();
        let single_run = ProductionRun { raw_material_cost_usd: 5.0, cycle_time_hours: 1.0, quantity: 1 };
        let batch_run = ProductionRun { raw_material_cost_usd: 5.0, cycle_time_hours: 1.0, quantity: 10 };
        let single = estimate_manufacturing_cost(single_run, &rates);
        let batch_10 = estimate_manufacturing_cost(batch_run, &rates);
        assert!(batch_10.unit_total_usd < single.unit_total_usd);
    }
}
