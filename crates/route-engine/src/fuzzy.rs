//! 模糊匹配
//!
//! 提供 Jaccard 相似度、编辑距离（Levenshtein distance）计算，
//! 以及多种匹配模式：精确匹配、前缀匹配、包含匹配、模糊匹配。

/// 搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// 匹配的文本内容
    pub text: String,
    /// 匹配得分（0.0 ~ 1.0）
    pub score: f64,
    /// 可选的源文件路径
    pub file_path: Option<String>,
    /// 行号
    pub line: usize,
}

/// 模糊匹配器
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    /// 匹配模式
    pub pattern: String,
    /// 相似度阈值（0.0 ~ 1.0），低于此值的模糊匹配结果将被过滤
    pub threshold: f64,
}

impl FuzzyMatch {
    /// 创建一个新的模糊匹配器
    pub fn new(pattern: &str, threshold: f64) -> Self {
        FuzzyMatch {
            pattern: pattern.to_string(),
            threshold,
        }
    }

    /// 对文本执行匹配，返回匹配结果
    pub fn match_text(&self, text: &str, file_path: Option<&str>, line: usize) -> Option<SearchResult> {
        let pattern = self.pattern.trim();
        let text = text.trim();

        if pattern.is_empty() || text.is_empty() {
            return None;
        }

        // 1. 精确匹配
        if text == pattern {
            return Some(SearchResult {
                text: text.to_string(),
                score: 1.0,
                file_path: file_path.map(|p| p.to_string()),
                line,
            });
        }

        // 2. 前缀匹配
        if text.starts_with(pattern) {
            return Some(SearchResult {
                text: text.to_string(),
                score: 0.8,
                file_path: file_path.map(|p| p.to_string()),
                line,
            });
        }

        // 3. 包含匹配
        if text.contains(pattern) {
            return Some(SearchResult {
                text: text.to_string(),
                score: 0.6,
                file_path: file_path.map(|p| p.to_string()),
                line,
            });
        }

        // 4. 模糊匹配（基于 Jaccard 相似度）
        let jaccard_score = jaccard_similarity(text, pattern);
        if jaccard_score >= self.threshold {
            // 将 Jaccard 得分映射到 0.1 ~ 0.5 区间
            let fuzzy_score = 0.1 + jaccard_score * 0.4;
            let score = fuzzy_score.min(0.5).max(0.1);
            return Some(SearchResult {
                text: text.to_string(),
                score,
                file_path: file_path.map(|p| p.to_string()),
                line,
            });
        }

        None
    }
}

/// 计算 Jaccard 相似度（字符级 + 分词级综合）
pub fn jaccard_similarity(s1: &str, s2: &str) -> f64 {
    if s1.is_empty() && s2.is_empty() {
        return 1.0;
    }
    if s1.is_empty() || s2.is_empty() {
        return 0.0;
    }

    // 字符级 Jaccard
    let set1_char: std::collections::HashSet<char> = s1.chars().collect();
    let set2_char: std::collections::HashSet<char> = s2.chars().collect();
    let char_intersection = set1_char.intersection(&set2_char).count();
    let char_union = set1_char.union(&set2_char).count();
    let char_jaccard = if char_union > 0 {
        char_intersection as f64 / char_union as f64
    } else {
        0.0
    };

    // 分词级 Jaccard — 对英文按空格分词，对中文按字符分词
    let tokens1 = tokenize(s1);
    let tokens2 = tokenize(s2);
    let set1_token: std::collections::HashSet<String> = tokens1.into_iter().collect();
    let set2_token: std::collections::HashSet<String> = tokens2.into_iter().collect();
    let token_intersection = set1_token.intersection(&set2_token).count();
    let token_union = set1_token.union(&set2_token).count();
    let token_jaccard = if token_union > 0 {
        token_intersection as f64 / token_union as f64
    } else {
        0.0
    };

    // 综合：字符级权重 0.4，分词级权重 0.6
    char_jaccard * 0.4 + token_jaccard * 0.6
}

/// 分词：对英文按空格和标点分词，对中文按字符分词
fn tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else if ch.is_ascii_whitespace() || ch.is_ascii_punctuation() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            // 非 ASCII 字符（如中文），直接作为单个 token
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            tokens.push(ch.to_string());
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// 计算编辑距离（Levenshtein distance）
pub fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();

    // 优化：如果其中一个为空字符串
    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    // 使用两行滚动数组优化空间
    let mut prev: Vec<usize> = (0..=len2).collect();
    let mut curr: Vec<usize> = vec![0; len2 + 1];

    for (i, ch1) in s1.chars().enumerate() {
        curr[0] = i + 1;
        for (j, ch2) in s2.chars().enumerate() {
            let cost = if ch1 == ch2 { 0 } else { 1 };
            curr[j + 1] = std::cmp::min(
                std::cmp::min(curr[j] + 1, prev[j + 1] + 1),
                prev[j] + cost,
            );
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[len2]
}

/// 计算归一化的编辑距离相似度（0.0 ~ 1.0）
pub fn levenshtein_similarity(s1: &str, s2: &str) -> f64 {
    let dist = levenshtein_distance(s1, s2);
    let max_len = std::cmp::max(s1.chars().count(), s2.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    1.0 - (dist as f64 / max_len as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let fm = FuzzyMatch::new("hello", 0.3);
        let result = fm.match_text("hello", Some("test.rs"), 1);
        assert!(result.is_some());
        assert_eq!(result.unwrap().score, 1.0);
    }

    #[test]
    fn test_prefix_match() {
        let fm = FuzzyMatch::new("hel", 0.3);
        let result = fm.match_text("hello world", Some("test.rs"), 1);
        assert!(result.is_some());
        assert_eq!(result.unwrap().score, 0.8);
    }

    #[test]
    fn test_contains_match() {
        let fm = FuzzyMatch::new("world", 0.3);
        let result = fm.match_text("hello world", Some("test.rs"), 1);
        assert!(result.is_some());
        assert_eq!(result.unwrap().score, 0.6);
    }

    #[test]
    fn test_fuzzy_match() {
        let fm = FuzzyMatch::new("helloworld", 0.3);
        let result = fm.match_text("hello world", Some("test.rs"), 1);
        assert!(result.is_some());
        let score = result.unwrap().score;
        assert!(score >= 0.1 && score <= 0.5);
    }

    #[test]
    fn test_no_match() {
        let fm = FuzzyMatch::new("xyzabc", 0.5);
        let result = fm.match_text("hello world", Some("test.rs"), 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_jaccard_identical() {
        let score = jaccard_similarity("hello", "hello");
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_jaccard_empty() {
        assert_eq!(jaccard_similarity("", ""), 1.0);
        assert_eq!(jaccard_similarity("a", ""), 0.0);
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
        assert_eq!(levenshtein_distance("", "abc"), 3);
    }

    #[test]
    fn test_levenshtein_similarity() {
        let sim = levenshtein_similarity("hello", "hello");
        assert!((sim - 1.0).abs() < 1e-6);

        let sim = levenshtein_similarity("hello", "world");
        assert!(sim < 1.0);
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("hello world");
        assert_eq!(tokens, vec!["hello", "world"]);

        let tokens = tokenize("你好世界");
        assert_eq!(tokens, vec!["你", "好", "世", "界"]);
    }
}