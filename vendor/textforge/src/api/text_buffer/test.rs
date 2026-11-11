use super::*;

fn options(limit: usize) -> TextBufferOptions {
    TextBufferOptions::new(limit, std::env::temp_dir())
}

#[test]
fn threshold_and_consuming_materialization() {
    let small = TextBuffer::from_text("abcd", options(4)).unwrap();
    assert!(!small.is_spilled());
    assert_eq!(small.len_bytes(), 4);
    assert_eq!(small.into_string().unwrap(), "abcd");
    let large = TextBuffer::from_text("abcde", options(4)).unwrap();
    assert!(large.is_spilled());
    assert_eq!(large.into_string().unwrap(), "abcde");
}

#[test]
fn zero_threshold_empty_and_explicit_spill() {
    let mut empty = TextBuffer::from_text("", options(0)).unwrap();
    assert!(empty.is_empty());
    empty.spill().unwrap();
    assert!(empty.is_spilled());
    assert_eq!(empty.into_string().unwrap(), "");
}

#[test]
fn unicode_code_points_survive_one_byte_reads() {
    struct OneByte<'a>(&'a [u8]);
    impl Read for OneByte<'_> {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let len = output.len().min(1);
            self.0.read(&mut output[..len])
        }
    }
    let input = "é 👩🏽‍💻 🇧🇷\0";
    let buffer = TextBuffer::from_reader(OneByte(input.as_bytes()), options(1)).unwrap();
    assert_eq!(buffer.into_string().unwrap(), input);
}

#[test]
fn rejects_invalid_and_incomplete_utf8() {
    for bytes in [&[0xff][..], &[0xf0, 0x9f][..], &[b'a', 0xc3, b'b'][..]] {
        assert!(matches!(
            TextBuffer::from_reader(bytes, options(1))
                .unwrap_err()
                .error_code,
            TextForgeErrorCode::InvalidUtf8Error(_)
        ));
    }
}

#[test]
fn independent_reads_and_copies_ignore_previous_cursor() {
    let mut buffer = TextBuffer::from_text("abc👩🏽‍💻", options(1)).unwrap();
    let mut part = [0; 2];
    assert_eq!(buffer.read_at(1, &mut part).unwrap(), 2);
    assert_eq!(&part, b"bc");
    for _ in 0..2 {
        let mut result = Vec::new();
        buffer.copy_to(&mut result).unwrap();
        assert_eq!(result, "abc👩🏽‍💻".as_bytes());
    }
    assert_eq!(buffer.into_string().unwrap(), "abc👩🏽‍💻");
}

#[test]
fn reports_io_errors_without_exposing_tempfile() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing");
    assert!(TextBuffer::from_text("text", TextBufferOptions::new(0, missing)).is_err());
    struct BadWriter;
    impl Write for BadWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("test failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = TextBuffer::from_text("text", options(1)).unwrap();
    assert!(matches!(
        buffer.copy_to(BadWriter).unwrap_err().error_code,
        TextForgeErrorCode::FileWritingError(_)
    ));
    assert_eq!(buffer.into_string().unwrap(), "text");
}

#[test]
fn bounded_operations_match_strings_in_both_storage_modes() {
    for threshold in [0, 1, 1024 * 1024] {
        for text in ["", "a", "é", "👩🏽‍💻", "🇧🇷\n"] {
            for count in [0, 1, 2, 3, 17, 65537] {
                let mut buffer = TextBuffer::from_text(text, options(threshold))
                    .unwrap()
                    .repeat(count)
                    .unwrap()
                    .prepend("<")
                    .unwrap();
                buffer.append_text(">").unwrap();
                assert_eq!(
                    buffer.into_string().unwrap(),
                    format!("<{}>", text.repeat(count))
                );
            }
        }
    }
}

#[test]
fn reader_failure_and_repeat_overflow_return_errors() {
    struct BadReader;
    impl Read for BadReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("test failure"))
        }
    }
    assert!(TextBuffer::from_reader(BadReader, options(1)).is_err());
    assert!(
        TextBuffer::from_text("ab", options(1))
            .unwrap()
            .repeat(usize::MAX)
            .is_err()
    );
}

#[test]
fn retained_owned_text_transfers_allocation_and_shared_noops_reuse_snapshot() {
    let text = "é 👩🏽‍💻".repeat(1000);
    let pointer = text.as_ptr();
    let buffer = TextBuffer::from_owned(text, options(1));
    let text = buffer.into_string().unwrap();
    assert_eq!(pointer, text.as_ptr());
    let shared: Arc<str> = Arc::from(text);
    let buffer = TextBuffer::from_shared(Arc::clone(&shared), options(0));
    let buffer = buffer.transform_with(|text| Ok(text)).unwrap();
    let actual = buffer.into_shared(&shared).unwrap();
    assert!(Arc::ptr_eq(&shared, &actual));
    let unchanged = TextBuffer::from_owned(shared.to_string(), options(0))
        .into_shared(&shared)
        .unwrap();
    assert!(Arc::ptr_eq(&shared, &unchanged));
}

#[test]
fn retained_native_operations_reads_and_explicit_spill_preserve_unicode() {
    for original in ["", "é", "é 👩🏽‍💻 🇧🇷"] {
        let shared: Arc<str> = Arc::from(original);
        let mut buffer = TextBuffer::from_shared(shared, options(0))
            .prepend("<")
            .unwrap()
            .repeat(3)
            .unwrap();
        buffer.append_text(">").unwrap();
        let expected = format!("{}>", format!("<{original}").repeat(3));
        let mut bytes = Vec::new();
        buffer.copy_to(&mut bytes).unwrap();
        assert_eq!(bytes, expected.as_bytes());
        let mut range = [0; 5];
        let count = buffer.read_at(1, &mut range).unwrap();
        assert_eq!(&range[..count], &expected.as_bytes()[1..1 + count]);
        assert_eq!(buffer.read_at(u64::MAX, &mut range).unwrap(), 0);
        buffer.spill().unwrap();
        assert!(buffer.is_spilled());
        assert_eq!(buffer.into_string().unwrap(), expected);
    }
}

#[test]
fn retaining_is_internal_and_explicit_file_policy_still_spills() {
    let public = TextBuffer::from_text("abc", options(1))
        .unwrap()
        .transform_with(|text| Ok(Cow::Owned(text.to_uppercase())))
        .unwrap();
    assert!(public.is_spilled());
    assert_eq!(public.into_string().unwrap(), "ABC");
    let adaptive = TextBuffer::from_text("abc", options(1))
        .unwrap()
        .retain_materialized()
        .prepend("<")
        .unwrap()
        .repeat(1)
        .unwrap()
        .transform_with(|text| Ok(Cow::Owned(text.to_uppercase())))
        .unwrap();
    assert!(!adaptive.is_spilled());
    assert_eq!(adaptive.into_string().unwrap(), "<ABC");
}

#[test]
fn failed_explicit_spill_does_not_destroy_retained_text() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing");
    let mut buffer = TextBuffer::from_shared(Arc::from("é 🦀"), TextBufferOptions::new(0, missing));
    assert!(buffer.spill().is_err());
    assert_eq!(buffer.into_string().unwrap(), "é 🦀");
}

#[test]
fn reads_at_extreme_eof_offsets_are_consistent_for_memory_and_disk() {
    for threshold in [0, 1024] {
        let mut buffer = TextBuffer::from_text("🍌é", options(threshold)).unwrap();
        let mut bytes = [0; 8];
        assert_eq!(buffer.read_at(u64::MAX, &mut bytes).unwrap(), 0);
        assert_eq!(buffer.read_at(buffer.len_bytes(), &mut bytes).unwrap(), 0);
        assert_eq!(buffer.read_at(0, &mut []).unwrap(), 0);
        assert_eq!(buffer.read_at(0, &mut bytes).unwrap(), 6);
        assert_eq!(&bytes[..6], "🍌é".as_bytes());
    }
}
