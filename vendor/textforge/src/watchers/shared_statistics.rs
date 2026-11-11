//! Shared pure statistics for exact default-watcher function-item types.
use super::default_watchers;
use std::any::TypeId;

const CHARACTERS: u8 = 1;
const WORDS: u8 = 2;
const LINES: u8 = 4;

/// Recognizes exact pure built-in function items; arbitrary callbacks stay opaque.
/// Function pointers and wrapping closures are intentionally not identified.
///
/// ```
/// use textforge::watchers::{WatcherList, default_watchers, ExecutionWindows};
/// let mut list = WatcherList::new();
/// list.set_watcher("words", default_watchers::word_count);
/// list.schedule_watcher("words", ExecutionWindows::All)?;
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SharedMetric {
    Characters,
    Words,
    Lines,
    CharacterDelta,
    WordDelta,
}

fn type_of<T: 'static>(_: T) -> TypeId {
    TypeId::of::<T>()
}

impl SharedMetric {
    pub(super) fn identify<F: 'static>() -> Option<Self> {
        let kind = TypeId::of::<F>();
        if kind == type_of(default_watchers::current_len_chars) {
            Some(Self::Characters)
        } else if kind == type_of(default_watchers::word_count) {
            Some(Self::Words)
        } else if kind == type_of(default_watchers::line_count) {
            Some(Self::Lines)
        } else if kind == type_of(default_watchers::char_delta) {
            Some(Self::CharacterDelta)
        } else if kind == type_of(default_watchers::word_count_delta) {
            Some(Self::WordDelta)
        } else {
            None
        }
    }

    pub(super) fn current_mask(self) -> u8 {
        match self {
            Self::Characters | Self::CharacterDelta => CHARACTERS,
            Self::Words | Self::WordDelta => WORDS,
            Self::Lines => LINES,
        }
    }
    pub(super) fn before_mask(self) -> u8 {
        match self {
            Self::CharacterDelta => CHARACTERS,
            Self::WordDelta => WORDS,
            _ => 0,
        }
    }
    pub(super) fn evaluate(self, current: &Statistics, before: &Statistics) -> String {
        match self {
            Self::Characters => current.characters.to_string(),
            Self::Words => current.words.to_string(),
            Self::Lines => current.lines.to_string(),
            Self::CharacterDelta => {
                (current.characters as i64 - before.characters as i64).to_string()
            }
            Self::WordDelta => (current.words as i64 - before.words as i64).to_string(),
        }
    }
}

/// Per-step counts share one pass without retaining text beyond the callbacks.
///
/// ```
/// use textforge::watchers::{WatcherList, default_watchers, ExecutionWindows, WatcherContext};
/// let mut list = WatcherList::new();
/// list.set_watcher("words", default_watchers::word_count);
/// list.set_watcher("chars", default_watchers::current_len_chars);
/// list.schedule_watcher("words", ExecutionWindows::All)?;
/// list.schedule_watcher("chars", ExecutionWindows::All)?;
/// list.run_watchers(WatcherContext::new("é 🦀", "", None::<&str>, "null"))?;
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Default, Debug)]
pub(super) struct Statistics {
    characters: usize,
    words: usize,
    lines: usize,
}

impl Statistics {
    pub(super) fn compute(text: &str, mask: u8) -> Self {
        match mask {
            0 => Self::default(),
            CHARACTERS => Self {
                characters: text.chars().count(),
                ..Self::default()
            },
            WORDS => Self {
                words: text.split_whitespace().count(),
                ..Self::default()
            },
            LINES => Self {
                lines: text.lines().count(),
                ..Self::default()
            },
            _ => {
                let mut counts = Self::default();
                let mut in_word = false;
                for character in text.chars() {
                    counts.characters += 1;
                    let whitespace = character.is_whitespace();
                    counts.words += usize::from(!whitespace && !in_word);
                    in_word = !whitespace;
                    counts.lines += usize::from(character == '\n');
                }
                counts.lines += usize::from(!text.is_empty() && !text.ends_with('\n'));
                counts
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fused_counts_match_standard_library_for_all_requested_combinations() {
        for text in [
            "",
            "x",
            "\n",
            "\r",
            "\r\n",
            "a\r\nb\n",
            " \t\u{b}\u{c}\r\n",
            "é 👩🏽‍💻 🇧🇷",
            "a\u{85}b\u{a0}c\u{2028}d\u{3000}",
            "\n\nlast",
        ] {
            for mask in 1..=7 {
                let counts = Statistics::compute(text, mask);
                if mask & CHARACTERS != 0 {
                    assert_eq!(counts.characters, text.chars().count(), "{text:?}");
                }
                if mask & WORDS != 0 {
                    assert_eq!(counts.words, text.split_whitespace().count(), "{text:?}");
                }
                if mask & LINES != 0 {
                    assert_eq!(counts.lines, text.lines().count(), "{text:?}");
                }
            }
        }
    }
    #[test]
    fn function_pointers_and_custom_closures_are_not_treated_as_builtins() {
        fn identify<F: 'static>(_: F) -> Option<SharedMetric> {
            SharedMetric::identify::<F>()
        }
        assert_eq!(
            identify(default_watchers::word_count),
            Some(SharedMetric::Words)
        );
        let pointer: fn(super::super::WatcherContext) -> String = default_watchers::word_count;
        assert_eq!(identify(pointer), None);
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let captured = std::sync::Arc::clone(&calls);
        let custom = move |context| {
            captured.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            default_watchers::word_count(context)
        };
        assert_eq!(identify(custom), None);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}
