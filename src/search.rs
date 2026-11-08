//! Search matching (baseline 13.1/13.2): direct name/path scoring plus the
//! P2-02 pinyin layer.

use crate::pinyin_index::{PinyinIndex, is_pinyin_query, pinyin_score_prepared};

pub const SCORE_EXACT: u32 = 1000;
pub const SCORE_PREFIX: u32 = 900;
pub const SCORE_CONTAINS: u32 = 800;
pub const SCORE_FUZZY: u32 = 700;
pub const SCORE_PATH: u32 = 600;

/// One normalized query shared by every item in a search. Unicode lowercase
/// is intentional: ASCII-only normalization changes existing matching rules.
pub(crate) struct PreparedQuery {
    normalized: String,
    pinyin_eligible: bool,
}

impl PreparedQuery {
    pub(crate) fn new(query: &str) -> Self {
        let normalized = query.trim().to_lowercase();
        let pinyin_eligible = is_pinyin_query(&normalized);
        Self {
            normalized,
            pinyin_eligible,
        }
    }

    pub(crate) fn score(&self, name: &str, path: &str) -> Option<u32> {
        let query = self.normalized.as_str();
        if query.is_empty() {
            return Some(0);
        }
        let name = name.to_lowercase();
        if name == query {
            return Some(SCORE_EXACT);
        }
        if name.starts_with(query) {
            return Some(SCORE_PREFIX);
        }
        if name.contains(query) {
            return Some(SCORE_CONTAINS);
        }
        if is_subsequence(query, &name) {
            return Some(SCORE_FUZZY);
        }
        if path.to_lowercase().contains(query) {
            return Some(SCORE_PATH);
        }
        None
    }

    /// Direct-name matches already outrank every possible pinyin score.
    /// A path match (600) still needs comparison with pinyin (up to 690).
    pub(crate) fn needs_pinyin(&self, direct: Option<u32>) -> bool {
        self.pinyin_eligible && direct.is_none_or(|score| score < SCORE_FUZZY)
    }

    pub(crate) fn with_pinyin(
        &self,
        direct: Option<u32>,
        index: Option<&PinyinIndex>,
    ) -> Option<u32> {
        if !self.needs_pinyin(direct) {
            return direct;
        }
        let pinyin = index.and_then(|index| pinyin_score_prepared(&self.normalized, index));
        match (direct, pinyin) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }
}

/// Scores one item against the query; `None` means no match.
/// Empty queries match every item with score 0 so callers can show the scope.
pub fn score(query: &str, name: &str, path: &str) -> Option<u32> {
    PreparedQuery::new(query).score(name, path)
}

/// P2-03 keyboard selection movement (baseline 13.4). Returns -1 for an
/// empty result set and wraps around both ends.
pub fn move_selection(current: i32, count: usize, delta: i32) -> i32 {
    if count == 0 {
        return -1;
    }
    let count = count as i32;
    if current < 0 {
        return if delta < 0 { count - 1 } else { 0 };
    }
    (current + delta).rem_euclid(count)
}

/// Direct name/path match and the P2-02 pinyin index are scored
/// independently; the better rank wins. Pinyin scores sit below direct-name
/// fuzzy (700) and above path (600), so a name that matches directly never
/// loses to its own path match and vice versa.
pub fn score_with_pinyin(
    query: &str,
    name: &str,
    path: &str,
    index: Option<&PinyinIndex>,
) -> Option<u32> {
    let query = PreparedQuery::new(query);
    query.with_pinyin(query.score(name, path), index)
}

/// Whether all query characters appear in order in `haystack`.
fn is_subsequence(query: &str, haystack: &str) -> bool {
    let mut haystack = haystack.chars();
    query
        .chars()
        .all(|wanted| haystack.by_ref().any(|candidate| candidate == wanted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_prefix_and_contains_rank_in_order() {
        assert_eq!(score("chrome", "Chrome", ""), Some(SCORE_EXACT));
        assert_eq!(score("chro", "Chrome", ""), Some(SCORE_PREFIX));
        assert_eq!(score("hrom", "Chrome", ""), Some(SCORE_CONTAINS));
    }

    #[test]
    fn fuzzy_subsequence_matches_name_characters_in_order() {
        assert_eq!(score("phtp", "Photoshop", ""), Some(SCORE_FUZZY));
        assert_eq!(score("psth", "Photoshop", ""), None, "order matters");
    }

    #[test]
    fn path_matching_is_case_insensitive_and_lowest_rank() {
        assert_eq!(
            score("tools", "Unrelated", r"C:\Tools\Editor.exe"),
            Some(SCORE_PATH)
        );
        assert_eq!(
            score("TOOLS", "Unrelated", r"C:\Tools\Editor.exe"),
            Some(SCORE_PATH)
        );
        assert_eq!(score("missing", "Unrelated", r"C:\Tools\Editor.exe"), None);
    }

    #[test]
    fn empty_query_matches_everything_and_trim_is_applied() {
        assert_eq!(score("", "Anything", ""), Some(0));
        assert_eq!(score("   ", "Anything", ""), Some(0));
        assert_eq!(score(" chrome ", "Chrome", ""), Some(SCORE_EXACT));
    }

    #[test]
    fn chinese_names_match_by_substring() {
        assert_eq!(score("微信", "微信", ""), Some(SCORE_EXACT));
        assert_eq!(score("微", "微信", ""), Some(SCORE_PREFIX));
        assert_eq!(score("信", "微信", ""), Some(SCORE_CONTAINS));
    }

    // 修复：拼音精确匹配不得被同项的较低分路径匹配覆盖。
    #[test]
    fn pinyin_match_wins_over_a_lower_path_match() {
        let index = PinyinIndex::build("微信");
        assert_eq!(
            score_with_pinyin("weixin", "微信", r"C:\weixin\app.exe", Some(&index)),
            Some(690),
            "path (600) must not shadow the pinyin exact match (690)"
        );
    }

    #[test]
    fn prepared_query_preserves_unicode_lowercase_and_pinyin_eligibility() {
        let query = PreparedQuery::new("\u{2003}ΣΟΣ\u{2003}");
        assert_eq!(query.normalized, "σος");
        assert_eq!(query.score("ΣΟΣ", ""), Some(SCORE_EXACT));
        assert!(!query.needs_pinyin(None));
        let query = PreparedQuery::new("  İ  ");
        assert_eq!(query.normalized, "i\u{307}");
        assert_eq!(query.score("İstanbul", ""), Some(SCORE_PREFIX));
        assert!(!query.needs_pinyin(None));
        let query = PreparedQuery::new(" K ");
        assert_eq!(query.normalized, "k");
        assert!(
            query.needs_pinyin(None),
            "Unicode Kelvin sign folds to eligible ASCII"
        );
        assert_eq!(
            query.with_pinyin(None, Some(&PinyinIndex::build("开"))),
            Some(685)
        );
        for raw in ["", "  ", "微信", "two words", "x-y", "é"] {
            assert!(!PreparedQuery::new(raw).needs_pinyin(None), "{raw:?}");
        }
    }

    #[test]
    fn prepared_pinyin_gating_keeps_path_competition_and_direct_name_precedence() {
        let query = PreparedQuery::new(" WEIXIN ");
        let index = PinyinIndex::build("微信");
        for direct in [None, Some(SCORE_PATH)] {
            assert!(query.needs_pinyin(direct));
            assert_eq!(query.with_pinyin(direct, Some(&index)), Some(690));
        }
        for direct in [SCORE_FUZZY, SCORE_CONTAINS, SCORE_PREFIX, SCORE_EXACT] {
            assert!(!query.needs_pinyin(Some(direct)));
            assert_eq!(query.with_pinyin(Some(direct), Some(&index)), Some(direct));
        }
        assert_eq!(query.with_pinyin(Some(SCORE_PATH), None), Some(SCORE_PATH));
        assert_eq!(
            PreparedQuery::new(" ").with_pinyin(Some(0), Some(&index)),
            Some(0)
        );
    }

    #[test]
    fn public_score_wrappers_keep_unicode_trim_and_optional_index_behavior() {
        let index = PinyinIndex::build("微信");
        for (query, name, path, direct, combined) in [
            (" CAFÉ ", "café", "", Some(1000), Some(1000)),
            (" ΣΟΣ ", "ΣΟΣ", "", Some(1000), Some(1000)),
            (" K ", "K", "", Some(1000), Some(1000)),
            (" WEIXIN ", "微信", "C:/weixin/app", Some(600), Some(690)),
            (" WX ", "微信", "", None, Some(685)),
            (" wx! ", "微信", "", None, None),
            (" ", "微信", "", Some(0), Some(0)),
        ] {
            assert_eq!(score(query, name, path), direct, "{query:?}");
            assert_eq!(
                score_with_pinyin(query, name, path, None),
                direct,
                "{query:?}"
            );
            assert_eq!(
                score_with_pinyin(query, name, path, Some(&index)),
                combined,
                "{query:?}"
            );
        }
    }

    #[test]
    fn selection_moves_and_wraps_without_a_result() {
        assert_eq!(move_selection(0, 3, 1), 1);
        assert_eq!(move_selection(2, 3, 1), 0, "down wraps to the first row");
        assert_eq!(move_selection(0, 3, -1), 2, "up wraps to the last row");
        assert_eq!(move_selection(-1, 3, 1), 0);
        assert_eq!(move_selection(-1, 3, -1), 2);
        assert_eq!(move_selection(0, 0, 1), -1);
        assert_eq!(move_selection(0, 1, 1), 0);
    }
}
