//! The `printer.*` expression values (spec sections 2.3 and 3.5).

use partsmith_model::{EvaluationContext, Quantity};

use crate::profile::{FitClasses, PrinterProfile};

/// Every name a profile can supply, sorted by byte order.
const NAMES: [&str; 6] = [
    "printer.fit.loose",
    "printer.fit.press",
    "printer.fit.slip",
    "printer.fit.snug",
    "printer.layer_height",
    "printer.nozzle",
];

/// The `printer.*` values of one profile, as an expression evaluation context.
#[derive(Clone, Debug, PartialEq)]
pub struct PrinterValues {
    /// The profile's display name, for the reason of `printer.nozzle` on resin.
    profile_name: String,
    /// Fit class clearances in mm.
    fit: FitClasses,
    /// Layer height in mm.
    layer_height: f64,
    /// Nozzle diameter in mm; `None` for resin.
    nozzle_diameter: Option<f64>,
}

impl PrinterValues {
    /// Captures the values of a profile.
    pub fn new(profile: &PrinterProfile) -> PrinterValues {
        PrinterValues {
            profile_name: profile.name.clone(),
            fit: profile.fit,
            layer_height: profile.layer_height,
            nozzle_diameter: profile.nozzle_diameter,
        }
    }

    /// The value in mm of a supported name, matched exactly.
    fn millimeters(&self, name: &str) -> Option<f64> {
        match name {
            "printer.fit.press" => Some(self.fit.press),
            "printer.fit.snug" => Some(self.fit.snug),
            "printer.fit.slip" => Some(self.fit.slip),
            "printer.fit.loose" => Some(self.fit.loose),
            "printer.layer_height" => Some(self.layer_height),
            "printer.nozzle" => self.nozzle_diameter,
            _ => None,
        }
    }
}

impl EvaluationContext for PrinterValues {
    /// `Some(Quantity::length(v))` for a supported name (section 3.5), `None` for any other name.
    fn resolve(&self, name: &str) -> Option<Quantity> {
        self.millimeters(name).map(Quantity::length)
    }

    /// The supported names, sorted by byte order.
    fn names(&self) -> Vec<String> {
        NAMES
            .into_iter()
            .filter(|name| self.millimeters(name).is_some())
            .map(String::from)
            .collect()
    }

    /// Why `printer.nozzle` has no value on a resin profile (section 3.5); `None` for every other name.
    fn reason(&self, name: &str) -> Option<String> {
        (name == "printer.nozzle" && self.nozzle_diameter.is_none()).then(|| {
            format!(
                "printer.nozzle has no value with '{}', a resin printer with no nozzle. Pick an FDM printer, or write a length instead.",
                self.profile_name
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{generic_fdm, generic_resin};

    #[test]
    fn the_name_list_is_sorted_by_bytes() {
        let mut sorted = NAMES;
        sorted.sort_unstable();
        assert_eq!(NAMES, sorted);
    }

    #[test]
    fn every_listed_name_resolves() {
        for profile in [generic_fdm(), generic_resin()] {
            let values = PrinterValues::new(&profile);
            for name in values.names() {
                assert!(values.resolve(&name).is_some(), "{name}");
            }
        }
    }

    #[test]
    fn values_are_copies_of_the_profile() {
        let mut profile = generic_fdm();
        profile.fit.loose = 0.5;
        let values = PrinterValues::new(&profile);
        profile.fit.loose = 0.6;
        assert_eq!(
            values.resolve("printer.fit.loose"),
            Some(Quantity::length(0.5))
        );
    }

    #[test]
    fn trailing_whitespace_does_not_match() {
        let values = PrinterValues::new(&generic_fdm());
        assert_eq!(values.resolve("printer.nozzle "), None);
    }
}
