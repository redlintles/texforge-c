// watchers/default_watchers.rs
use std::collections::HashSet;

use crate::watchers::WatcherContext;

pub fn current_len_bytes(ctx: WatcherContext) -> String {
    ctx.current.len().to_string()
}

pub fn current_len_chars(ctx: WatcherContext) -> String {
    ctx.current.chars().count().to_string()
}

pub fn byte_delta(ctx: WatcherContext) -> String {
    let before = ctx.before.len() as i64;
    let after = ctx.current.len() as i64;
    (after - before).to_string()
}

pub fn char_delta(ctx: WatcherContext) -> String {
    let before = ctx.before.chars().count() as i64;
    let after = ctx.current.chars().count() as i64;
    (after - before).to_string()
}

pub fn word_count(ctx: WatcherContext) -> String {
    ctx.current.split_whitespace().count().to_string()
}

pub fn word_count_delta(ctx: WatcherContext) -> String {
    let before = ctx.before.split_whitespace().count() as i64;
    let after = ctx.current.split_whitespace().count() as i64;
    (after - before).to_string()
}

pub fn line_count(ctx: WatcherContext) -> String {
    ctx.current.lines().count().to_string()
}

pub fn is_empty(ctx: WatcherContext) -> String {
    ctx.current.is_empty().to_string()
}

pub fn is_unchanged(ctx: WatcherContext) -> String {
    (ctx.before == ctx.current).to_string()
}

pub fn whitespace_count(ctx: WatcherContext) -> String {
    ctx.current
        .chars()
        .filter(|c| c.is_whitespace())
        .count()
        .to_string()
}

// is_numeric() cobre dígitos unicode além de ASCII; troque por is_ascii_digit()
// se você quiser um watcher estritamente ASCII.
pub fn digit_count(ctx: WatcherContext) -> String {
    ctx.current
        .chars()
        .filter(|c| c.is_numeric())
        .count()
        .to_string()
}

pub fn alpha_count(ctx: WatcherContext) -> String {
    ctx.current
        .chars()
        .filter(|c| c.is_alphabetic())
        .count()
        .to_string()
}

pub fn uppercase_count(ctx: WatcherContext) -> String {
    ctx.current
        .chars()
        .filter(|c| c.is_uppercase())
        .count()
        .to_string()
}

pub fn lowercase_count(ctx: WatcherContext) -> String {
    ctx.current
        .chars()
        .filter(|c| c.is_lowercase())
        .count()
        .to_string()
}

pub fn unique_char_count(ctx: WatcherContext) -> String {
    let set: HashSet<char> = ctx.current.chars().collect();
    set.len().to_string()
}

#[cfg(all(test, feature = "test_access"))]
mod tests {
    use std::sync::Arc;

    use super::*;

    // src/watchers/default_watchers.rs — helper de teste
    fn ctx(before: &str, current: &str, instruction: &str) -> WatcherContext {
        WatcherContext {
            current: Arc::from(current),
            before: Arc::from(before),
            after: Some(Arc::from(current)),
            instruction: Arc::from(instruction),
        }
    }

    #[test]
    fn test_current_len_bytes_counts_utf8_bytes_not_chars() {
        // 'é' ocupa 2 bytes em UTF-8; "abcdé" = 4 ASCII + 2 bytes = 6 bytes, 5 chars
        let c = ctx("abcd", "abcdé", "add_to_end");
        assert_eq!(current_len_bytes(c), "6");
    }

    #[test]
    fn test_current_len_chars_counts_scalar_values() {
        let c = ctx("abcd", "abcdé", "add_to_end");
        assert_eq!(current_len_chars(c), "5");
    }

    #[test]
    fn test_byte_delta_positive_on_growth() {
        let c = ctx("ab", "abcd", "add_to_end");
        assert_eq!(byte_delta(c), "2");
    }

    #[test]
    fn test_byte_delta_negative_on_shrink() {
        let c = ctx("abcdef", "ab", "delete_after");
        assert_eq!(byte_delta(c), "-4");
    }

    #[test]
    fn test_char_delta_negative_on_shrink_no_panic() {
        // Esse é exatamente o caso que quebrava a implementação original com usize
        let c = ctx("banana", "ba", "trim_both_sides");
        assert_eq!(char_delta(c), "-4");
    }

    #[test]
    fn test_word_count_basic() {
        let c = ctx("", "banana laranja cheia", "add_to_end");
        assert_eq!(word_count(c), "3");
    }

    #[test]
    fn test_word_count_delta_negative() {
        let c = ctx("banana laranja cheia", "banana", "delete_last");
        assert_eq!(word_count_delta(c), "-2");
    }

    #[test]
    fn test_line_count() {
        let c = ctx("", "linha1\nlinha2\nlinha3", "add_to_end");
        assert_eq!(line_count(c), "3");
    }

    #[test]
    fn test_is_empty_true() {
        let c = ctx("banana", "", "delete_first");
        assert_eq!(is_empty(c), "true");
    }

    #[test]
    fn test_is_empty_false() {
        let c = ctx("", "banana", "add_to_end");
        assert_eq!(is_empty(c), "false");
    }

    #[test]
    fn test_is_unchanged_true_when_instruction_is_noop() {
        let c = ctx("banana", "banana", "trim_both_sides");
        assert_eq!(is_unchanged(c), "true");
    }

    #[test]
    fn test_is_unchanged_false_when_text_changes() {
        let c = ctx("banana", "BANANA", "to_uppercase_all");
        assert_eq!(is_unchanged(c), "false");
    }

    #[test]
    fn test_whitespace_count() {
        let c = ctx("", "a b  c\td", "add_to_end");
        assert_eq!(whitespace_count(c), "4");
    }

    #[test]
    fn test_digit_count() {
        let c = ctx("", "abc123def45", "add_to_end");
        assert_eq!(digit_count(c), "5");
    }

    #[test]
    fn test_alpha_count() {
        let c = ctx("", "abc123", "add_to_end");
        assert_eq!(alpha_count(c), "3");
    }

    #[test]
    fn test_uppercase_count() {
        let c = ctx("", "BaNaNa", "to_uppercase_chunk");
        assert_eq!(uppercase_count(c), "3");
    }

    #[test]
    fn test_lowercase_count() {
        let c = ctx("", "BaNaNa", "to_lowercase_chunk");
        assert_eq!(lowercase_count(c), "3");
    }

    #[test]
    fn test_unique_char_count() {
        let c = ctx("", "banana", "add_to_end");
        assert_eq!(unique_char_count(c), "3"); // b, a, n
    }

    #[test]
    fn test_unique_char_count_empty_string() {
        let c = ctx("", "", "trim_both_sides");
        assert_eq!(unique_char_count(c), "0");
    }
}
