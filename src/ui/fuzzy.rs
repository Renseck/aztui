//! Shared fuzzy-matching helpers used by the searchable list views.

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::style::Style;
use ratatui::text::Span;

thread_local! {
    static MATCHER: SkimMatcherV2 = SkimMatcherV2::default();
}

/* ============================================================================================== */
/// Fuzzy-matches `needle` against `haystack`, returning a score and the matched
/// character indices. An empty needle is a passthrough match (score 0, no
/// indices) so callers keep their original, unfiltered ordering.
pub fn fuzzy_match(haystack: &str, needle: &str) -> Option<(i64, Vec<usize>)> {
    if needle.is_empty() {
        return Some((0, Vec::new()));
    }
    MATCHER.with(|m| m.fuzzy_indices(haystack, needle))
}

/* ============================================================================================== */
/// Builds per-character spans for `text`, styling matched character positions
/// with `hit` and the rest with `base`.
pub fn highlight(text: &str, indices: &[usize], base: Style, hit: Style) -> Vec<Span<'static>> {
    text.chars()
        .enumerate()
        .map(|(i, ch)| {
            let style = if indices.contains(&i) { hit } else { base };
            Span::styled(ch.to_string(), style)
        })
        .collect()
}

/* ============================================================================================== */
/*                                              Tests                                             */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_subsequence() {
        assert!(fuzzy_match("prod-westeu-platform-01", "pwp1").is_some());
    }

    #[test]
    fn no_match_returns_none() {
        assert!(fuzzy_match("storage-account", "xyzq").is_none());
    }

    #[test]
    fn empty_needle_is_passthrough() {
        assert_eq!(fuzzy_match("anything", ""), Some((0, Vec::new())));
    }

    #[test]
    fn indices_point_at_matched_chars() {
        let (_score, idx) = fuzzy_match("azure", "az").unwrap();
        assert_eq!(idx, vec![0, 1]);
    }

    #[test]
    fn contiguous_prefix_scores_higher_than_scattered() {
        let prefix = fuzzy_match("teams", "te").unwrap().0;
        let scattered = fuzzy_match("table-entry", "te").unwrap().0;
        assert!(prefix > scattered, "prefix={prefix} scattered={scattered}");
    }

    #[test]
    fn highlight_marks_only_matched_indices() {
        let base = Style::default();
        let hit = Style::default().add_modifier(ratatui::style::Modifier::BOLD);
        let spans = highlight("az", &[0], base, hit);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].style, hit);
        assert_eq!(spans[1].style, base);
    }
}