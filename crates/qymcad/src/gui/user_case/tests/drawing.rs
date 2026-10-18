/// A sketch tool by its number, its name in a report, and the clicks it is used with.
pub(super) struct Drawing {
    pub(super) tool: u8,
    pub(super) name: &'static str,
    pub(super) clicks: &'static [(f64, f64)],
}

pub(super) fn drawing(tool: u8, name: &'static str, clicks: &'static [(f64, f64)]) -> Drawing {
    Drawing { tool, name, clicks }
}
