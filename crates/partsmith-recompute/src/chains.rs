//! The running state of every body and its writer chain (recompute.md section 3.5, "Writer chains").

use std::collections::BTreeMap;

use partsmith_features::BodyState;
use partsmith_model::Id;

/// Every body of a run: its current state, its provider and its writer chain.
#[derive(Debug, Default)]
pub(crate) struct Bodies {
    /// The bodies by id.
    bodies: BTreeMap<Id, Chain>,
}

/// One body during and after a run.
#[derive(Debug)]
struct Chain {
    /// Its current state.
    current: BodyState,
    /// Position of its provider: the last active record that declares it written and ran, was
    /// reused or failed.
    provider: Option<usize>,
    /// Its writer chain in the shape builds read it (`BuildInputs::history`): each writer's id and
    /// the body's state just before it, in timeline order.
    writers: Vec<(Id, BodyState)>,
    /// The timeline position of each writer of `writers`.
    positions: Vec<usize>,
}

impl Bodies {
    /// The bodies `ids`, all empty, with no writers.
    pub(crate) fn empty<'a>(ids: impl IntoIterator<Item = &'a Id>) -> Bodies {
        let empty = BodyState::empty();
        Bodies {
            bodies: ids
                .into_iter()
                .map(|id| {
                    let chain = Chain {
                        current: empty.clone(),
                        provider: None,
                        writers: Vec::new(),
                        positions: Vec::new(),
                    };
                    (id.clone(), chain)
                })
                .collect(),
        }
    }

    /// The current state of `body`, if the document has it.
    pub(crate) fn current(&self, body: &Id) -> Option<&BodyState> {
        self.bodies.get(body).map(|chain| &chain.current)
    }

    /// The position of the current provider of `body`.
    pub(crate) fn provider(&self, body: &Id) -> Option<usize> {
        self.bodies.get(body).and_then(|chain| chain.provider)
    }

    /// Makes `state` the current state of `body`.
    pub(crate) fn set(&mut self, body: &Id, state: BodyState) {
        if let Some(chain) = self.bodies.get_mut(body) {
            chain.current = state;
        }
    }

    /// Records that the record `writer` at `position` is done with `body`, before its update is
    /// applied: it becomes the body's provider and joins its writer chain with the body's current
    /// state, the state just before it. A record joins a chain once, even when its dependencies
    /// name the body twice.
    pub(crate) fn written(&mut self, body: &Id, writer: &Id, position: usize) {
        if let Some(chain) = self.bodies.get_mut(body) {
            chain.provider = Some(position);
            if chain.positions.last() != Some(&position) {
                chain.writers.push((writer.clone(), chain.current.clone()));
                chain.positions.push(position);
            }
        }
    }

    /// Takes the writer chains of `bodies` out, as a build reads them; `restore_histories` puts
    /// them back. A body named twice is taken once, so its chain is never replaced by an empty one.
    pub(crate) fn take_histories<'a>(
        &mut self,
        bodies: impl IntoIterator<Item = &'a Id>,
    ) -> BTreeMap<Id, Vec<(Id, BodyState)>> {
        let mut histories = BTreeMap::new();
        for body in bodies {
            if let Some(chain) = self.bodies.get_mut(body) {
                histories
                    .entry(body.clone())
                    .or_insert_with(|| std::mem::take(&mut chain.writers));
            }
        }
        histories
    }

    /// Puts back the writer chains `take_histories` took.
    pub(crate) fn restore_histories(&mut self, histories: BTreeMap<Id, Vec<(Id, BodyState)>>) {
        for (body, writers) in histories {
            if let Some(chain) = self.bodies.get_mut(&body) {
                chain.writers = writers;
            }
        }
    }

    /// The positions of the writers of `body` before `position`, in timeline order.
    pub(crate) fn writers_before(&self, body: &Id, position: usize) -> &[usize] {
        self.bodies.get(body).map_or(&[], |chain| {
            let end = chain.positions.partition_point(|&writer| writer < position);
            &chain.positions[..end]
        })
    }

    /// The state of `body` just before the record at `position`, with its provider's position:
    /// the state just before the body's first writer at or after `position`, else its current
    /// state (section 3.5).
    pub(crate) fn state_before(
        &self,
        body: &Id,
        position: usize,
    ) -> Option<(&BodyState, Option<usize>)> {
        let chain = self.bodies.get(body)?;
        let next = chain.positions.partition_point(|&writer| writer < position);
        Some(match chain.writers.get(next) {
            Some((_, before)) => (
                before,
                next.checked_sub(1)
                    .map(|previous| chain.positions[previous]),
            ),
            None => (&chain.current, chain.provider),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::id;

    /// Bodies `b-1` and `b-2`, where `b-1` was written at positions 1 and 3 by `f-1` and `f-3`.
    fn written_twice() -> (Bodies, [BodyState; 3]) {
        let states = [BodyState::empty(), BodyState::empty(), BodyState::empty()];
        let mut bodies = Bodies::empty(&[id("b-1"), id("b-2")]);
        bodies.set(&id("b-1"), states[0].clone());
        bodies.written(&id("b-1"), &id("f-1"), 1);
        bodies.set(&id("b-1"), states[1].clone());
        bodies.written(&id("b-1"), &id("f-3"), 3);
        bodies.set(&id("b-1"), states[2].clone());
        (bodies, states)
    }

    /// True when `state` is the very state `expected` (a clone of it).
    fn same(state: &BodyState, expected: &BodyState) -> bool {
        std::sync::Arc::ptr_eq(state.naming(), expected.naming())
            && state.is_empty() == expected.is_empty()
    }

    #[test]
    fn a_state_before_a_record_is_the_one_before_the_next_writer() {
        let (bodies, states) = written_twice();

        let (before_first, provider) = bodies.state_before(&id("b-1"), 0).expect("b-1");
        assert!(same(before_first, &states[0]));
        assert_eq!(provider, None);
        let (between, provider) = bodies.state_before(&id("b-1"), 2).expect("b-1");
        assert!(same(between, &states[1]));
        assert_eq!(provider, Some(1));
        let (after, provider) = bodies.state_before(&id("b-1"), 4).expect("b-1");
        assert!(same(after, &states[2]));
        assert_eq!(provider, Some(3));
        assert!(bodies.state_before(&id("b-9"), 0).is_none());
    }

    #[test]
    fn writers_before_a_position_come_in_timeline_order() {
        let (bodies, _) = written_twice();

        let positions = bodies.writers_before(&id("b-1"), 3);

        assert_eq!(positions, [1]);
        assert_eq!(bodies.writers_before(&id("b-2"), 9).len(), 0);
        assert_eq!(bodies.provider(&id("b-1")), Some(3));
    }

    #[test]
    fn histories_taken_for_a_build_come_back() {
        let (mut bodies, _) = written_twice();

        let histories = bodies.take_histories(&[id("b-1"), id("b-9")]);
        let writers: Vec<String> = histories[&id("b-1")]
            .iter()
            .map(|(writer, _)| writer.to_string())
            .collect();
        bodies.restore_histories(histories);

        assert_eq!(writers, ["f-1", "f-3"]);
        assert_eq!(bodies.writers_before(&id("b-1"), 9).len(), 2);
    }

    #[test]
    fn a_body_named_twice_is_taken_once_and_comes_back_whole() {
        let (mut bodies, states) = written_twice();

        let histories = bodies.take_histories(&[id("b-1"), id("b-1")]);
        assert_eq!(histories[&id("b-1")].len(), 2);
        bodies.restore_histories(histories);

        assert_eq!(bodies.writers_before(&id("b-1"), 9).len(), 2);
        let (between, _) = bodies.state_before(&id("b-1"), 2).expect("b-1");
        assert!(same(between, &states[1]));
    }

    #[test]
    fn a_writer_that_names_its_body_twice_joins_the_chain_once() {
        let (mut bodies, _) = written_twice();

        bodies.written(&id("b-2"), &id("f-4"), 4);
        bodies.written(&id("b-2"), &id("f-4"), 4);

        assert_eq!(bodies.writers_before(&id("b-2"), 9).len(), 1);
        assert_eq!(bodies.provider(&id("b-2")), Some(4));
    }
}
