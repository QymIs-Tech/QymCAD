//! Choosing feeds and speeds from the material and the tool.
//!
//! A simple engineering model: the spindle speed comes from the cutting speed Vc and the feed rate from the chip
//! load. The values are sensible defaults for a router or a mill and are meant to be adjusted to the machine and
//! the tool at hand.

/// A material and its baseline cutting data.
#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub name: &'static str,
    /// The cutting speed Vc, in metres per minute.
    pub vc: f64,
    /// The chip load at a diameter of 6 mm, in mm per tooth.
    pub chip: f64,
}

/// The built-in table of materials.
pub fn materials() -> &'static [Material] {
    &[
        Material { name: "material-foam", vc: 350.0, chip: 0.12 },
        Material { name: "material-wood", vc: 250.0, chip: 0.10 },
        Material { name: "material-plastic", vc: 200.0, chip: 0.08 },
        Material { name: "material-aluminium", vc: 150.0, chip: 0.05 },
        Material { name: "material-brass", vc: 120.0, chip: 0.05 },
        Material { name: "material-steel", vc: 60.0, chip: 0.03 },
    ]
}

/// A recommendation of cutting data.
#[derive(Clone, Copy, Debug)]
pub struct Recommendation {
    pub rpm: f64,
    /// The cutting feed rate, in mm per minute.
    pub feed: f64,
    /// The plunge feed rate, in mm per minute.
    pub plunge: f64,
}

/// Choose the spindle speed and the feed rates for a material, a cutter diameter and a number of flutes.
pub fn recommend(m: &Material, diameter: f64, flutes: u32) -> Recommendation {
    let d = diameter.max(0.1);
    let flutes = flutes.max(1) as f64;
    // the speed from the cutting speed: n = Vc·1000 / (π·D)
    let rpm = (m.vc * 1000.0) / (std::f64::consts::PI * d);
    // the chip load scales slightly with diameter: a thinner cutter takes a smaller chip
    let chip = m.chip * (d / 6.0).sqrt().clamp(0.4, 1.6);
    let feed = rpm * flutes * chip;
    Recommendation { rpm, feed, plunge: feed * 0.3 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aluminium_6mm_2flute_in_range() {
        let alu = materials().iter().find(|m| m.name.starts_with("material-aluminium")).unwrap();
        let r = recommend(alu, 6.0, 2);
        // n is about 150000/(π·6), roughly 7958 rpm
        assert!((r.rpm - 7958.0).abs() < 50.0, "rpm={}", r.rpm);
        // the feed is about 7958·2·0.05, roughly 796 mm/min
        assert!(r.feed > 600.0 && r.feed < 1000.0, "feed={}", r.feed);
        assert!((r.plunge - r.feed * 0.3).abs() < 1e-6);
    }

    #[test]
    fn thinner_tool_higher_rpm() {
        let wood = &materials()[1];
        let big = recommend(wood, 12.0, 2);
        let small = recommend(wood, 3.0, 2);
        assert!(small.rpm > big.rpm, "a thinner cutter runs faster");
    }
}
