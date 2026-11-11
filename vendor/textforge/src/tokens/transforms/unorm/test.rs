use super::*;

#[test]
fn canonical_and_compatibility_forms_preserve_their_distinctions() {
    let cases = [
        ("nfc", "e\u{301}", "é"),
        ("nfd", "é", "e\u{301}"),
        ("nfkc", "Ａﬁ²", "Afi2"),
        ("nfkd", "Ａﬁ²é", "Afi2e\u{301}"),
        ("nfc", "Ａﬁ²", "Ａﬁ²"),
        ("nfd", "Ａﬁ²", "Ａﬁ²"),
        ("nfc", "\u{1100}\u{1161}\u{11a8}", "각"),
        ("nfd", "각", "\u{1100}\u{1161}\u{11a8}"),
        ("nfd", "a\u{301}\u{323}", "a\u{323}\u{301}"),
        ("nfc", "\u{301}\u{323}", "\u{323}\u{301}"),
    ];
    for (mode, input, expected) in cases {
        let token = Unorm::new(mode).unwrap();
        let result = token.transform(input.into(), None).unwrap();
        assert_eq!(result, expected, "{mode}");
        assert_eq!(
            token.transform(result, None).unwrap(),
            expected,
            "idempotence: {mode}"
        );
    }
}

#[test]
fn empty_ascii_emoji_and_long_combining_sequences() {
    for mode in ["nfc", "nfd", "nfkc", "nfkd"] {
        let token = Unorm::new(mode).unwrap();
        for input in ["", "ASCII\0\r\n", "👩🏽‍💻🍌", "\u{fe0f}\u{200d}"] {
            assert_eq!(token.transform(input.into(), None).unwrap(), input);
        }
        let input = format!("a{}", "\u{301}".repeat(1000));
        let result = token.transform(input.into(), None).unwrap();
        assert!(
            !result.contains('\u{034f}'),
            "Must not insert stream-safe separators"
        );
        assert_eq!(token.transform(result.clone(), None).unwrap(), result);
    }
}

#[test]
fn already_normalized_inputs_retain_borrowing_or_owned_allocation() {
    for (mode, text) in [
        ("nfc", "café🍌"),
        ("nfd", "cafe\u{301}🍌"),
        ("nfkc", "café🍌"),
        ("nfkd", "cafe\u{301}🍌"),
        ("nfc", "q\u{301}🍌"),
        ("nfkc", "q\u{301}🍌"),
    ] {
        let token = Unorm::new(mode).unwrap();
        let borrowed = token.transform(Cow::Borrowed(text), None).unwrap();
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        assert_eq!(borrowed.as_ptr(), text.as_ptr());
        let owned = text.to_owned();
        let pointer = owned.as_ptr();
        let result = token.transform(Cow::Owned(owned), None).unwrap();
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result.as_ptr(), pointer);
    }
}

#[test]
fn invalid_parameters_do_not_change_the_last_valid_mode() {
    let mut token = Unorm::new("nfd").unwrap();
    for invalid in ["NFC", "", " nfc", "nfc ", "unknown"] {
        let error = token
            .set_params(&[TextForgeParamTypes::String(invalid.into())])
            .unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::InvalidParameters(_)
        ));
        assert_eq!(error.instruction, "unorm");
        assert_eq!(error.input, invalid);
    }
    assert!(token.set_params(&[]).is_err());
    assert!(token.set_params(&[1usize.into()]).is_err());
    assert!(
        token
            .set_params(&["nfc".to_owned().into(), "nfd".to_owned().into()])
            .is_err()
    );
    assert_eq!(token.to_textforge_line(), "unorm nfd;\n");
    assert_eq!(token.transform("é".into(), None).unwrap(), "e\u{301}");
    assert_eq!(Unorm::default().to_textforge_line(), "unorm nfc;\n");
}

#[cfg(feature = "bytecode")]
#[test]
fn bytecode_contains_big_endian_opcode_and_mode_parameter() {
    for mode in ["nfc", "nfd", "nfkc", "nfkd"] {
        let bytes = Unorm::new(mode).unwrap().to_bytecode().unwrap();
        assert_eq!(
            u64::from_be_bytes(bytes[..8].try_into().unwrap()) as usize,
            bytes.len() - 8
        );
        assert_eq!(&bytes[8..12], &0x41u32.to_be_bytes());
        assert_eq!(bytes[12], 1);
        assert!(bytes.ends_with(mode.as_bytes()));
    }
}

#[test]
fn unresolved_interpolation_is_serializable_but_cannot_execute_directly() {
    for expression in ["{{form}}", "n{{suffix}}", "[[ form ]]"] {
        let token = Unorm::new(expression).unwrap();
        let line = token.to_textforge_line();
        let words = shell_words::split(line.trim_end_matches([';', '\n'])).unwrap();
        assert_eq!(words, vec!["unorm", expression]);
        let error = token.transform("é".into(), None).unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::InvalidParameters(_)
        ));
        assert_eq!(error.input, expression);
    }
}
