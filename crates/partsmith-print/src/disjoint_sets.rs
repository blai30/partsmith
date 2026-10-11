//! Disjoint sets (union-find) for grouping triangles, vertices and slice loops.

/// A partition of `0..count` into groups, merged pairwise.
#[derive(Clone, Debug)]
pub(crate) struct DisjointSets {
    /// Each item's parent; a root is its own parent.
    parents: Vec<usize>,
}

impl DisjointSets {
    /// Every item in a group of its own.
    pub(crate) fn new(count: usize) -> DisjointSets {
        DisjointSets {
            parents: (0..count).collect(),
        }
    }

    /// The root of the item's group, halving the path on the way.
    pub(crate) fn find(&mut self, mut item: usize) -> usize {
        while self.parents[item] != item {
            let grandparent = self.parents[self.parents[item]];
            self.parents[item] = grandparent;
            item = grandparent;
        }
        item
    }

    /// Merges the groups of two items.
    pub(crate) fn union(&mut self, first: usize, second: usize) {
        let first_root = self.find(first);
        let second_root = self.find(second);
        // The lower root wins, which keeps trees shallow enough with path halving.
        if first_root < second_root {
            self.parents[second_root] = first_root;
        } else {
            self.parents[first_root] = second_root;
        }
    }

    /// The groups of the given items, each listing its items in the order given, and the groups
    /// ordered by their first item.
    pub(crate) fn groups(&mut self, items: impl IntoIterator<Item = usize>) -> Vec<Vec<usize>> {
        let mut group_of_root: Vec<Option<usize>> = vec![None; self.parents.len()];
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for item in items {
            let root = self.find(item);
            let group = *group_of_root[root].get_or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            groups[group].push(item);
        }
        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_start_alone() {
        let mut sets = DisjointSets::new(3);
        assert_eq!(sets.groups(0..3), vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn union_is_transitive() {
        let mut sets = DisjointSets::new(5);
        sets.union(4, 2);
        sets.union(2, 0);
        assert_eq!(sets.find(4), sets.find(0));
        assert_ne!(sets.find(1), sets.find(0));
    }

    #[test]
    fn groups_follow_the_order_of_their_first_item() {
        let mut sets = DisjointSets::new(6);
        sets.union(5, 1);
        sets.union(3, 4);
        assert_eq!(
            sets.groups(0..6),
            vec![vec![0], vec![1, 5], vec![2], vec![3, 4]]
        );
    }

    #[test]
    fn groups_cover_only_the_given_items() {
        let mut sets = DisjointSets::new(4);
        sets.union(0, 3);
        assert_eq!(sets.groups([3, 2]), vec![vec![3], vec![2]]);
    }

    #[test]
    fn union_of_an_item_with_itself_changes_nothing() {
        let mut sets = DisjointSets::new(2);
        sets.union(1, 1);
        assert_eq!(sets.groups(0..2), vec![vec![0], vec![1]]);
    }
}
