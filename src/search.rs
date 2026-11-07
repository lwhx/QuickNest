//! Search matching (baseline 13.1/13.2): direct name/path scoring plus the
//! P2-02 pinyin layer.

use crate::pinyin_index::{PinyinIndex, pinyin_score};

pub const SCORE_EXACT: u32 = 1000;
pub const SCORE_PREFIX: u32 = 900;
pub const SCORE_CONTAINS: u32 = 800;
pub const SCORE_FUZZY: u32 = 700;
pub const SCORE_PATH: u32 = 600;

/// Scores one item against the query; `None` means no match.
/// Empty queries match every item with score 0 so callers can show the scope.
pub fn score(query: &str, name: &str, path: &str) -> Option<u32> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let name = name.to_lowercase();
    if name == query {
        return Some(SCORE_EXACT);
    }
    if name.starts_with(&query) {
        return Some(SCORE_PREFIX);
    }
    if name.contains(&query) {
        return Some(SCORE_CONTAINS);
    }
    if is_subsequence(&query, &name) {
        return Some(SCORE_FUZZY);
    }
    if path.to_lowercase().contains(&query) {
        return Some(SCORE_PATH);
    }
    None
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
    let direct = score(query, name, path);
    let pinyin = index.and_then(|index| pinyin_score(query, index));
    match (direct, pinyin) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
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
