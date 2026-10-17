//! The edit set that changes the document units, keeping sizes on request (model.md section 3.12).

use std::collections::BTreeMap;

use crate::expr::first_positions;
use crate::{
    Document, Edit, Expr, ExprLink, FeatureInspector, FeatureRecord, FieldUnit, Parameter, Unit,
    Units,
};

impl Document {
    /// The edits that set the document units to `units`; with `keep_sizes`, also the edits that
    /// add the current unit to every unit-dependent bare number, so those sizes do not change
    /// (model.md section 3.12). They form one transaction for `UndoLog::commit`; the document is
    /// not changed.
    pub fn units_edits(
        &self,
        units: Units,
        keep_sizes: bool,
        inspector: &dyn FeatureInspector,
    ) -> Vec<Edit> {
        if units == self.units {
            return Vec::new();
        }
        let mut edits = vec![Edit::SetUnits { units }];
        if keep_sizes {
            edits.extend(BareNumbers::of(self, inspector).edits(self.units.unit()));
        }
        edits
    }

    /// How many unit-dependent bare numbers the document has: the expressions `units_edits` with
    /// `keep_sizes` rewrites (model.md section 3.12).
    pub fn bare_numbers(&self, inspector: &dyn FeatureInspector) -> usize {
        BareNumbers::of(self, inspector).count()
    }
}

/// The unit-dependent bare numbers of a document, which Keep sizes rewrites.
struct BareNumbers<'a> {
    /// The parameters to rewrite, in document order, each with the byte offset just after its
    /// number literal.
    parameters: Vec<(&'a Parameter, usize)>,
    /// The feature records to rewrite, in timeline order, each with the links to rewrite and the
    /// byte offset just after each one's number literal.
    features: Vec<(&'a FeatureRecord, Vec<(ExprLink, usize)>)>,
}

impl<'a> BareNumbers<'a> {
    /// Finds them, parsing every expression once. Only records whose inspection succeeds take
    /// part, suppressed and rolled-back ones included; expressions that do not parse are neither
    /// rewritten nor followed.
    fn of(document: &'a Document, inspector: &dyn FeatureInspector) -> BareNumbers<'a> {
        let positions = first_positions(&document.parameters);
        let analyses: Vec<Option<Analysis>> = document
            .parameters
            .iter()
            .map(|parameter| Analysis::of(&parameter.expr, &positions))
            .collect();
        let mut starts = Vec::new();
        let mut features = Vec::new();
        for feature in &document.features {
            let Ok(links) = inspector.inspect(feature) else {
                continue;
            };
            let analysis = RecordAnalysis::of(feature, links.expressions, &positions);
            starts.extend(analysis.starts);
            if !analysis.numbers.is_empty() {
                features.push((feature, analysis.numbers));
            }
        }
        let uses: Vec<&[Use]> = analyses
            .iter()
            .map(|analysis| analysis.as_ref().map_or(&[][..], |analysis| &analysis.uses))
            .collect();
        let parameters = document
            .parameters
            .iter()
            .zip(&analyses)
            .zip(reach(&uses, starts))
            .filter_map(|((parameter, analysis), ways)| {
                let end = analysis.as_ref()?.bare_number_end?;
                ways.is_size_only().then_some((parameter, end))
            })
            .collect();
        BareNumbers {
            parameters,
            features,
        }
    }

    /// How many expressions they are.
    fn count(&self) -> usize {
        let links: usize = self.features.iter().map(|(_, numbers)| numbers.len()).sum();
        self.parameters.len() + links
    }

    /// One `UpdateParameter` per parameter, then one `UpdateFeature` per record, each a copy of
    /// the record with every bare number written with `unit`.
    fn edits(self, unit: Unit) -> impl Iterator<Item = Edit> {
        let parameters =
            self.parameters
                .into_iter()
                .map(move |(parameter, end)| Edit::UpdateParameter {
                    parameter: Parameter {
                        expr: parameter.expr.with_unit_at(end, unit),
                        ..parameter.clone()
                    },
                });
        let features = self.features.into_iter().map(move |(feature, numbers)| {
            let mut updated = feature.clone();
            for (link, end) in &numbers {
                link.replace_in(&mut updated.params, &link.expr.with_unit_at(*end, unit));
            }
            Edit::UpdateFeature { feature: updated }
        });
        parameters.chain(features)
    }
}

/// What Keep sizes needs to know about the links of one feature record.
struct RecordAnalysis {
    /// The links whose bare numbers it rewrites, each with the byte offset just after its number
    /// literal.
    numbers: Vec<(ExprLink, usize)>,
    /// The parameters the links name directly, each with the way a link reaches it (the starts
    /// of `reach`).
    starts: Vec<(usize, Way)>,
}

impl RecordAnalysis {
    /// The analysis of `links`, the links of `feature`, whose names are looked up in `positions`.
    fn of(
        feature: &FeatureRecord,
        links: Vec<ExprLink>,
        positions: &BTreeMap<&str, usize>,
    ) -> RecordAnalysis {
        let mut numbers = Vec::new();
        let mut starts = Vec::new();
        for link in links {
            let Some(analysis) = Analysis::of(&link.expr, positions) else {
                continue;
            };
            let is_length = link.unit == FieldUnit::Length;
            let way = if is_length { Way::Size } else { Way::Other };
            starts.extend(onward(&analysis.uses, way));
            if let Some(end) = analysis.bare_number_end
                && is_length
                && link.is_found_in(&feature.params)
            {
                numbers.push((link, end));
            }
        }
        RecordAnalysis { numbers, starts }
    }
}

/// What Keep sizes needs to know about one expression, from a single parse.
struct Analysis {
    /// For a bare number, the byte offset just after its number literal.
    bare_number_end: Option<usize>,
    /// The parameters the expression names, in first-use order.
    uses: Vec<Use>,
}

impl Analysis {
    /// The analysis of `expr`, whose names are looked up in `positions`; `None` when it does not
    /// parse. Dotted names and names that are not parameters are not uses.
    fn of(expr: &Expr, positions: &BTreeMap<&str, usize>) -> Option<Analysis> {
        let tree = expr.parse().ok()?;
        let mut uses: Vec<Use> = Vec::new();
        tree.visit_parameters(&mut |name, promoting| {
            let Some(&parameter) = positions.get(name) else {
                return;
            };
            match uses.iter_mut().find(|used| used.parameter == parameter) {
                Some(used) => used.promoted &= promoting,
                None => uses.push(Use {
                    parameter,
                    promoted: promoting,
                }),
            }
        });
        Some(Analysis {
            bare_number_end: tree.bare_number_end(expr.as_str()),
            uses,
        })
    }
}

/// A parameter that an expression names.
#[derive(Clone, Copy)]
struct Use {
    /// The parameter's position in the document.
    parameter: usize,
    /// True when every occurrence of its name is in `Expr::promoted_names`.
    promoted: bool,
}

/// How a link reaches a parameter.
#[derive(Clone, Copy)]
enum Way {
    /// From a `Length` link, with every name on the way promoted: the parameter is a size there.
    Size,
    /// From a `Length` link through a name that is not promoted: a factor, a ratio or an argument.
    Factor,
    /// From a link with another unit.
    Other,
}

impl Way {
    /// The way on through a use that is promoted or not.
    fn through(self, promoted: bool) -> Way {
        match self {
            Way::Size if promoted => Way::Size,
            Way::Size | Way::Factor => Way::Factor,
            Way::Other => Way::Other,
        }
    }
}

/// The ways links reach one parameter.
#[derive(Clone, Copy, Default)]
struct Ways {
    /// Reached as a size.
    size: bool,
    /// Reached as a factor.
    factor: bool,
    /// Reached by another unit.
    other: bool,
}

impl Ways {
    /// Records `way` and returns true when it is new.
    fn insert(&mut self, way: Way) -> bool {
        let seen = match way {
            Way::Size => &mut self.size,
            Way::Factor => &mut self.factor,
            Way::Other => &mut self.other,
        };
        !std::mem::replace(seen, true)
    }

    /// True for a parameter that is a size wherever links reach it, so a unit keeps all its uses.
    fn is_size_only(self) -> bool {
        self.size && !self.factor && !self.other
    }
}

/// The parameters `uses` name, each with the way on from `way` through its use.
fn onward(uses: &[Use], way: Way) -> impl Iterator<Item = (usize, Way)> + '_ {
    uses.iter()
        .map(move |used| (used.parameter, way.through(used.promoted)))
}

/// The ways links reach each parameter, from the parameters the links name directly (`starts`),
/// following `uses` of each parameter. Each parameter is followed at most once per way, so
/// cycles end.
fn reach(uses: &[&[Use]], starts: Vec<(usize, Way)>) -> Vec<Ways> {
    let mut reached = vec![Ways::default(); uses.len()];
    let mut pending = starts;
    while let Some((parameter, way)) = pending.pop() {
        if reached[parameter].insert(way) {
            pending.extend(onward(uses[parameter], way));
        }
    }
    reached
}

#[cfg(test)]
mod tests {
    use super::*;

    fn used(parameter: usize, promoted: bool) -> Use {
        Use {
            parameter,
            promoted,
        }
    }

    #[test]
    fn a_size_stays_a_size_only_through_promoted_uses() {
        assert!(matches!(Way::Size.through(true), Way::Size));
        assert!(matches!(Way::Size.through(false), Way::Factor));
        assert!(matches!(Way::Factor.through(true), Way::Factor));
        assert!(matches!(Way::Other.through(true), Way::Other));
    }

    #[test]
    fn reaching_follows_chains_and_ends_in_cycles() {
        let first = [used(1, true)];
        let second = [used(0, true), used(2, false)];
        let uses: [&[Use]; 3] = [&first, &second, &[]];

        let reached = reach(&uses, vec![(0, Way::Size)]);

        assert!(reached[0].is_size_only());
        assert!(reached[1].is_size_only());
        assert!(reached[2].factor && !reached[2].size);
    }

    #[test]
    fn a_parameter_another_unit_also_reaches_is_not_only_a_size() {
        let uses: [&[Use]; 1] = [&[]];

        let reached = reach(&uses, vec![(0, Way::Size), (0, Way::Other)]);

        assert!(reached[0].size && !reached[0].is_size_only());
    }

    #[test]
    fn analysis_merges_repeated_names_and_skips_unknown_ones() {
        let positions = BTreeMap::from([("wall", 0)]);

        let analysis = Analysis::of(&Expr::new("wall + wall * wall + other"), &positions)
            .expect("the expression parses");

        assert_eq!(analysis.bare_number_end, None);
        assert_eq!(analysis.uses.len(), 1);
        assert!(!analysis.uses[0].promoted);
    }
}
