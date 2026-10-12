//! Disjoint sets of indices, shared by the solver's components, identification and substitution and by the profile's chains.

/// Union-find over the indices `0..len()`. The root of every set is its lowest index, so walking
/// the roots in index order visits the sets in the order of their first member.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct UnionFind {
    parents: Vec<usize>,
}

impl UnionFind {
    /// `count` sets of one index each.
    pub(crate) fn new(count: usize) -> UnionFind {
        UnionFind {
            parents: (0..count).collect(),
        }
    }

    /// Adds a set holding only the next index and returns that index.
    pub(crate) fn push(&mut self) -> usize {
        let index = self.parents.len();
        self.parents.push(index);
        index
    }

    /// The root of the set that holds `index`.
    pub(crate) fn root(&self, mut index: usize) -> usize {
        while self.parents[index] != index {
            index = self.parents[index];
        }
        index
    }

    /// True when `index` is the root of its set.
    pub(crate) fn is_root(&self, index: usize) -> bool {
        self.parents[index] == index
    }

    /// Merges the sets that hold `first` and `second` and returns the root of the merged set.
    pub(crate) fn union(&mut self, first: usize, second: usize) -> usize {
        let (first, second) = (self.root(first), self.root(second));
        let (low, high) = (first.min(second), first.max(second));
        self.parents[high] = low;
        low
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_index_starts_in_its_own_set() {
        let sets = UnionFind::new(3);

        assert_eq!([sets.root(0), sets.root(1), sets.root(2)], [0, 1, 2]);
    }

    #[test]
    fn the_lowest_index_becomes_the_root() {
        let mut sets = UnionFind::new(5);

        sets.union(4, 2);
        let root = sets.union(3, 4);

        assert_eq!(root, 2);
        assert_eq!([sets.root(2), sets.root(3), sets.root(4)], [2, 2, 2]);
        assert!(sets.is_root(2) && !sets.is_root(4));
        assert_eq!(sets.root(1), 1);
    }

    #[test]
    fn union_within_one_set_changes_nothing() {
        let mut sets = UnionFind::new(2);
        sets.union(0, 1);

        assert_eq!(sets.union(1, 0), 0);
        assert_eq!(sets.root(1), 0);
    }

    #[test]
    fn pushed_indices_follow_the_existing_ones() {
        let mut sets = UnionFind::default();

        assert_eq!(sets.push(), 0);
        assert_eq!(sets.push(), 1);
        sets.union(0, 1);
        assert_eq!(sets.push(), 2);
        assert_eq!(sets.root(1), 0);
        assert_eq!(sets.root(2), 2);
    }
}
