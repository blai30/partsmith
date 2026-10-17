//! Suggestions for unknown names (expressions.md section 3.18).

/// Largest edit distance at which an unknown name is close to a parameter name (expressions.md section 3.18).
pub const SUGGESTION_DISTANCE: usize = 2;

/// Levenshtein distance between two texts, counted in Unicode scalar values, case-sensitive
/// (expressions.md section 3.18).
pub fn edit_distance(first: &str, second: &str) -> usize {
    let target: Vec<char> = second.chars().collect();
    // Distances from the first `i` characters of `first` to every prefix of `second`, one row per `i`.
    let mut previous: Vec<usize> = (0..=target.len()).collect();
    let mut current = vec![0; target.len() + 1];
    for (i, source) in first.chars().enumerate() {
        current[0] = i + 1;
        for (position, &character) in target.iter().enumerate() {
            let substitution = previous[position] + usize::from(source != character);
            let deletion = previous[position + 1] + 1;
            let insertion = current[position] + 1;
            current[position + 1] = substitution.min(deletion).min(insertion);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[target.len()]
}

/// The one candidate other than `name` within `SUGGESTION_DISTANCE` of it, or `None` when there
/// is none or there are several, so a suggestion is never a choice between lookalikes. A candidate
/// listed more than once counts once.
pub(crate) fn suggestion<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let length = name.chars().count();
    let mut found: Option<&str> = None;
    for candidate in candidates {
        // The distance is at least the difference in length, so most candidates end here.
        let is_close = candidate != name
            && candidate.chars().count().abs_diff(length) <= SUGGESTION_DISTANCE
            && edit_distance(name, candidate) <= SUGGESTION_DISTANCE;
        if !is_close || found == Some(candidate) {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(candidate);
    }
    found.map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_counts_insertions_deletions_and_substitutions() {
        assert_eq!(edit_distance("wall", "walls"), 1);
        assert_eq!(edit_distance("walls", "wall"), 1);
        assert_eq!(edit_distance("wall", "wail"), 1);
        assert_eq!(edit_distance("", ""), 0);
    }

    #[test]
    fn distance_counts_characters_not_bytes() {
        assert_eq!(edit_distance("ää", "a"), 2);
    }

    #[test]
    fn the_one_close_candidate_is_suggested() {
        assert_eq!(
            suggestion("hieght", ["width", "height"]),
            Some("height".to_string())
        );
    }

    #[test]
    fn two_close_candidates_give_no_suggestion() {
        assert_eq!(suggestion("wal", ["wall", "bad"]), None);
    }

    #[test]
    fn a_repeated_candidate_counts_once() {
        assert_eq!(
            suggestion("wal", ["wall", "wall"]),
            Some("wall".to_string())
        );
    }

    #[test]
    fn the_name_itself_is_not_a_suggestion() {
        assert_eq!(suggestion("wall", ["wall"]), None);
    }

    #[test]
    fn candidates_beyond_the_distance_are_ignored() {
        assert_eq!(suggestion("w", ["wall", "width"]), None);
    }
}
