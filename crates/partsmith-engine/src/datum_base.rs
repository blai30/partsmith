//! A datum plane's new base (engine.md section 3.22, *A datum plane's new base*): while later
//! features build on the sketches a datum plane passes its motion on to, a new base that changes
//! that motion is refused. `attach_plane` answers a datum plane's pick with it, and the repair
//! actions and the cause reverts that would store a refused base are left out.

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};

use partsmith_features::{
    BuildContext, BuildInputs, CancelToken, DatumResult, OutputData, PlaneRef, Values,
};
use partsmith_model::{Edit, EmptyContext, FeatureRecord, Id, ParameterValues, Scope};
use partsmith_naming::Motion;
use partsmith_sketch::SketchPlane;
use serde_json::{Map, Value};

use crate::frames::motions_match;
use crate::run::{FeatureView, RunView};
use crate::sketch::{DATUM_PLANE_TYPE, SKETCH_TYPE};
use crate::state::built_output;
use crate::uses::uses;

/// The params key of a datum plane's base.
const BASE: &str = "base";

/// The rule for the new bases of one datum plane of a run. The motion a new base must keep is
/// read once, when it is first needed.
pub(crate) struct BaseRule<'a, 'v> {
    /// The datum plane in its run.
    datum: &'a FeatureView<'v>,
    /// The motion its old base gives it while later features build on its sketches; `None` when
    /// it takes any base.
    kept: OnceCell<Option<Motion>>,
}

impl<'a, 'v> BaseRule<'a, 'v> {
    /// The rule for `datum`, a datum plane in its run.
    pub(crate) fn of(datum: &'a FeatureView<'v>) -> Self {
        BaseRule {
            datum,
            kept: OnceCell::new(),
        }
    }

    /// True when the datum plane refuses `base` as its new base: later features build on its
    /// sketches, its old base gives it a known motion, and `base` gives it another motion or
    /// none.
    pub(crate) fn refuses(&self, base: &Value) -> bool {
        let Some(kept) = self.kept.get_or_init(|| self.kept_motion()) else {
            return false;
        };
        motion_with(self.datum, base).is_none_or(|motion| !motions_match(&motion, kept))
    }

    /// True when one of `edits` updates the datum plane with a base other than its record's that
    /// it refuses.
    pub(crate) fn refuses_edits(&self, edits: &[Edit]) -> bool {
        let current = self.datum.record.params.get(BASE);
        edits.iter().any(|edit| match edit {
            Edit::UpdateFeature { feature } if feature.id == *self.datum.id() => feature
                .params
                .get(BASE)
                .is_some_and(|base| current != Some(base) && self.refuses(base)),
            _ => false,
        })
    }

    /// The motion a new base must keep: the one the old base gives the datum plane, when later
    /// features build on its sketches. The old base is its base when the open transaction began,
    /// and outside a transaction its base now.
    fn kept_motion(&self) -> Option<Motion> {
        let run = self.datum.run;
        if self.datum.record.kind != DATUM_PLANE_TYPE
            || !sketches_are_built_on(run, self.datum.id())
        {
            return None;
        }
        let old = run
            .record_before_transaction(self.datum.id())?
            .params
            .get(BASE)?;
        motion_with(self.datum, old)
    }
}

/// True when `edit` updates a datum plane of `run` with a base other than its record's that the
/// datum plane refuses.
pub(crate) fn refused_by_its_datum(run: &RunView<'_>, edit: &Edit) -> bool {
    let Edit::UpdateFeature { feature } = edit else {
        return false;
    };
    run.feature(&feature.id)
        .is_some_and(|datum| BaseRule::of(&datum).refuses_edits(std::slice::from_ref(edit)))
}

/// The base a datum plane stores for a picked plane, built by a host as for a sketch: its kind
/// and its origin plane, datum, or face's body and face, without a face's fallback.
pub(crate) fn picked_base(plane: &SketchPlane) -> PlaneRef {
    match plane {
        SketchPlane::Origin { plane } => PlaneRef::Origin { plane: *plane },
        SketchPlane::Datum { feature } => PlaneRef::Datum {
            feature: feature.clone(),
        },
        SketchPlane::Face { body, face, .. } => PlaneRef::Face {
            body: body.clone(),
            face: face.clone(),
        },
    }
}

/// True when another feature uses a sketch of `datum`: a sketch on it, or on a datum plane on
/// it at any depth (`list_uses`, section 3.24). Read from the recipe alone.
fn sketches_are_built_on(run: &RunView<'_>, datum: &Id) -> bool {
    let features = &run.document.features;
    let mut datums = BTreeSet::from([datum]);
    loop {
        let before = datums.len();
        for record in features {
            if record.kind == DATUM_PLANE_TYPE && on_one_of(record, BASE, &datums) {
                datums.insert(&record.id);
            }
        }
        if datums.len() == before {
            break;
        }
    }
    let sketches: BTreeSet<&Id> = features
        .iter()
        .filter(|record| record.kind == SKETCH_TYPE && on_one_of(record, "plane", &datums))
        .map(|record| &record.id)
        .collect();
    !sketches.is_empty()
        && uses(run.registry, run.document)
            .iter()
            .any(|used| used.user != used.feature && sketches.contains(&used.feature))
}

/// True when the plane `record` stores at `key` is `{kind: datum, feature}` with `feature` one
/// of `datums`.
fn on_one_of(record: &FeatureRecord, key: &str, datums: &BTreeSet<&Id>) -> bool {
    let Some(plane) = record.params.get(key) else {
        return false;
    };
    plane.get("kind").and_then(Value::as_str) == Some("datum")
        && plane
            .get("feature")
            .and_then(Value::as_str)
            .is_some_and(|feature| datums.iter().any(|datum| datum.as_str() == feature))
}

/// The motion the datum plane passes on with its record holding `base` alone, with no offset and
/// no flip (features/datum_plane.md section 3.2 step 4): the datum plane as the registry builds
/// it in the state just before it in its run, with the outputs of the features before it there.
/// `None` when that build fails.
fn motion_with(datum: &FeatureView<'_>, base: &Value) -> Option<Motion> {
    let run = datum.run;
    let registry = run.registry;
    let mut record = FeatureRecord::new(datum.id().clone(), DATUM_PLANE_TYPE);
    record.params = Map::from_iter([(BASE.to_string(), base.clone())]);
    let record = registry.validate_and_canonicalize(&record).ok()?.record;
    let dependencies = registry.dependencies(&record).ok()?;
    let manifest = registry.get(&record.kind)?.manifest();
    let bodies = dependencies
        .bodies_read
        .iter()
        .map(|link| (link.id.clone(), datum.state_before(&link.id)))
        .collect();
    let upstream = dependencies
        .features
        .iter()
        .filter_map(|link| {
            let used = run
                .feature(&link.id)
                .filter(|used| used.position < datum.position)?;
            built_output(used.report)?;
            Some((link.id.clone(), used.report.output.clone()?))
        })
        .collect();
    let parameters = ParameterValues::default();
    let scope = Scope {
        units: run.document.units,
        parameters: &parameters,
        context: &EmptyContext,
    };
    let values = Values::evaluate(&dependencies.expressions, &scope, manifest);
    // A datum plane reads no writer chain and no asset, and nothing cancels this build.
    let (history, assets, cancel) = (BTreeMap::new(), BTreeMap::new(), CancelToken::new());
    let context = BuildContext::new(BuildInputs {
        feature: &record.id,
        manifest,
        units: run.document.units,
        values: &values,
        bodies: &bodies,
        history: &history,
        upstream: &upstream,
        assets: &assets,
        cancel: &cancel,
    });
    match registry.build(&record, &context).ok()?.data {
        OutputData::Datum(DatumResult::Plane { motion, .. }) => Some(motion),
        _ => None,
    }
}
