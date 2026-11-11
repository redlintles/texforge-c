//! Internal UTF-8 range operations that preserve borrowed slices and owned buffers.
use std::{borrow::Cow, ops::Range};

/// Keep an already validated byte range without copying an owned allocation.
pub(crate) fn keep(input: Cow<'_, str>, range: Range<usize>) -> Cow<'_, str> {
    match input {
        Cow::Borrowed(text) => Cow::Borrowed(&text[range]),
        Cow::Owned(mut text) => {
            text.truncate(range.end);
            text.drain(..range.start);
            Cow::Owned(text)
        }
    }
}

/// Replace an already validated byte range, preserving unchanged input.
pub(crate) fn replace<'a>(
    input: Cow<'a, str>,
    range: Range<usize>,
    replacement: &str,
) -> Cow<'a, str> {
    if &input[range.clone()] == replacement {
        return input;
    }
    match input {
        Cow::Borrowed(text) => {
            let mut output = String::with_capacity(text.len() - range.len() + replacement.len());
            output.push_str(&text[..range.start]);
            output.push_str(replacement);
            output.push_str(&text[range.end..]);
            Cow::Owned(output)
        }
        Cow::Owned(mut text) => {
            text.replace_range(range, replacement);
            Cow::Owned(text)
        }
    }
}

/// Append a word with only its first Unicode scalar uppercased.
pub(crate) fn capitalize_into(output: &mut String, word: &str) {
    let mut chars = word.chars();
    if let Some(first) = chars.next() {
        output.extend(first.to_uppercase());
        output.push_str(chars.as_str());
    }
}

/// Filter logical lines, allocating only after the first rejected line.
pub(crate) fn filter_lines(
    input: Cow<'_, str>,
    mut accept: impl FnMut(&str) -> bool,
) -> Cow<'_, str> {
    let mut output: Option<String> = None;
    let mut offset = 0;
    for (text, ending) in super::lines::lines(&input) {
        if accept(text) {
            if let Some(output) = output.as_mut() {
                output.push_str(text);
                output.push_str(ending);
            }
        } else if output.is_none() {
            let mut result = String::with_capacity(input.len());
            result.push_str(&input[..offset]);
            output = Some(result);
        }
        offset += text.len() + ending.len();
    }
    output.map_or(input, Cow::Owned)
}

/// Append a logical line using the first observed terminator, or LF.
pub(crate) fn append_line<'a>(input: Cow<'a, str>, text: &str) -> Cow<'a, str> {
    let ending = super::lines::lines(&input)
        .find_map(|(_, e)| (!e.is_empty()).then_some(e))
        .unwrap_or("\n");
    let last = super::lines::lines(&input).last();
    // A terminator is at most one Unicode scalar or the two bytes of CRLF.
    let mut ending_bytes = [0_u8; 4];
    ending_bytes[..ending.len()].copy_from_slice(ending.as_bytes());
    let ending_len = ending.len();
    let needs_before = last.is_some_and(|(_, e)| e.is_empty());
    let needs_after = last.is_some_and(|(_, e)| !e.is_empty()) || text.is_empty();
    let ending = std::str::from_utf8(&ending_bytes[..ending_len]);
    // The source bytes came from a valid str; retain a safe fallback.
    let ending = ending.unwrap_or("\n");
    let mut output = input.into_owned();
    output
        .reserve(text.len() + (usize::from(needs_before) + usize::from(needs_after)) * ending_len);
    if needs_before {
        output.push_str(ending);
    }
    output.push_str(text);
    if needs_after {
        output.push_str(ending);
    }
    Cow::Owned(output)
}

/// Insert a logical line; the caller validates the one-based line index.
pub(crate) fn insert_line<'a>(input: Cow<'a, str>, line: usize, text: &str) -> Cow<'a, str> {
    let count = super::lines::lines(&input).count();
    if line == count.saturating_add(1) {
        return append_line(input, text);
    }
    let ending = super::lines::lines(&input)
        .find_map(|(_, e)| (!e.is_empty()).then_some(e))
        .unwrap_or("\n");
    let offset: usize = super::lines::lines(&input)
        .take(line - 1)
        .map(|(t, e)| t.len() + e.len())
        .sum();
    let mut inserted = String::with_capacity(text.len() + ending.len());
    inserted.push_str(text);
    inserted.push_str(ending);
    replace(input, offset..offset, &inserted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_keep_borrows_and_reuse_owned_storage() {
        let text = "é🍌e\u{301}";
        let output = keep(Cow::Borrowed(text), 2..6);
        assert!(matches!(output, Cow::Borrowed("🍌")));
        let mut text = String::with_capacity(128);
        text.push_str("é🍌e\u{301}");
        let pointer = text.as_ptr();
        let output = keep(Cow::Owned(text), 2..6);
        assert_eq!(output, "🍌");
        assert_eq!(output.as_ptr(), pointer);
        let output = replace(output, 0..4, "ß");
        assert_eq!(output, "ß");
        assert_eq!(output.as_ptr(), pointer);
    }

    #[test]
    fn unchanged_replacement_retains_borrow_and_empty_ranges_are_valid() {
        assert!(matches!(
            replace(Cow::Borrowed("🍌"), 0..4, "🍌"),
            Cow::Borrowed(_)
        ));
        assert!(matches!(keep(Cow::Borrowed(""), 0..0), Cow::Borrowed("")));
        assert_eq!(replace(Cow::Borrowed("é"), 2..2, "🍌"), "é🍌");
        assert_eq!(replace(Cow::Borrowed("é"), 0..2, ""), "");
    }

    #[test]
    fn filters_delay_materialization_and_preserve_line_endings() {
        let input = "é\r\n🍌\u{2028}e\u{301}";
        assert!(matches!(
            filter_lines(Cow::Borrowed(input), |_| true),
            Cow::Borrowed(_)
        ));
        assert_eq!(
            filter_lines(Cow::Borrowed(input), |line| line != "🍌"),
            "é\r\ne\u{301}"
        );
        assert_eq!(filter_lines(Cow::Borrowed(input), |_| false), "");
        assert_eq!(filter_lines(Cow::Borrowed(""), |_| false), "");
    }

    #[test]
    fn capitalization_expands_unicode_without_modifying_the_tail() {
        let mut output = String::new();
        capitalize_into(&mut output, "ßé🍌e\u{301}");
        assert_eq!(output, "SSé🍌e\u{301}");
    }
}
