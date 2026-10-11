//! The print health summary over all analyzed bodies (spec sections 2.7 and 3.10).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::format::format_count;
use crate::report::{BodyReport, Health, Severity};

/// The status bar badge for everything analyzed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HealthSummary {
    /// Worst health over the bodies; `ok` when there are none.
    pub health: Health,
    /// Error findings, including omitted ones.
    pub errors: u32,
    /// Warning findings, including omitted ones.
    pub warnings: u32,
    /// Info findings, including omitted ones.
    pub infos: u32,
    /// Badge text (section 3.10).
    pub label: String,
}

/// Sums up the reports of all analyzed bodies.
pub fn summarize(reports: &[&BodyReport]) -> HealthSummary {
    let health = reports
        .iter()
        .map(|report| report.health)
        .max()
        .unwrap_or(Health::Ok);
    let mut counts = SeverityCounts::default();
    for result in reports.iter().flat_map(|report| &report.checks) {
        for finding in &result.findings {
            counts.add(finding.severity, 1);
        }
        // Omitted findings share the severity of the listed ones.
        if let Some(last) = result.findings.last() {
            counts.add(last.severity, result.omitted);
        }
    }
    HealthSummary {
        health,
        errors: counts.errors,
        warnings: counts.warnings,
        infos: counts.infos,
        label: label(counts.errors, counts.warnings),
    }
}

/// Findings counted by severity.
#[derive(Clone, Copy, Debug, Default)]
struct SeverityCounts {
    /// Error findings.
    errors: u32,
    /// Warning findings.
    warnings: u32,
    /// Info findings.
    infos: u32,
}

impl SeverityCounts {
    /// Adds `count` findings of one severity.
    fn add(&mut self, severity: Severity, count: u32) {
        let total = match severity {
            Severity::Error => &mut self.errors,
            Severity::Warning => &mut self.warnings,
            Severity::Info => &mut self.infos,
        };
        *total = total.saturating_add(count);
    }
}

/// `"{e} errors, {w} warnings"`, `"{e} errors"`, `"{w} warnings"` or `"Ready to print"`, with
/// singulars for 1.
fn label(errors: u32, warnings: u32) -> String {
    let error_text = format_count(errors as usize, "error", "errors");
    let warning_text = format_count(warnings as usize, "warning", "warnings");
    match (errors, warnings) {
        (0, 0) => "Ready to print".to_string(),
        (_, 0) => error_text,
        (0, _) => warning_text,
        _ => format!("{error_text}, {warning_text}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_names_errors_and_warnings() {
        assert_eq!(label(0, 0), "Ready to print");
        assert_eq!(label(1, 0), "1 error");
        assert_eq!(label(3, 0), "3 errors");
        assert_eq!(label(0, 1), "1 warning");
        assert_eq!(label(1, 1), "1 error, 1 warning");
        assert_eq!(label(2, 5), "2 errors, 5 warnings");
    }
}
