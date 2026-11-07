//! P2-02 pinyin index (baseline 13.3).
//!
//! For every registered item name the index stores a normalized form, full
//! pinyin variants and initials variants. The `pinyin` crate provides the
//! character readings; its heteronym list supplies the common polyphone
//! candidates. Variants are capped so a name with many polyphones cannot
//! explode combinatorially.

use pinyin::ToPinyinMulti;

/// Upper bound for generated full/initial variants per name.
const MAX_VARIANTS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinyinIndex {
    /// Lowercase name without separators; Han characters are kept.
    pub normalized: String,
    /// Concatenated pinyin (ASCII letters/digits kept as-is), one per variant.
    pub full: Vec<String>,
    /// First letter of every syllable, one per variant.
    pub initials: Vec<String>,
}

impl PinyinIndex {
    pub fn build(name: &str) -> Self {
        let mut syllables: Vec<Vec<String>> = Vec::new();
        let mut normalized = String::new();
        for character in name.chars() {
            if character.is_ascii_alphanumeric() {
                let ascii = character.to_ascii_lowercase().to_string();
                normalized.push_str(&ascii);
                syllables.push(vec![ascii]);
            } else if character.is_whitespace() || character.is_ascii_punctuation() {
                // Separators do not participate in pinyin matching.
            } else if let Some(readings) = character.to_pinyin_multi() {
                let mut options: Vec<String> = readings
                    .into_iter()
                    .map(|pinyin| pinyin.plain().to_string())
                    .collect();
                options.sort();
                options.dedup();
                normalized.push(character);
                syllables.push(options);
            } else {
                let lower = character.to_lowercase().to_string();
                normalized.push_str(&lower);
                syllables.push(vec![lower]);
            }
        }
        Self {
            normalized,
            full: build_variants(&syllables, false),
            initials: build_variants(&syllables, true),
        }
    }
}

fn build_variants(syllables: &[Vec<String>], first_letter: bool) -> Vec<String> {
    let mut variants = vec![String::new()];
    for options in syllables {
        let mut next = Vec::new();
        'prefix: for prefix in &variants {
            for option in options {
                if next.len() >= MAX_VARIANTS {
                    break 'prefix;
                }
                let piece = if first_letter {
                    option
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .unwrap_or_default()
                } else {
                    option.clone()
                };
                next.push(format!("{prefix}{piece}"));
            }
        }
        variants = next;
    }
    variants.sort();
    variants.dedup();
    variants
}

/// Scores a pinyin query. Every pinyin score stays below the direct-name
/// fuzzy score (700) and above the path score (600), per P2-02 ordering.
pub fn pinyin_score(query: &str, index: &PinyinIndex) -> Option<u32> {
    let query = query.trim().to_lowercase();
    if query.is_empty() || !query.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    if index.full.iter().any(|variant| variant == &query) {
        return Some(690);
    }
    if index.initials.iter().any(|variant| variant == &query) {
        return Some(685);
    }
    if index.full.iter().any(|variant| variant.starts_with(&query)) {
        return Some(680);
    }
    if index
        .initials
        .iter()
        .any(|variant| variant.starts_with(&query))
    {
        return Some(675);
    }
    if index.full.iter().any(|variant| variant.contains(&query)) {
        return Some(670);
    }
    if index
        .initials
        .iter()
        .any(|variant| variant.contains(&query))
    {
        return Some(665);
    }
    if index
        .full
        .iter()
        .any(|variant| is_subsequence(&query, variant))
    {
        return Some(660);
    }
    None
}

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
    fn wechat_full_and_initials() {
        let index = PinyinIndex::build("微信");
        assert!(index.full.contains(&"weixin".to_string()));
        assert!(index.initials.contains(&"wx".to_string()));
        assert_eq!(pinyin_score("weixin", &index), Some(690));
        assert_eq!(pinyin_score("wx", &index), Some(685));
    }

    #[test]
    fn netease_initials_prefix_matches_wyy() {
        let index = PinyinIndex::build("网易云音乐");
        assert!(index.full.iter().any(|full| full == "wangyiyunyinyue"));
        assert!(index.initials.contains(&"wyyyy".to_string()));
        assert_eq!(pinyin_score("wyy", &index), Some(675));
    }

    #[test]
    fn polyphone_candidates_include_yue_for_music() {
        let index = PinyinIndex::build("音乐");
        assert!(
            index.full.iter().any(|full| full == "yinyue"),
            "乐 should offer the yue candidate: {:?}",
            index.full
        );
        assert!(
            index.full.iter().any(|full| full == "yinle"),
            "乐 should keep the le reading as well"
        );
        assert_eq!(pinyin_score("yinyue", &index), Some(690));
    }

    #[test]
    fn separators_and_ascii_are_normalized() {
        let index = PinyinIndex::build("微信 WeChat");
        assert_eq!(index.normalized, "微信wechat");
        assert!(
            index
                .full
                .iter()
                .any(|full| full.starts_with("weixinwechat"))
        );
        assert_eq!(
            pinyin_score("微信", &index),
            None,
            "Chinese queries use name matching"
        );
    }

    #[test]
    fn variant_growth_is_bounded() {
        let index = PinyinIndex::build("乐乐乐乐乐乐乐乐乐乐乐乐");
        assert!(index.full.len() <= MAX_VARIANTS);
        assert!(index.initials.len() <= MAX_VARIANTS);
    }
}
